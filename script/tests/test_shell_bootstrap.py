from pathlib import Path
import re
import shutil
import subprocess
import unittest


BOOTSTRAP = Path(__file__).resolve().parents[2] / "app/assets/bundled/bootstrap"


class ShellBootstrapTests(unittest.TestCase):
    def bash_function(self, name):
        source = (BOOTSTRAP / "bash_body.sh").read_text()
        return re.search(rf"{name} \(\) \{{.*?\n    \}}", source, re.S).group()

    def fish_function(self, name):
        source = (BOOTSTRAP / "fish.sh").read_text()
        return re.search(rf"^function {name}(?: |\n).*?^end$", source, re.M | re.S).group()

    def test_bash_hex_encoding_preserves_literal_bytes_without_added_newline(self):
        function = self.bash_function("warp_hex_encode_string")
        for value in ["-n", "back\\slash", "two\nlines", "Привіт"]:
            with self.subTest(value=value):
                result = subprocess.run(
                    ["bash", "-c", function + '\nwarp_hex_encode_string "$1"', "--", value],
                    capture_output=True, text=True, check=True,
                )
                self.assertEqual(result.stdout, value.encode().hex())

    @unittest.skipUnless(shutil.which("fish"), "fish is required")
    def test_fish_hex_encoding_preserves_literal_bytes_without_added_newline(self):
        function = self.fish_function("warp_hex_encode_string")
        for value in ["-n", "back\\slash", "two\nlines", "Привіт"]:
            with self.subTest(value=value):
                result = subprocess.run(
                    ["fish", "-c", function + '\nwarp_hex_encode_string "$argv[1]"', "--", value],
                    capture_output=True, text=True, check=True,
                )
                self.assertEqual(result.stdout, value.encode().hex())

    @unittest.skipUnless(shutil.which("fish"), "fish is required")
    def test_fish_preexec_cancels_generators_only_for_user_commands(self):
        functions = '''
function warp_escape_json
    printf '%s' "$argv"
end
function warp_send_json_message
end
function warp_maybe_send_reset_grid_osc
end
function kill
    set -ga killed "$argv"
end
set -g _warp_generator_pids 123 456
'''
        functions += self.fish_function("warp_preexec")
        for command, expected in [
            ("   warp_run_generator_command 1 true", ""),
            ("echo user-command", "-9 123\n-9 456\n"),
        ]:
            with self.subTest(command=command):
                result = subprocess.run(
                    ["fish", "-c", functions + '''
warp_preexec "$argv[1]"
if test (count $killed) -gt 0
    printf '%s\\n' $killed
end
''', "--", command],
                    capture_output=True, text=True, check=True,
                )
                self.assertEqual(result.stdout, expected)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is required")
    def test_ssh_zsh_initialization_disables_rc_options(self):
        for name in ["bash_body.sh", "fish.sh", "zsh_body.sh"]:
            with self.subTest(name=name):
                source = (BOOTSTRAP / name).read_text()
                options = re.search(r"(unsetopt ZLE.*?); WARP_SESSION_ID", source).group(1)
                subprocess.run(
                    ["zsh", "-c", options + "; [[ ! -o RCS && ! -o GLOBAL_RCS ]]"],
                    capture_output=True, check=True,
                )


if __name__ == "__main__":
    unittest.main()
