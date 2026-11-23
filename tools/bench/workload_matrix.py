"""Collect a reproducible Node comparison in one consumer project, one product at a time."""

import json
from math import isfinite
from pathlib import Path
import subprocess

from workload_coverage import summarize_coverage
from workload_drawing import audit_drawings
from workload_reference import NODE_PRODUCTS, compare_reference


HERE = Path(__file__).resolve().parent


def write_report(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as file:
        json.dump(value, file, ensure_ascii=False, indent=2, allow_nan=False)
        file.write("\n")


def markdown_report(coverage, comparisons):
    planned = coverage["planned"]
    labeled = planned["with_expected_labels"]
    lines = [
        "# Node SVG benchmark", "",
        f"Scenarios {planned['scenarios']} · requests {planned['requests']} · distinct sources {planned['distinct_sources']} "
        f"· requests with expected labels {labeled['requests']}.", "",
    ]
    for name, run in coverage["collection"]["runs"].items():
        identity = f"{run['package']}@{run['version']}" if "version" in run else "collection failed"
        lines.append(f"- {name} · {identity} ([raw report]({run['report']}))")
    drawings = planned.get("with_drawing_facts")
    if drawings:
        lines.extend(["", f"Drawing checks apply to {drawings['requests']} requests across {drawings['distinct_sources']} distinct sources. "
                      "A ratio on those scenarios requires both products to pass."])
    if "drawing_audit" in coverage:
        lines.extend(["", "[Per-product drawing evidence](drawing-audit.json)"])
    for report, observation in zip(comparisons, coverage["comparisons"], strict=True):
        left, right = report["left"]["engine"], report["right"]["engine"]
        lines.extend(["", f"## {left} and {right}", ""])
        families = observation.get("family_baselines")
        if families:
            lines.append(f"Family baselines observed {families['reported']}/{families['planned']}. "
                         f"Both products had repeatable output on {families['both_stable']}.")
            lines.append("")
            for side, name in (("left", left), ("right", right)):
                counts = families[side]
                gaps = ", ".join(f"{gap['family']} ({', '.join(gap['statuses'])}, {gap['state']})" for gap in counts["gaps"])
                lines.append(f"- {name} completed {counts['completed']}/{families['reported']}, repeatable {counts['stable']}."
                             + (f" Gaps include {gaps}." if gaps else ""))
            lines.append("")
        lines.extend([
            f"{report['samples']} warm batches per scenario. Times are diagnostic medians in milliseconds.", "",
            f"| Scenario | Both passed artifact checks | Both stable | {left} ms | {right} ms | {right} / {left} |" + (" Drawings (left / right) |" if drawings else ""),
            "| --- | ---: | ---: | ---: | ---: | ---: |" + (" --- |" if drawings else ""),
        ])
        for row in report["scenarios"]:
            batch = row["complete_batch_ms"] or {}
            values = ["—" if batch.get(k) is None else f"{batch[k]:.3f}" for k in ("left", "right", "right_over_left")]
            name = row["id"].replace("|", "\\|").replace("\n", " ")
            if drawings:
                checks = row.get("drawing_checks")
                values.append(f"{checks['left']} / {checks['right']}" if checks else "—")
            lines.append(f"| {name} | {row['common_completed']}/{row['planned_requests']} | {row['common_stable']} | "
                         + " | ".join(values) + " |")
        if report["ratio_withheld_reason"]:
            lines.extend(["", report["ratio_withheld_reason"]])
    lines.extend(["", "[Coverage and incomplete requests](coverage.json) · [Frozen inputs](plan.json) · "
                  "[Consumer dependency lock](consumer/package-lock.json)", ""])
    return "\n".join(lines)


def collect_node_matrix(args):
    raw_plan = args.plan.read_bytes()
    coverage = summarize_coverage(raw_plan)
    if args.check_drawings and "with_drawing_facts" not in coverage["planned"]:
        raise ValueError("the selected plan has no declared drawing facts")
    if not coverage["planned"]["requests"]:
        raise ValueError("the Node matrix requires a plan with at least one SVG request")
    project = HERE / "node-reference"
    manifest = (project / "package.json").read_bytes()
    lock = (project / "package-lock.json").read_bytes()
    versions = json.loads(manifest)["dependencies"]
    if versions != json.loads(lock)["packages"][""]["dependencies"]:
        raise ValueError("reference package.json and package-lock.json select different versions")
    peers = args.peer or [name for name in NODE_PRODUCTS if name != "merman"]
    if len(set(peers)) != len(peers) or any(name == "merman" or name not in NODE_PRODUCTS for name in peers):
        raise ValueError("choose distinct registered Node peers for Merman")
    if args.samples < 1:
        raise ValueError("samples must be positive")
    if not isfinite(args.timeout_seconds) or not 0 < args.timeout_seconds <= 2_147_483.647:
        raise ValueError("timeout-seconds must fit a positive Node timer")
    candidate = args.merman_module.resolve(strict=True) if args.merman_module else None
    dest = args.out_dir.resolve()
    dest.mkdir(parents=True, exist_ok=True)
    products = ["merman", *peers]
    comparisons = {peer: f"merman-vs-{peer}.json" for peer in peers}
    files = ["plan.json", "consumer/package.json", "consumer/package-lock.json", "coverage.json", "summary.md",
             *(f"{name}.json" for name in products), *comparisons.values()]
    if args.check_drawings:
        files.extend(["drawings", "drawing-audit.json"])
    for name in files:
        if (dest / name).exists():
            raise FileExistsError(f"{dest / name} already exists. Select a fresh result directory.")
    plan = dest / "plan.json"
    with plan.open("xb") as file:
        file.write(raw_plan)
    (dest / "consumer").mkdir(exist_ok=True)
    for name, contents in (("package.json", manifest), ("package-lock.json", lock)):
        with (dest / "consumer" / name).open("xb") as file:
            file.write(contents)

    accepted, runs, compared, errors = {}, {}, [], {}
    for name in products:
        report_path = dest / f"{name}.json"
        cmd = ["node", str(HERE / "workload_node.mjs"), "--engine", name, "--plan", str(plan),
               "--samples", str(args.samples), "--timeout-seconds", str(args.timeout_seconds)]
        if name == "merman" and candidate:
            cmd.extend(["--module", str(candidate)])
        if args.check_drawings:
            cmd.extend(["--drawing-dir", str(dest / "drawings" / name)])
        try:
            with report_path.open("xb") as raw:
                proc = subprocess.run(cmd, cwd=project, stdout=raw)
            report = json.loads(report_path.read_bytes())
            if not isinstance(report, dict):
                raise ValueError("collector output must be a JSON object")
            if proc.returncode or report.get("error"):
                raise ValueError(report.get("error") or f"collector exited with status {proc.returncode}")
            if report["plan_sha256"] != coverage["plan_sha256"] or report["engine"] != name:
                raise ValueError("collector returned a different plan or engine")
            package = report["package"]
            if not (name == "merman" and candidate) and versions.get(package["name"]) != package["version"]:
                raise ValueError(f"{package['name']}@{package['version']} differs from the locked reference selection")
            accepted[name] = report
            runs[name] = {"report": report_path.name, "package": package["name"], "version": package["version"]}
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors[name] = str(error)
            runs[name] = {"report": report_path.name, "error": str(error)}

    drawing_products = {}
    if args.check_drawings and accepted:
        try:
            audit = audit_drawings(plan, [(dest / f"{name}.json", raw) for name, raw in accepted.items()], chrome=args.chrome)
            drawing_products = {p["engine"]: p["scenarios"] for p in audit["products"]}
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            errors["drawing-audit.json"] = str(error)
            audit = {"plan_sha256": coverage["plan_sha256"], "error": str(error)}
        write_report(dest / "drawing-audit.json", audit)
        coverage["drawing_audit"] = "drawing-audit.json"

    for peer in peers:
        if "merman" in accepted and peer in accepted:
            try:
                result = compare_reference(accepted["merman"], accepted[peer], drawing_products=drawing_products)
                summary = summarize_coverage(raw_plan, [result])["comparisons"][0]
                write_report(dest / comparisons[peer], result)
                coverage["comparisons"].append(summary)
                compared.append(result)
            except (ValueError, KeyError, TypeError) as error:
                errors[comparisons[peer]] = str(error)
    coverage["collection"] = {"status": "incomplete" if errors else "complete", "runs": runs, "errors": errors}
    write_report(dest / "coverage.json", coverage)
    with (dest / "summary.md").open("x", encoding="utf-8", newline="\n") as file:
        file.write(markdown_report(coverage, compared))
    if errors:
        raise ValueError(f"incomplete Node collection, see {dest / 'coverage.json'}")
    return dest / "summary.md"
