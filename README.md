# dWarp

dWarp is a local terminal fork of [Warp](https://github.com/warpdotdev/warp),
based on [OpenWarp](https://github.com/zerx-lab/warp). It retains terminal blocks,
completion, keybindings, local files and workflows, SSH, and integrations with
third-party CLI agents.

Warp account login, Warp Drive sync, and the built-in Warp agent are removed.
AI command suggestions use a provider configured under Settings → Agents →
Providers. Provider keys stay in the operating system's secure storage.
CLI agents manage their own accounts, models, and MCP servers.

## Rendering

GPU rendering is the default. Enable **Use CPU renderer** in settings and restart,
or launch with:

```bash
WARP_CPU_RENDERER=1 dwarp
WARP_CPU_RENDERER=1 WARP_CPU_FPS=30 dwarp
WARP_CPU_RENDERER=1 WARP_CPU_TERMINAL_FPS=10 dwarp
```

CPU rendering avoids a GPU rendering device for the window. The desktop compositor
can still use the GPU. Native rendering behavior must be verified on each platform.

## Build and run

```bash
./script/bootstrap
./script/run
```

The application binary is `dwarp`. The build does not require Warp's private
channel configuration repository or a Warp server.

```bash
cargo build --release --bin dwarp
cargo run --bin dwarp
```

On Linux, build and install with:

```bash
./script/build-and-install.sh
```

The script installs the `dwarp` launcher, desktop entry, and icons. It retains
complete build logs under `logs/build-and-install/` and removes its scratch files
on success or failure. Compiled outputs and shared Cargo caches are preserved.

On Windows, run `script\build-and-install.bat`. Add `-BuildOnly` to build the
installer without opening its installation wizard. See
[Windows build instructions](docs/windows-build.md) for prerequisites.

See [WARP.md](WARP.md) for development commands and code conventions.

## Uninstall

See [uninstall instructions](docs/uninstall.md) for Linux, Windows, and macOS.
Uninstall preserves user data by default. `script/uninstall.sh --purge` on
Linux/macOS or `script\uninstall.bat -Purge` on Windows also deletes current-user
data. Use the documented dry-run option to inspect the affected paths.

## License and upstream

`crates/warpui` and `crates/warpui_core` use the MIT license. The rest of this
repository uses AGPL-3.0-only. See the license files for details.

dWarp is independent of Warp, Inc. Upstream changes are reviewed and selectively
adapted to the retained local features.
