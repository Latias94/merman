"""Audit declared drawing facts on the exact SVG bytes retained by workload collectors."""

from collections.abc import Iterable
from copy import deepcopy
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import stat
import subprocess

from .coverage import _indexed, _schema_v1, _source_digest, ordered_requests
from .reference import NODE_PRODUCTS, _output, _product, expected_outcome, node_worker_failures


OBSERVER = (Path(__file__).resolve().parents[1] / "workload_drawing_browser.mjs")
IDENTITY = ("fixture", "diagram_id", "source_sha256")


def _declared(facts):
    arrows = [dict(kind="arrow", **arrow) for arrow in facts.get("arrows", [])]
    members = [dict(kind="membership", group=group, node=node)
               for group, nodes in facts.get("groups", {}).items() for node in nodes]
    widths = [dict(kind="label-width", node=node, label=label) for node, label in facts.get("label_widths", {}).items()]
    messages = [dict(kind="sequence-message-count", label=label, expected=count)
                for label, count in facts.get("sequence_messages", {}).items()]
    return arrows + members + widths + messages


def drawing_case(request, index):
    return {**{key: request[key] for key in IDENTITY}, "request_index": index, "status": "unavailable",
            "facts": [dict(f, status="unavailable") for f in _declared(request["drawing_facts"])]}


def _inventory(raw):
    from .workloads import parse_workload_json, validate_drawing_facts

    plan = parse_workload_json(raw, "drawing plan")
    if not _schema_v1(plan):
        raise ValueError("expected workload plan schema_version 1")
    scenarios = _indexed(plan.get("scenarios"), "id")
    inventory = {}
    for name, scenario in scenarios.items():
        cases = []
        for index, request in enumerate(ordered_requests(scenario)):
            if not isinstance(request.get("fixture"), str) or not request["fixture"]:
                raise ValueError(f"expected a fixture in {name!r} at index {index}")
            request["source_sha256"] = _source_digest(request)
            if "drawing_facts" in request:
                facts = validate_drawing_facts(request["drawing_facts"], f"{name}/{request['diagram_id']}")
                request["drawing_facts"] = {"arrows": [], "groups": {}, "label_widths": {}, **facts}
                cases.append(drawing_case(request, index))
        inventory[name] = dict(occurrences=len(cases), facts=sum(len(c["facts"]) for c in cases), cases=cases)
    return scenarios, inventory


def _batches(scenario, row, index, node, samples):
    name = scenario["id"]
    if not isinstance(row, dict) or row.get("id") != name or (
        "index" in row and (type(row["index"]) is not int or row["index"] != index)
    ):
        raise ValueError(f"different scenario identity at index {index}, expected {name!r}")
    requests = row.get("requests")
    planned = scenario["requests"]
    if not isinstance(requests, list) or len(requests) != len(planned):
        raise ValueError(f"incomplete request sequence in {name!r}")
    for j, (want, got) in enumerate(zip(planned, requests)):
        if (not isinstance(got, dict) or any(got.get(k) != want[k] for k in IDENTITY)
                or got.get("expected_labels", []) != want.get("expected_labels", [])
                or expected_outcome(got) != expected_outcome(want)):
            raise ValueError(f"different request identity in {name!r} at index {j}")
        required = "drawing_facts" in want
        if got.get("drawing_required", required) is not required:
            raise ValueError(f"drawing_required disagrees with the plan in {name!r} at index {j}")
    if node and row.get("status") in ("failed", "timeout"):
        return None
    batches = row.get("warm_samples" if node else "batches")
    if not isinstance(batches, list) or len(batches) != samples:
        raise ValueError(f"incomplete samples in {name!r}")
    if node:
        batches = [row.get("cold_batch"), *batches]
    for batch in batches:
        results = batch.get("results") if isinstance(batch, dict) else None
        if not isinstance(results, list) or len(results) != len(planned):
            raise ValueError(f"incomplete batch results in {name!r}")
        if any(not isinstance(r, dict) or not isinstance(r.get("status"), str) or not r["status"] for r in results):
            raise ValueError(f"missing collector result status in {name!r}")
    return [b["results"] for b in batches]


def _svg_path(root, name):
    if not isinstance(name, str):
        raise ValueError("missing SVG output filename")
    relative = PureWindowsPath(name)
    if relative.anchor or ":" in name or ".." in relative.parts or relative.suffix.lower() != ".svg":
        raise ValueError(f"unsafe SVG output filename: {name!r}")
    path = root.joinpath(*relative.parts).resolve()
    if not path.is_relative_to(root):
        raise ValueError(f"SVG artifact resolves outside its directory: {name!r}")
    return path


