use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::ExitStatusExt,
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use command::{Stdio, blocking::Command};
use instant::Instant;
use nix::{
    fcntl::{FcntlArg, FdFlag, OFlag, fcntl},
    pty::openpty,
    sys::{
        signal::{Signal, killpg},
        termios::{self, LocalFlags, SetArg, Termios},
    },
    unistd::Pid,
};
use serde::{Deserialize, Serialize};

use crate::RunArgs;

const REQUEST: u8 = 1;
const OUTPUT: u8 = 2;
const EXIT: u8 = 3;
const INPUT: u8 = 4;
const MAX_FRAME: usize = 4 * 1024 * 1024;
const MAX_PENDING_INPUT: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Request {
    argv: Vec<String>,
    cwd: PathBuf,
    environment: Vec<(OsString, OsString)>,
    log_directory: PathBuf,
    timeout: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct Completion {
    exit_code: i32,
    reason: Option<String>,
}

fn write_frame(stream: &mut UnixStream, kind: u8, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_FRAME,
        "command protocol frame is too large"
    );
    stream.write_all(&[kind])?;
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}

fn read_frame(stream: &mut UnixStream) -> Result<(u8, Vec<u8>)> {
    let mut header = [0; 5];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header[1..].try_into()?) as usize;
    ensure!(length <= MAX_FRAME, "command protocol frame is too large");
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok((header[0], bytes))
}

fn private_directory(path: &Path) -> Result<()> {
    let info = fs::symlink_metadata(path)?;
    ensure!(
        info.is_dir() && info.uid() == nix::unistd::getuid().as_raw() && info.mode() & 0o077 == 0,
        "expected an owned, private directory: {}",
        path.display()
    );
    Ok(())
}

struct Interrupt {
    flag: Arc<AtomicBool>,
    handlers: Vec<signal_hook::SigId>,
}

impl Interrupt {
    fn new() -> Result<Self> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut handlers = Vec::new();
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            handlers.push(signal_hook::flag::register(signal, flag.clone())?);
        }
        Ok(Self { flag, handlers })
    }

    fn received(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

impl Drop for Interrupt {
    fn drop(&mut self) {
        for handler in &self.handlers {
            signal_hook::low_level::unregister(*handler);
        }
    }
}

fn poll(descriptors: &mut [libc::pollfd]) -> Result<()> {
    // SAFETY: the slice supplies valid storage for the stated number of descriptors.
    let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, 50) };
    if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

