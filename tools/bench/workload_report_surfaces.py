"""Portable reader projections of recorded public workload evidence."""

from collections import Counter
from hashlib import sha256
import json
from math import isfinite
from pathlib import Path
import re

if __package__:
    from .weekly_workload_baseline import work_outcomes
    from .workload_coverage import _indexed, _schema_v1, _source_digest, summarize_coverage
    from .workload_reference import compare_reference
else:
    from weekly_workload_baseline import work_outcomes
    from workload_coverage import _indexed, _schema_v1, _source_digest, summarize_coverage
    from workload_reference import compare_reference


VERSION = r"v?\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?(?:\+[a-zA-Z0-9.-]+)?"
CLI_STATUSES = {"ok", "timeout", "exit_error", "process_error", "observer-limit", "invalid_output",
                "failed_svg_check", "expected-rejection", "unexpected-success"}


def _read_report(path):
    raw = path.read_bytes()
    report = json.loads(raw)
    if not isinstance(report, dict):
        raise ValueError("expected a JSON report object")
    return report, sha256(raw).hexdigest()


def _digest(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ValueError("expected a SHA-256 fingerprint")
    return value


def _identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.+-]*", value):
        raise ValueError("expected a portable identifier")
    return value


def _host(platform, arch):
    return {"platform": _identifier(platform), "arch": _identifier(arch)}


def _cli_product(product):
    engine = product["engine"]
    prefix = "merman-cli" if engine == "merman" else r"(?:mmdc|@mermaid-js/mermaid-cli)"
    version = product["version"].strip()
    if not re.fullmatch(rf"(?:{prefix} )?{VERSION}", version, re.ASCII):
        raise ValueError("expected an identifiable CLI version from the version probe")
    identity = product["artifact_identity"]
    artifact = dict(scope=identity["scope"], status=identity["status"], files=[
        {key: file[key] for key in ("role", "bytes", "sha256")}
        for file in identity["files"] if file.get("role") in ("executable", "script")
    ])
    if engine == "mmdc":
        artifact["browser"] = {"status": "unobserved"}
    result = dict(engine=engine, version=version, artifact_identity=artifact)
    decoder = product.get("png_decoder")
    if isinstance(decoder, dict) and decoder.get("name") and decoder.get("version"):
        result["png_decoder"] = {key: _identifier(decoder[key]) for key in ("name", "version")}
    return result


def _cli_row(row, global_reason):
    fields = ("planned_requests", "common_completed", "common_stable", "common_expected_rejections")
    result = dict(id=_identifier(row["id"]), **{key: row[key] for key in fields})
    reason = global_reason
    if "drawing_checks" in row:
        result["drawing_checks"] = row["drawing_checks"]
        if not all(status == "passed" for status in row["drawing_checks"].values()):
            reason = reason or "Required SVG drawing facts lack a passing audit for both products."

    def ratio_reason(timing):
        if any(value is not None and not isfinite(value) for value in timing.values()):
            raise ValueError("nonfinite CLI timing")
        return (reason or "A median time is zero.") if timing["right_over_left"] is None else None

    groups = [dict(group, ratio_withheld_reason=ratio_reason(group["render_ms"])) for group in row["render_timing_groups"]]
    batch = row["complete_batch_ms"]
    row_reason = reason
    if not groups:
        row_reason = reason or "No common stable reviewed output class."
    result.update(
        render_timing_groups=groups, common_render_ms=row["common_render_ms"], complete_batch_ms=batch,
        ratio_withheld_reason=row_reason,
        complete_batch_withheld_reason=ratio_reason(batch) if batch is not None
        else "No single common stable reviewed class covers every planned request.",
    )
    outputs = []
    for request in row["requests"]:
        item = dict(source_sha256=request["source_sha256"], expected_outcome=request["expected_outcome"])
        for side in ("left", "right"):
            output = request[side]
            statuses = {status if status in CLI_STATUSES else "other" for status in output["statuses"]}
            item[side] = dict(state=output["state"], statuses=sorted(statuses))
            if output["state"] == "stable":
                fields = ("output_bytes", "output_class", "dimensions_px")
                item[side].update({key: output[key] for key in fields if key in output})
        outputs.append(item)
    result["requests"] = outputs
    return result


