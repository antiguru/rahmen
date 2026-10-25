"""Run the slideshow wrapper against temporary files and mocked devices."""

import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest


MOCK_COMMAND = r'''
import os
from pathlib import Path
import signal
import subprocess
import sys

root = Path(os.environ["FIXTURE_ROOT"])
command = Path(sys.argv[0]).name
mode = os.environ.get("FAILURE_MODE", "")
with (root / "events").open("a") as events:
    events.write(command + " " + " ".join(sys.argv[1:]) + "\n")

if command == "tty":
    print(os.environ.get("FIXTURE_TTY", "/dev/tty1"))
elif command == "sleep":
    os.kill(os.getppid(), signal.SIGTERM)
elif command == "find":
    count_file = root / "attempts"
    count = int(count_file.read_text()) + 1 if count_file.exists() else 1
    count_file.write_text(str(count))
    if count > 2 or mode in ("terminate", "hangup"):
        os.kill(os.getppid(), signal.SIGHUP if mode == "hangup" else signal.SIGTERM)
        sys.exit(1)
    if mode == "find":
        print("partial.jpg")
        sys.exit(1)
    sys.exit(subprocess.call([os.environ["REAL_FIND"], *sys.argv[1:]]))
elif command == "shuf":
    sys.stdout.write(sys.stdin.read())
    if mode == "shuf":
        sys.exit(1)
elif command == "mktemp":
    if mode == "mktemp":
        sys.exit(1)
    if mode == "redirection":
        print(root / "missing-directory" / "list.tmp")
    else:
        sys.exit(subprocess.call([os.environ["REAL_MKTEMP"], *sys.argv[1:]]))
elif command == "mv":
    if mode == "mv":
        sys.exit(1)
    sys.exit(subprocess.call([os.environ["REAL_MV"], *sys.argv[1:]]))
elif command == "rahmen":
    (root / "displayed").write_text(Path(sys.argv[-1]).read_text())
    sys.exit(1)
else:
    raise AssertionError(command)
'''


class PicframeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.images = self.root / "images"
        self.images.mkdir()
        (self.images / "local-picframe").touch()
        self.home = self.root / "home"
        self.home.mkdir()
        self.image_list = self.home / "pic1.list"
        self.image_list.write_text("previous.jpg\n")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for command in ("tty", "sleep", "find", "shuf", "mktemp", "mv", "rahmen"):
            path = self.home / command if command == "rahmen" else self.bin / command
            path.write_text(f"#!{sys.executable}\n" + MOCK_COMMAND)
            path.chmod(0o755)

        # Substitute deployment paths in a temporary copy to keep the real
        # console, home directory, framebuffer, and mount out of these tests.
        source = Path(__file__).with_name("picframe.sh").read_text()
        source = source.replace("/media/picframe/", str(self.images) + "/")
        source = source.replace("/home/pi/", str(self.home) + "/")
        self.script = self.root / "picframe.sh"
        self.script.write_text(source)
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"])
        self.env.update(
            FIXTURE_ROOT=str(self.root),
            REAL_FIND=shutil.which("find"),
            REAL_MKTEMP=shutil.which("mktemp"),
            REAL_MV=shutil.which("mv"),
        )

    def run_script(self, mode="", tty="/dev/tty1"):
        env = dict(self.env, FAILURE_MODE=mode, FIXTURE_TTY=tty)
        process = subprocess.Popen(
            ["bash", str(self.script)], env=env, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, start_new_session=True,
        )
        try:
            self.output = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            self.fail("slideshow wrapper failed to terminate")
        self.events = (self.root / "events").read_text().splitlines()
        return process.returncode

    def assert_retry_without_display(self):
        self.assertIn("sleep 60", self.events)
        self.assertFalse(any(event.startswith("rahmen ") for event in self.events), self.events)
        self.assertEqual(self.image_list.read_text(), "previous.jpg\n")
        self.assertEqual(sorted(path.name for path in self.home.iterdir()), ["pic1.list", "rahmen"])

    def test_failed_player_waits_before_regenerating(self):
        (self.images / "photo.jpg").touch()
        self.run_script()
        commands = [event.split()[0] for event in self.events]
        self.assertIn("sleep 60", self.events)
        self.assertEqual(commands.count("find"), 1)
        self.assertLess(commands.index("rahmen"), commands.index("sleep"))

    def test_missing_player_waits_before_regenerating(self):
        (self.images / "photo.jpg").touch()
        (self.home / "rahmen").unlink()
        self.run_script()
        self.assertIn("sleep 60", self.events)
        self.assertEqual(sum(event.startswith("find ") for event in self.events), 1)

    def test_failed_generation_preserves_previous_list(self):
        for failure in ("find", "shuf", "mktemp", "redirection", "mv"):
            with self.subTest(failure=failure):
                (self.root / "events").unlink(missing_ok=True)
                (self.root / "attempts").unlink(missing_ok=True)
                self.image_list.write_text("previous.jpg\n")
                (self.images / "photo.jpg").touch()
                self.run_script(failure)
                self.assert_retry_without_display()

    def test_empty_generation_preserves_previous_list(self):
        self.run_script()
        self.assert_retry_without_display()

    def test_supported_extensions_are_case_insensitive(self):
        names = ["one.jpg", "two.JPG", "three.jpeg", "four.JpEg", "five.png", "six.PNG"]
        for name in names + ["ignore.gif", "ignore.png.txt"]:
            (self.images / name).touch()
        (self.images / "directory.jpg").mkdir()
        self.run_script()
        displayed = (self.root / "displayed").read_text().splitlines()
        self.assertEqual(set(displayed), {str(self.images / name) for name in names})

    def test_termination_cleans_temporary_list(self):
        for mode, exit_code in (("terminate", 143), ("hangup", 129)):
            with self.subTest(signal=mode):
                (self.root / "attempts").unlink(missing_ok=True)
                actual_code = self.run_script(mode)
                self.assertEqual(self.image_list.read_text(), "previous.jpg\n")
                self.assertEqual(
                    sorted(path.name for path in self.home.iterdir()),
                    ["pic1.list", "rahmen"],
                )
                self.assertEqual(actual_code, exit_code)

    def test_other_consoles_exit_without_accessing_images(self):
        self.assertEqual(self.run_script(tty="/dev/pts/1"), 0)
        self.assertEqual(self.events, ["tty "])

    def test_missing_mount_waits_without_running_player(self):
        (self.images / "local-picframe").unlink()
        self.run_script()
        self.assert_retry_without_display()
        self.assertFalse(any(event.startswith("find ") for event in self.events))


if __name__ == "__main__":
    unittest.main()
