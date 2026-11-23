"""Collect a reproducible Node comparison in one consumer project, one product at a time."""

import json
from math import isfinite
from pathlib import Path
from statistics import median
import subprocess

from workload_coverage import summarize_coverage
from workload_drawing import audit_drawings
from workload_reference import NODE_PRODUCTS, compare_reference, node_worker_failures


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
    collection = coverage["collection"]
    lines.extend([f"Collection {collection['status']}.", ""])
    for name, run in collection["runs"].items():
        identity = f"{run['package']}@{run['version']}" if "version" in run else "collection failed"
        lines.append(f"- {name} · {identity} ([raw report]({run['report']}))")
        for failure in run.get("worker_failures", []):
            lines.append(f"  - {failure['id']} · worker {failure['status']} · requests unmeasured")
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
            f"Each measured worker supplied {report['samples']} warm batches. Times below are complete-batch medians in milliseconds.", "",
            f"| Scenario | Both passed artifact checks | Both stable | Stable in same reviewed class | {left} ms | {right} ms | {right} / {left} |" + (" Drawings (left / right) |" if drawings else ""),
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: |" + (" --- |" if drawings else ""),
        ])
        for row in report["scenarios"]:
            batch = row["complete_batch_ms"] or {}
            values = ["—" if batch.get(k) is None else f"{batch[k]:.3f}" for k in ("left", "right", "right_over_left")]
            name = row["id"].replace("|", "\\|").replace("\n", " ")
            if drawings:
                checks = row.get("drawing_checks")
                values.append(f"{checks['left']} / {checks['right']}" if checks else "—")
            matched = sum(len(group["request_indices"]) for group in row["render_timing_groups"])
            lines.append(f"| {name} | {row['common_completed']}/{row['planned_requests']} | {row['common_stable']} | {matched} | "
                         + " | ".join(values) + " |")
        for row in report["scenarios"]:
            if row["common_expected_rejections"]:
                lines.extend(["", f"{row['id']} · both products rejected {row['common_expected_rejections']} requests as expected."])
            if row["complete_batch_ms"] is None:
                for group in row["render_timing_groups"]:
                    times = group["render_ms"]
                    lines.extend(["", f"{row['id']} · {group['expected_outcome']} ({group['output_class']}) · {len(group['request_indices'])} matched requests · "
                                  f"summed render time {left} {times['left']:.3f} ms, {right} {times['right']:.3f} ms."])
        if report["ratio_withheld_reason"]:
            lines.extend(["", report["ratio_withheld_reason"]])
    lines.extend(["", "[Coverage and incomplete requests](coverage.json) · [Frozen inputs](plan.json) · "
                  "[Consumer dependency lock](consumer/package-lock.json)", ""])
    return "\n".join(lines)


