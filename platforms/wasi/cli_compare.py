"""Compare a built CLI and WASI reactor using identical sources, fonts and policy."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

from common import FIXTURES, PROTOCOL, ROOT, compare_pixels, font_inputs, require, sha, write_json


SWIMLANE = """---
config:
  layout: dagre
  fontFamily: Cousine
  flowchart:
    wrappingWidth: 120
    htmlLabels: true
---
swimlane-beta LR
  subgraph Customer
    request[Request service]
    receive[Receive update]
  end
  subgraph Support
    triage[Triage request]
    answer[Send answer]
  end
  subgraph Engineering
    investigate[Investigate issue]
    fix[Prepare fix]
  end
  request --> triage
  triage -->|Known issue| answer
  triage -->|Needs code change| investigate
  investigate --> fix --> answer
  answer --> receive
"""


def compare(args):
    spec = importlib.util.spec_from_file_location(
        "merman_wasi_example", ROOT / "distribution/sdk/examples/python-wasi/client.py")
    example = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(example)
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=False)
    module = Path(args.module).resolve()
    cli = Path(args.cli).resolve()
    fonts = [(ROOT / path).read_bytes() for path in FIXTURES["font_files"]]
    font_flags = [flag for path in FIXTURES["font_files"] for flag in ("--font-file", str(ROOT / path))]
    # Compare explicit common layout semantics, not optional-loader defaults.
    cases = {"basic": FIXTURES["sources"]["dagre"], "swimlane": SWIMLANE}
    if args.flavor != "standard":
        cases["math"] = FIXTURES["sources"]["math"].replace(
            "config:\n", "config:\n  layout: dagre\n", 1)
    if args.flavor == "full":
        cases["elk"] = FIXTURES["sources"]["elk"]
    # The full 2x Swimlane raster exceeds the smaller example's default budget.
    fuel = 50_000_000_000
    report = {"module_sha256": sha(module.read_bytes()), "cli_sha256": sha(cli.read_bytes()),
              "flavor": args.flavor, "fuel_per_call": fuel,
              "fonts": font_inputs()[2], "comparisons": {}}
    with example.Client(module.read_bytes(), options=FIXTURES["engine_options"], fonts=fonts, fuel=fuel) as client:
        for name, source in cases.items():
            path = output / (name + ".mmd")
            path.write_bytes(source.encode())
            report["comparisons"][name] = {"source_sha256": sha(source.encode())}
            for operation in ("svg", "png"):
                # One background value controls both the root canvas and raster matte.
                background = "#f1e2d3" if operation == "png" else "transparent"
                options = {"svg": {"root_background_color": background}}
                flags = ["--svg-pipeline", "resvg-safe"] if operation == "svg" else ["--scale", "2"]
                if operation == "png":
                    options["raster"] = {"scale": 2, "matte": background}
                data, metadata = client.execute(operation, source.encode(), options)
                wasi_output = output / (name + ".wasi." + operation)
                cli_output = output / (name + ".cli." + operation)
                wasi_output.write_bytes(data)
                write_json(Path(str(wasi_output) + ".metadata.json"), metadata)
                subprocess.run(
                    [str(cli), "render", str(path), "--output", str(cli_output), "--format", operation,
                     "--quiet", "--runtime", "deterministic", "--resource-profile", "interactive",
                     "--svg-id", FIXTURES["engine_options"]["svg"]["diagram_id"],
                     "--font-measurement", "native", "--font-system", "off", "--background", background,
                     *font_flags, *flags], check=True, timeout=PROTOCOL["timeout_seconds"],
                )
                if operation == "png":
                    evidence = compare_pixels(wasi_output, cli_output)
                else:
                    require(data == cli_output.read_bytes(), "CLI/WASI SVG mismatch: " + name)
                    evidence = {"sha256": sha(data)}
                report["comparisons"][name][operation] = evidence
    write_json(output / "comparison.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True)
    parser.add_argument("--module", required=True)
    parser.add_argument("--flavor", required=True, choices=("standard", "math", "full"))
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    print(json.dumps(compare(args)))


if __name__ == "__main__":
    main()
