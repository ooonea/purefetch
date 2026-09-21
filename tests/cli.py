import fcntl
import os
from pathlib import Path
import pty
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest


COMMAND = sys.argv[1:]


class CliTests(unittest.TestCase):
    def run_fetch(self, *args, **kwargs):
        result = subprocess.run(
            [*COMMAND, *args], capture_output=True, text=True, timeout=10, **kwargs
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def test_command_changes_process_group(self):
        script = "import os,time; os.setpgid(0,os.getpgid(os.getppid())); time.sleep(5)"
        command = "exec " + shlex.join([sys.executable, "-c", script])
        start = time.monotonic()
        self.run_fetch("--no-config", "--no-logo", "--modules", "Test", "--exec", "Test:" + command)
        self.assertLess(time.monotonic() - start, 4)

    def test_exec_override(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "config"
            config.write_text("exec Test:printf old\nno-logo\n")
            for modules in [[], ["--modules", "test"]]:
                output = self.run_fetch(
                    "--config", str(config), "--exec", "TEST:printf new", *modules
                )
                self.assertEqual(re.findall(r"(?im)^test\s+(\w+)$", output), ["new"])

    def test_narrow_terminal(self):
        with tempfile.TemporaryDirectory() as directory:
            logo = Path(directory) / "logo"
            logo.write_text("L" * 30 + "\n")
            for width, expected in [(39, "Test"), (40, "Test  …"), (42, "Test  12…")]:
                master, slave = pty.openpty()
                try:
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, width, 0, 0))
                    with subprocess.Popen(
                        [*COMMAND, "--no-config", "--no-color", "--logo-file", str(logo),
                         "--modules", "Test", "--exec", "Test:printf 1234567890"],
                        stdout=slave, stderr=slave,
                    ) as process:
                        process.wait(timeout=10)
                        os.set_blocking(master, False)
                        data = b""
                        while True:
                            try:
                                chunk = os.read(master, 4096)
                            except BlockingIOError:
                                break
                            if not chunk:
                                break
                            data += chunk
                        self.assertEqual(process.returncode, 0)
                        row = data.decode().splitlines()[-1]
                        self.assertEqual(row.strip(), expected)
                        self.assertLessEqual(len(row), width)
                finally:
                    os.close(master)
                    os.close(slave)

    @unittest.skipUnless(sys.platform == "linux", "Linux session detection")
    def test_unrelated_compositor(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / "sway"
            fake.symlink_to(shutil.which("bash"))
            with subprocess.Popen([str(fake), "-c", "read -r line"], stdin=subprocess.PIPE) as unrelated:
                try:
                    self.assertIsNone(unrelated.poll())
                    self.assertEqual(Path(f"/proc/{unrelated.pid}/comm").read_text().strip(), "sway")
                    output = self.run_fetch(
                        "--no-config", "--no-logo", "--modules", "wm",
                        env={**os.environ, "XDG_CURRENT_DESKTOP": "GNOME",
                             "XDG_SESSION_TYPE": "wayland"},
                    )
                    self.assertIn("Mutter (Wayland)", output)
                    self.assertNotIn("Sway", output)
                finally:
                    unrelated.terminate()
                    unrelated.wait()


unittest.main(argv=[sys.argv[0]])
