import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parents[1]


class BuildDependencyTests(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory(prefix="dwarp-dependency-test-")
        self.addCleanup(workspace.cleanup)
        self.root = Path(workspace.name)
        (self.root / "script").mkdir()
        (self.root / "bin").mkdir()
        self.env = dict(os.environ, PATH=f"{self.root / 'bin'}:{os.environ['PATH']}")

    def executable(self, name, contents):
        path = self.root / name
        path.write_text("#!/usr/bin/env bash\n" + contents + "\n")
        path.chmod(0o755)

    def run_script(self, name):
        (self.root / "script" / name).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(SCRIPTS / name, self.root / "script" / name)
        return subprocess.run(
            [str(self.root / "script" / name)], cwd=self.root,
            env=self.env, capture_output=True, text=True,
        )

    def test_linux_build_setup_needs_no_cargo_tools_or_private_access(self):
        self.executable("bin/uname", "echo Linux")
        self.executable("bin/cargo", "exit 99")
        self.executable("script/install_channel_config", "exit 98")
        result = self.run_script("install_cargo_build_deps")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_macos_build_setup_keeps_native_toolchain_dependency(self):
        self.executable("bin/uname", "echo Darwin")
        (self.root / "script/macos").mkdir()
        self.executable("script/macos/install_build_deps", "touch metal-toolchain-requested")
        result = self.run_script("install_cargo_build_deps")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((self.root / "metal-toolchain-requested").exists())

    def test_arch_setup_installs_native_dependencies_once_without_removed_libraries(self):
        (self.root / "script/linux").mkdir()
        shutil.copy2(SCRIPTS / "linux/install_build_deps", self.root / "script/linux")
        shutil.copy2(SCRIPTS / "install_cargo_build_deps", self.root / "script")
        self.executable("script/install_rust", "exit 0")
        self.executable("bin/uname", "echo Linux")
        self.executable("bin/sudo", 'printf "%s\\n" "$*" >> package-args')
        shell_env = self.root / "shell-env"
        shell_env.write_text('''source() {
  if [[ $1 == /etc/os-release ]]; then
    ID=arch
    ID_LIKE=""
  else
    builtin source "$@"
  fi
}
''')
        self.env["BASH_ENV"] = str(shell_env)
        result = self.run_script("linux/install_runtime_deps")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("Unknown Linux distribution", result.stdout)
        calls = (self.root / "package-args").read_text().splitlines()
        self.assertEqual(len(calls), 2)
        self.assertTrue(all(call.startswith("pacman -S --needed --noconfirm") for call in calls))
        packages = " ".join(calls).split()
        self.assertIn("protobuf", packages)
        self.assertIn("libxkbcommon-x11", packages)
        self.assertNotIn("alsa-lib", packages)
        self.assertNotIn("libgit2", packages)

    def test_run_uses_oss_without_installing_or_selecting_private_config(self):
        self.executable("bin/uname", "echo Linux")
        self.executable("bin/warp-channel-config", "exit 0")
        self.executable("bin/cargo", 'printf "%s\\n" "$@" > cargo-args')
        self.executable("script/install_channel_config", "exit 98")
        self.env.pop("WARP_BIN_NAME", None)
        self.env.pop("WARP_CHANNEL", None)
        result = self.run_script("run")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(
            (self.root / "cargo-args").read_text().splitlines(),
            ["run", "--bin", "dwarp", "--features", "gui"],
        )


if __name__ == "__main__":
    unittest.main()
