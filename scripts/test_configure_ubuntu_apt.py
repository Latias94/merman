from pathlib import Path
import tempfile
import unittest

from scripts.configure_ubuntu_apt import APT_OPTIONS, configure_apt


class UbuntuAptTests(unittest.TestCase):
    def test_runner_sources_and_mirror_priorities_remain_byte_identical(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sources.list.d").mkdir()
            (root / "apt.conf.d").mkdir()
            originals = {
                "sources.list": (
                    "deb [arch=arm64 signed-by=/ubuntu.gpg] "
                    "http://ports.ubuntu.com/ubuntu-ports noble main\n"
                ),
                "sources.list.d/ubuntu.sources": (
                    "Types: deb deb-src\nURIs: mirror+file:/etc/apt/apt-mirrors.txt\n"
                    "Suites: noble noble-updates noble-backports\n"
                    "Components: main restricted universe multiverse\n"
                    "Architectures: amd64 arm64\n"
                    "Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n\n"
                    "Types: deb\nURIs: http://security.ubuntu.com/ubuntu/\n"
                    "Suites: noble-security\nComponents: main universe\n"
                    "Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n"
                ),
                "sources.list.d/third-party.list": (
                    "deb https://packages.microsoft.com/repos/code stable main\n"
                ),
                "apt-mirrors.txt": (
                    "# Runner-managed mirror choices\n"
                    "http://us-east-2.ec2.archive.ubuntu.com/ubuntu/\tpriority:1\n"
                    "http://azure.archive.ubuntu.com/ubuntu/\tpriority:2 arch:amd64\n"
                    "http://archive.ubuntu.com/ubuntu/\n"
                ),
                "apt.conf.d/01runner": 'Acquire::Check-Valid-Until "true";\n',
            }
            for name, text in originals.items():
                (root / name).write_text(text, encoding="utf-8")
            snapshot = {path: path.read_bytes() for path in root.rglob("*") if path.is_file()}

            configure_apt(root)

            for path, content in snapshot.items():
                self.assertEqual(path.read_bytes(), content, path)
            options = root / "apt.conf.d/zzzz-merman-ci"
            self.assertEqual(options.read_text(encoding="utf-8"), APT_OPTIONS)
            self.assertEqual(
                {path for path in root.rglob("*") if path.is_file()},
                set(snapshot) | {options},
            )
            snapshot[options] = options.read_bytes()
            configure_apt(root)
            self.assertEqual(
                {path: path.read_bytes() for path in root.rglob("*") if path.is_file()},
                snapshot,
            )

    def test_update_and_child_installers_retain_bounded_strict_transport(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "apt.conf.d").mkdir()

            configure_apt(root)

            config = (root / "apt.conf.d/zzzz-merman-ci").read_text(encoding="utf-8")
            self.assertIn('Acquire::Retries "3";', config)
            self.assertIn('Acquire::http::Timeout "30";', config)
            self.assertIn('Acquire::https::Timeout "30";', config)
            self.assertIn('APT::Update::Error-Mode "any";', config)


if __name__ == "__main__":
    unittest.main()