def load_cli(plan: Path, left: Path, right: Path) -> dict:
    """Compare Merman (left) and mmdc (right) on their frozen request intersection."""
    raw_plan = plan.read_bytes()
    reports, hashes = zip(*(_read_report(path) for path in (left, right)))
    if any(not _schema_v1(report) or report.get("surface") != "cli" for report in reports):
        raise ValueError("expected two schema_version 1 raw CLI reports")
    if tuple(report.get("engine") for report in reports) != ("merman", "mmdc"):
        raise ValueError("expected Merman on the left and mmdc on the right")
    plan_sha = sha256(raw_plan).hexdigest()
    if any(report.get("plan_sha256") != plan_sha for report in reports):
        raise ValueError("CLI report plan_sha256 differs from the supplied plan bytes")
    try:
        declarations = {}
        if reports[0].get("format") == "svg":
            # Older raw reports omit drawing_required. A declaration alone cannot pass an audit.
            for scenario in json.loads(raw_plan)["scenarios"]:
                cases = [{"request_index": index} for index, request in enumerate(scenario["requests"])
                         if "drawing_facts" in request]
                if cases:
                    declarations[scenario["id"]] = {"cases": cases}
        comparison = compare_reference(*reports, drawing_products={engine: declarations for engine in ("merman", "mmdc")})
        inventory = summarize_coverage(raw_plan, [comparison])
    except (KeyError, IndexError, AttributeError, TypeError) as error:
        raise ValueError("incomplete CLI plan or raw report") from error
    planned = inventory["planned"]
    rows = [_cli_row(row, comparison["ratio_withheld_reason"]) for row in comparison["scenarios"]]
    for row, covered in zip(rows, inventory["comparisons"][0]["scenarios"], strict=True):
        row["stable_coverage"] = covered["common_stable"]
    coverage = dict(planned_scenarios=planned["scenarios"], reported_scenarios=len(rows),
                    planned_requests=planned["requests"], reported_requests=sum(row["planned_requests"] for row in rows),
                    planned_distinct_sources=planned["distinct_sources"])
    for key in ("common_completed", "common_stable", "common_expected_rejections"):
        coverage[key] = sum(row[key] for row in rows)
    sides = ("left", "right")
    return dict(
        surface="cli", classification="diagnostic", format=comparison["format"], samples=comparison["samples"],
        sample_unit="One sequential batch per sample and product, with a fresh CLI process per diagram. "
                    "Render groups use median sums of process milliseconds. Complete batches use median wall milliseconds.",
        provenance=dict(plan_sha256=plan_sha, report_schema_version=1, reports_sha256=dict(zip(sides, hashes)),
                        collector_sha256={side: _digest(value) if value else None
                                          for side, value in comparison["collector_sha256"].items()}),
        host=_host(comparison["host"]["platform"], comparison["host"]["arch"]),
        products={side: _cli_product(comparison[side]) for side in sides},
        render_settings=comparison["render_settings"], output_scope=comparison["output_scope"],
        ratio_withheld_reason=comparison["ratio_withheld_reason"], coverage=coverage, rows=rows,
    )


