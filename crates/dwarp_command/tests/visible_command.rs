#![cfg(unix)]

use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::PermissionsExt,
    },
    path::PathBuf,
    thread,
    time::Duration,
};

use command::{Stdio, blocking::Command};
use instant::Instant;
use nix::{
    pty::openpty,
    sys::signal::{Signal, kill},
    unistd::{Pid, ttyname},
};

struct Terminal {
    directory: tempfile::TempDir,
    _master: File,
    _slave: File,
    tty: PathBuf,
}

impl Terminal {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let pty = openpty(None, None).unwrap();
        let tty = ttyname(pty.slave).unwrap();
        // SAFETY: each descriptor returned by openpty is transferred to a single File.
        let master = unsafe { File::from_raw_fd(pty.master) };
        let slave = unsafe { File::from_raw_fd(pty.slave) };
        let launcher = directory.path().join("launcher");
        fs::write(
            &launcher,
            r#"#!/bin/sh
while [ "$#" -gt 0 ] && [ "$1" != --execute ]; do
    printf '%s\n' "$1" >> "$DWARP_TEST_ARGUMENTS"
    shift
done
shift
exec "$@" < "$DWARP_TEST_TTY" > "$DWARP_TEST_TTY" 2>&1
"#,
        )
        .unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            directory,
            _master: master,
            _slave: slave,
            tty,
        }
    }

    fn command(&self) -> Command {
        // Do not inherit a restrictive umask from the caller's command bridge.
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "umask 022; exec \"$@\"", "native-test"])
            .arg(env!("CARGO_BIN_EXE_dwarp-run"));
        command
            .arg("--warp")
            .arg(self.directory.path().join("launcher"))
            .arg("--cwd")
            .arg(self.directory.path())
            .env("XDG_STATE_HOME", self.directory.path().join("state"))
            .env("DWARP_TEST_TTY", &self.tty)
            .env(
                "DWARP_TEST_ARGUMENTS",
                self.directory.path().join("arguments"),
            );
        command
    }

    fn wait_for(&self, name: &str) {
        let started = Instant::now();
        while !self.directory.path().join(name).exists() {
            assert!(started.elapsed() < Duration::from_secs(8), "missing {name}");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn streams_output_from_a_real_pty_and_preserves_exit_status() {
    let terminal = Terminal::new();
    let output = terminal.command().args(["--", "bash", "-c",
        "test -t 0 && test -t 1 && test -t 2 || exit 19; printf 'visible output\\n'; printf 'error output\\n' >&2; exit 17"
    ]).output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(17),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("visible output"));
    assert!(text.contains("error output"));
    let arguments = fs::read_to_string(terminal.directory.path().join("arguments")).unwrap();
    assert!(arguments.contains("--no-focus"));
    assert!(arguments.contains("--command-group"));
    let state = terminal.directory.path().join("state/dwarp/agent-runs");
    let log = fs::read_dir(state).unwrap().next().unwrap().unwrap().path();
    assert_eq!(
        fs::metadata(log.join("output.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(
        fs::read_to_string(log.join("output.log"))
            .unwrap()
            .contains("visible output")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(log.join("result.json")).unwrap())
            .unwrap()["exit_code"],
        17
    );
}

#[test]
fn prompt_without_a_newline_is_visible_before_the_user_answers() {
    let terminal = Terminal::new();
    let caller = terminal
        .command()
        .args([
            "--",
            "bash",
            "-c",
            "printf 'No-newline prompt: '; read -r answer; printf 'received: %s\\n' \"$answer\"",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut master = terminal._master.try_clone().unwrap();
    let started = Instant::now();
    let mut display = Vec::new();
    let mut visible = false;
    while started.elapsed() < Duration::from_secs(4) {
        let mut descriptor = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: descriptor provides valid writable storage for exactly one pollfd.
        if unsafe { libc::poll(&mut descriptor, 1, 50) } > 0 {
            let mut bytes = [0; 4096];
            let count = master.read(&mut bytes).unwrap();
            display.extend_from_slice(&bytes[..count]);
            // The worker's introductory header also contains the argv. Only a prompt
            // following its header's newline proves the command's output was flushed.
            if display.ends_with(b"\r\nNo-newline prompt: ") {
                visible = true;
                break;
            }
        }
    }
    if visible {
        master.write_all(b"hello\n").unwrap();
    } else {
        let _ = kill(Pid::from_raw(caller.id() as i32), Signal::SIGTERM);
    }
    let output = caller.wait_with_output().unwrap();
    assert!(
        visible,
        "prompt was buffered until user input: {}",
        String::from_utf8_lossy(&display)
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("received: hello"));
}

#[test]
fn literal_arguments_environment_and_directory_reach_the_command() {
    let terminal = Terminal::new();
    let output = terminal
        .command()
        .env("NATIVE_COMMAND_VALUE", "from caller")
        .args([
            "--",
            "bash",
            "-c",
            "printf '%s|%s|%s' \"$PWD\" \"$NATIVE_COMMAND_VALUE\" \"$1\"",
            "bash",
            "$(touch injected); a'b",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = format!(
        "{}|from caller|$(touch injected); a'b",
        terminal.directory.path().display()
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    assert!(!terminal.directory.path().join("injected").exists());
}

#[test]
fn focus_and_separate_tab_are_explicit() {
    let terminal = Terminal::new();
    let output = terminal
        .command()
        .args(["--focus", "--new-tab", "--", "true"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let arguments = fs::read_to_string(terminal.directory.path().join("arguments")).unwrap();
    assert!(!arguments.contains("--no-focus"));
    assert!(!arguments.contains("--command-group"));
}

#[test]
fn timeout_cancels_the_command() {
    let terminal = Terminal::new();
    let started = Instant::now();
    let output = terminal
        .command()
        .args(["--timeout", "1", "--", "sleep", "30"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(124),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < Duration::from_secs(8));
}

#[test]
fn timeout_escalates_when_the_command_ignores_interrupt_and_terminate() {
    let terminal = Terminal::new();
    let started = Instant::now();
    let output = terminal
        .command()
        .args([
            "--timeout",
            "1",
            "--",
            "bash",
            "-c",
            "trap '' INT TERM; exec sleep 30",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(124),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() >= Duration::from_secs(4));
    assert!(started.elapsed() < Duration::from_secs(9));
}

#[test]
fn caller_disconnection_cancels_the_foreground_process_group() {
    let terminal = Terminal::new();
    let mut caller = terminal
        .command()
        .args([
            "--",
            "bash",
            "-c",
            "trap 'touch cancelled; exit 0' INT TERM; touch started; while :; do sleep 1; done",
        ])
        .spawn()
        .unwrap();
    terminal.wait_for("started");
    kill(Pid::from_raw(caller.id() as i32), Signal::SIGKILL).unwrap();
    caller.wait().unwrap();
    terminal.wait_for("cancelled");
}

#[test]
fn vanished_launcher_fails_without_executing_the_command() {
    let terminal = Terminal::new();
    fs::remove_file(terminal.directory.path().join("launcher")).unwrap();
    let output = terminal
        .command()
        .args(["--", "touch", "should-not-exist"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(125));
    assert!(!terminal.directory.path().join("should-not-exist").exists());
}

#[test]
fn launch_timeout_preserves_a_launcher_that_may_be_the_gui() {
    let terminal = Terminal::new();
    fs::write(
        terminal.directory.path().join("launcher"),
        "#!/bin/sh\nprintf '%s' \"$$\" > \"$DWARP_TEST_ARGUMENTS\"\nexec sleep 30\n",
    )
    .unwrap();
    let output = terminal
        .command()
        .args([
            "--focus",
            "--launch-timeout",
            "1",
            "--",
            "touch",
            "should-not-exist",
        ])
        .output()
        .unwrap();
    let pid = fs::read_to_string(terminal.directory.path().join("arguments"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let alive = kill(Pid::from_raw(pid), None::<Signal>).is_ok();
    let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
    assert!(alive, "a launch timeout closed the GUI process");
    assert_eq!(output.status.code(), Some(125));
    assert!(!terminal.directory.path().join("should-not-exist").exists());
}
