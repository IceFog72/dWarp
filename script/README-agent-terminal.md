# Visible commands in dWarp

`dwarp run` runs a command inside the installed dWarp application and streams
its output and exit status to its caller. The implementation is native Rust:
it requires no Python interpreter, file polling, or heartbeat files.
`dwarp-run` and `script/run-in-dwarp` are shell aliases that exec `dwarp run`.
Rebuild and reinstall dWarp before using these aliases with this version.

```bash
dwarp run --group Codex --cwd /absolute/project -- cargo --version
dwarp run --group Codex --timeout 120 -- bash -lc 'printf "first\n"; printf "second\n"'
```

Put options before `--`. Everything afterward is a program and literal
arguments. Invoke a shell explicitly for pipelines, redirects, or expansion.
The command receives the caller's environment and requested working directory.
Executable paths preserve symlink names, including rustup shims.

Commands use a dedicated **Codex** tab by default. Select another name with
`--group NAME` or `DWARP_COMMAND_GROUP`. Idle panes are reused; concurrent
commands split within the owned tab. Ordinary user tabs are never claimed by
title. Group ownership lasts for the running application session. Busy panes,
typed input, pending launches, and remote sessions prevent reuse.
`--new-tab` requests a separate tab instead of a group.

## Focus and interactive prompts

Ordinary commands stay in the background. Keep dWarp running with an open
window. A background request cannot start a new GUI instance.
Use `--focus` when the user must answer a terminal prompt:

```bash
dwarp run --focus --group Codex -- sudo -v
```

This raises dWarp and selects the command's pane. The user types passwords
directly there. Input keystrokes are not logged; output is mirrored after the
command's terminal has applied its echo settings. Never supply passwords through
chat, tool input, `sudo -S`, arguments, or environment variables.

Commands have a PTY, so disable pagers for unattended output, for example with
`git --no-pager` or `SYSTEMD_PAGER=cat`. Leave interactive prompts available.

## Native transport and cancellation

The caller opens an account-private Unix socket and launches the native worker
inside dWarp using its existing local terminal IPC. A framed connection carries
the request, raw output bytes, and completion status. The worker gives the
command a controlling PTY and forwards input and terminal size changes from
dWarp. Stdout and stderr share one terminal stream. The socket path is removed
as soon as the worker receives its request; there is no persistent listener.

Interrupting the caller or losing its connection cancels the command's foreground
process group. Closing the dWarp terminal also cancels it. Cancellation sends
SIGINT, then SIGTERM after two seconds, then SIGKILL after another two seconds
if necessary. Deliberately detached sessions are not contained.

The caller returns the command's exit code, `124` for a command timeout, `130`
for cancellation, or `125` for a launch/protocol failure. Invalid CLI arguments
are rejected by the argument parser. `--timeout` and `--launch-timeout` accept
positive integer seconds. The launch timeout defaults to 30 seconds. A worker
that starts after its caller has gone cannot receive a request or execute it.

Logs remain under `${XDG_STATE_HOME:-~/.local/state}/dwarp/agent-runs/native-*`.
Each invocation prints its exact private directory. `output.log` contains raw
terminal output, including ANSI sequences. `result.json` contains completion
status and any cancellation/failure reason. `launcher.log` captures launch errors.
The environment is sent over the private socket and is not saved in these logs.
Command output can contain secrets printed by the program. Retain or remove
completed logs according to the user's instructions.

Linux is the exercised platform. The native worker uses Unix PTYs. Windows
requires a ConPTY backend. macOS background launches currently fail because its
local terminal launch IPC is not implemented. No TCP listener is opened.

The coding assistant still uses its registered shell tool to invoke `dwarp run`
and receive output. This does not replace the assistant application's execution
backend or restore Warp's removed built-in agent.

## Requested follow-up

System/default-terminal launches should optionally find a tab already open in
the requested folder and add a split there. Add an on/off setting and a
configurable split limit, initially four. At the limit, open another tab.