pub(super) fn client(args: &RunArgs, worker_prefix: &[&str]) -> Result<i32> {
    ensure!(
        !args.command.is_empty() && !args.command[0].is_empty(),
        "a program is required"
    );
    ensure!(
        args.command.iter().all(|arg| !arg.contains('\0')),
        "arguments cannot contain NUL"
    );
    ensure!(
        !args.group.is_empty()
            && args.group.len() <= 128
            && !args.group.chars().any(char::is_control),
        "command group must contain 1 to 128 bytes without control characters"
    );
    let cwd = args
        .cwd
        .clone()
        .unwrap_or(env::current_dir()?)
        .canonicalize()?;
    ensure!(cwd.is_dir(), "working directory is not a directory");
    let state = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .context("HOME or XDG_STATE_HOME is required for retained command logs")?
        .join("dwarp/agent-runs");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&state)?;
    private_directory(&state)?;
    let log_directory = tempfile::Builder::new()
        .prefix("native-")
        .tempdir_in(state)?
        .keep();
    fs::set_permissions(&log_directory, fs::Permissions::from_mode(0o700))?;
    eprintln!("dWarp command log: {}", log_directory.display());
    let launcher_log = log_directory.join("launcher.log");
    let launcher_output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&launcher_log)?;
    // Keep the socket path short enough for sockaddr_un, independently of the log location.
    let socket_directory = tempfile::Builder::new()
        .prefix("dwarp-command-")
        .tempdir_in("/tmp")?;
    fs::set_permissions(socket_directory.path(), fs::Permissions::from_mode(0o700))?;
    let socket = socket_directory.path().join("session");
    let listener = UnixListener::bind(&socket)?;
    listener.set_nonblocking(true)?;
    let interrupt = Interrupt::new()?;
    let mut launcher = Command::new(&args.warp);
    if !args.focus {
        launcher.arg("--no-focus");
    }
    launcher.arg("--working-directory").arg(&cwd);
    if !args.new_tab {
        launcher.arg("--command-group").arg(&args.group);
    }
    launcher
        .arg("--execute")
        .arg(env::current_exe()?)
        .args(worker_prefix)
        .arg("--worker")
        .arg(&socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(launcher_output);
    let mut launcher = launcher.spawn().context("could not start dWarp launcher")?;
    let started = Instant::now();
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
        if interrupt.received() || started.elapsed() >= Duration::from_secs(args.launch_timeout) {
            // A focused launch may itself be the GUI. Cancel this request by
            // dropping its socket, without closing the user's application.
            let _ = launcher.try_wait();
            if interrupt.received() {
                return Ok(130);
            }
            anyhow::bail!(
                "dWarp did not start the command; rebuild/restart dWarp and keep a window open"
            );
        }
        if let Some(status) = launcher.try_wait()?
            && !status.success()
        {
            let error = fs::read_to_string(&launcher_log)?;
            anyhow::bail!("dWarp launcher failed ({status}): {}", error.trim());
        }
        thread::sleep(Duration::from_millis(20));
    };
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = Request {
        argv: args.command.clone(),
        cwd,
        environment: env::vars_os().collect(),
        log_directory,
        timeout: args.timeout,
    };
    write_frame(&mut stream, REQUEST, &serde_json::to_vec(&request)?)?;
    if args.stdin {
        // A separate writer keeps a stalled worker from blocking output reads. Its
        // failure only ends forwarding: the command's exit status still arrives.
        let mut writer = stream.try_clone()?;
        thread::spawn(move || {
            let mut buffer = [0; 16384];
            loop {
                match io::stdin().read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        if write_frame(&mut writer, INPUT, &buffer[..count]).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        });
    }
    loop {
        if interrupt.received() {
            return Ok(130);
        } // Closing the socket cancels the worker's process group.
        let mut descriptors = [libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        poll(&mut descriptors)?;
        if descriptors[0].revents == 0 {
            continue;
        }
        let (kind, bytes) = read_frame(&mut stream)
            .context("visible command disconnected without a completion status")?;
        match kind {
            OUTPUT => {
                io::stdout().write_all(&bytes)?;
                io::stdout().flush()?;
            }
            EXIT => {
                let result: Completion = serde_json::from_slice(&bytes)?;
                if let Some(reason) = result.reason {
                    eprintln!("dWarp command: {reason}");
                }
                let _ = launcher.try_wait();
                return Ok(result.exit_code);
            }
            other => anyhow::bail!("unexpected command protocol message: {other}"),
        }
    }
}

struct RawTerminal(Option<Termios>);

impl RawTerminal {
    fn new() -> Result<Self> {
        let previous = termios::tcgetattr(0)?;
        let mut raw = previous.clone();
        termios::cfmakeraw(&mut raw);
        termios::tcsetattr(0, SetArg::TCSANOW, &raw)?;
        Ok(Self(Some(previous)))
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        if let Some(previous) = &self.0 {
            let _ = termios::tcsetattr(0, SetArg::TCSANOW, previous);
        }
    }
}

struct ChildGroup(std::process::Child);

impl ChildGroup {
    fn signal(&self, signal: Signal) {
        let _ = killpg(Pid::from_raw(self.0.id() as i32), signal);
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            self.signal(Signal::SIGKILL);
            let _ = self.0.wait();
        }
    }
}

/// Password prompts clear ECHO but keep line editing. Full-screen programs and remote
/// shells clear both, so their input stays allowed.
fn hides_input(master: &File) -> bool {
    termios::tcgetattr(master.as_raw_fd()).is_ok_and(|terminal| {
        !terminal.local_flags.contains(LocalFlags::ECHO)
            && terminal.local_flags.contains(LocalFlags::ICANON)
    })
}