def _candidates(report_path, report, row, i, j, results):
    node = report["surface"] == "node"
    directory = report.get("drawing_dir" if node else "run_dir")
    if node:
        name = row["requests"][j].get("drawing_artifact")
        if name is None:
            raise FileNotFoundError("collector did not retain a drawing_artifact for this request")
        if name != f"{i}-{j}.svg":
            raise ValueError("drawing_artifact differs from the planned occurrence filename")
    if not isinstance(directory, str) or ".." in PureWindowsPath(directory).parts or not (
        PureWindowsPath(directory).is_absolute() or PurePosixPath(directory).is_absolute()
    ):
        raise ValueError("expected an absolute artifact directory in the raw report")
    if node:
        # Keep this root lexical under the resolved bundle so a relocated directory link cannot escape it.
        root = report_path.parent.resolve() / "drawings" / report["engine"]
        yield _svg_path(root, name), results[-1], report["samples"] - 1
    original = Path(directory)
    if original.is_absolute():
        root = original.resolve()
        if node:
            yield _svg_path(root, name), results[-1], report["samples"] - 1
        else:
            paths = [_svg_path(root, result.get("output")) for result in results]
            # CLI postflight hashes every output after the run. Aliased files cannot prove repeatability.
            if len(set(paths)) != len(paths):
                raise ValueError("multiple CLI samples refer to the same output file")
            for index, (path, result) in enumerate(zip(paths, results)):
                yield path, result, index


def _read_artifact(report_path, report, row, i, j, results):
    node = report["surface"] == "node"
    for path, sample, sample_index in _candidates(report_path, report, row, i, j, results):
        try:
            if not stat.S_ISREG(path.stat().st_mode):
                raise ValueError(f"expected a regular SVG file: {path}")
            data = path.read_bytes()
        except FileNotFoundError:
            continue
        digest = hashlib.sha256(data).hexdigest()
        if (len(data) != sample["output_bytes" if node else "bytes"]
                or digest != sample["output_sha256" if node else "sha256"]):
            raise ValueError(f"retained SVG bytes or SHA-256 differ from the raw result: {path}")
        return data.decode("utf-8"), dict(path=str(path), bytes=len(data), sha256=digest, sample_index=sample_index)
    raise FileNotFoundError("recorded SVG artifact is missing")


def _collect(report_path, report, scenarios, inventory, plan_sha256, pending):
    if not _schema_v1(report) or report.get("surface") not in ("node", "cli"):
        raise ValueError("expected a schema_version 1 Node or CLI raw report")
    node = report["surface"] == "node"
    if report.get("output_format" if node else "format") != "svg":
        raise ValueError("drawing audit requires SVG output from the raw collector")
    if report.get("plan_sha256") != plan_sha256:
        raise ValueError("report plan_sha256 differs from the supplied plan bytes")
    engine = report.get("engine")
    if not isinstance(engine, str) or engine not in (NODE_PRODUCTS if node else ("merman", "mmdc")):
        raise ValueError("unrecognized collector engine")
    identity = report.get("package" if node else "version_probe")
    if not isinstance(identity, dict) or (node and not isinstance(identity.get("name"), str)):
        raise ValueError("missing collector product identity")
    samples = report.get("samples")
    if type(samples) is not int or samples < 1:
        raise ValueError("samples must be a positive integer")
    product = dict(_product(report, node), surface=report["surface"], format="svg", samples=samples,
                   collector_sha256=report.get("collector_sha256"), report_path=str(report_path),
                   report_status=report.get("status"), scenarios=deepcopy(inventory))
    if not isinstance(product.get("version"), str) or not product["version"].strip():
        raise ValueError("missing collector product version")
    for key in ("error", "failed_scenario"):
        if key in report:
            product[key] = report[key]
    rows = report.get("scenarios")
    if not isinstance(rows, list):
        raise ValueError("expected collector scenarios")
    failed = report.get("failed_scenario")
    if failed is not None:
        # Historical Node reports identify one failed worker after a completed prefix.
        if (not node or not isinstance(failed, dict) or report.get("status") != "failed"
                or len(rows) >= len(scenarios) or type(failed.get("index")) is not int
                or failed["index"] != len(rows) or failed.get("id") != list(scenarios)[len(rows)]
                or failed.get("status") not in ("failed", "timeout")):
            raise ValueError("inconsistent failed_scenario in Node report")
    elif len(rows) != len(scenarios):
        raise ValueError("incomplete scenario sequence")
    elif node:
        node_worker_failures(report)
    for i, (name, scenario) in enumerate(scenarios.items()):
        cases = product["scenarios"][name]["cases"]
        if i >= len(rows):
            for case in cases:
                case["reason"] = "scenario-failed" if i == len(rows) else "not-collected"
            continue
        row = rows[i]
        batches = _batches(scenario, row, i, node, samples)
        if batches is None:
            for case in cases:
                case.update(reason=f"worker-{row['status']}", detail=row["error"])
            continue
        for case in cases:
            j = case["request_index"]
            results = [batch[j] for batch in batches]
            for result in results:
                size = result.get("output_bytes" if node else "bytes")
                digest = result.get("output_sha256" if node else "sha256")
                if (size is not None and type(size) is not int) or (digest is not None and not isinstance(digest, str)):
                    raise ValueError(f"invalid collector output identity in {name!r} at index {j}")
            output = _output(results, node, False, expected_outcome(row["requests"][j]))
            case["output"] = output
            if output["state"] != "stable":
                case["reason"] = "renderer-failed" if output["state"] == "missing" else output["state"]
                continue
            try:
                svg, case["artifact"] = _read_artifact(report_path, report, row, i, j, results)
            except FileNotFoundError as error:
                case.update(reason="missing-artifact", detail=str(error))
            except (OSError, ValueError, RuntimeError) as error:
                case.update(reason="invalid-artifact", detail=str(error))
            else:
                pending.append((case, dict(svg=svg, facts=scenario["requests"][j]["drawing_facts"])))
    return product


