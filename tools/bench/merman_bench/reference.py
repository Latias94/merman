"""Compare two external workload reports on their fixed, repeatable request intersection."""

import json
import re
from math import isfinite
from pathlib import Path
from statistics import median

from .svg import EXPECTED_OUTCOMES


NODE_PRODUCTS = json.loads((Path(__file__).resolve().parents[1] / "workloads.json").read_bytes())["node_products"]
LAYOUTS = ("dagre", "elk", "shoal")


def valid_node_variant_label(label):
    return (isinstance(label, str)
            and re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9_.-]{0,62}[A-Za-z0-9_-])?", label) is not None
            and re.fullmatch(r"CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9]", label.split(".")[0], re.IGNORECASE) is None)


def product_label(product):
    if "variant" in product:
        return product["variant"]["label"]
    return product["engine"]


def expected_outcome(request):
    outcome = request.get("expected_outcome", "draw")
    if outcome not in EXPECTED_OUTCOMES:
        raise ValueError(f"invalid expected_outcome: {outcome!r}")
    return outcome


def node_worker_failures(report):
    """Only an ordered row for every planned worker can account for a failed collection."""
    if "failed_scenario" in report:
        raise ValueError("incomplete Node plan after a failed scenario")
    if report.get("error"):
        raise ValueError(report["error"])
    rows = report.get("scenarios")
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise ValueError("expected Node scenario rows")
    count = report.get("planned_scenarios")
    if "planned_scenarios" in report and (type(count) is not int or count != len(rows)):
        raise ValueError("incomplete Node scenario sequence")
    failures = []
    for i, row in enumerate(rows):
        if count is not None and (type(row.get("index")) is not int or row["index"] != i):
            raise ValueError("unordered Node scenario sequence")
        if "status" not in row:
            continue
        requests = row.get("requests")
        if (count is None or row["status"] not in ("failed", "timeout")
                or not isinstance(requests, list) or not requests
                or type(row.get("unmeasured_requests")) is not int or row["unmeasured_requests"] != len(requests)
                or not isinstance(row.get("error"), str) or not row["error"]
                or row.get("init_ms") is not None or row.get("first_render_ms") is not None
                or row.get("cold_batch") is not None or row.get("warm_samples") != []):
            raise ValueError("invalid unmeasured Node scenario")
        failures.append({key: row[key] for key in ("index", "id", "status", "error")})
    status = report.get("status")
    if status not in (None, "ok", "failed") or bool(failures) != (status == "failed"):
        raise ValueError("incomplete Node plan or inconsistent worker failures")
    return failures


def _artifact_identity(report, scope, roles):
    identity = report.get("artifact_identity")
    if identity is None:
        return dict(scope=scope, status="unknown", files=[], reason="This collector did not record product files")
    if (not isinstance(identity, dict) or identity.get("scope") != scope
            or identity.get("status") not in ("recorded", "unknown")):
        raise ValueError("invalid product artifact identity")
    files = identity.get("files")
    if not isinstance(files, list) or any(
        not isinstance(file, dict) or not isinstance(file.get("path"), str) or not file["path"]
        or type(file.get("bytes")) is not int or file["bytes"] < 0
        or not isinstance(file.get("sha256"), str) or not re.fullmatch("[0-9a-f]{64}", file["sha256"])
        for file in files
    ):
        raise ValueError("invalid product artifact file evidence")
    if identity["status"] == "recorded" and [file.get("role") for file in files] != roles:
        raise ValueError("incomplete product artifact identity")
    if identity["status"] == "unknown" and not (isinstance(identity.get("reason"), str) and identity["reason"].strip()):
        raise ValueError("unknown product artifact identity needs a reason")
    return identity


