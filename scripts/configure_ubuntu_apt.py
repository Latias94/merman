#!/usr/bin/env python3
"""Configure the Ubuntu package transport used by Merman's CI runners."""

from pathlib import Path
import re


MIRROR = "https://mirrors.mit.edu"
UBUNTU_URI = re.compile(
    r"https?://(?:(?:azure\.archive|archive|security)\.ubuntu\.com/ubuntu"
    r"|ports\.ubuntu\.com/ubuntu-ports)(?=/|\s|$)"
)
APT_OPTIONS = '''Acquire::Retries "3";
Acquire::http::Timeout "30";
Acquire::https::Timeout "30";
APT::Update::Error-Mode "any";
'''


def configure_apt(root: Path = Path("/etc/apt")) -> None:
    """Keep repository declarations and signing policy; replace known URI prefixes."""
    sources = [root / "sources.list"]
    sources.extend(sorted((root / "sources.list.d").glob("*.list")))
    sources.extend(sorted((root / "sources.list.d").glob("*.sources")))
    sources.extend(sorted(root.glob("*mirrors*.txt")))
    for path in sources:
        if not path.is_file():
            continue
        original = path.read_text(encoding="utf-8")
        updated = UBUNTU_URI.sub(
            lambda match: MIRROR + (
                "/ubuntu-ports" if "/ubuntu-ports" in match[0] else "/ubuntu"
            ),
            original,
        )
        if updated != original:
            path.write_text(updated, encoding="utf-8")
            print(f"Configured Ubuntu CI package mirror in {path}")
    # These settings also apply to APT invoked through sudo by browser installers.
    (root / "apt.conf.d" / "zzzz-merman-ci").write_text(APT_OPTIONS, encoding="utf-8")


if __name__ == "__main__":
    configure_apt()
