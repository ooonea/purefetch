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

    @unittest.skipUnless(sys.platform == "linux", "Linux package managers")
    def test_native_package_queries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, body in {
                "rpm": '[ "$1" = -qa ] && [ "$2" = --queryformat ] || exit 1\nprintf "bash\\nlibfoo\\ngpg-pubkey\\n"',
                "xbps-query": 'test "$*" = "-l" || exit 1\nprintf "ii bash-5.3 shell\\nii musl-1.2 libc\\nuu pending-1.0 pending\\n"',
            }.items():
                helper = root / name
                helper.write_text("#!/bin/sh\n" + body + "\n")
                helper.chmod(0o755)
            env = {**os.environ, "PATH": directory}
            rpm = root / "rpm"
            output = self.run_fetch("--no-config", "--no-logo", "--modules", "packages", env=env)
            self.assertIn("2 (rpm)", output)
            self.assertIn("2 (xbps)", output)
            rpm.write_text('#!/bin/sh\nprintf "partial\\n"\nexit 1\n')
            output = self.run_fetch("--no-config", "--no-logo", "--modules", "packages", env=env)
            self.assertNotIn("(rpm)", output)

    def test_nix_user_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            profile = root / "profile"
            profile.mkdir()
            (root / ".nix-profile").symlink_to(profile, target_is_directory=True)
            state = root / "state"
            (state / "nix").mkdir(parents=True)
            (state / "nix/profile").symlink_to(profile, target_is_directory=True)
            helper = root / "nix-store"
            helper.write_text('#!/bin/sh\n[ "$1" = --query ] && [ "$2" = --requisites ] && [ "$3" = ' + shlex.quote(str(profile)) + ' ] || exit 1\nprintf "/nix/store/hash-hello-2.12.1\\n/nix/store/hash-hello-2.12.1-doc\\n/nix/store/hash-source\\n"\n')
            helper.chmod(0o755)
            output = self.run_fetch(
                "--no-config", "--no-logo", "--modules", "packages",
                env={**os.environ, "HOME": directory, "XDG_STATE_HOME": str(state), "PATH": directory},
            )
            self.assertEqual(output.count("1 (nix-user)"), 1)

    @unittest.skipUnless(sys.platform == "darwin", "macOS package managers")
    def test_homebrew_packages(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ["Cellar/hello", "Cellar/world", "Cellar/.metadata", "Caskroom/app"]:
                (root / name).mkdir(parents=True)
            (root / "Cellar/not-a-package").touch()
            output = self.run_fetch(
                "--no-config", "--no-logo", "--modules", "packages",
                env={**os.environ, "HOMEBREW_PREFIX": directory},
            )
            self.assertIn("2 (brew)", output)
            self.assertIn("1 (brew-cask)", output)

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
