"""Qualify the same reactor in wazero and wasmtime, serially, without Cargo."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from common import (HERE, PROTOCOL, compare_pixels, require, sha, validate_png, validate_svg,
                    write_json)


def run(args):
    module = Path(args.module).resolve()
    module_sha = sha(module.read_bytes())
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    reports, failures = {}, []
    for runtime in ("wazero", "wasmtime"):
        directory = output / runtime
        catalog = directory / "runtime-catalog.raw.json"
        directory.mkdir(parents=True, exist_ok=True)
        common_args = ["--module", str(module), "--flavor", args.flavor,
                       "--catalog-out", str(catalog), "--output-dir", str(directory)]
        command = (["go", "-C", str(HERE / "wazero"), "run", "-mod=readonly", "-p=1", "."]
                   if runtime == "wazero" else [sys.executable, str(HERE / "wasmtime_smoke.py")])
        env = os.environ.copy()
        env.setdefault("GOMAXPROCS", "2")
        process = subprocess.run(command + common_args, capture_output=True, text=True,
                                 env=env, timeout=PROTOCOL["timeout_seconds"] + 180)
        (directory / "stderr.log").write_text(process.stderr)
        try:
            report = json.loads(process.stdout)
            require(process.returncode == 0 and report.get("ok") is True,
                    report.get("error", "runtime exited " + str(process.returncode)))
            require(report["module_sha256"] == module_sha, "host qualified different module bytes")
            require(report["checks"] == PROTOCOL["checks"], "host omitted a required smoke check")
            reports[runtime] = report
        except (ValueError, KeyError) as error:
            failures.append({"runtime": runtime, "error": str(error),
                             "stdout": process.stdout[-8000:], "stderr": process.stderr[-8000:]})
    require(not failures, "runtime qualification failed: " + json.dumps(failures))
    first, second = reports["wazero"], reports["wasmtime"]
    for key in ("fixtures_sha256", "fonts", "metadata_ids_called"):
        require(first[key] == second[key], "runtime input/evidence mismatch: " + key)
    catalogs = [(output / runtime / "runtime-catalog.raw.json").read_bytes()
                for runtime in ("wazero", "wasmtime")]
    require(catalogs[0] == catalogs[1], "runtime catalogs differ for the same reactor")
    comparisons = {}
    names = ["basic"] + (["math"] if args.flavor != "standard" else [])
    for name in names:
        # Revalidate each host's persisted output with the shared content oracle, including
        # wazero's Go-owned transport checks. Runtime equality remains a separate assertion.
        for runtime in ("wazero", "wasmtime"):
            directory = output / runtime
            viewport = validate_svg((directory / (name + ".svg")).read_bytes(),
                                    json.loads((directory / (name + ".svg.metadata.json")).read_bytes()),
                                    mathematical=name == "math")
            validate_png((directory / (name + ".png")).read_bytes(),
                         json.loads((directory / (name + ".png.metadata.json")).read_bytes()),
                         viewport, mathematical=name == "math")
        comparisons[name + ".png"] = compare_pixels(output / "wazero" / (name + ".png"),
                                                     output / "wasmtime" / (name + ".png"))
        for runtime in ("wazero", "wasmtime"):
            artifact = reports[runtime]["artifacts"][name + ".png"]
            require(artifact["rgba8_sha256"] == comparisons[name + ".png"]["rgba8_sha256"],
                    "runtime pixel evidence disagrees with decoded PNG")
    catalog_out = Path(args.catalog_out)
    catalog_out.parent.mkdir(parents=True, exist_ok=True)
    catalog_out.write_bytes(catalogs[0])
    report = {"schema_version": 1, "ok": True, "flavor": args.flavor,
              "module_sha256": module_sha, "fixtures_sha256": first["fixtures_sha256"],
              "protocol_sha256": sha((HERE / "protocol.json").read_bytes()),
              "catalog_sha256": sha(catalogs[0]), "pixel_comparisons": comparisons,
              "runtimes": {name: {"runtime_version": value["runtime_version"],
                                     "smoke_json": name + "/smoke.json"}
                           for name, value in reports.items()}}
    write_json(output / "smoke.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", required=True)
    parser.add_argument("--flavor", required=True, choices=("standard", "math", "full"))
    parser.add_argument("--catalog-out", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    try:
        report = run(args)
    except Exception as error:
        report = {"schema_version": 1, "ok": False, "error": str(error)}
        write_json(Path(args.output_dir) / "smoke.json", report)
        print(json.dumps(report))
        return 1
    print(json.dumps(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
