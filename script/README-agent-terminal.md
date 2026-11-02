# Visible commands in dWarp

`run-in-dwarp` runs a command inside the installed dWarp application and mirrors
its output to the calling tool. This lets you watch commands from an external
coding assistant in the terminal. It does not restore Warp's removed built-in
agent or connect to a Warp service.

```bash
./script/run-in-dwarp --cwd /mnt/Data/Apps/warp -- cargo --version
./script/run-in-dwarp --timeout 120 -- python3 -m unittest discover -s script/tests -p test_run_in_dwarp.py
```

Put wrapper options before `--`. Everything afterward is a program and its
literal arguments. For shell syntax, explicitly invoke a shell:

```bash
./script/run-in-dwarp -- bash -lc 'printf "first\n"; printf "second\n"'
```

Commands use a dedicated **Codex** tab by default. An idle terminal pane is
reused; concurrent commands get split panes in that tab. Choose `--group NAME`
to own another tab, or set `DWARP_COMMAND_GROUP`. This never selects a user's
ordinary tab merely because its title happens to match. Group ownership lasts
for the running application session and is not restored onto a user's saved tabs.
Typed input, pending launches, busy commands, and remote sessions prevent reuse.
The requested working directory is applied before each command in the shell.

Grouped launches require rebuilding dWarp and restarting an older running
instance. An old installed launcher is rejected with an explicit error.
`--new-tab` requests a separate tab on the rebuilt app and still uses background launches.

The application uses its normal local `--execute` launcher. The command runs in
a nested PTY, so stdin, stdout, and stderr remain terminals. Output appears in
dWarp as it happens and returns to the calling tool. You can type into prompts
in the dWarp pane. Closing the pane or interrupting the caller cancels the task.
Cancellation sends SIGINT, then SIGTERM after two seconds, then SIGKILL after
another two seconds if the command still runs. A disconnected caller is detected
through a heartbeat within 15 seconds. This is process-group cancellation, not
containment of programs that deliberately detach into separate sessions.

The wrapper returns the command's exit code. Wrapper failures return `125`,
timeouts return `124`, and cancellation returns `130`. Launches time out after
30 seconds unless `--launch-timeout` changes that limit. An expired or cancelled
launch cannot execute later. A task directory can be claimed by only one worker.

Logs and metadata remain under
`${XDG_STATE_HOME:-~/.local/state}/dwarp/agent-runs/`. Each invocation prints its
exact directory. The directory and files are private to your account. `output.log`
contains raw terminal bytes, including ANSI sequences and merged stdout/stderr.
`result.json` records completion and the exit status. `launcher.log` captures
launcher errors. Logs can contain command arguments and output, so remove old
completed task directories when you no longer need them. Temporary test fixtures
are cleaned automatically.

The command inherits the dWarp shell's environment. Pass overrides explicitly,
for example `-- env CARGO_BUILD_JOBS=4 cargo check`. Executable paths preserve
symlink names so rustup and other executable shims continue to work.

This implementation requires Python 3 and POSIX PTYs. It is tested on Linux.
macOS has the required PTY facilities but has not been exercised here. Windows
requires a separate ConPTY implementation. No network listener is opened.

The coding assistant still uses its registered command tool to launch this
wrapper and collect output. The command itself executes in dWarp. This does not
replace the assistant application's registered execution backend.

## Requested follow-up

System/default-terminal launches should optionally find a tab already open in
the requested folder and add a split there. Add an on/off setting and a
configurable split limit, initially four. At the limit, open another tab. Review
this after the external-command workflow is complete.

Agent commands always use `--no-focus`. Keep dWarp running with an open window.
The runner does not raise the application or start a GUI when it is closed.
Background local terminal IPC is implemented on Linux and Windows. macOS
background requests currently fail explicitly because its local terminal IPC
is not implemented. The POSIX runner itself has no Windows PTY backend.