def _fact_key(fact):
    kind = fact.get("kind")
    if kind == "arrow":
        fields = ("edge", "from", "to")
    elif kind == "membership":
        fields = ("group", "node")
    elif kind == "label-width":
        fields = ("node", "label")
    elif kind == "sequence-message-count":
        label, count = fact.get("label"), fact.get("expected")
        if not isinstance(label, str) or not label.strip() or type(count) is not int or count < 1:
            raise ValueError("invalid drawing observer message count identity")
        if fact.get("status") in ("passed", "failed"):
            observed = fact.get("observed")
            if (type(observed) is not int or observed < 0
                    or (count == observed) != (fact["status"] == "passed")):
                raise ValueError("drawing observer verdict disagrees with the observed message count")
        return kind, label, count
    else:
        raise ValueError("invalid drawing observer fact kind")
    values = tuple(fact.get(key) for key in fields)
    if not all(isinstance(v, str) and v for v in values):
        raise ValueError("invalid drawing observer fact identity")
    return (kind, *values)


def _status(statuses):
    if not statuses:
        return "not-declared"
    if statuses == {"passed"}:
        return "passed"
    return "failed" if "failed" in statuses else "unavailable"


def observe_drawings(pending, chrome):
    if not pending:
        return None
    command = ["node", str(OBSERVER)]
    if chrome is not None:
        command.extend(["--chrome", str(chrome)])
    observer_sha256 = hashlib.sha256(OBSERVER.read_bytes()).hexdigest()
    payload = json.dumps({"jobs": [job for _, job in pending]}, ensure_ascii=False).encode("utf-8")
    result = subprocess.run(command, input=payload, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise ValueError(f"drawing observer exited with status {result.returncode}: {detail}")
    reply = json.loads(result.stdout)
    if (not isinstance(reply, dict) or not isinstance(reply.get("browser"), str) or not reply["browser"].strip()
            or reply.get("observer_sha256") != observer_sha256):
        raise ValueError("missing browser identity or mismatched drawing observer SHA-256")
    rows = reply.get("results")
    if not isinstance(rows, list) or len(rows) != len(pending):
        raise ValueError("drawing observer returned a different job count")
    for (case, _), observed in zip(pending, rows):
        ordered = [_fact_key(fact) for fact in case["facts"]]
        if not isinstance(observed, list) or len(observed) != len(ordered):
            raise ValueError(f"drawing observer returned a different fact count for {case['diagram_id']!r}")
        by_key = {}
        for fact in observed:
            if not isinstance(fact, dict) or fact.get("status") not in ("passed", "failed", "unavailable"):
                raise ValueError("invalid drawing observer fact status")
            by_key[_fact_key(fact)] = fact
        if set(ordered) != by_key.keys():
            raise ValueError(f"drawing observer returned different fact identities for {case['diagram_id']!r}")
        case["facts"] = [by_key[key] for key in ordered]
        case["status"] = _status({f["status"] for f in observed})
    return {key: reply[key] for key in ("browser", "observer_sha256", "environment") if key in reply}


def audit_drawings(
    plan_path: Path | str, reports: Iterable[tuple[Path | str, dict]], *, chrome: Path | str | None = None,
) -> dict:
    """Audit (raw report path, decoded report) pairs against a single frozen plan in one browser run.

    Artifact sample_index is zero-based in warm_samples (Node) or batches (CLI).
    Only declared occurrences appear in cases. A failed fact takes precedence over unavailable
    evidence. A scenario without declarations has status not-declared. Invalid report bindings
    or observer responses raise an error. Ordinary renderer, artifact and geometry outcomes stay
    attached to the product and occurrence that produced them.
    """
    raw = Path(plan_path).read_bytes()
    plan_sha256 = hashlib.sha256(raw).hexdigest()
    scenarios, inventory = _inventory(raw)
    planned = {key: sum(s[key] for s in inventory.values()) for key in ("occurrences", "facts")}
    pending = []
    products = [_collect(Path(path), report, scenarios, inventory, plan_sha256, pending) for path, report in reports]
    observer = observe_drawings(pending, chrome)
    for product in products:
        for scenario in product["scenarios"].values():
            scenario["status"] = _status({c["status"] for c in scenario["cases"]})
    return dict(schema_version=1, plan_sha256=plan_sha256, planned=planned, products=products, observer=observer)
