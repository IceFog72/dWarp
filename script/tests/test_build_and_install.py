import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import tomllib
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "build-and-install.sh"


class BuildAndInstallCleanupTests(unittest.TestCase):
    def setUp(self):
        self.workspace = tempfile.TemporaryDirectory(prefix="dwarp-build-test-")
        self.addCleanup(self.workspace.cleanup)
        self.root = Path(self.workspace.name)
        scripts = self.root / "script"
        (scripts / "linux").mkdir(parents=True)
        shutil.copy2(SCRIPT, scripts / SCRIPT.name)
        for name in ("install_build_deps", "install_runtime_deps"):
            self.executable(scripts / "linux" / name, f'echo {name} >> "$PWD/dependency-calls"')
        with (scripts / "linux/install_runtime_deps").open("a") as runtime:
            runtime.write('"$PWD/script/linux/install_build_deps"\n')
        tools = self.root / "bin"
        tools.mkdir()
        self.executable(tools / "cargo", '''
mkdir -p "$CARGO_TARGET_DIR/release-lto"
printf '%s\\n' "$@" > "$CARGO_TARGET_DIR/cargo-args"
while [[ $# -gt 0 ]]; do
  if [[ $1 == --bin ]]; then binary=$2; fi
  shift
done
printf '#!/bin/sh\\n' > "$CARGO_TARGET_DIR/release-lto/$binary"
chmod +x "$CARGO_TARGET_DIR/release-lto/$binary"
printf scratch > "$TMPDIR/compiler-temp"
if [[ ${CHECK_BUILD_WAIT:-0} == 1 ]]; then sleep 60; fi
exit "${CHECK_BUILD_STATUS:-0}"
''')
        self.executable(tools / "sudo", '''
if [[ ${CHECK_INSTALL_FAIL:-0} == 1 && ${!#} == /usr/local/bin/dwarp ]]; then
  exit 29
fi
exit 0
''')
        self.env = dict(
            os.environ,
            PATH=f"{tools}:{os.environ['PATH']}",
            CARGO_TARGET_DIR=str(self.root / "target"),
            WARP_CPU_RENDERER="1",
        )

    def executable(self, path, contents):
        path.write_text("#!/usr/bin/env bash\n" + contents + "\n")
        path.chmod(0o755)

    def run_build(self, channel="oss", **changes):
        return subprocess.run(
            [str(self.root / "script" / SCRIPT.name), channel],
            env=dict(self.env, **changes), capture_output=True, text=True,
        )

    def assert_clean_with_log(self, status):
        self.assertEqual(list((self.root / "target").glob("build-install.*")), [])
        self.assertTrue((self.root / "target/release-lto/dwarp").exists())
        logs = list((self.root / "logs/build-and-install").glob("*.log"))
        self.assertEqual(len(logs), 1)
        self.assertIn(f"exited with status {status}", logs[0].read_text())

    def test_success_keeps_binary_and_log_but_removes_scratch(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_clean_with_log(0)
        self.assertEqual(
            (self.root / "dependency-calls").read_text().splitlines(),
            ["install_runtime_deps", "install_build_deps"],
        )

    def test_build_channels_only_request_declared_features(self):
        manifest = SCRIPT.parents[1] / "app/Cargo.toml"
        declared_features = tomllib.loads(manifest.read_text())["features"]
        for channel, binary in (("oss", "dwarp"),):
            with self.subTest(channel=channel):
                result = self.run_build(channel)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                args = (self.root / "target/cargo-args").read_text().splitlines()
                self.assertEqual(args[args.index("--bin") + 1], binary)
                selected = args[args.index("--features") + 1].split(",")
                self.assertEqual(set(selected) - declared_features.keys(), set())

    def test_removed_channels_fail_before_build_or_dependency_install(self):
        for channel in ("dev", "local", "stable", "preview"):
            with self.subTest(channel=channel):
                result = self.run_build(channel)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("Unknown channel", result.stdout + result.stderr)
                self.assertFalse((self.root / "dependency-calls").exists())
                self.assertFalse((self.root / "target/cargo-args").exists())

    def test_build_failure_removes_scratch_and_keeps_exit_status(self):
        result = self.run_build(CHECK_BUILD_STATUS="23")
        self.assertEqual(result.returncode, 23, result.stdout + result.stderr)
        self.assert_clean_with_log(23)

    def test_wrapper_install_failure_removes_temporary_launcher(self):
        result = self.run_build(CHECK_INSTALL_FAIL="1")
        self.assertEqual(result.returncode, 29, result.stdout + result.stderr)
        self.assert_clean_with_log(29)

    def test_interrupted_build_removes_scratch_and_keeps_log(self):
        process = subprocess.Popen(
            [str(self.root / "script" / SCRIPT.name)],
            env=dict(self.env, CHECK_BUILD_WAIT="1"),
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                if list((self.root / "target").glob("build-install.*/compiler-temp")):
                    break
                time.sleep(0.01)
            self.assertTrue(list((self.root / "target").glob("build-install.*/compiler-temp")))
            os.killpg(process.pid, signal.SIGTERM)
            self.assertEqual(process.wait(timeout=5), 143)
            self.assert_clean_with_log(143)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


if __name__ == "__main__":
    unittest.main()