def _process_order(products, index):
    """Pair each rotation with its reverse to balance precedence and rotate product positions."""
    start = (index // 2 * 2) % len(products)
    order = products[start:] + products[:start]
    return order[::-1] if index % 2 else order


def _paired_samples(comparison, raw, reference_products, reference_outputs, same_drawing_observer):
    """One eligible complete-batch median per fresh worker, checked against prior rounds."""
    product_issues = []
    for side in ("left", "right"):
        product = comparison[side]
        engine = product["engine"]
        if product["artifact_identity"]["status"] != "recorded":
            continue
        identity = dict(product=product, host=comparison["host"], collector_sha256=comparison["collector_sha256"][side])
        previous = reference_products.setdefault(engine, identity)
        for field, value in identity.items():
            if previous[field] != value:
                product_issues.append(f"{engine} {field.removesuffix('_sha256')} identity changed across rounds")

    samples = {}
    for index, row in enumerate(comparison["scenarios"]):
        issues = product_issues.copy()
        batch = row["complete_batch_ms"]
        if comparison["ratio_withheld_reason"]:
            issues.append(comparison["ratio_withheld_reason"])
        for side, failure in row.get("worker_failures", {}).items():
            issues.append(f"{comparison[side]['engine']} worker {failure['status']}")
        checks = row.get("drawing_checks", {})
        issues.extend(f"{comparison[side]['engine']} drawing {status}" for side, status in checks.items() if status != "passed")
        if checks and all(status == "passed" for status in checks.values()) and not same_drawing_observer:
            issues.append("drawing observer identity missing or changed across rounds")

        if batch is None or batch["right_over_left"] is None:
            if not issues:
                issues.append(f"no eligible complete-batch ratio ({row['common_stable']}/{row['planned_requests']} stable requests, "
                              f"{row['common_expected_rejections']} expected rejections)")
        elif not isfinite(batch["right_over_left"]) or batch["right_over_left"] <= 0:
            issues.append("complete-batch ratio must be finite and positive")
        else:
            # Within-worker stability covers the cold output and every warm batch.
            for side in ("left", "right"):
                engine = comparison[side]["engine"]
                cold = raw[engine]["scenarios"][index]["cold_batch"]["results"]
                for i, (request, result) in enumerate(zip(row["requests"], cold, strict=True)):
                    signature = (result["output_sha256"], result["output_bytes"], request[side]["output_class"])
                    if reference_outputs.setdefault((engine, row["id"], i), signature) != signature:
                        issues.append(f"{engine} output changed at request {i + 1} ({request['diagram_id']})")
                        break
        samples[row["id"]] = {"withheld_reason": ". ".join(issues)} if issues else batch
    return samples


def _summarize_pairs(observations):
    pairs = [sample for sample in observations if "right_over_left" in sample]
    result = dict(eligible_pairs=len(pairs), rounds=observations, per_process_median_ms=None,
                  paired_right_over_left=None, direction_counts=None)
    if len(pairs) != len(observations):
        return dict(status="partial" if pairs else "unavailable", **result)

    def distribution(values):
        return dict(min=min(values), median=median(values), max=max(values))

    ratios = [pair["right_over_left"] for pair in pairs]
    result["per_process_median_ms"] = {side: distribution([pair[side] for pair in pairs]) for side in ("left", "right")}
    result["paired_right_over_left"] = distribution(ratios)
    result["direction_counts"] = dict(below_one=sum(r < 1 for r in ratios), equal_one=ratios.count(1),
                                      above_one=sum(r > 1 for r in ratios))
    return dict(status="complete", **result)


def _collect_process_rounds(dest, products, inventory, rounds, warm_batches, collect):
    samples = {peer: {s["id"]: [] for s in inventory["scenarios"]} for peer in products[1:]}
    history, reference_products, reference_outputs = [], {}, {}
    reference_observer = None
    for index in range(rounds):
        order = _process_order(products, index)
        directory = dest / "rounds" / f"{index + 1:03d}"
        directory.mkdir(parents=True)
        coverage, raw, compared, observer = collect(directory, order)
        if reference_observer is None:
            reference_observer = observer
        collection = coverage["collection"]
        history.append(dict(round=index + 1, order=order, status=collection["status"],
                            coverage=(directory / "coverage.json").relative_to(dest).as_posix()))
        for peer, scenarios in samples.items():
            if peer in compared:
                observations = _paired_samples(compared[peer], raw, reference_products, reference_outputs,
                                               observer is not None and observer == reference_observer)
            else:
                keys = ("merman", peer, f"merman-vs-{peer}.json")
                errors = collection["errors"]
                reason = ". ".join(f"{key}: {errors[key]}" for key in keys if key in errors)
                observations = {name: {"withheld_reason": reason} for name in scenarios}
            for name, sample in observations.items():
                scenarios[name].append(dict(round=index + 1, **sample))

    return dict(schema_version=1, classification="diagnostic", plan_sha256=inventory["plan_sha256"],
                process_rounds=rounds, warm_batches_per_worker=warm_batches, left="merman",
                reference_identities=reference_products, rounds=history,
                collection_status="incomplete" if any(r["status"] == "incomplete" for r in history) else "complete",
                comparisons={peer: {name: _summarize_pairs(series) for name, series in scenarios.items()}
                             for peer, scenarios in samples.items()})


def _rounds_markdown(report):
    rounds = report["process_rounds"]
    lines = ["# Node SVG process rounds", "",
             f"Diagnostic collection {report['collection_status']}. Scheduled {rounds} fresh workers per product and scenario. "
             f"Each completed worker supplies {report['warm_batches_per_worker']} warm batches. "
             "The second round of each pair reverses the product order. Requests stay in document order.", "",
             "Each pair divides the peer's complete-batch median by Merman's median in the same round. "
             "Ratios below 1 favor the peer. Ratios above 1 favor Merman. Distributions require every scheduled pair.", ""]
    for peer, scenarios in report["comparisons"].items():
        lines.extend([f"## Merman and {peer}", "", "| Scenario | Eligible pairs | Status | Peer / Merman (min, median, max) | Below / equal / above 1 |",
                      "| --- | ---: | --- | --- | --- |"])
        for name, row in scenarios.items():
            ratio, counts = row["paired_right_over_left"], row["direction_counts"]
            spread = ", ".join(f"{ratio[key]:.3f}" for key in ("min", "median", "max")) if ratio else "—"
            direction = f"{counts['below_one']} / {counts['equal_one']} / {counts['above_one']}" if counts else "—"
            reasons = dict.fromkeys(r["withheld_reason"] for r in row["rounds"] if "withheld_reason" in r)
            status = " · ".join([row["status"], *reasons]).replace("|", "\\|").replace("\n", " ")
            name = name.replace("|", "\\|").replace("\n", " ")
            lines.append(f"| {name} | {row['eligible_pairs']}/{rounds} | {status} | {spread} | {direction} |")
        lines.append("")
    for item in report["rounds"]:
        lines.append(f"- [Round {item['round']}]({item['coverage']}) · {' → '.join(item['order'])} · {item['status']}")
    lines.extend(["", "[Per-round measurements and withheld reasons](process-rounds.json) · [Frozen inputs](plan.json) · "
                  "[Consumer dependency lock](consumer/package-lock.json)", ""])
    return "\n".join(lines)


def collect_node_matrix(args):
    raw_plan = args.plan.read_bytes()
    inventory = summarize_coverage(raw_plan)
    if args.check_drawings and "with_drawing_facts" not in inventory["planned"]:
        raise ValueError("the selected plan has no declared drawing facts")
    if not inventory["planned"]["requests"]:
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
    rounds = getattr(args, "process_rounds", None)
    if rounds is not None and (type(rounds) is not int or rounds < 2 or rounds % 2):
        raise ValueError("process-rounds must be an even integer of at least 2")
    if not isfinite(args.timeout_seconds) or not 0 < args.timeout_seconds <= 2_147_483.647:
        raise ValueError("timeout-seconds must fit a positive Node timer")
    candidate = args.merman_module.resolve(strict=True) if args.merman_module else None
    dest = args.out_dir.resolve()
    dest.mkdir(parents=True, exist_ok=True)
    products = ["merman", *peers]
    files = ["plan.json", "consumer/package.json", "consumer/package-lock.json", "summary.md"]
    if rounds is None:
        files.extend(["coverage.json", *(f"{name}.json" for name in products), *(f"merman-vs-{peer}.json" for peer in peers)])
        if args.check_drawings:
            files.extend(["drawings", "drawing-audit.json"])
    else:
        files.extend(["rounds", "process-rounds.json"])
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

    def collect(directory, order):
        return _collect_round(args, plan, raw_plan, directory, order, peers, candidate, versions, inventory)

    if rounds is None:
        coverage, _, compared, _ = collect(dest, products)
        summary = markdown_report(coverage, compared.values())
        evidence = "coverage.json"
        incomplete = bool(coverage["collection"]["errors"])
    else:
        report = _collect_process_rounds(dest, products, inventory, rounds, args.samples, collect)
        evidence = "process-rounds.json"
        write_report(dest / evidence, report)
        summary = _rounds_markdown(report)
        incomplete = report["collection_status"] == "incomplete"
    with (dest / "summary.md").open("x", encoding="utf-8", newline="\n") as file:
        file.write(summary)
    if incomplete:
        raise ValueError(f"incomplete Node collection, see {dest / evidence}")
    return dest / "summary.md"


def _collect_round(args, plan, raw_plan, dest, order, peers, candidate, versions, inventory):
    coverage = dict(inventory, comparisons=[])
    accepted, runs, compared, errors = {}, {}, {}, {}
    for name in order:
        report_path = dest / f"{name}.json"
        cmd = ["node", str(HERE / "workload_node.mjs"), "--engine", name, "--plan", str(plan),
               "--samples", str(args.samples), "--timeout-seconds", str(args.timeout_seconds)]
        if name == "merman" and candidate:
            cmd.extend(["--module", str(candidate)])
        if args.check_drawings:
            cmd.extend(["--drawing-dir", str(dest / "drawings" / name)])
        try:
            with report_path.open("xb") as raw:
                proc = subprocess.run(cmd, cwd=HERE / "node-reference", stdout=raw)
            report = json.loads(report_path.read_bytes())
            if not isinstance(report, dict):
                raise ValueError("collector output must be a JSON object")
            failures = node_worker_failures(report)
            if proc.returncode != int(bool(failures)):
                raise ValueError(f"collector exited with status {proc.returncode} inconsistent with its scenario rows")
            if report["plan_sha256"] != coverage["plan_sha256"] or report["engine"] != name:
                raise ValueError("collector returned a different plan or engine")
            if (getattr(args, "process_rounds", None) and
                    (report.get("samples") != args.samples or report.get("timeout_seconds") != args.timeout_seconds)):
                raise ValueError("collector returned a different sample count or timeout")
            if [row["id"] for row in report["scenarios"]] != [s["id"] for s in coverage["scenarios"]]:
                raise ValueError("collector returned an incomplete or different scenario selection")
            package = report["package"]
            if not (name == "merman" and candidate) and versions.get(package["name"]) != package["version"]:
                raise ValueError(f"{package['name']}@{package['version']} differs from the locked reference selection")
            accepted[name] = report
            runs[name] = {"report": report_path.name, "package": package["name"], "version": package["version"]}
            if failures:
                runs[name]["worker_failures"] = failures
                errors[name] = f"Missing measurements from {len(failures)} of {len(report['scenarios'])} scenario workers"
        except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
            errors[name] = str(error)
            runs[name] = {"report": report_path.name, "error": str(error)}

    drawing_products = {}
    observer = None
    if args.check_drawings and accepted:
        try:
            audit = audit_drawings(plan, [(dest / f"{name}.json", raw) for name, raw in accepted.items()], chrome=args.chrome)
            drawing_products = {p["engine"]: p["scenarios"] for p in audit["products"]}
            observer = audit.get("observer")
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            errors["drawing-audit.json"] = str(error)
            audit = {"plan_sha256": coverage["plan_sha256"], "error": str(error)}
        write_report(dest / "drawing-audit.json", audit)
        coverage["drawing_audit"] = "drawing-audit.json"

    for peer in peers:
        if "merman" in accepted and peer in accepted:
            filename = f"merman-vs-{peer}.json"
            try:
                result = compare_reference(accepted["merman"], accepted[peer], drawing_products=drawing_products)
                summary = summarize_coverage(raw_plan, [result])["comparisons"][0]
                write_report(dest / filename, result)
                coverage["comparisons"].append(summary)
                compared[peer] = result
            except (ValueError, KeyError, TypeError) as error:
                errors[filename] = str(error)
    coverage["collection"] = {"status": "incomplete" if errors else "complete", "runs": runs, "errors": errors}
    write_report(dest / "coverage.json", coverage)
    return coverage, accepted, compared, observer
