#!/usr/bin/env python3
"""Run the container's public CLI with no network and a read-only root filesystem."""

from __future__ import annotations

import argparse
import json
import subprocess
import xml.etree.ElementTree as ET


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image")
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    command = [
        "docker", "run", "--rm", "--read-only", "--network", "none",
        "--tmpfs", "/tmp:rw,size=64m", "-i", args.image,
    ]

    def run(*arguments: str, source: bytes = b"") -> bytes:
        result = subprocess.run(
            [*command, *arguments], input=source, capture_output=True, check=True, timeout=60,
        )
        return result.stdout

    assert run("--version").decode().strip() == f"merman-cli {args.version}"
    capabilities = json.loads(run("capabilities", "--json"))
    assert capabilities["package"]["version"] == args.version
    source = b"flowchart LR\n A[Merman] --> B[Container]\n"
    assert ET.fromstring(run("render", "-", "--format", "svg", source=source)).tag.endswith("svg")
    assert run("render", "-", "--format", "png", source=source).startswith(b"\x89PNG\r\n\x1a\n")
    assert run("render", "-", "--format", "jpg", source=source).startswith(b"\xff\xd8")
    assert run("render", "-", "--format", "pdf", source=source).startswith(b"%PDF-")
    text = run("render", "-", "--format", "unicode", source=source)
    assert b"Merman" in text and b"Container" in text
    print(f"verified container {args.image}: {args.version}, SVG, PNG, JPEG, PDF, Unicode")


if __name__ == "__main__":
    main()