def _product(report, node):
    engine = report["engine"]
    if node:
        package = report.get("package", {})
        product = NODE_PRODUCTS.get(engine, {})
        name, version = package.get("name"), package.get("version")
        backend = (product.get("backend") if name == product.get("package")
                   else product.get("alternatives", {}).get(name))
        if (
            not backend or report.get("backend") != backend
            or report.get("render_api") != product["render_api"]
            or not isinstance(version, str) or not version
            or ("major" in product and version.partition(".")[0] != str(product["major"]))
        ):
            raise ValueError("unrecognized Node package, version, backend or render API")
        roles = {"napi": ["native-addon"], "node-wasm": ["wasm-glue", "wasm-module"], "javascript": []}[backend]
        identity = _artifact_identity(report, "node-entry-and-transport" if roles else "node-entry", ["entry", *roles])
        if identity["status"] == "recorded":
            entry = identity["files"][0]
            if (package.get("entry", entry["path"]) != entry["path"]
                    or package.get("entry_sha256", entry["sha256"]) != entry["sha256"]):
                raise ValueError("product artifact evidence differs from the Node entry")
        variant = report.get("variant")
        if "variant" in report:
            if (not isinstance(variant, dict) or variant.keys() - {"site_layout"} != {"label", "options_json"}
                    or not valid_node_variant_label(variant["label"])):
                raise ValueError("invalid Node variant identity")
            options = variant["options_json"]
            if options is not None and (engine != "merman" or not isinstance(options, str) or not options.strip()):
                raise ValueError("explicit Options JSON requires a Merman variant")
            if "site_layout" in variant and (engine != "merman" or variant["site_layout"] not in LAYOUTS):
                raise ValueError("a site layout requires a Merman variant and a supported backend")
        return dict(
            engine=engine,
            package=name,
            version=version,
            entry_sha256=package.get("entry_sha256"),
            artifact_identity=identity,
            backend=backend,
            render_api=product["render_api"],
            node_project=report.get("node_project"),
            **({"variant": variant} if variant else {}),
        )
    if not report.get("version") or report.get("version_probe", {}).get("status") != "ok":
        raise ValueError(f"missing verified CLI version for {engine}")
    fields = ("engine", "version", "binary", "font_files", "extra_args", "render_environment")
    product = {key: report.get(key) for key in fields}
    prefix = report.get("command_prefix") or []
    roles = ["executable", "script"] if len(prefix) > 1 else ["executable"]
    identity = _artifact_identity(report, "command-prefix", roles)
    if identity["status"] == "recorded":
        expected_paths = [prefix[0], prefix[-1]] if len(prefix) > 1 else prefix
        if not isinstance(prefix, list) or [file["path"] for file in identity["files"]] != expected_paths:
            raise ValueError("product artifact evidence differs from the CLI command prefix")
    product.update(artifact_identity=identity, command_prefix=report.get("command_prefix"))
    if report.get("format") == "png":
        product["png_decoder"] = report.get("png_decoder")
    return product


def _output(results, node, png, outcome="draw"):
    if not results:
        return dict(state="unmeasured", statuses=["not-measured"], output_bytes=[])
    if any(not isinstance(r, dict) or not isinstance(r.get("status"), str) or not r["status"]
           or r.get("expected_outcome", outcome) != outcome for r in results):
        raise ValueError(f"invalid collector result for expected_outcome {outcome!r}")
    if outcome == "draw" and any(r["status"] == "ok" and r.get("output_class", "draw") != "draw" for r in results):
        raise ValueError("draw result has an incompatible output_class")
    size, digest = ("output_bytes", "output_sha256") if node else ("bytes", "sha256")
    statuses = sorted({r["status"] for r in results})
    sizes = {r.get(size) for r in results}
    hashes = {r.get(digest) for r in results}
    summary = dict(statuses=statuses, output_bytes=sorted(n for n in sizes if n is not None))
    dimensions = set()
    if png:
        dimensions = {(r.get("width"), r.get("height")) for r in results}
        summary["dimensions_px"] = [list(d) for d in sorted(d for d in dimensions if None not in d)]

    measurements = sizes | {n for d in dimensions for n in d}
    identified = all(isinstance(h, str) and h for h in hashes) and all(
        type(n) is int and n > 0 for n in measurements
    )
    if statuses == ["expected-rejection"]:
        state = "expected-rejection"
    elif "ok" not in statuses:
        state = "missing"
    elif len(statuses) > 1:
        state = "intermittent"
    elif len(results) < 2 or not identified:
        state = "unverified-output"
    elif len(hashes) > 1 or len(sizes) > 1 or len(dimensions) > 1:
        state = "unstable-output"
    else:
        state = "stable"
        # Historical draw checks already required paint. Other policies need the reader's verdict.
        default = "draw" if outcome == "draw" else None
        kind = results[0].get("output_class", default)
        if kind in ("draw", "empty") and all(r.get("output_class", default) == kind for r in results):
            summary["output_class"] = kind
    return dict(state=state, **summary)


def _ms(value):
    if type(value) not in (int, float) or not isfinite(value) or value < 0:
        raise ValueError(f"invalid elapsed milliseconds: {value!r}")
    return value


def _timing(timed_batches, measure, allow_ratio):
    left, right = (median(_ms(measure(batch)) for batch in batches) for batches in timed_batches)
    return dict(
        left=left,
        right=right,
        right_over_left=right / left if allow_ratio and min(left, right) > 0 else None,
    )


