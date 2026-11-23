"""Compare two external workload reports on their fixed, repeatable request intersection."""

import json
from math import isfinite
from pathlib import Path
from statistics import median


NODE_PRODUCTS = json.loads(Path(__file__).with_name("workload_node_products.json").read_bytes())


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
        return dict(
            engine=engine,
            package=name,
            version=version,
            entry_sha256=package.get("entry_sha256"),
            backend=backend,
            render_api=product["render_api"],
            node_project=report.get("node_project"),
        )
    if not report.get("version") or report.get("version_probe", {}).get("status") != "ok":
        raise ValueError(f"missing verified CLI version for {engine}")
    fields = ("engine", "version", "binary", "font_files", "extra_args", "render_environment")
    return {key: report.get(key) for key in fields}


def _output(results, node, png):
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
    if "ok" not in statuses:
        state = "missing"
    elif len(statuses) > 1:
        state = "intermittent"
    elif len(results) < 2 or not identified:
        state = "unverified-output"
    elif len(hashes) > 1 or len(sizes) > 1 or len(dimensions) > 1:
        state = "unstable-output"
    else:
        state = "stable"
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


def _scenario(left, right, node, png, samples, allow_ratio):
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

    timed, observed = [], []
    for row in (left, right):
        batches = row["warm_samples" if node else "batches"]
        all_batches = [row["cold_batch"], *batches] if node else batches
        if len(batches) != samples or any(len(b["results"]) != planned for b in all_batches):
            raise ValueError(f"incomplete sample or request sequence in {name!r}")
        timed.append(batches)
        observed.append(all_batches)

    requests, common = [], []
    completed = 0
    for i, (a, b) in enumerate(zip(left["requests"], right["requests"])):
        keys = ("fixture", "diagram_id", "source_sha256")
        same_labels = a.get("expected_labels", []) == b.get("expected_labels", [])
        if not same_labels or any(not a.get(k) or a[k] != b.get(k) for k in keys):
            raise ValueError(f"different request identity in {name!r} at index {i}")
        outputs = [_output([batch["results"][i] for batch in side], node, png) for side in observed]
        request = dict(
            fixture=a["fixture"], diagram_id=a["diagram_id"], source_sha256=a["source_sha256"],
            left=outputs[0], right=outputs[1],
        )
        if a.get("expected_labels"):
            request["expected_labels"] = a["expected_labels"]
        requests.append(request)
        if all(output["statuses"] == ["ok"] for output in outputs):
            completed += 1
        if all(output["state"] == "stable" for output in outputs):
            common.append(i)

    result = dict(
        id=name,
        planned_requests=planned,
        common_completed=completed,
        common_stable=len(common),
        requests=requests,
        common_render_ms=None,
        complete_batch_ms=None,
    )
    if common:

        def render_ms(batch):
            return sum(_ms(batch["results"][i]["elapsed_ms"]) for i in common)

        result["common_render_ms"] = _timing(timed, render_ms, allow_ratio)
        if len(common) == planned:
            wall = "complete_batch_ms" if node else "wall_ms"
            result["complete_batch_ms"] = _timing(timed, lambda batch: batch[wall], allow_ratio)
    return result


def compare_reference(left: dict, right: dict) -> dict:
    """Use warm Node batches (cold also checks identity) or fresh CLI processes per request.

    common_completed counts requests that passed every adapter check on both sides. common_stable
    also requires repeat output identity within each product. Sum those request times per sample
    before taking the median. Complete batches use the adapter wall clock. Ratios divide right by left.
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
    if node and any(report.get("failed_scenario") for report in (left, right)):
        raise ValueError("incomplete Node plan after a failed scenario")
    engines = {left.get("engine"), right.get("engine")}
    valid_pair = (len(engines) == 2 and engines <= NODE_PRODUCTS.keys()) if node else engines == {"merman", "mmdc"}
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
    collectors = [report.get("collector_sha256") for report in (left, right)]
    if not collectors[0] or collectors[0] != collectors[1]:
        reason = "The collector or its output checks differ or lack a recorded fingerprint."
        withheld = f"{withheld} {reason}" if withheld else reason
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
        scenarios=[
            _scenario(a, b, node, fmt == "png", samples, withheld is None)
            for a, b in zip(rows, right["scenarios"])
        ],
    )
