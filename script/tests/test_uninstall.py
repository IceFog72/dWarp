"""Exercise destructive cleanup in a temporary home, with system paths blocked."""
import argparse
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("uninstall", Path(__file__).parents[1] / "uninstall.py")
uninstall = importlib.util.module_from_spec(spec)
spec.loader.exec_module(uninstall)


class UninstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        uninstall.home = self.home
        uninstall.args = argparse.Namespace(dry_run=False, purge=False, legacy_only=False)
        uninstall.removed_paths.clear()

    def write(self, relative, content="fixture"):
        path = self.home / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    def cleanup_linux(self):
        def sandbox(command):
            self.assertEqual(command[:3], ["rm", "-rf", "--"])
            path = Path(command[3])
            self.assertTrue(path.is_relative_to(self.home), command)
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path)
            else:
                path.unlink(missing_ok=True)

        real_remove = uninstall.remove

        def remove_user_path(path, system=False):
            if system:
                self.assertTrue(str(path).startswith(("/usr/", "/opt/warpdotdev/")))
            else:
                self.assertTrue(path.is_relative_to(self.home), path)
                real_remove(path)

        with patch.dict(os.environ, {
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_DATA_HOME": str(self.home / ".local/share"),
            "XDG_CACHE_HOME": str(self.home / ".cache"),
            "XDG_STATE_HOME": str(self.home / ".local/state"),
        }), patch.object(uninstall.shutil, "which", return_value=None), \
                patch.object(uninstall, "remove", side_effect=remove_user_path), \
                patch.object(uninstall, "run", side_effect=sandbox), \
                patch.object(uninstall, "purge_linux_credentials") as credentials, \
                contextlib.redirect_stdout(io.StringIO()):
            uninstall.linux()
            self.assertEqual(credentials.call_count, int(uninstall.args.purge))

    def test_normal_uninstall_preserves_user_data_and_unrelated_apps(self):
        binary = self.write(".local/bin/dwarp")
        legacy = self.write(".local/bin/warp-terminal-oss.real")
        launcher = self.write(".local/share/applications/dev.warp.dWarp.desktop")
        panel = self.write(".config/xfce4/panel/launcher-1/1.desktop", "Exec=dwarp %U\n")
        settings = self.write(".config/openwarp/settings.json")
        upstream = self.write(".local/bin/warp")
        cloudflare = self.write(".local/share/applications/com.cloudflare.WarpTaskbar.desktop")
        mime = self.write(".config/mimeapps.list", "[Default Applications]\nx-scheme-handler/dwarp=dev.warp.dWarp.desktop;other.desktop;\ntext/plain=editor.desktop;\n")
        self.cleanup_linux()
        for path in [binary, legacy, launcher, panel]:
            self.assertFalse(path.exists(), path)
        for path in [settings, upstream, cloudflare]:
            self.assertTrue(path.exists(), path)
        self.assertEqual(mime.read_text(), "[Default Applications]\nx-scheme-handler/dwarp=other.desktop;\ntext/plain=editor.desktop;\n")

    def test_legacy_cleanup_preserves_dwarp_and_shared_data(self):
        uninstall.args.legacy_only = True
        old = self.write(".local/bin/warp-oss")
        new = self.write(".local/bin/dwarp")
        data = self.write(".openwarp/settings.json")
        self.cleanup_linux()
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())
        self.assertTrue(data.exists())

    def test_purge_removes_exact_data_directories_only(self):
        uninstall.args.purge = True
        owned = [self.write(path + "/file") for path in [
            ".openwarp", ".config/openwarp", ".cache/openwarp", ".local/state/openwarp", ".local/share/openwarp", ".warp-oss/remote-server",
        ]]
        protected = [self.write(path) for path in [
            ".ssh/id_ed25519", ".warp/settings.json", "project/.warp/workflow.yaml", ".config/openwarp-custom/settings.json",
        ]]
        self.cleanup_linux()
        self.assertTrue(all(not path.exists() for path in owned))
        self.assertTrue(all(path.exists() for path in protected))

    def test_linux_credentials_only_revoke_this_apps_service(self):
        metadata = "000000ab I--Q--- 1 perm 3f010000 1000 1000 user keyring-rs:node:password@openwarp.ssh: 8\n" + \
                   "000000cd I--Q--- 1 perm 3f010000 1000 1000 user keyring-rs:node:password@other.ssh: 8\n"
        with patch.object(uninstall, "user_command", side_effect=lambda value: value), \
                patch.object(uninstall.shutil, "which", return_value="tool"), \
                patch.object(uninstall.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, metadata)), \
                patch.object(uninstall, "run") as commands:
            uninstall.purge_linux_credentials()
        self.assertEqual([call.args[0] for call in commands.call_args_list], [
            ["secret-tool", "clear", "service", "openwarp.ssh"],
            ["keyctl", "revoke", "171"],
        ])

    def test_dry_run_keeps_files_and_preferences(self):
        uninstall.args.dry_run = True
        file = self.write(".openwarp/settings.json")
        preferences = self.write(".config/mimeapps.list", "handler=dev.warp.dWarp.desktop;other.desktop;\n")
        with contextlib.redirect_stdout(io.StringIO()):
            uninstall.remove(file)
            uninstall.clean_preferences(preferences, {"dev.warp.dWarp.desktop"})
        self.assertTrue(file.exists())
        self.assertIn("dev.warp.dWarp.desktop", preferences.read_text())


if __name__ == "__main__":
    unittest.main()