def _work_row(name, topology, row):
    dimensions = {}
    for key in ("nodes", "edges"):
        value = topology[key]
        if type(value) is not int or value < 0 or type(row[key]) is not int or row[key] != value:
            raise ValueError("native topology dimensions disagree with the plan")
        dimensions[key] = value
    if any(type(row[key]) is not int or row[key] <= 0 for key in ("default_budget", "high_budget")):
        raise ValueError("invalid native work budgets")
    outcomes = work_outcomes(row)
    boundary = row["boundary"]
    minimum = consumed = adjacent = None
    if boundary is None:
        if set(outcomes.values()) != {"source_max_edges_rejected"}:
            raise ValueError("missing native boundary without a consistent source maxEdges rejection")
    else:
        minimum = boundary["minimum_passing_budget"]
        consumed = boundary["svg_consumed_layout_work_units"]
        below = boundary["minimum_minus_one"]
        if (outcomes["layout_high"] != "accepted" or outcomes["svg_high"] != "accepted" or boundary["backend"] != "elk"
                or type(minimum) is not int or not 0 < minimum <= row["high_budget"]
                or below["status"] != "layout_work_rejected" or below["id"] != "max_layout_work_units"
                or below["phase"] != "layout_model" or below["maximum"] != minimum - 1 or below["actual"] != minimum
                or type(consumed) is not int or consumed < 0 or consumed != row["svg"]["high"]["layout_work_units"]):
            raise ValueError("native work boundary lacks its adjacent layout rejection or high-budget SVG observation")
        adjacent = {key: below[key] for key in ("status", "maximum", "actual")}
    order = {key: _identifier(topology[key]) for key in ("canonical_id", "statement_order") if key in topology}
    return dict(
        id=_identifier(name), **dimensions, **order, source_sha256=_source_digest(topology),
        default_budget=row["default_budget"], high_budget=row["high_budget"], **outcomes,
        minimum_passing_budget=minimum, svg_consumed_layout_work_units=consumed,
        minimum_minus_one=adjacent,
        boundary_withheld_reason=outcomes["layout_high"] if boundary is None else None,
    )


def load_rust(plan: Path, report: Path) -> dict:
    """Read a single native default-ELK layout-work observation with its public SVG probes."""
    raw_plan = plan.read_bytes()
    frozen = json.loads(raw_plan)
    native, report_sha = _read_report(report)
    recipe = dict(metric="work", operation="layout-work", samples=1, warmups=0, site_css_bytes=0, site_layout=None)
    if (not _schema_v1(frozen) or not _schema_v1(native) or native.get("surface") != "rust"
            or native.get("options") != recipe):
        raise ValueError("expected a schema_version 1 Rust layout-work report and workload plan")
    plan_sha = sha256(raw_plan).hexdigest()
    if native.get("plan_sha256") != plan_sha:
        raise ValueError("Rust report plan_sha256 differs from the supplied plan bytes")
    topologies = _indexed(frozen.get("topologies"), "id")
    observed = native.get("rows")
    if not isinstance(observed, dict) or not observed or not observed.keys() <= topologies.keys():
        raise ValueError("Rust report must select known topologies from the plan")
    host = native.get("host")
    if not isinstance(host, list) or len(host) != 3:
        raise ValueError("expected a native measurement host")
    try:
        rows = [_work_row(name, topology, observed[name]) for name, topology in topologies.items() if name in observed]
    except (KeyError, TypeError, AttributeError) as error:
        raise ValueError("incomplete native layout or SVG work evidence") from error
    coverage = dict(planned_topologies=len(topologies), reported_topologies=len(rows))
    for boundary in ("layout_default", "layout_high", "svg_default", "svg_high"):
        coverage[boundary] = dict(Counter(row[boundary] for row in rows))
    revision = native.get("revision")
    return dict(
        surface="rust", classification="same-implementation-observation", operation="layout-work", samples=1,
        sample_unit="One frozen Flowchart topology per native probe. Minimum passing budget is measured on the "
                    "layout writer. Consumed layout work units are recorded on the high-budget public SVG.",
        provenance=dict(plan_sha256=plan_sha, report_sha256=report_sha, report_schema_version=1,
                        runner_sha256=_digest(native.get("runner_sha256")), harness_sha256=_digest(native.get("harness_sha256"))),
        host=_host(host[0], host[1]),
        products={"merman": dict(engine="merman", version=None, revision=_identifier(revision) if revision is not None else None)},
        coverage=coverage, rows=rows,
    )


