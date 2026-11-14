#!/usr/bin/env python3
"""Bound Ubuntu CI package transport without overriding runner-selected repositories."""

from pathlib import Path


APT_OPTIONS = '''Acquire::Retries "3";
Acquire::http::Timeout "30";
Acquire::https::Timeout "30";
APT::Update::Error-Mode "any";
'''


def configure_apt(root: Path = Path("/etc/apt")) -> None:
    """Keep repository selection and signing policy under the runner's ownership."""
    # These settings also apply to APT invoked through sudo by browser installers.
    (root / "apt.conf.d" / "zzzz-merman-ci").write_text(APT_OPTIONS, encoding="utf-8")


if __name__ == "__main__":
    configure_apt()