fn size(fd: RawFd) -> libc::winsize {
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: size is writable storage for the ioctl's winsize output.
    unsafe {
        libc::ioctl(fd, libc::TIOCGWINSZ, &mut size);
    }
    size
}

pub(super) fn worker(socket: &Path) -> Result<i32> {
    private_directory(socket.parent().context("missing socket directory")?)?;
    let metadata = fs::symlink_metadata(socket)?;
    ensure!(
        metadata.file_type().is_socket() && metadata.uid() == nix::unistd::getuid().as_raw(),
        "invalid command socket"
    );
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let (kind, bytes) = read_frame(&mut stream)?;
    ensure!(kind == REQUEST, "expected a command request");
    let request: Request = serde_json::from_slice(&bytes)?;
    // The accepted connection owns this session now. Remove its discovery path before
    // execution so even SIGKILL of the caller cannot leave a socket directory behind.
    fs::remove_file(socket)?;
    fs::remove_dir(socket.parent().context("missing socket directory")?)?;
    private_directory(&request.log_directory)?;
    let mut log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(request.log_directory.join("output.log"))?;
    // Exclusive log creation gives each request a single worker owner.
    let result = match execute(&request, &mut stream, &mut log) {
        Ok(result) => result,
        Err(error) => Completion {
            exit_code: 125,
            reason: Some(format!("{error:#}")),
        },
    };
    let bytes = serde_json::to_vec(&result)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(request.log_directory.join("result.json"))?;
    file.write_all(&bytes)?;
    let _ = write_frame(&mut stream, EXIT, &bytes);
    Ok(result.exit_code)
}

