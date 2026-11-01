# Build and install on Windows

Run from Command Prompt or PowerShell in a Windows checkout:

```bat
script\build-and-install.bat
```

This builds the OSS channel with the existing release bundler, packages its
runtime DLLs and resources with Inno Setup, and opens the installation wizard.
The wizard uses a per-user installation and does not reboot Windows automatically.
It remains interactive so you can review any request to close a running app.
The script stops if the build, packaging, or installation fails.

```bat
rem Build the installer without running it.
script\build-and-install.bat -BuildOnly

rem Select a target architecture and version label.
script\build-and-install.bat -Arch x64 -ReleaseTag v0.0.0-local
script\build-and-install.bat -Arch arm64 -BuildOnly
```

The default target is x64. ARM64 builds also need the corresponding Visual Studio
C++ build tools. Cross-built installers must be installed on a compatible machine.
The default local version label is `v0.0.0-local`.

## Prerequisites

Install these tools and reopen your terminal so their PATH changes take effect:

- Rust through rustup. The checkout selects its pinned toolchain from
  `rust-toolchain.toml`.
- Git for Windows.
- Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK.
- CMake and Protocol Buffers (`protoc`).
- Node.js and Yarn. Enable the project's Yarn version through Corepack if needed.
- Inno Setup. The script finds `ISCC.exe` on PATH or in a standard Program Files
  installation directory.

The script installs the selected Rust target and, if missing, the same
`cargo-about` version used by the release tooling. It does not install tests or
require a Google Cloud login.

## Output and CPU mode

The installer is written to `script\windows\Output\OpenWarpSetup.exe`, or
`OpenWarpSetup-arm64.exe` for ARM64. Cargo build output honors `CARGO_TARGET_DIR`.
The build includes both renderers. GPU rendering stays the default. Enable
**Use CPU renderer** in Settings and restart the installed app to select CPU mode.

The batch file delegates to `script\windows\build-and-install.ps1`. Its execution
policy override applies only to that PowerShell process.

Windows build, install, and first-frame rendering checks remain pending on a
Windows host. Tests are deferred until the remaining implementation work is done.
