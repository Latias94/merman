"""Collect a reproducible Node comparison in one consumer project, one product at a time."""

import json
from math import isfinite
from pathlib import Path
import re
from statistics import median
import subprocess

from .coverage import summarize_coverage
from .drawing import audit_drawings
from .reference import LAYOUTS, NODE_PRODUCTS, compare_reference, node_worker_failures, product_label, valid_node_variant_label


HERE = Path(__file__).resolve().parents[1]


def write_report(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as file:
        json.dump(value, file, ensure_ascii=False, indent=2, allow_nan=False)
        file.write("\n")


def _comparison_file(left, right):
    return f"{left}-vs-{right}.json"


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
        left, right = product_label(report["left"]), product_label(report["right"])
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
        label = product_label(product)
        if product["artifact_identity"]["status"] != "recorded":
            continue
        identity = dict(product=product, host=comparison["host"], collector_sha256=comparison["collector_sha256"][side])
        previous = reference_products.setdefault(label, identity)
        for field, value in identity.items():
            if previous[field] != value:
                product_issues.append(f"{label} {field.removesuffix('_sha256')} identity changed across rounds")

    samples = {}
    for index, row in enumerate(comparison["scenarios"]):
        issues = product_issues.copy()
        batch = row["complete_batch_ms"]
        if comparison["ratio_withheld_reason"]:
            issues.append(comparison["ratio_withheld_reason"])
        for side, failure in row.get("worker_failures", {}).items():
            issues.append(f"{product_label(comparison[side])} worker {failure['status']}")
        checks = row.get("drawing_checks", {})
        issues.extend(f"{product_label(comparison[side])} drawing {status}" for side, status in checks.items() if status != "passed")
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
                label = product_label(comparison[side])
                cold = raw[label]["scenarios"][index]["cold_batch"]["results"]
                for i, (request, result) in enumerate(zip(row["requests"], cold, strict=True)):
                    signature = (result["output_sha256"], result["output_bytes"], request[side]["output_class"])
                    if reference_outputs.setdefault((label, row["id"], i), signature) != signature:
                        issues.append(f"{label} output changed at request {i + 1} ({request['diagram_id']})")
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
    left, *peers = products
    samples = {peer: {s["id"]: [] for s in inventory["scenarios"]} for peer in peers}
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
                keys = (left, peer, _comparison_file(left, peer))
                errors = collection["errors"]
                reason = ". ".join(f"{key}: {errors[key]}" for key in keys if key in errors)
                observations = {name: {"withheld_reason": reason} for name in scenarios}
            for name, sample in observations.items():
                scenarios[name].append(dict(round=index + 1, **sample))

    return dict(schema_version=1, classification="diagnostic", plan_sha256=inventory["plan_sha256"],
                process_rounds=rounds, warm_batches_per_worker=warm_batches, left=left,
                reference_identities=reference_products, rounds=history,
                collection_status="incomplete" if any(r["status"] == "incomplete" for r in history) else "complete",
                comparisons={peer: {name: _summarize_pairs(series) for name, series in scenarios.items()}
                             for peer, scenarios in samples.items()})


def _rounds_markdown(report):
    rounds = report["process_rounds"]
    left = report["left"] if "variants" in report else "Merman"
    lines = ["# Node SVG process rounds", "",
             f"Diagnostic collection {report['collection_status']}. Scheduled {rounds} fresh workers per product and scenario. "
             f"Each completed worker supplies {report['warm_batches_per_worker']} warm batches. "
             "The second round of each pair reverses the product order. Requests stay in document order.", "",
             f"Each pair divides the right-hand worker's complete-batch median by {left}'s median in the same round. "
             "Distributions require every scheduled pair.", ""]
    for peer, scenarios in report["comparisons"].items():
        lines.extend([f"## {left} and {peer}", "", f"| Scenario | Eligible pairs | Status | {peer} / {left} (min, median, max) | Below / equal / above 1 |",
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
    if "variants" in report:
        lines.extend(["", "[Frozen variant settings](variants.json)"])
    lines.extend(["", "[Per-round measurements and withheld reasons](process-rounds.json) · [Frozen inputs](plan.json) · "
                  "[Consumer dependency lock](consumer/package-lock.json)", ""])
    return "\n".join(lines)


def _variant_recipes(specs, layouts=()):
    recipes, labels = {}, set()
    for spec in specs:
        label, equals, selection = spec.partition("=")
        engine, _, tail = selection.partition(":")
        if not equals or not valid_node_variant_label(label) or engine not in NODE_PRODUCTS:
            raise ValueError(f"expected --variant LABEL=ENGINE[:MODULE][:OPTIONS_JSON] for a registered Node engine: {spec!r}")
        if label.casefold() in labels:
            raise ValueError(f"duplicate Node variant label: {label}")
        labels.add(label.casefold())
        if tail.lstrip().startswith("{"):
            module, separator, options = "", ":", tail
        else:
            # Skip the drive colon of an absolute Windows path.
            drive = tail[:2] if re.match(r"^[A-Za-z]:[/\\]", tail) else ""
            path_tail, separator, options = tail[len(drive):].partition(":")
            module = drive + path_tail
        if separator and (engine != "merman" or not options.strip()):
            raise ValueError(f"{label}: explicit Options JSON requires a Merman variant and nonempty text")
        recipes[label] = dict(engine=engine, module=Path(module).absolute() if module else None,
                              options_json=options if separator else None)
    if len(recipes) < 2:
        raise ValueError("repeat --variant at least twice")
    for spec in layouts:
        label, _, layout = spec.partition("=")
        recipe = recipes.get(label)
        if recipe is None or recipe["engine"] != "merman" or layout not in LAYOUTS:
            raise ValueError(f"expected --variant-layout LABEL=LAYOUT for a Merman variant: {spec!r}")
        if "site_layout" in recipe:
            raise ValueError(f"duplicate site layout for Node variant {label}")
        recipe["site_layout"] = layout
    left, *peers = recipes
    filenames = ["coverage.json", "drawing-audit.json", *(f"{name}.json" for name in recipes),
                 *(_comparison_file(left, name) for name in peers)]
    if len(filenames) != len({name.casefold() for name in filenames}):
        raise ValueError("Node variant labels collide with report filenames")
    return recipes


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
    variants = getattr(args, "variant", None)
    layouts = getattr(args, "variant_layout", None) or ()
    rounds = getattr(args, "process_rounds", None)
    if variants:
        if args.peer or args.merman_module:
            raise ValueError("--variant cannot be combined with --peer or --merman-module")
        recipes = _variant_recipes(variants, layouts)
        if rounds is None:
            rounds = 2
    else:
        if layouts:
            raise ValueError("--variant-layout requires --variant")
        peers = args.peer or [name for name in NODE_PRODUCTS if name != "merman"]
        if len(set(peers)) != len(peers) or any(name == "merman" or name not in NODE_PRODUCTS for name in peers):
            raise ValueError("choose distinct registered Node peers for Merman")
        candidate = args.merman_module.resolve(strict=True) if args.merman_module else None
        recipes = {name: dict(engine=name, module=candidate if name == "merman" else None) for name in ("merman", *peers)}
    if args.samples < 1:
        raise ValueError("samples must be positive")
    if rounds is not None and (type(rounds) is not int or rounds < 2 or rounds % 2):
        raise ValueError("process-rounds must be an even integer of at least 2")
    if not isfinite(args.timeout_seconds) or not 0 < args.timeout_seconds <= 2_147_483.647:
        raise ValueError("timeout-seconds must fit a positive Node timer")
    warmup = {}
    for unit in ("calls", "ms"):
        limit = getattr(args, f"warmup_{unit}", None)
        if limit is not None:
            if type(limit) is not int or not 0 <= limit < 2**53:
                raise ValueError(f"warmup-{unit} must be a nonnegative safe integer")
            warmup[f"max_{unit}"] = limit
    dest = args.out_dir.resolve()
    dest.mkdir(parents=True, exist_ok=True)
    products = list(recipes)
    left, *peers = products
    files = ["plan.json", "consumer/package.json", "consumer/package-lock.json", "summary.md"]
    if rounds is None:
        files.extend(["coverage.json", *(f"{name}.json" for name in products), *(_comparison_file(left, peer) for peer in peers)])
        if args.check_drawings:
            files.extend(["drawings", "drawing-audit.json"])
    else:
        files.extend(["rounds", "process-rounds.json"])
    if variants:
        files.append("variants.json")
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
    if variants:
        frozen = [dict(recipe, label=name, module=str(recipe["module"]) if recipe["module"] else None)
                  for name, recipe in recipes.items()]
        write_report(dest / "variants.json", dict(schema_version=1, plan_sha256=inventory["plan_sha256"], left=left,
                     process_rounds=rounds, samples=args.samples, timeout_seconds=args.timeout_seconds,
                     warmup_overrides=warmup, variants=frozen))

    def collect(directory, order):
        return _collect_round(args, plan, raw_plan, directory, order, recipes, versions, inventory, warmup, rounds is not None)

    if rounds is None:
        coverage, _, compared, _ = collect(dest, products)
        summary = markdown_report(coverage, compared.values())
        evidence = "coverage.json"
        incomplete = bool(coverage["collection"]["errors"])
    else:
        report = _collect_process_rounds(dest, products, inventory, rounds, args.samples, collect)
        if variants:
            report["variants"] = "variants.json"
        evidence = "process-rounds.json"
        write_report(dest / evidence, report)
        summary = _rounds_markdown(report)
        incomplete = report["collection_status"] == "incomplete"
    with (dest / "summary.md").open("x", encoding="utf-8", newline="\n") as file:
        file.write(summary)
    if incomplete:
        raise ValueError(f"incomplete Node collection, see {dest / evidence}")
    return dest / "summary.md"


def _collect_round(args, plan, raw_plan, dest, order, recipes, versions, inventory, warmup, check_samples):
    coverage = dict(inventory, comparisons=[])
    accepted, runs, compared, errors = {}, {}, {}, {}
    for name in order:
        recipe = recipes[name]
        engine, module = recipe["engine"], recipe["module"]
        variant = dict(label=name, options_json=recipe["options_json"]) if "options_json" in recipe else None
        if variant and "site_layout" in recipe:
            variant["site_layout"] = recipe["site_layout"]
        report_path = dest / f"{name}.json"
        cmd = ["node", str(HERE / "collectors/node_runner.mjs"), "--engine", engine, "--plan", str(plan),
               "--samples", str(args.samples), "--timeout-seconds", str(args.timeout_seconds)]
        if module:
            cmd.extend(["--module", str(module)])
        if variant:
            cmd.extend(["--variant-label", name])
            if variant["options_json"] is not None:
                cmd.append(f"--options-json={variant['options_json']}")
            if "site_layout" in variant:
                cmd.extend(["--site-layout", variant["site_layout"]])
        for key, limit in warmup.items():
            cmd.extend([f"--warmup-{key.removeprefix('max_')}", str(limit)])
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
            if report["plan_sha256"] != coverage["plan_sha256"] or report["engine"] != engine:
                raise ValueError("collector returned a different plan or engine")
            if report.get("variant") != variant:
                raise ValueError("collector returned different variant settings")
            if (check_samples and
                    (report.get("samples") != args.samples or report.get("timeout_seconds") != args.timeout_seconds)):
                raise ValueError("collector returned a different sample count or timeout")
            if warmup or variant:
                observed = report.get("warmup")
                if (not isinstance(observed, dict) or any(type(observed.get(key)) is not int or observed[key] < 0
                                                         for key in ("max_calls", "max_ms"))
                        or any(observed[key] != limit for key, limit in warmup.items())):
                    raise ValueError("collector returned a different or missing warmup policy")
            if [row["id"] for row in report["scenarios"]] != [s["id"] for s in coverage["scenarios"]]:
                raise ValueError("collector returned an incomplete or different scenario selection")
            package = report["package"]
            if variant and module and Path(package["entry"]).resolve() != module.resolve():
                raise ValueError("collector returned a different module entry")
            if not module and versions.get(package["name"]) != package["version"]:
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
            drawing_products = {product_label(p): p["scenarios"] for p in audit["products"]}
            observer = audit.get("observer")
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            errors["drawing-audit.json"] = str(error)
            audit = {"plan_sha256": coverage["plan_sha256"], "error": str(error)}
        write_report(dest / "drawing-audit.json", audit)
        coverage["drawing_audit"] = "drawing-audit.json"

    left, *peers = recipes
    if "options_json" in recipes[left]:
        coverage["variants"] = "../../variants.json"
    for peer in peers:
        if left in accepted and peer in accepted:
            filename = _comparison_file(left, peer)
            try:
                result = compare_reference(accepted[left], accepted[peer], drawing_products=drawing_products)
                summary = summarize_coverage(raw_plan, [result])["comparisons"][0]
                write_report(dest / filename, result)
                coverage["comparisons"].append(summary)
                compared[peer] = result
            except (ValueError, KeyError, TypeError) as error:
                errors[filename] = str(error)
    coverage["collection"] = {"status": "incomplete" if errors else "complete", "runs": runs, "errors": errors}
    write_report(dest / "coverage.json", coverage)
    return coverage, accepted, compared, observer