fn execute(request: &Request, stream: &mut UnixStream, log: &mut File) -> Result<Completion> {
    ensure!(!request.argv.is_empty(), "missing command");
    let interrupt = Interrupt::new()?;
    let pty = openpty(None, None)?;
    // SAFETY: openpty returned two new descriptors, each transferred to exactly one File.
    let mut master = unsafe { File::from_raw_fd(pty.master) };
    let slave = unsafe { File::from_raw_fd(pty.slave) };
    fcntl(master.as_raw_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;
    fcntl(slave.as_raw_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;
    let original_size = size(0);
    // SAFETY: original_size points to a valid winsize, and the descriptor belongs to the PTY.
    ensure!(
        unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCSWINSZ, &original_size) } == 0,
        "could not set command terminal size"
    );
    let mut command = Command::new(&request.argv[0]);
    command
        .args(&request.argv[1..])
        .current_dir(&request.cwd)
        .env_clear()
        .envs(request.environment.iter().cloned())
        .stdin(slave.try_clone()?)
        .stdout(slave.try_clone()?)
        .stderr(slave.try_clone()?);
    // SAFETY: only async-signal-safe syscalls run between fork and exec. The slave is already fd 0.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let _raw = RawTerminal::new()?;
    let mut child = ChildGroup(command.spawn().context("could not execute command")?);
    drop(slave);
    drop(command);
    fcntl(master.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    println!(
        "\r\ndWarp command: {:?}\r\nWorking directory: {}\r",
        request.argv,
        request.cwd.display()
    );
    io::stdout().flush()?;
    let started = Instant::now();
    let mut cancellation: Option<(Instant, &str)> = None;
    let mut last_signal = None;
    let mut finished = None;
    let mut child_status = None;
    let mut output_open = true;
    let mut input_open = true;
    let mut connected = true;
    let mut refusing = false;
    let mut input = Vec::new();
    let mut buffer = [0; 16384];
    loop {
        if interrupt.received() && cancellation.is_none() {
            cancellation = Some((Instant::now(), "cancelled"));
        }
        if request
            .timeout
            .is_some_and(|timeout| started.elapsed() >= Duration::from_secs(timeout))
            && cancellation.is_none()
        {
            cancellation = Some((Instant::now(), "command timed out"));
        }
        if let Some(at) = cancellation
            .map(|entry| entry.0)
            .filter(|_| child_status.is_none())
        {
            let signal = if at.elapsed() >= Duration::from_secs(4) {
                Signal::SIGKILL
            } else if at.elapsed() >= Duration::from_secs(2) {
                Signal::SIGTERM
            } else {
                Signal::SIGINT
            };
            if last_signal != Some(signal) {
                child.signal(signal);
                last_signal = Some(signal);
            }
        }
        if child_status.is_none()
            && let Some(status) = child.0.try_wait()?
        {
            if cancellation.is_some() {
                // Reap the whole foreground group even when its leader exits first.
                child.signal(Signal::SIGKILL);
            }
            child_status = Some(status);
            finished = Some(Instant::now());
        }
        if finished.is_some_and(|at| !output_open || at.elapsed() >= Duration::from_secs(1)) {
            break;
        }
        let dimensions = size(0);
        // SAFETY: dimensions is a valid winsize and master is an open PTY descriptor.
        unsafe {
            libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &dimensions);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: if output_open { master.as_raw_fd() } else { -1 },
                events: libc::POLLIN | if input.is_empty() { 0 } else { libc::POLLOUT },
                revents: 0,
            },
            libc::pollfd {
                fd: if input_open && input.len() < 65536 {
                    0
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if connected { stream.as_raw_fd() } else { -1 },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        poll(&mut descriptors)?;
        if descriptors[2].revents != 0 {
            match read_frame(stream) {
                Ok((INPUT, bytes)) => {
                    let refusal = if hides_input(&master) {
                        Some("caller input refused: the terminal is hiding typed characters")
                    } else if input.len() + bytes.len() > MAX_PENDING_INPUT {
                        Some("caller input refused: too much input is waiting for the command")
                    } else {
                        input.extend_from_slice(&bytes);
                        None
                    };
                    if refusal.is_none() {
                        refusing = false;
                    } else if let Some(refusal) = refusal.filter(|_| !refusing) {
                        refusing = true;
                        let notice = format!("\r\ndWarp: {refusal}\r\n");
                        let _ = io::stdout()
                            .write_all(notice.as_bytes())
                            .and_then(|()| io::stdout().flush());
                        if write_frame(stream, OUTPUT, notice.as_bytes()).is_err() {
                            connected = false;
                            cancellation.get_or_insert((Instant::now(), "caller disconnected"));
                        }
                    }
                }
                Ok((other, _)) => anyhow::bail!("unexpected caller message: {other}"),
                Err(_) => {
                    connected = false;
                    cancellation.get_or_insert((Instant::now(), "caller disconnected"));
                }
            }
        }
        if descriptors[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            match io::stdin().read(&mut buffer) {
                Ok(0) => {
                    input_open = false;
                    cancellation.get_or_insert((Instant::now(), "terminal disconnected"));
                }
                Ok(count) => input.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        if descriptors[0].revents & libc::POLLOUT != 0 {
            match master.write(&input) {
                Ok(count) => {
                    input.drain(..count);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if descriptors[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            match master.read(&mut buffer) {
                Ok(0) => output_open = false,
                Ok(count) => {
                    let bytes = &buffer[..count];
                    log.write_all(bytes)?;
                    if io::stdout()
                        .write_all(bytes)
                        .and_then(|()| io::stdout().flush())
                        .is_err()
                    {
                        cancellation.get_or_insert((Instant::now(), "terminal disconnected"));
                    }
                    if connected && write_frame(stream, OUTPUT, bytes).is_err() {
                        connected = false;
                        cancellation.get_or_insert((Instant::now(), "caller disconnected"));
                    }
                }
                Err(error) if error.raw_os_error() == Some(libc::EIO) => output_open = false,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    let status = child_status.context("command ended without an exit status")?;
    let exit_code = if let Some((_, reason)) = cancellation {
        if reason == "command timed out" {
            124
        } else {
            130
        }
    } else {
        status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
    };
    Ok(Completion {
        exit_code,
        reason: cancellation.map(|(_, reason)| reason.to_owned()),
    })
}
