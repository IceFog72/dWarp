# Terminal launches

Ordinary Linux and Windows invocations send a new terminal tab to the running
instance. `--new-window` requests a window in that instance. Development data
profiles use separate IPC endpoints.

```sh
dwarp --working-directory /path/to/project
dwarp --new-window --working-directory /path/to/project
dwarp --working-directory /path/to/project --execute python3 script.py --verbose
dwarp -d /path/to/project -e sh -lc 'printf "%s\n" "$PWD"'
```

Put `--execute` (`-e`) last. Everything after it is part of the executable's
argument vector. Shell syntax is interpreted only when a shell is explicitly
requested, as in the final example. The executable must be a file or resolve in
the caller's PATH. Shell functions, aliases and builtins are not executable files.
Missing directories and executables fail before creating a terminal. Paths and
arguments must be valid UTF-8 and cannot contain NUL.

The directory and argument vector travel through local IPC. The new terminal
waits for shell integration, then passes literal arguments to its managed shell.
The terminal stays open after the command finishes. Public custom URLs do not
accept executable requests. Cold launches restore saved windows and create the
requested terminal without adding an extra default window.

## Linux desktop selection

The OSS desktop entry declares `X-TerminalArgExec=--execute` and
`X-TerminalArgDir=--working-directory` for
[xdg-terminal-exec](https://github.com/Vladimir-csp/xdg-terminal-exec).
When that launcher is installed, users can select `dev.warp.dWarp.desktop`
through its configuration.

`script/build-and-install.sh` also installs an XFCE terminal helper. Select
dWarp in XFCE's Default Applications chooser. On supported XFCE installations,
Warp's default-terminal Settings action opens that chooser. Cancelling the chooser
does not mark Warp as selected. This integration does not change a system-wide
`x-terminal-emulator` alternative.

## Validation pending

Tests and native launch checks are deferred until the remaining desktop work is
finished. Windows and macOS native behavior has not been verified on this Linux
host. Exact argument handling through legacy PowerShell needs native coverage.
Direct macOS executable launches still need existing-instance routing.