def _scenario(left, right, node, png, samples, allow_ratio, drawings):
    name = left["id"]
    planned = len(left["requests"])
    if (
        not name
        or right["id"] != name
        or left.get("index") != right.get("index")
        or not planned
        or len(right["requests"]) != planned
    ):
        raise ValueError(f"different or empty scenario selection at {name!r}")

    timed, observed, worker_failures = [], [], {}
    for side, row in (("left", left), ("right", right)):
        if node and "status" in row:
            worker_failures[side] = {key: row[key] for key in ("status", "error")}
            timed.append([])
            observed.append([])
            continue
        batches = row.get("warm_samples" if node else "batches")
        if not isinstance(batches, list) or len(batches) != samples:
            raise ValueError(f"incomplete samples in {name!r}")
        all_batches = [row.get("cold_batch"), *batches] if node else batches
        if any(not isinstance(b, dict) or not isinstance(b.get("results"), list) or len(b["results"]) != planned
               for b in all_batches):
            raise ValueError(f"incomplete sample or request sequence in {name!r}")
        timed.append(batches)
        observed.append(all_batches)

    requests, timing_groups = [], {}
    completed = stable = rejections = 0
    # Older raw reports omit the request flag. An audit binds the declared indices to the plan.
    declared = {case["request_index"] for side in drawings.values() for case in side.get("cases", [])}
    for i, (a, b) in enumerate(zip(left["requests"], right["requests"])):
        keys = ("fixture", "diagram_id", "source_sha256")
        same_labels = a.get("expected_labels", []) == b.get("expected_labels", [])
        outcome = expected_outcome(a)
        drawing_required = a.get("drawing_required", i in declared)
        if (not same_labels or outcome != expected_outcome(b)
                or type(drawing_required) is not bool or drawing_required is not b.get("drawing_required", i in declared)
                or (i in declared and not drawing_required)
                or any(not a.get(k) or a[k] != b.get(k) for k in keys)):
            raise ValueError(f"different request identity in {name!r} at index {i}")
        outputs = [_output([batch["results"][i] for batch in side], node, png, outcome) for side in observed]
        for output in outputs:
            if ((outcome == "reject" and "ok" in output["statuses"])
                    or (outcome != "reject" and "expected-rejection" in output["statuses"])):
                raise ValueError(f"collector status disagrees with expected_outcome in {name!r} at index {i}")
        request = dict(
            fixture=a["fixture"], diagram_id=a["diagram_id"], source_sha256=a["source_sha256"], expected_outcome=outcome,
            left=outputs[0], right=outputs[1],
        )
        if a.get("expected_labels"):
            request["expected_labels"] = a["expected_labels"]
        if drawing_required:
            request["drawing_required"] = True
        requests.append(request)
        if all(output["statuses"] == ["ok"] for output in outputs):
            completed += 1
        if all(output["state"] == "expected-rejection" for output in outputs):
            rejections += 1
        if all(output["state"] == "stable" for output in outputs):
            stable += 1
            kind = outputs[0].get("output_class")
            if outcome in ("draw", "allow-empty") and kind and kind == outputs[1].get("output_class"):
                timing_groups.setdefault((outcome, kind), []).append(i)

    result = dict(
        id=name,
        planned_requests=planned,
        common_completed=completed,
        common_stable=stable,
        common_expected_rejections=rejections,
        requests=requests,
        common_render_ms=None,
        complete_batch_ms=None,
    )
    if worker_failures:
        result["worker_failures"] = worker_failures
    if any(r.get("drawing_required") for r in requests):
        checks = {side: audit.get("status", "not-checked") for side, audit in drawings.items()}
        result["drawing_checks"] = checks
        allow_ratio = allow_ratio and all(s == "passed" for s in checks.values())
    result["render_timing_groups"] = []
    for (outcome, kind), indices in timing_groups.items():
        def render_ms(batch):
            return sum(_ms(batch["results"][i]["elapsed_ms"]) for i in indices)

        timing = _timing(timed, render_ms, allow_ratio)
        result["render_timing_groups"].append(dict(
            expected_outcome=outcome, output_class=kind, request_indices=indices, render_ms=timing,
        ))
        if len(timing_groups) == 1:
            result["common_render_ms"] = timing
        if len(indices) == planned:
            wall = "complete_batch_ms" if node else "wall_ms"
            result["complete_batch_ms"] = _timing(timed, lambda batch: batch[wall], allow_ratio)
    return result


