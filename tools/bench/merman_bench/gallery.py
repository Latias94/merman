"""Compare native Flowchart and Agentflow layouts with optional Pele drawings offline."""

from collections import Counter
import json
import os
from pathlib import Path
import re
from statistics import median
import subprocess
import sys
from tempfile import TemporaryDirectory
from xml.etree import ElementTree as ET

from .workloads import HERE, LAYOUTS, build_runner, check_layout_evidence, child, digest, load, rust_signatures
from .reference import node_worker_failures


def svg_card(svg, backend):
    root = ET.fromstring(svg)
    if backend == "pele":
        if {"pele-flowchart", "pele-agentflow"}.isdisjoint(root.get("class", "").split()):
            return {"error": "Pele did not return a Flowchart or Agentflow drawing"}
        classes = Counter(token for group in root.iter("{http://www.w3.org/2000/svg}g")
                          for token in group.get("class", "").split())
        nodes, edges = classes["pele-node"], classes["pele-edge"]
    else:
        nodes = len(root.findall(".//*[@data-et='node']"))
        edges = len(root.findall(".//*[@data-et='edge']"))
    _, _, width, height = map(float, re.split(r"[\s,]+", root.attrib["viewBox"].strip()))
    return dict(svg=svg, width=width, height=height, nodes=nodes, edges=edges)


def render_card(runner, request, backend, recorded, resource_profile=None, *, lock_layout=False):
    job = dict(task="measure", metric="clock", operation="svg", requests=[request], site={"layout": backend},
               warmups=0, verify_only=True, verify_layout=True)
    if lock_layout:
        job["lock_site_layout"] = True
    if resource_profile:
        job["resource_profile"] = resource_profile
    response = child(runner, job)
    if lock_layout and response.get("site_layout_locked") is not True:
        raise ValueError("--override-frontmatter-layout requires a runner that confirms the site layout lock")
    if resource_profile and response.get("resource_profile") != resource_profile:
        raise ValueError(f"workload runner did not confirm resource profile {resource_profile!r}")
    outputs = response["outputs"]
    signatures, _ = rust_signatures(outputs, job["requests"], "svg")
    receipts = check_layout_evidence(response, job, signatures, backend, inventory=recorded is not None)
    if recorded is not None and (signatures != recorded["outputs"] or receipts != recorded["layout_evidence"]):
        raise ValueError(f"{request['diagram_id']}/{backend}: rendered SVG or backend differs from the comparison")

    signature = signatures[0]
    if "status" in signature:
        return {"error": signature.get("message", signature["status"])}
    card = svg_card(outputs[0], backend)
    if recorded is not None and not recorded.get("status") and recorded["samples"]:
        card["medianNs"] = median(sample["elapsed_ns"] for sample in recorded["samples"])
    return card


def retained_pele_card(drawings, index, row):
    if row.get("status") in ("failed", "timeout"):
        return {"error": row["error"]}

    warm_samples = row["warm_samples"]
    result, = warm_samples[-1]["results"]
    if result["status"] != "ok":
        return {"error": result.get("message") or result["status"]}
    filename = f"{index}-0.svg"
    if row["requests"][0].get("drawing_artifact") != filename:
        raise ValueError(f"{row['id']}: missing validated Pele drawing")
    data = (drawings / filename).read_bytes()
    if digest(data) != result["output_sha256"]:
        raise ValueError(f"{row['id']}: retained Pele SVG differs from the validated output")
    card = svg_card(data.decode("utf-8"), "pele")
    if "svg" in card and result.get("output_class") != "empty" and row["coverage"]["full_batch_comparable"]:
        times = [batch["results"][0]["elapsed_ms"] for batch in warm_samples]
        card["nodeTiming"] = dict(medianMs=median(times), warmMs=times, svgSha256=result["output_sha256"])
    return card


