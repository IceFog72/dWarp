import os
from pathlib import Path
import signal
import shutil
import subprocess
import tempfile
import time
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "cleanup-debug.sh"


class CleanupDebugTests(unittest.TestCase):
    def setUp(self):
        self.workspace = tempfile.TemporaryDirectory(prefix="dwarp-cleanup-test-")
        self.addCleanup(self.workspace.cleanup)
        self.root = Path(self.workspace.name)
        self.shared = self.root / "shared-target"
        self.shared.mkdir()
        (self.shared / "keep").write_text("existing build output")
        self.env = dict(
            os.environ,
            DWARP_DEBUG_TMPDIR=str(self.root),
            CARGO_TARGET_DIR=str(self.shared),
        )

    def assert_clean(self):
        self.assertEqual(list(self.root.glob("dwarp-debug.*")), [])
        self.assertEqual((self.shared / "keep").read_text(), "existing build output")

    def test_success_removes_private_output_and_keeps_logs(self):
        log = self.root / "retained.log"
        result = subprocess.run(
            [
                str(SCRIPT), "bash", "-c",
                'printf artifact > "$CARGO_TARGET_DIR/output"; printf retained > "$1"',
                "check", str(log),
            ],
            env=self.env, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(log.read_text(), "retained")
        self.assert_clean()

    def test_failure_preserves_exit_code_and_removes_output(self):
        result = subprocess.run(
            [str(SCRIPT), "bash", "-c", 'touch "$CARGO_TARGET_DIR/output"; exit 23'],
            env=self.env, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assert_clean()

    def test_missing_command_removes_output(self):
        result = subprocess.run(
            [str(SCRIPT), "/nonexistent/dwarp-check"],
            env=self.env, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 127, result.stderr)
        self.assert_clean()

    @unittest.skipUnless(Path("/dev/full").exists(), "requires /dev/full")
    def test_out_of_space_failure_removes_partial_output(self):
        result = subprocess.run(
            [str(SCRIPT), "bash", "-c",
             'touch "$CARGO_TARGET_DIR/partial"; printf dependency > /dev/full'],
            env=self.env, capture_output=True, text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No space left on device", result.stderr)
        self.assert_clean()

    def test_default_uses_project_disk_instead_of_tmpdir(self):
        scripts = self.root / "script"
        scripts.mkdir()
        copied_script = scripts / SCRIPT.name
        shutil.copy2(SCRIPT, copied_script)
        env = dict(self.env, TMPDIR="/nonexistent/dwarp-tmp")
        env.pop("DWARP_DEBUG_TMPDIR")
        result = subprocess.run(
            [str(copied_script), "bash", "-c", 'printf "%s" "$CARGO_TARGET_DIR"'],
            env=env, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(result.stdout).parent, self.root / "target")
        self.assertFalse(Path(result.stdout).exists())

    def test_clean_removes_abandoned_output_but_preserves_active_and_shared_paths(self):
        tools = self.root / "bin"
        tools.mkdir()
        pgrep = tools / "pgrep"
        pgrep.write_text("#!/bin/sh\nexit 1\n")
        pgrep.chmod(0o755)
        abandoned = self.root / "dwarp-debug.12345678"
        abandoned.mkdir()
        (abandoned / "partial-build").write_text("incomplete")
        active = self.root / "dwarp-debug.abcdefgh"
        active.mkdir()
        (active / ".owner-pid").write_text(str(os.getpid()))
        link = self.root / "dwarp-debug.symlink1"
        link.symlink_to(self.shared, target_is_directory=True)
        result = subprocess.run(
            [str(SCRIPT), "--clean", str(self.root)],
            env=dict(self.env, PATH=f"{tools}:{os.environ['PATH']}"),
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(abandoned.exists())
        self.assertTrue(active.exists())
        self.assertTrue(link.is_symlink())
        self.assertEqual((self.shared / "keep").read_text(), "existing build output")

    def test_clean_refuses_while_a_build_is_running(self):
        tools = self.root / "bin"
        tools.mkdir()
        pgrep = tools / "pgrep"
        pgrep.write_text("#!/bin/sh\nexit 0\n")
        pgrep.chmod(0o755)
        abandoned = self.root / "dwarp-debug.12345678"
        abandoned.mkdir()
        result = subprocess.run(
            [str(SCRIPT), "--clean", str(self.root)],
            env=dict(self.env, PATH=f"{tools}:{os.environ['PATH']}"),
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertTrue(abandoned.exists())

    def test_termination_removes_output(self):
        ready = self.root / "ready"
        process = subprocess.Popen(
            [str(SCRIPT), "bash", "-c", 'touch "$1"; sleep 60', "check", str(ready)],
            env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(ready.exists())
            os.killpg(process.pid, signal.SIGTERM)
            self.assertEqual(process.wait(timeout=5), 143)
            self.assert_clean()
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


if __name__ == "__main__":
    unittest.main()
