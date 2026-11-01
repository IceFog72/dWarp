#!/usr/bin/env python3
"""Uninstall this fork on Linux/macOS without touching upstream Warp installations."""
import argparse
import os
from pathlib import Path
import pwd
import re
import shlex
import shutil
import subprocess
import sys


def run(command):
    print(shlex.join(command), flush=True)
    if not args.dry_run:
        subprocess.run(command, check=True)


removed_paths = set()


def remove(path, system=False):
    if path in removed_paths:
        return
    removed_paths.add(path)
    if not path.exists() and not path.is_symlink():
        return
    command = ["rm", "-rf", "--", str(path)]
    if system and os.geteuid() != 0:
        command = ["sudo", "-n", *command]
    run(command)


def clean_preferences(path, identifiers):
    if not path.is_file():
        return
    lines = path.read_text().splitlines(keepends=True)
    result = []
    for line in lines:
        key, separator, value = line.partition("=")
        if not separator or line.lstrip().startswith(("#", ";")):
            result.append(line)
            continue
        items = value.strip().split(";")
        remaining = [item for item in items if item and item not in identifiers]
        if any(item in identifiers for item in items):
            if remaining:
                suffix = ";" if value.strip().endswith(";") else ""
                result.append(key + "=" + ";".join(remaining) + suffix + "\n")
        else:
            result.append(line)
    if result != lines:
        print(f"Remove dWarp references from {path}", flush=True)
        if not args.dry_run:
            path.write_text("".join(result))


def user_command(command):
    if os.geteuid() == 0 and os.environ.get("SUDO_USER"):
        username = os.environ["SUDO_USER"]
        if sys.platform == "linux":
            uid = pwd.getpwnam(username).pw_uid
            command = ["env", f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus",
                       f"XDG_RUNTIME_DIR=/run/user/{uid}", *command]
        return ["sudo", "-H", "-u", username, *command]
    return command


def purge_linux_credentials():
    if not shutil.which("secret-tool"):
        raise RuntimeError("purge requires secret-tool (libsecret tools) to delete SSH credentials")
    run(user_command(["secret-tool", "clear", "service", "openwarp.ssh"]))
    # keyring also caches SSH credentials in the Linux kernel keyring.
    # /proc/keys contains metadata only. Never read individual key payloads.
    keys = subprocess.run(user_command(["cat", "/proc/keys"]),
                          capture_output=True, text=True, check=True)
    for line in keys.stdout.splitlines():
        if re.search(r"\buser\s+keyring-rs:[^\s]+@openwarp\.ssh(?::\s+\d+)?\s*$", line):
            if not shutil.which("keyctl"):
                raise RuntimeError("purge requires keyctl (keyutils) to revoke cached SSH credentials")
            key_id = str(int(line.split()[0], 16))
            run(user_command(["keyctl", "revoke", key_id]))


def purge_macos_credentials():
    command = user_command(["security", "delete-generic-password", "-s", "openwarp.ssh"])
    if args.dry_run:
        print(shlex.join(command) + " (repeat until no matching items remain)", flush=True)
        return
    while True:
        # Never request or print password values. Only delete this app's service.
        result = subprocess.run(command, capture_output=True)
        if result.returncode == 44:  # errSecItemNotFound (-25300), truncated to an exit code.
            return
        if result.returncode != 0:
            raise RuntimeError("could not delete SSH credentials from Keychain")


