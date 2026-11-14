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
                "https://mirrors.mit.edu/ubuntu\tpriority:0\n"
                "http://azure.archive.ubuntu.com/ubuntu/\tpriority:1\n"
                "http://archive.ubuntu.com/ubuntu/\tpriority:2\n"
                "https://security.ubuntu.com/ubuntu\n",
            )
            self.assertEqual(
                ports.read_text(encoding="utf-8"),
                "deb [arch=arm64 signed-by=/ubuntu.gpg] "
                f"mirror+file:{root.as_posix()}/merman-ci-http-ports.ubuntu.com-mirrors.txt jammy main\n",
            )
            self.assertEqual(
                (root / "merman-ci-http-ports.ubuntu.com-mirrors.txt").read_text(encoding="utf-8"),
                "https://mirrors.mit.edu/ubuntu-ports\tpriority:1\n"
                "http://ports.ubuntu.com/ubuntu-ports\tpriority:2\n",
            )
            self.assertEqual(
                (root / "apt.conf.d" / "zzzz-merman-ci").read_text(encoding="utf-8"),
                APT_OPTIONS,
            )
            snapshot = {path: path.read_bytes() for path in root.rglob("*") if path.is_file()}
            configure_apt(root)
            self.assertEqual(
                {path: path.read_bytes() for path in root.rglob("*") if path.is_file()},
                snapshot,
            )

    def test_direct_sources_use_native_failover_to_their_original_repository(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sources.list.d").mkdir()
            (root / "apt.conf.d").mkdir()
            for scheme, host, suffix in (
                ("http", "archive.ubuntu.com", "ubuntu"),
                ("http", "azure.archive.ubuntu.com", "ubuntu"),
                ("https", "security.ubuntu.com", "ubuntu"),
                ("http", "ports.ubuntu.com", "ubuntu-ports"),
            ):
                with self.subTest(host=host):
                    uri = f"{scheme}://{host}/{suffix}/"
                    sources = root / "sources.list.d" / f"{host}.sources"
                    sources.write_text(
                        f"Types: deb deb-src\nURIs: {uri}\n"
                        "Suites: noble-security\nComponents: main universe\n"
                        "Architectures: amd64 arm64\nSigned-By: /ubuntu.gpg\n",
                        encoding="utf-8",
                    )

                    configure_apt(root)

                    mirrors = root / f"merman-ci-{scheme}-{host}-mirrors.txt"
                    self.assertEqual(
                        sources.read_text(encoding="utf-8"),
                        f"Types: deb deb-src\nURIs: mirror+file:{mirrors.as_posix()}\n"
                        "Suites: noble-security\nComponents: main universe\n"
                        "Architectures: amd64 arm64\nSigned-By: /ubuntu.gpg\n",
                    )
                    self.assertEqual(
                        mirrors.read_text(encoding="utf-8"),
                        f"https://mirrors.mit.edu/{suffix}\tpriority:1\n{uri}\tpriority:2\n",
                    )

    def test_existing_mirror_metadata_and_external_repositories_are_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sources.list.d").mkdir()
            (root / "apt.conf.d").mkdir()
            mirrors = root / "apt-mirrors.txt"
            original = (
                "# Runner-managed choices\n"
                "http://archive.ubuntu.com/ubuntu\tpriority:5 arch:amd64\n"
                "http://ports.ubuntu.com/ubuntu-ports\tpriority:6 arch:arm64\n"
                "https://example.org/ubuntu\tpriority:7\n"
            )
            mirrors.write_text(original, encoding="utf-8")
            sources = root / "sources.list.d" / "external.list"
            external = "deb https://packages.microsoft.com/repos/code stable main\n"
            sources.write_text(external, encoding="utf-8")

            configure_apt(root)

            self.assertEqual(
                mirrors.read_text(encoding="utf-8"),
                "https://mirrors.mit.edu/ubuntu\tpriority:0\n"
                "https://mirrors.mit.edu/ubuntu-ports\tpriority:0\n" + original,
            )
            self.assertEqual(sources.read_text(encoding="utf-8"), external)


if __name__ == "__main__":
    unittest.main()
