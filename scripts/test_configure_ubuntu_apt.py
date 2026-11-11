from pathlib import Path
import tempfile
import unittest

from scripts.configure_ubuntu_apt import APT_OPTIONS, configure_apt


class UbuntuAptTests(unittest.TestCase):
    def test_known_sources_keep_signing_architectures_and_mirror_indirection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sources.list.d").mkdir()
            (root / "apt.conf.d").mkdir()
            sources = root / "sources.list.d" / "ubuntu.sources"
            original = (
                "Types: deb\nURIs: mirror+file:/etc/apt/apt-mirrors.txt\n"
                "Suites: noble noble-updates noble-security\n"
                "Components: main restricted universe multiverse\n"
                "Architectures: amd64 arm64\n"
                "Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n"
            )
            sources.write_text(original, encoding="utf-8")
            mirrors = root / "apt-mirrors.txt"
            mirrors.write_text(
                "http://azure.archive.ubuntu.com/ubuntu/\tpriority:1\n"
                "http://archive.ubuntu.com/ubuntu/\tpriority:2\n"
                "https://security.ubuntu.com/ubuntu\n",
                encoding="utf-8",
            )
            ports = root / "sources.list"
            ports.write_text(
                "deb [arch=arm64 signed-by=/ubuntu.gpg] "
                "http://ports.ubuntu.com/ubuntu-ports jammy main\n",
                encoding="utf-8",
            )
            unrelated = root / "sources.list.d" / "third-party.list"
            external = "deb https://packages.microsoft.com/repos/code stable main\n"
            unrelated.write_text(external, encoding="utf-8")

            configure_apt(root)

            self.assertEqual(sources.read_text(encoding="utf-8"), original)
            self.assertEqual(unrelated.read_text(encoding="utf-8"), external)
            self.assertEqual(
                mirrors.read_text(encoding="utf-8"),
                "https://mirrors.mit.edu/ubuntu/\tpriority:1\n"
                "https://mirrors.mit.edu/ubuntu/\tpriority:2\n"
                "https://mirrors.mit.edu/ubuntu\n",
            )
            self.assertEqual(
                ports.read_text(encoding="utf-8"),
                "deb [arch=arm64 signed-by=/ubuntu.gpg] "
                "https://mirrors.mit.edu/ubuntu-ports jammy main\n",
            )
            self.assertEqual(
                (root / "apt.conf.d" / "zzzz-merman-ci").read_text(encoding="utf-8"),
                APT_OPTIONS,
            )
            configure_apt(root)
            self.assertEqual(sources.read_text(encoding="utf-8"), original)


if __name__ == "__main__":
    unittest.main()