def _browser_interval(by_side, ratios, metric, iterations, *, include_partial=False):
    samples = dict(left=0, right=0)
    medians = dict(left=None, right=None)
    for side, metrics in by_side.items():
        stats = metrics[metric]
        if stats is None:
            continue
        count, value = stats["count"], stats["median"]
        if (type(count) is not int or not 0 < count <= iterations
                or type(value) not in (int, float) or not isfinite(value) or value < 0):
            raise ValueError(f"invalid browser {metric} statistics")
        samples[side] = count
        if count == iterations or include_partial:
            medians[side] = value
    ratio = ratios[metric]
    if ratio is not None and (
        type(ratio) not in (int, float) or not isfinite(ratio) or min(samples.values()) != iterations
        or not medians["left"] or ratio != medians["right"] / medians["left"]
    ):
        raise ValueError(f"browser ratio disagrees with its {metric} statistics")
    medians["right_over_left"] = ratio
    return samples, medians


def _browser_mode(evidence, mode, metric, source, versions, run):
    status = evidence["status"]
    report = evidence["report"]
    if evidence["mode"] != mode or status not in ("success", "failure", "skipped"):
        raise ValueError("unexpected browser corpus mode or outcome")
    if report is not None and report["schemaVersion"] != 7:
        raise ValueError("unexpected browser mode report schema")
    result = dict(metric=metric, status=status, seed=evidence["seed"])
    if type(result["seed"]) is not int or not 0 <= result["seed"] <= 0xffffffff:
        raise ValueError("invalid browser sample plan seed")
    if status != "success":
        # A stopped controller can retain partial samples without publishing this interval.
        count = None if report is not None else 0
        result.update(samples=dict(left=count, right=count), median_ms=dict(left=None, right=None, right_over_left=None),
                      ratio_withheld_reason=status)
        return result, None
    plan = report["plan"]
    if (evidence["failure"] is not None or evidence["skipReason"] is not None or report["terminalStatus"] != "success"
            or plan["mode"] != mode or plan["seed"] != evidence["seed"] or plan["iterations"] != run["iterations"]
            or (mode == "warm" and plan["warmups"] != run["warmups"])
            or report["versions"]["expected"] != versions
            or report["versions"]["observed"] != {key: [value] for key, value in versions.items()}):
        raise ValueError("browser mode report differs from its corpus plan, status or versions")
    raw_source = report["input"]["source"].encode("utf-8")
    if len(raw_source) != source["bytes"] or sha256(raw_source).hexdigest() != source["sha256"]:
        raise ValueError("browser report source differs from the selected fixture")
    aggregates = report["aggregates"]
    by_side = dict(left=aggregates["engines"]["merman"], right=aggregates["engines"]["mermaid"])
    ratios = aggregates["ratios"]
    iterations = run["iterations"]
    samples, medians = _browser_interval(by_side, ratios, metric, iterations, include_partial=True)
    result.update(samples=samples, median_ms=medians,
                  ratio_withheld_reason="Withheld by the browser report." if medians["right_over_left"] is None else None)
    budgeted = "warmBudgetedSvgMs"
    if mode == "warm" and any(budgeted in fields for fields in (ratios, *by_side.values())):
        # This interval has no per-side counts in the receipt. Publish only complete medians.
        _, result["budgeted_svg_ms"] = _browser_interval(by_side, ratios, budgeted, iterations)
    env = report["environment"]
    platform = env["platform"]
    if not re.fullmatch(r"[A-Za-z0-9_.-]+(?: [A-Za-z0-9_.-]+)*", platform):
        raise ValueError("invalid browser platform")
    chrome = re.search(r"\b(?:Headless)?Chrome/(\d+(?:\.\d+){1,3})\b", env["userAgent"])
    return result, dict(platform=platform, arch=None, chromium_version=chrome[1] if chrome else None)


