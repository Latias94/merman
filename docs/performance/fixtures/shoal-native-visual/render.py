"""Render the Shoal visual corpus with a native Merman CLI."""

import argparse
import json
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="path to merman-cli")
    parser.add_argument("--out-dir", required=True, type=Path, help="new directory for PNGs")
    args = parser.parse_args()

    corpus = Path(__file__).resolve().parent
    cases = json.loads((corpus / "manifest.json").read_text(encoding="utf-8"))
    if not cases or len({case["id"] for case in cases}) != len(cases):
        parser.error("the manifest must have cases with distinct IDs")
    try:
        binary = args.binary.resolve(strict=True)
        args.out_dir.mkdir(parents=True)
    except OSError as error:
        parser.error(str(error))

    failed = []
    for case in cases:
        case_id = case["id"]
        output = args.out_dir / f"{case_id}.png"
        print(f"{case_id}  {case['review']}", flush=True)
        command = [
            binary, "render", corpus / case["source"], "--output", output,
            "--format", "png", "--scale", "2",
            "--font-measurement", "native", "--font-system", "on",
            "--theme", case["theme"], "--appearance", case["appearance"],
            "--raster-max-width", "16384", "--raster-max-height", "16384",
            "--raster-max-pixels", "100000000",
        ]
        if "background" in case:
            command.extend(["--background", case["background"]])
        result = subprocess.run(command, stderr=subprocess.PIPE, encoding="utf-8", errors="replace")
        if result.stderr:
            sys.stderr.write(result.stderr)
        if (
            result.returncode
            or "Raster output was constrained from" in result.stderr
            or not output.is_file()
        ):
            print(f"{case_id}  FAILED to produce a full-scale PNG", file=sys.stderr)
            failed.append(case_id)

    print(f"{len(cases) - len(failed)}/{len(cases)} full-scale PNGs in {args.out_dir}")
    if failed:
        print(f"Review failures for {', '.join(failed)}", file=sys.stderr)
    return int(bool(failed))


if __name__ == "__main__":
    sys.exit(main())
