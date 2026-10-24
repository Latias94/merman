"""Drive a built native C probe with the same fonts, sources and options as WASI."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from common import (FIXTURES, FIXTURE_BYTES, ROOT, PROTOCOL, compare_pixels, font_inputs,
                    require, save_render, sha, validate_catalog, validate_native_error, write_json)


def run(args):
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    probe = str(Path(args.probe).resolve())
    fonts = ROOT / Path(FIXTURES["font_files"][0]).parent
    artifacts, errors = {}, {}
    catalog_bytes = None

    def execute(name, source, operation, options, expected_code=None, capability=None, scenario=None):
        nonlocal catalog_bytes
        directory = output / scenario if scenario else output
        directory.mkdir(parents=True, exist_ok=True)
        source_path = directory / (name + ".source.mmd")
        options_path = directory / (name + ".options.json")
        data_path = directory / (name + "." + operation)
        meta_path = directory / (name + "." + operation + ".metadata.json")
        catalog_path = directory / (name + ".catalog.raw.json")
        source_path.write_bytes(source.encode())
        write_json(options_path, options)
        process = subprocess.run([probe, str(fonts), str(source_path), str(options_path),
                                  operation, str(data_path), str(meta_path), str(catalog_path)],
                                 capture_output=True, timeout=PROTOCOL["timeout_seconds"])
        require(process.returncode in (0, 3), "native host failure: " + process.stderr.decode(errors="replace"))
        metadata = json.loads(meta_path.read_bytes())
        raw_catalog = catalog_path.read_bytes()
        if catalog_bytes is None:
            catalog_bytes = raw_catalog
            validate_catalog(json.loads(raw_catalog), args.flavor, transport_version=3)
        require(raw_catalog == catalog_bytes, "native catalog changed between identical probe processes")
        if expected_code or capability:
            require(process.returncode == 3, "native operation unexpectedly succeeded: " + name)
            errors[(scenario or name) + "." + operation] = validate_native_error(metadata, expected_code, capability)
            return None
        require(process.returncode == 0, "native ordinary error: " + json.dumps(metadata))
        data = data_path.read_bytes()
        viewport = artifacts.get((scenario or ("math" if name == "math" else "basic")) + ".svg", {}).get("svg_viewport")
        artifacts[(scenario or name) + "." + operation] = save_render(directory, name, operation, data, metadata, viewport)
        return metadata

    for name in ("basic", "math"):
        for operation in ("svg", "png"):
            options = dict(FIXTURES["engine_options"])
            if operation == "png":
                options.update(FIXTURES["png_options"])
            execute(name, FIXTURES["sources"][name], operation, options,
                    capability="math" if name == "math" and args.flavor == "standard" else None)
    execute("parse-error", FIXTURES["sources"]["parse_error"], "svg", FIXTURES["engine_options"],
            expected_code="parse-error")
    execute("elk", FIXTURES["sources"]["elk"], "svg", FIXTURES["engine_options"])
    if args.flavor != "full":
        execute("dagre", FIXTURES["sources"]["dagre"], "svg", FIXTURES["engine_options"])
        require((output / "elk.svg").read_bytes() == (output / "dagre.svg").read_bytes(),
                "native absent ELK did not use byte-equivalent Dagre family fallback")
    if args.flavor != "standard":
        for operation in ("svg", "png"):
            options = dict(FIXTURES["engine_options"])
            options.update(FIXTURES["constrained_options"])
            if operation == "png":
                options.update(FIXTURES["png_options"])
            execute("math", FIXTURES["sources"]["math"], operation, options,
                    scenario="constrained.constructor")
            key = "sha256" if operation == "svg" else "rgba8_sha256"
            require(artifacts["constrained.constructor." + operation][key] == artifacts["math." + operation][key],
                    "native legacy constrained constructor changed formula output")
    require(catalog_bytes is not None, "native probe emitted no catalog")
    catalog_out = Path(args.catalog_out) if args.catalog_out else output / "runtime-catalog.raw.json"
    catalog_out.parent.mkdir(parents=True, exist_ok=True)
    catalog_out.write_bytes(catalog_bytes)
    comparisons = {}
    if args.wasi_dir:
        wasi = Path(args.wasi_dir)
        for runtime in ("wazero", "wasmtime"):
            report = json.loads((wasi / runtime / "smoke.json").read_bytes())
            require(report["ok"] is True and report["flavor"] == args.flavor and
                    report["fixtures_sha256"] == sha(FIXTURE_BYTES) and report["fonts"] == font_inputs()[2],
                    "native/WASI evidence inputs differ")
            for filename in ["basic.png"] + (["math.png", "constrained.constructor/math.png"]
                                              if args.flavor != "standard" else []):
                comparisons[runtime + "/" + filename] = compare_pixels(output / filename,
                                                                         wasi / runtime / filename)
    report = {"schema_version": 1, "ok": True, "runtime": "native-c", "flavor": args.flavor,
              "fixtures_sha256": sha(FIXTURE_BYTES), "fonts": font_inputs()[2],
              "catalog_sha256": sha(catalog_bytes), "artifacts": artifacts, "errors": errors,
              "pixel_comparisons": comparisons}
    write_json(output / "smoke.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", required=True)
    parser.add_argument("--flavor", required=True, choices=("standard", "math", "full"))
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--catalog-out")
    parser.add_argument("--wasi-dir", help="two-runtime smoke output root for exact RGBA8 comparison")
    args = parser.parse_args()
    try:
        report = run(args)
    except Exception as error:
        report = {"schema_version": 1, "ok": False, "runtime": "native-c", "error": str(error)}
        write_json(Path(args.output_dir) / "smoke.json", report)
        print(json.dumps(report))
        return 1
    print(json.dumps(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
