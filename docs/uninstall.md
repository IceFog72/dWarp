# Uninstall dWarp

Close dWarp/OpenWarp and its terminal sessions first. These scripts remove this
fork's installation and legacy OpenWarp launchers. They leave upstream Warp and
Cloudflare WARP alone. They do not remove the source checkout, build outputs,
installer downloads, remote SSH hosts, or other users' data.

The default keeps settings, history, SSH connections, provider credentials, and
custom themes. Purge permanently deletes those files for the invoking user,
including the legacy `.openwarp` directory. Back up anything you need first.

## Linux

Run from the repository root. Python 3 is required.

```sh
./script/uninstall.sh --dry-run
sudo ./script/uninstall.sh
sudo ./script/uninstall.sh --purge
```

The script removes installed `dwarp`, `warp-oss`, and `warp-terminal-oss`
launchers, desktop entries, icons, XFCE helpers and copied panel launchers,
and references in preferred-terminal and MIME configuration. Registered DEB,
RPM, and Arch installations are removed through their package manager.
Purge also clears SSH credentials from Secret Service and the Linux kernel
keyring. It requires `secret-tool` (libsecret tools), `keyctl` (keyutils) when
cached credentials exist, and an unlocked desktop keyring.
System installation paths require sudo. With sudo, purge still targets the
invoking user's home, rather than root's home. Custom XDG paths are honored
when present in the script's environment.

To remove only the old installation before installing dWarp:

```sh
sudo ./script/uninstall.sh --legacy-only
```

`--legacy-only` always preserves the settings shared with dWarp. The Linux
build-and-install script runs this cleanup after a successful build. Select
dWarp again in your desktop's preferred-terminal chooser if OpenWarp was selected.

## Windows

From the repository root, run these in Command Prompt:

```bat
script\uninstall.bat -WhatIf
script\uninstall.bat
script\uninstall.bat -Purge
```

The BAT file calls `script\windows\uninstall.ps1`. It runs the registered
OSS Inno Setup uninstaller, which removes tracked installation files, context
menus, shortcuts, and its PATH entry. It also cleans legacy integration remnants
and the app's runtime URL handlers. Purge also deletes this fork's SSH entries
from Windows Credential Manager. Use `-LegacyOnly` to remove only a registered
OpenWarp installation while preserving dWarp and shared data.

An all-users installation requires an administrator terminal under the same
Windows account. Purge affects that account only. An unregistered portable copy
must be deleted from its download/extraction directory manually.

## macOS

Python 3 is required. Preview first, then run with sudo if the app is installed
in `/Applications` or its CLI symlink is in `/usr/local/bin` or `/opt/homebrew/bin`.

```sh
./script/uninstall.sh --dry-run
sudo ./script/uninstall.sh
sudo ./script/uninstall.sh --purge
```

The script unregisters OpenWarp/dWarp app bundles in `/Applications` and
`~/Applications`, removes them and their CLI symlinks, and optionally deletes
this user's legacy settings, preferences, caches, logs, and saved application
state, and its SSH credentials in Keychain. For a custom app location, move the bundle to Trash yourself. Remove
any remaining pinned Dock shortcut through the Dock menu.

## Existing settings after the rename

The product name is **dWarp** and its command is **dwarp**. Storage identifiers
remain `openwarp`/`dev.openwarp.OpenWarp` so existing settings continue to load.
The uninstall scripts use those exact legacy paths when purging. The Windows
installer keeps its OSS registration ID to upgrade an existing installation.
The new `dwarp://` URL scheme also accepts legacy `openwarp://` links.
