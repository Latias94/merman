#!/usr/bin/env python3
"""Configure the Ubuntu package transport used by Merman's CI runners."""

from pathlib import Path
import re


MIRROR = "https://mirrors.mit.edu"
UBUNTU_URI = re.compile(
    r"(?P<scheme>https?)://(?P<origin>(?:azure\.archive|archive|security)\.ubuntu\.com/ubuntu"
    r"|ports\.ubuntu\.com/ubuntu-ports)/?(?=\s|$)"
)
APT_OPTIONS = '''Acquire::Retries "3";
Acquire::http::Timeout "30";
Acquire::https::Timeout "30";
APT::Update::Error-Mode "any";
'''


def configure_apt(root: Path = Path("/etc/apt")) -> None:
    """Prefer MIT with native APT failover; retain signing policy and original sources."""
    # Runner-provided mirror lists already have native failover and may carry metadata.
    # Prepend a preference instead of replacing their independent fallback hosts.
    for path in sorted(root.glob("*mirrors*.txt")):
        if path.name.startswith("merman-ci-") or not path.is_file():
            continue
        original = path.read_text(encoding="utf-8")
        mirrors = dict.fromkeys(
            f"{MIRROR}/{match['origin'].split('/', 1)[1]}\tpriority:0\n"
            for match in UBUNTU_URI.finditer(original)
        )
        missing = "".join(line for line in mirrors if line not in original.splitlines(keepends=True))
        if missing:
            path.write_text(missing + original, encoding="utf-8")
            print(f"Configured Ubuntu CI package mirrors in {path}")

    def mirror_source(match: re.Match[str]) -> str:
        host, archive = match["origin"].split("/", 1)
        mirrors = root / f"merman-ci-{match['scheme']}-{host}-mirrors.txt"
        mirrors.write_text(
            f"{MIRROR}/{archive}\tpriority:1\n{match[0]}\tpriority:2\n",
            encoding="utf-8",
        )
        return f"mirror+file:{mirrors.as_posix()}"

    sources = [root / "sources.list"]
    sources.extend(sorted((root / "sources.list.d").glob("*.list")))
    sources.extend(sorted((root / "sources.list.d").glob("*.sources")))
    for path in sources:
        if not path.is_file():
            continue
        original = path.read_text(encoding="utf-8")
        updated = UBUNTU_URI.sub(mirror_source, original)
        if updated != original:
            path.write_text(updated, encoding="utf-8")
            print(f"Configured Ubuntu CI package mirror in {path}")
    # These settings also apply to APT invoked through sudo by browser installers.
    (root / "apt.conf.d" / "zzzz-merman-ci").write_text(APT_OPTIONS, encoding="utf-8")


if __name__ == "__main__":
    configure_apt()
