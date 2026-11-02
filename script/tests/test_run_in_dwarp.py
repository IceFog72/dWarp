import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "run-in-dwarp"


class NativeLauncherTests(unittest.TestCase):
    def test_delegates_literal_arguments_and_exit_status_to_native_cli(self):
        with tempfile.TemporaryDirectory(prefix="dwarp-launcher-test-") as directory:
            root = Path(directory)
            executable = root / "dwarp"
            executable.write_text(
                '#!/bin/bash\nprintf "%s\\0" "$@" > "$ARGUMENT_FILE"\nexit 17\n'
            )
            executable.chmod(0o700)
            arguments = ["--group", "Codex", "--", "bash", "-lc", "echo '$HOME'; a b"]
            result = subprocess.run(
                ["bash", str(SCRIPT), *arguments],
                env=dict(os.environ, PATH=f"{root}:{os.environ['PATH']}", ARGUMENT_FILE=str(root / "arguments")),
                capture_output=True,
            )
            self.assertEqual(result.returncode, 17, result.stderr)
            self.assertEqual((root / "arguments").read_bytes().split(b"\0")[:-1],
                             [value.encode() for value in ["run", *arguments]])


if __name__ == "__main__":
    unittest.main()