def load_browser(path: Path) -> dict:
    """Read publishable SVG intervals and any recorded warm budgeted SVG interval from the playground corpus."""
    corpus, report_sha = _read_report(path)
    if (corpus.get("kind") != "merman-browser-family-corpus" or corpus.get("schemaVersion") != 2
            or corpus.get("benchmarkReportSchemaVersion") != 7
            or corpus.get("execution") != {"fixtureIsolation": "fresh-browser-process-per-fixture"}):
        raise ValueError("expected a playground v2 browser corpus with v7 mode reports")
    try:
        catalog, run, counts = corpus["catalog"], corpus["run"], corpus["coverage"]
        if (catalog["id"] != "playground-generated-examples/family-baseline" or catalog["role"] != "family-baseline"
                or catalog["source"] != "playground/src/generated/examples.ts"
                or not re.fullmatch(r"mermaid@[0-9][A-Za-z0-9.+-]*", catalog["mermaidBaseline"])):
            raise ValueError("unexpected browser corpus catalog")
        if (type(run["iterations"]) is not int or run["iterations"] < 2 or run["iterations"] % 2
                or type(run["warmups"]) is not int or run["warmups"] < 0
                or type(run["masterSeed"]) is not int or not 0 <= run["masterSeed"] <= 0xffffffff):
            raise ValueError("invalid browser corpus sample plan")
        versions = {engine: _identifier(corpus["versions"][engine]) for engine in ("merman", "mermaid")}
        if not all(re.fullmatch(VERSION, value, re.ASCII) for value in versions.values()):
            raise ValueError("expected browser product versions")
        fixtures = _indexed(corpus["fixtures"], "id")
        if not fixtures or list(fixtures) != run["order"]:
            raise ValueError("browser corpus fixture order differs from the selected plan")
        rows, host = [], None
        for name, fixture in fixtures.items():
            source = fixture["source"]
            row = dict(id=_identifier(name), family=_identifier(fixture["family"]), status=fixture["status"],
                       attempted=fixture["attempted"], source_sha256=_digest(source["sha256"]), source_bytes=source["bytes"])
            if type(row["source_bytes"]) is not int or row["source_bytes"] < 0 or type(row["attempted"]) is not bool:
                raise ValueError("invalid browser fixture identity")
            for key, mode, metric in (("cold", "realm-cold", "firstPublishableSvgMs"), ("warm", "warm", "warmPublishableSvgMs")):
                row[key], observed_host = _browser_mode(fixture[key], mode, metric, source, versions, run)
                if observed_host is not None:
                    if not row["attempted"] or (host is not None and host != observed_host):
                        raise ValueError("inconsistent browser corpus host or attempted fixture")
                    host = observed_host
            success = fixture["failure"] is None and row["cold"]["status"] == row["warm"]["status"] == "success"
            if row["status"] != ("success" if success else "failure"):
                raise ValueError("inconsistent browser fixture outcome")
            rows.append(row)
        succeeded = sum(row["status"] == "success" for row in rows)
        terminal = corpus["terminalStatus"]
        if (terminal not in ("success", "complete-with-errors", "cancelled", "invalidated")
                or (terminal == "success") != (succeeded == len(rows))
                or type(catalog["availableFamilies"]) is not int or catalog["availableFamilies"] < len(rows)
                or counts != dict(availableFamilies=catalog["availableFamilies"], selectedFamilies=len(rows),
                                  attemptedFamilies=sum(row["attempted"] for row in rows), succeededFamilies=succeeded,
                                  failedFamilies=len(rows) - succeeded)):
            raise ValueError("browser corpus coverage disagrees with its fixtures")
    except (TypeError, KeyError, AttributeError) as error:
        raise ValueError("incomplete browser corpus evidence") from error
    return dict(
        surface="browser", classification="diagnostic", terminal_status=terminal, samples=run["iterations"],
        sample_unit="Each measured block runs both engines once in balanced AB/BA order. A fresh Chromium process hosts "
                    "each fixture. Cold uses a new realm and warm reuses the engine.",
        provenance=dict(report_sha256=report_sha, corpus_schema_version=2, report_schema_version=7,
                        catalog=catalog["id"], mermaid_baseline=catalog["mermaidBaseline"],
                        master_seed=run["masterSeed"], warmups=run["warmups"]),
        host=host or dict(platform=None, arch=None, chromium_version=None),
        products={"left": dict(engine="merman", backend="browser-wasm", version=versions["merman"]),
                  "right": dict(engine="mermaid", backend="javascript", version=versions["mermaid"])},
        coverage={f"{key}_families": counts[f"{key}Families"] for key in ("available", "selected", "attempted", "succeeded", "failed")},
        rows=rows,
    )