def linux():
    ids = ["dev.warp.OpenWarp"]
    commands = ["warp-oss", "warp-terminal-oss"]
    packages = ["warp-terminal-oss"]
    if not args.legacy_only:
        ids.append("dev.warp.dWarp")
        commands.append("dwarp")
        packages.append("dwarp")
    # Let the owning package manager remove registered installations first.
    for package in packages:
        command = None
        if shutil.which("dpkg-query"):
            status = subprocess.run(["dpkg-query", "-W", "-f=${Status}", package],
                                    capture_output=True, text=True)
            if status.returncode == 0 and status.stdout == "install ok installed":
                command = ["apt-get", "remove", "-y", package]
        elif shutil.which("rpm"):
            if subprocess.run(["rpm", "-q", package], stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL).returncode == 0:
                command = ["rpm", "-e", package]
        elif shutil.which("pacman"):
            if subprocess.run(["pacman", "-Q", package], stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL).returncode == 0:
                command = ["pacman", "-R", "--noconfirm", package]
        if command:
            run((["sudo", "-n"] if os.geteuid() != 0 else []) + command)
    for base, system in [(Path("/usr/local"), True), (Path("/usr"), True),
                         (home / ".local", False)]:
        for command in commands:
            for suffix in ["", ".real"]:
                remove(base / "bin" / (command + suffix), system)
        for desktop_id in ids:
            for directory in ["share/applications", "share/xfce4/helpers"]:
                remove(base / directory / (desktop_id + ".desktop"), system)
            icon_root = base / "share/icons/hicolor"
            if icon_root.is_dir():
                for icon in icon_root.glob("*/apps/" + desktop_id + ".*"):
                    if icon.suffix in (".png", ".svg", ".xpm"):
                        remove(icon, system)
    for package in packages:
        remove(Path("/opt/warpdotdev") / package, True)
    data = Path(os.environ.get("XDG_DATA_HOME", str(home / ".local/share")))
    config = Path(os.environ.get("XDG_CONFIG_HOME", str(home / ".config")))
    # XDG_DATA_HOME can differ from ~/.local/share.
    for desktop_id in ids:
        remove(data / "applications" / (desktop_id + ".desktop"))
        remove(data / "xfce4/helpers" / (desktop_id + ".desktop"))
        for icon in (data / "icons/hicolor").glob("*/apps/" + desktop_id + ".*"):
            if icon.suffix in (".png", ".svg", ".xpm"):
                remove(icon)
    identifiers = set(ids + [value + ".desktop" for value in ids] + commands)
    for directory in [config, data / "applications"]:
        for preferences in directory.glob("*mimeapps.list"):
            clean_preferences(preferences, identifiers)
    clean_preferences(config / "xfce4/helpers.rc", identifiers)
    for launcher in (config / "xfce4/panel").glob("launcher-*/*.desktop"):
        if launcher.is_file():
            for line in launcher.read_text().splitlines():
                if line.startswith("Exec="):
                    try:
                        executable = Path(shlex.split(line[5:])[0]).name
                    except (ValueError, IndexError):
                        continue
                    if executable in commands:
                        remove(launcher)
    for file in [config / "xdg-terminals.list", config / "xfce4-xdg-terminals.list"]:
        if file.is_file():
            lines = file.read_text().splitlines(keepends=True)
            result = [line for line in lines if line.strip() not in identifiers]
            if result != lines:
                print(f"Remove dWarp references from {file}", flush=True)
                if not args.dry_run:
                    file.write_text("".join(result))
    if args.purge:
        purge_linux_credentials()
        for base in [config, data,
                     Path(os.environ.get("XDG_CACHE_HOME", str(home / ".cache"))),
                     Path(os.environ.get("XDG_STATE_HOME", str(home / ".local/state")))]:
            remove(base / "openwarp")
        remove(home / ".openwarp")
        remove(home / ".warp-oss" / "remote-server")
    for share in [Path("/usr/share"), Path("/usr/local/share"), data]:
        system = share != data
        prefix = ["sudo", "-n"] if system and os.geteuid() != 0 else []
        if shutil.which("update-desktop-database") and (share / "applications").is_dir():
            run(prefix + ["update-desktop-database", str(share / "applications")])
        if shutil.which("gtk-update-icon-cache") and (share / "icons/hicolor").is_dir():
            run(prefix + ["gtk-update-icon-cache", "-f", "-t", str(share / "icons/hicolor")])


def macos():
    names = ["OpenWarp"] if args.legacy_only else ["OpenWarp", "dWarp"]
    lsregister = Path("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
    for parent in [Path("/Applications"), home / "Applications"]:
        for name in names:
            app = parent / (name + ".app")
            if app.exists():
                if lsregister.is_file():
                    run([str(lsregister), "-u", str(app)])
                remove(app, parent == Path("/Applications"))
    for name in ["warp-oss"] + ([] if args.legacy_only else ["dwarp"]):
        for parent in [Path("/usr/local/bin"), Path("/opt/homebrew/bin"), home / ".local/bin"]:
            link = parent / name
            # Do not remove a manually installed, unrelated executable.
            if link.is_symlink() and any(n + ".app/" in str(link.readlink()) for n in names):
                remove(link, parent != home / ".local/bin")
    if args.purge:
        purge_macos_credentials()
        preferences = home / "Library/Preferences/dev.openwarp.OpenWarp.plist"
        if preferences.exists():
            command = ["defaults", "delete", "dev.openwarp.OpenWarp"]
            if os.geteuid() == 0 and os.environ.get("SUDO_USER"):
                command = ["sudo", "-H", "-u", os.environ["SUDO_USER"], *command]
            run(command)
        for log in (home / "Library/Logs").glob("openwarp.log*"):
            if log.name == "openwarp.log" or log.name.startswith("openwarp.log."):
                remove(log)
        for relative in [".openwarp", ".warp-oss/remote-server",
                         "Library/Application Support/dev.openwarp.OpenWarp",
                         "Library/Caches/dev.openwarp.OpenWarp",
                         "Library/Logs/dev.openwarp.OpenWarp",
                         "Library/Preferences/dev.openwarp.OpenWarp.plist",
                         "Library/Saved Application State/dev.openwarp.OpenWarp.savedState"]:
            remove(home / relative)


def main():
    global args, home
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-only", action="store_true", help="remove OpenWarp binaries and launchers, keeping dWarp and user data")
    parser.add_argument("--purge", action="store_true", help="also permanently delete this user's settings, history, caches, and local credentials")
    parser.add_argument("--dry-run", action="store_true", help="print changes without modifying files")
    args = parser.parse_args()
    if args.legacy_only and args.purge:
        parser.error("--legacy-only preserves shared user data and cannot be combined with --purge")
    if sys.platform not in ("linux", "darwin"):
        parser.error("use script\\uninstall.bat on Windows")
    # sudo must clean the invoking user's data, never root's home by accident.
    home = Path(pwd.getpwnam(os.environ["SUDO_USER"]).pw_dir) if os.geteuid() == 0 and os.environ.get("SUDO_USER") else Path.home()
    if home == Path("/"):
        parser.error("cannot resolve a safe user home directory")
    try:
        linux() if sys.platform == "linux" else macos()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Uninstall incomplete: {error}. Run with sudo for system installations, after closing the app.", file=sys.stderr)
        return 1
    print("Dry run complete." if args.dry_run else "Uninstall complete." + (" User data preserved. Use --purge to delete it." if not args.purge else " User data deleted."))


if __name__ == "__main__":
    sys.exit(main())