def compare_reference(left: dict, right: dict, *, drawing_products=None) -> dict:
    """Use warm Node batches (cold also checks identity) or fresh CLI processes per request.

    common_completed counts requests that passed every adapter check on both sides. common_stable
    also requires repeat output identity within each product. Sum request times per warm sample
    within each matching reviewed output class. A complete-batch time requires that one class
    covers the entire plan row. Expected rejections have their own count. Ratios divide right by left.
    """
    for key in ("schema_version", "surface", "plan_sha256", "host", "process_lifecycle", "samples"):
        if not left.get(key) or left[key] != right.get(key):
            raise ValueError(f"incompatible reports: {key}")
    surface, host, samples = left["surface"], left["host"], left["samples"]
    if left["schema_version"] != 1 or surface not in ("node", "cli"):
        raise ValueError("expected schema_version 1 Node or CLI reports")
    if not isinstance(host, dict) or not all(host.get(k) for k in ("hostname", "platform", "arch")):
        raise ValueError("host must identify a hostname, platform and architecture")
    if type(samples) is not int or samples < 1:
        raise ValueError("samples must be a positive integer")

    node = surface == "node"
    if left.get("timeout_seconds") != right.get("timeout_seconds"):
        raise ValueError(f"incompatible {surface} timeout_seconds")
    if node:
        for report in (left, right):
            node_worker_failures(report)
    engines = {left.get("engine"), right.get("engine")}
    variants = sum("variant" in report for report in (left, right)) if node else 0
    if node:
        valid_pair = engines <= NODE_PRODUCTS.keys() and (variants == 2 or (variants == 0 and len(engines) == 2))
    else:
        valid_pair = engines == {"merman", "mmdc"}
    if not valid_pair:
        raise ValueError(f"incompatible {surface} engine pair")
    field = "output_format" if node else "format"
    fmt = left.get(field)
    formats = ("svg",) if node else ("svg", "png")
    if fmt != right.get(field) or fmt not in formats:
        raise ValueError(f"incompatible {surface} output format")
    if left["process_lifecycle"] != ("fresh-per-scenario" if node else "fresh-process-per-diagram"):
        raise ValueError(f"unexpected {surface} process_lifecycle")

    products = [_product(report, node) for report in (left, right)]
    if variants and product_label(products[0]).casefold() == product_label(products[1]).casefold():
        raise ValueError("Node comparison requires distinct variant labels")
    withheld = None
    if node and "mermaid-v13" in engines:
        project = left.get("node_project")
        files = project.get("digests") if isinstance(project, dict) else None
        if (not isinstance(files, dict) or not files.get("package.json") or len(files) < 2
                or not project.get("cwd") or project != right.get("node_project")):
            withheld = "Mermaid v13 discovers plugins in the consumer project. Run both products from the same project and dependency lock."
    elif not node:
        environments = [p["render_environment"] for p in products]
        browser_locations = {"PUPPETEER_CACHE_DIR", "PUPPETEER_EXECUTABLE_PATH"}
        if (
            any(p["font_files"] != [] or p["extra_args"] != [] for p in products)
            or not isinstance(environments[0], dict)
            or environments[0] != environments[1]
            or set(environments[0]) - browser_locations
        ):
            withheld = "Equivalent defaults cannot be established from CLI font files, extra arguments or environment."

    rows = left["scenarios"]
    if not rows or len(rows) != len(right["scenarios"]):
        raise ValueError("different or empty scenario selection")
    settings = "explicit, different or unknown" if withheld else "renderer defaults"
    if variants:
        settings = "declared Node variants"
    collectors = [report.get("collector_sha256") for report in (left, right)]
    if not collectors[0] or collectors[0] != collectors[1]:
        reason = "The collector or its output checks differ or lack a recorded fingerprint."
        withheld = f"{withheld} {reason}" if withheld else reason
    unknown = [product_label(p) for p in products if p["artifact_identity"]["status"] == "unknown"]
    if unknown:
        reason = f"Product artifact identity is unknown for {', '.join(unknown)}."
        withheld = f"{withheld} {reason}" if withheld else reason
    if fmt == "png":
        decoder = products[0]["png_decoder"]
        if (not isinstance(decoder, dict) or not decoder.get("name") or not decoder.get("version")
                or decoder != products[1]["png_decoder"]):
            reason = "The reports need matching PNG decoder identities to compare verified pixel content."
            withheld = f"{withheld} {reason}" if withheld else reason
    drawing_products = drawing_products or {}
    scenarios = []
    for a, b in zip(rows, right["scenarios"]):
        drawings = {
            side: drawing_products.get(product_label(product), {}).get(a["id"], {})
            for side, product in zip(("left", "right"), products, strict=True)
        }
        scenarios.append(_scenario(a, b, node, fmt == "png", samples, withheld is None, drawings))
    return dict(
        schema_version=1,
        classification="diagnostic",
        surface=surface,
        format=fmt,
        host=host,
        plan_sha256=left["plan_sha256"],
        left=products[0],
        right=products[1],
        collector_sha256={"left": collectors[0], "right": collectors[1]},
        samples=samples,
        timing_samples="warm batches" if node else "fresh process per diagram",
        render_settings=settings,
        output_scope="Merman native rasterization and mmdc browser screenshots use each renderer's fonts and dimensions"
        if fmt == "png"
        else "SVG at each renderer's font and layout settings",
        ratio_withheld_reason=withheld,
        scenarios=scenarios,
    )
