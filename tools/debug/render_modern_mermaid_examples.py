"""Render literal reference examples; execution success does not prove theme admission."""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

PRESETS = ("editor-light", "editor-dark", "brutalist", "spotless", "cyberpunk")
FORMATS = ("svg", "png", "pdf")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def signature_ok(data: bytes, fmt: str) -> bool:
    if fmt == "svg":
        return b"<svg" in data
    if fmt == "png":
        return data.startswith(b"\x89PNG\r\n\x1a\n")
    return data.startswith(b"%PDF-")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    ref = args.reference.resolve()
    cli = args.cli.resolve()
    work = args.output.resolve()
    source = ref / "MERMAID_EXAMPLES.md"
    text = source.read_text(encoding="utf-8")
    matches = list(re.finditer(r"^```mermaid[^\n]*\n(.*?)^```[ \t]*$", text, re.M | re.S))
    if not matches:
        parser.error("reference contains no fenced Mermaid examples")
    report = {
        "source_revision": subprocess.check_output(
            ["git", "-C", str(ref), "rev-parse", "HEAD"], text=True
        ).strip(),
        "source_file": source.name,
        "source_sha256": sha256(source),
        "cli_sha256": sha256(cli),
        "workspace_revision": subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
        ).strip(),
        "rows": [],
    }
    work.mkdir(parents=True, exist_ok=False)
    for index, match in enumerate(matches, 1):
        diagram = match.group(1)
        path = work / f"{index:02d}.mmd"
        path.write_text(diagram, encoding="utf-8")
        source_line = text[:match.start()].count("\n") + 2
        input_sha256 = sha256(path)
        successful = 0
        for preset in PRESETS:
            for fmt in FORMATS:
                output = work / f"{index:02d}-{preset}.{fmt}"
                command = [
                    str(cli), "render", "--quiet", "--format", fmt,
                    "--theme-preset", preset, "-o", str(output), str(path),
                ]
                row = {
                    "example": index,
                    "source_line": source_line,
                    "family": diagram.splitlines()[0].split()[0],
                    "input_sha256": input_sha256,
                    "preset": preset,
                    "format": fmt,
                }
                try:
                    run = subprocess.run(command, capture_output=True, text=True, timeout=30)
                    row.update(returncode=run.returncode, stderr=run.stderr)
                    if run.returncode == 0:
                        data = output.read_bytes()
                        row.update(
                            bytes=len(data),
                            sha256=hashlib.sha256(data).hexdigest(),
                            signature_ok=signature_ok(data, fmt),
                        )
                        successful += int(row["signature_ok"])
                except subprocess.TimeoutExpired:
                    row["returncode"] = "timeout"
                report["rows"].append(row)
        print(f"example {index}: {successful}/{len(PRESETS) * len(FORMATS)}", flush=True)
        (work / "results.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
    failed = sum(
        row["returncode"] != 0 or not row.get("signature_ok", False)
        for row in report["rows"]
    )
    print(f"Total: {len(report['rows'])} executions, {failed} failures")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