def render_pele_cards(cases):
    consumer = HERE / "node-reference"
    package_dir = consumer / "node_modules/pele"
    version = load(consumer / "package-lock.json")["packages"]["node_modules/pele"]["version"]
    install = "npm ci --prefix tools/bench/node-reference --ignore-scripts --no-audit --no-fund"
    manifest = package_dir / "package.json"
    if not manifest.is_file() or load(manifest).get("version") != version:
        raise ValueError(f"--pele needs the locked pele@{version}. Run `{install}` first.")

    scenarios = [{"id": name, "requests": [{"fixture": name, **request}]} for name, request, _ in cases]
    raw = json.dumps(dict(schema_version=1, suite="layout-gallery", scenarios=scenarios)).encode()
    cards = []
    with TemporaryDirectory(prefix="merman-layout-gallery-") as directory:
        plan = Path(directory) / "plan.json"
        drawings = Path(directory) / "drawings"
        plan.write_bytes(raw)
        run = subprocess.run(["node", HERE / "collectors/node_runner.mjs", "--engine", "pele", "--plan", plan,
                              "--samples", "3", "--drawing-dir", drawings, "--all-drawings"],
                             cwd=consumer, stdout=subprocess.PIPE,
                             env={**os.environ, "MERMAN_BENCH_PYTHON": sys.executable})
        report = json.loads(run.stdout)
        failures = node_worker_failures(report)
        if run.returncode != int(bool(failures)) or report["engine"] != "pele" or report["plan_sha256"] != digest(raw):
            raise ValueError("Pele collector did not return the requested gallery plan")
        package = report["package"]
        if (package["name"] != "pele" or package["version"] != version
                or not Path(package["entry"]).is_relative_to(package_dir.resolve())):
            raise ValueError(f"--pele requires pele@{version} from the locked consumer. Run `{install}`.")

        for index, ((name, request, _), row) in enumerate(zip(cases, report["scenarios"], strict=True)):
            identity, = row["requests"]
            if row["id"] != name or any(identity[key] != request[key] for key in ("source_sha256", "diagram_id")):
                raise ValueError(f"{name}: Pele collector returned a different request")
            cards.append(retained_pele_card(drawings, index, row))
    return dict(version=package["version"], nodeVersion=report["host"]["node"]), cards


def write_gallery(args):
    inputs = [path for path in [*args.sources, args.plan, args.comparison, args.runner] if path]
    if args.out.resolve() in {path.resolve() for path in inputs}:
        raise ValueError("choose an HTML output distinct from the sources, plan, comparison and runner")
    if not (args.sources or args.plan) or (args.comparison and not args.plan):
        raise ValueError("supply Mermaid files or a frozen --plan; --comparison requires its --plan")

    cases = []
    for index, path in enumerate(args.sources):
        source = path.read_bytes().decode("utf-8")
        request = dict(diagram_id=f"gallery-{index + 1}", source=source, source_sha256=digest(source.encode()))
        cases.append((path.name, request, False))

    report = load(args.comparison) if args.comparison else None
    if args.plan:
        raw = args.plan.read_bytes()
        plan = json.loads(raw)
        if plan["schema_version"] != 1:
            raise ValueError("unsupported workload plan")
        if report is not None and (report.get("surface") != "rust-layouts" or report.get("schema_version") != 1
                                   or report.get("plan_sha256") != digest(raw)):
            raise ValueError("comparison must be a rust-layouts report for these frozen plan bytes")
        by_id = {case["id"]: case for case in plan["scenarios"]}
        for name in report["rows"] if report is not None else by_id:
            if name not in by_id or len(by_id[name]["requests"]) != 1:
                raise ValueError(f"{name}: expected one drawing request in the frozen plan")
            request = by_id[name]["requests"][0]
            if digest(request["source"].encode()) != request["source_sha256"]:
                raise ValueError(f"{name}: frozen source digest mismatch")
            cases.append((name, request, report is not None))
    if not cases:
        raise ValueError("no drawings to display")

    gallery = {"cases": []}
    pele_cards = []
    if args.pele:
        gallery["pele"], pele_cards = render_pele_cards(cases)

    runner = args.runner.resolve() if args.runner else build_runner("clock", shoal=True)
    if report is not None:
        if digest(runner.read_bytes()) != report["runner_sha256"]:
            raise ValueError("use the clock executable that produced this comparison")
        run = report["runs"]["dagre"]["svg"]
        gallery["measurement"] = dict(warmups=run["options"]["warmups"], samples=run["options"]["samples"],
                                      host=" ".join(run["host"][:2]), revision=run["revision"],
                                      resourceProfile=run["options"].get("resource_profile", "interactive"))

    for index, (name, request, measured) in enumerate(cases):
        case = dict(name=name, source=request["source"], cards={})
        if measured:
            case["comparison"] = report["rows"][name]["status"]
        elif args.override_frontmatter_layout:
            case["nativeLayoutLocked"] = True
        for backend in LAYOUTS:
            run = report["runs"][backend]["svg"] if measured else None
            recorded = run["rows"][name] if run else None
            profile = run["options"].get("resource_profile") if run else None
            case["cards"][backend] = render_card(
                runner, request, backend, recorded, profile,
                lock_layout=case.get("nativeLayoutLocked", False),
            )
        if pele_cards:
            case["cards"]["pele"] = pele_cards[index]
        gallery["cases"].append(case)

    # A Mermaid label or an SVG style may contain HTML end tags. Keep all source in the JSON data block.
    data = json.dumps(gallery, ensure_ascii=False, allow_nan=False, separators=(",", ":")).replace("<", r"\u003c")
    template = (HERE / "workload_layout_gallery.html.in").read_text(encoding="utf-8")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(template.replace("__GALLERY_DATA__", data), encoding="utf-8", newline="\n")
