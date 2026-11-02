import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "run-in-dwarp"
loader = importlib.machinery.SourceFileLoader("run_in_dwarp", str(SCRIPT))
spec = importlib.util.spec_from_loader(loader.name, loader)
adapter = importlib.util.module_from_spec(spec)
loader.exec_module(adapter)


@unittest.skipUnless(os.name == "posix", "Requires POSIX PTYs")
class RunInDwarpTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="dwarp-adapter-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.cwd = self.root / "directory with spaces"
        self.cwd.mkdir()
        self.launcher = self.root / "fake-dwarp"
        self.launcher.write_text(f"#!{sys.executable}\n" + '''
import os, sys
if "--help" in sys.argv:
    print("--command-group --no-focus")
    sys.exit(0)
import json
from pathlib import Path
Path(os.environ["XDG_STATE_HOME"], "launcher-args.json").write_text(json.dumps(sys.argv[1:]))
command = sys.argv[sys.argv.index("--execute") + 1:]
os.execv(command[0], command)
''')
        self.launcher.chmod(0o700)
        self.env = dict(os.environ, XDG_STATE_HOME=str(self.root / "state"),
                        PYTHONDONTWRITEBYTECODE="1")

    def command(self, code, *arguments, options=()):
        return [sys.executable, str(SCRIPT), "--warp", str(self.launcher),
                "--cwd", str(self.cwd), *options, "--", sys.executable, "-c", code,
                *arguments]

    def run_command(self, code, *arguments, options=()):
        return subprocess.run(self.command(code, *arguments, options=options), env=self.env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=12)

    def task(self):
        directories = list((self.root / "state/dwarp/agent-runs").iterdir())
        self.assertEqual(len(directories), 1)
        return directories[0]

    def result(self):
        return json.loads((self.task() / "result.json").read_text())

    def test_command_has_a_pty_and_output_and_failure_status_are_preserved(self):
        result = self.run_command(
            "import os,sys; print('TTY',*[os.isatty(i) for i in range(3)]); "
            "print('error output',file=sys.stderr); sys.exit(23)"
        )
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertIn(b"TTY True True True", result.stdout)
        self.assertIn(b"error output", result.stdout)
        self.assertEqual(self.result()["exit_code"], 23)
        self.assertEqual((self.task() / "output.log").read_bytes(), result.stdout)
        forwarded = json.loads((self.root / "state/launcher-args.json").read_text())
        self.assertIn("--no-focus", forwarded)
        self.assertEqual(forwarded[forwarded.index("--command-group") + 1], "Codex")

    def test_old_launcher_is_rejected_even_with_new_tab(self):
        source = self.launcher.read_text().replace('print("--command-group --no-focus")', 'print("old launcher")')
        self.launcher.write_text(source)
        result = self.run_command("print('visible')")
        self.assertEqual(result.returncode, 125)
        self.assertIn(b"needs rebuilding", result.stderr)
        self.assertFalse((self.root / "state").exists())
        result = self.run_command("print('visible')", options=["--new-tab"])
        self.assertEqual(result.returncode, 125)
        self.assertFalse((self.root / "state").exists())

    def test_cwd_and_arguments_are_not_interpreted_by_a_shell(self):
        arguments = ['spaces and "quotes"', "$(touch unintended)", "line1\nline2", ""]
        result = self.run_command("import json,os,sys; print(json.dumps([os.getcwd(),sys.argv[1:]]))",
                                  *arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), [str(self.cwd), arguments])
        self.assertFalse((self.cwd / "unintended").exists())

    def test_large_and_binary_output_is_not_truncated(self):
        result = self.run_command("import os; os.write(1,b'x'*150000+b'\\x00\\xff')")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, b"x" * 150000 + b"\x00\xff")

    def test_executable_symlink_keeps_its_dispatch_name(self):
        dispatcher = self.cwd / "dispatcher"
        dispatcher.write_text('#!/bin/sh\nprintf "%s\\n" "${0##*/}"\n')
        dispatcher.chmod(0o700)
        (self.cwd / "cargo-shim").symlink_to(dispatcher)
        command = [sys.executable, str(SCRIPT), "--warp", str(self.launcher),
                   "--cwd", str(self.cwd), "--", "./cargo-shim"]
        result = subprocess.run(command, env=self.env, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), b"cargo-shim")

    def test_expired_caller_does_not_start_a_late_worker(self):
        task = self.root / "expired-task"
        task.mkdir(mode=0o700)
        adapter.write_json(task / "request.json", {
            "argv": [sys.executable, "-c", "open('unintended','w').close()"],
            "cwd": str(self.cwd), "timeout": None,
        })
        heartbeat = task / "heartbeat"
        heartbeat.touch()
        os.utime(heartbeat, (time.time() - 60, time.time() - 60))
        result = subprocess.run([sys.executable, str(SCRIPT), "_worker", str(task)],
                                env=self.env, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 130, result.stderr)
        self.assertFalse((self.cwd / "unintended").exists())

    def test_task_and_logs_are_private(self):
        result = self.run_command("print('private output')")
        self.assertEqual(result.returncode, 0, result.stderr)
        for path in [self.task(), *self.task().iterdir()]:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode) & 0o077, 0, path)

    def test_command_timeout_interrupts_the_child(self):
        result = self.run_command("import time; print('started',flush=True); time.sleep(30)",
                                  options=["--timeout", "0.2"])
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertEqual(self.result()["reason"], "command timed out")

    def test_cancellation_escalates_for_a_child_ignoring_signals(self):
        process = subprocess.Popen(self.command(
            "import signal,time; signal.signal(signal.SIGINT,signal.SIG_IGN); "
            "signal.signal(signal.SIGTERM,signal.SIG_IGN); print('ready',flush=True); time.sleep(30)"
        ), env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                tasks = list((self.root / "state/dwarp/agent-runs").glob("*/output.log"))
                if tasks and b"ready" in tasks[0].read_bytes():
                    break
                time.sleep(0.02)
            else:
                self.fail("Child did not become ready")
            process.send_signal(signal.SIGTERM)
            output, errors = process.communicate(timeout=7)
            self.assertEqual(process.returncode, 130, errors)
            self.assertIn(b"ready", output)
            self.assertEqual(self.result()["command_exit_code"], 137)
            child = json.loads((self.task() / "started.json").read_text())["command_pid"]
            with self.assertRaises(ProcessLookupError):
                os.kill(child, 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()

    def test_launch_timeout_prevents_delayed_execution(self):
        self.launcher.write_text(f'#!{sys.executable}\nprint("--command-group --no-focus")\n')
        result = self.run_command("open('unintended','w').close()",
                                  options=["--launch-timeout", "0.1"])
        self.assertEqual(result.returncode, 125, result.stderr)
        worker = subprocess.run([sys.executable, str(SCRIPT), "_worker", str(self.task())],
                                env=self.env, capture_output=True, timeout=3)
        self.assertEqual(worker.returncode, 130, worker.stderr)
        self.assertFalse((self.cwd / "unintended").exists())

    def test_duplicate_worker_does_not_execute_again(self):
        result = self.run_command("open('runs','a').write('one\\n')")
        self.assertEqual(result.returncode, 0, result.stderr)
        duplicate = subprocess.run([sys.executable, str(SCRIPT), "_worker", str(self.task())],
                                   env=self.env, capture_output=True, timeout=3)
        self.assertEqual(duplicate.returncode, 125)
        self.assertEqual((self.cwd / "runs").read_text(), "one\n")
        self.assertEqual(self.result()["exit_code"], 0)

    def test_missing_launcher_fails_without_running_command(self):
        self.launcher.unlink()
        result = self.run_command("open('unintended','w').close()")
        self.assertEqual(result.returncode, 125)
        self.assertFalse((self.cwd / "unintended").exists())

    def test_shared_or_symlinked_directory_is_rejected(self):
        folder = self.root / "unsafe"
        folder.mkdir(mode=0o755)
        folder.chmod(0o755)
        with self.assertRaises(ValueError):
            adapter.private_directory(folder)
        folder.chmod(0o700)
        link = self.root / "link"
        link.symlink_to(folder, target_is_directory=True)
        with self.assertRaises(ValueError):
            adapter.private_directory(link)


if __name__ == "__main__":
    unittest.main()
