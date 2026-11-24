"""Project finalized Node matrix evidence onto portable counts and independent pair timings."""

from collections import Counter, defaultdict
from hashlib import sha256
import json
from math import isfinite
from pathlib import Path
import re
from statistics import median
from sys import float_info

from .coverage import _schema_v1, _source_digest, summarize_coverage
from .reference import NODE_PRODUCTS, _product as reference_product, node_worker_failures


INCOMPLETE = {"missing-evidence", "evidence-mismatch", "missing-frozen-input", "frozen-input-mismatch",
              "missing-product", "worker-failure", "collection-incomplete"}


def _read(path, plan_hash):
    data = json.loads(path.read_bytes())
    if not _schema_v1(data):
        raise ValueError("expected schema version 1 Node evidence")
    if data.get("plan_sha256") != plan_hash:
        raise ValueError("Node evidence refers to a different frozen plan")
    return data


def _sidecar(path, digest):
    try:
        return _read(path, digest), None
    except FileNotFoundError:
        return {}, "missing-evidence"
    except ValueError:
        return {}, "evidence-mismatch"


def _text(value, pattern=r"[a-zA-Z0-9][a-zA-Z0-9_.+-]{0,127}"):
    return value if isinstance(value, str) and re.fullmatch(pattern, value) else None


def _hash(value):
    return _text(value, r"[0-9a-f]{64}")


def _product(engine, record, runs, scenarios):
    product = record.get("product", {})
    # Comparisons flatten package identity. A surviving raw worker keeps it nested.
    package = product["package"] if isinstance(product.get("package"), dict) else product
    name = package.get("name", product.get("package"))
    registry = NODE_PRODUCTS[engine]
    backends = {registry["package"]: registry["backend"], **registry.get("alternatives", {})}
    files = {f["role"]: _hash(f.get("sha256")) for f in product.get("artifact_identity", {}).get("files", [])
             if f.get("role") in ("entry", "native-addon", "wasm-glue", "wasm-module")}
    failures = Counter(f["status"] for run in runs for f in run.get("worker_failures", []))
    planned = len(scenarios) * len(runs)
    missing = sum(len(scenarios) if "package" not in run or "error" in run else len(run.get("worker_failures", [])) for run in runs)
    entry = files.pop("entry", None) or _hash(package.get("entry_sha256"))
    return dict(status="unavailable" if missing == planned else "incomplete" if missing else "complete",
                package=name if name in backends else None, version=_text(package.get("version"), r"v?\d[\da-zA-Z.+-]{0,127}"),
                backend=product.get("backend") if product.get("backend") == backends.get(name) else None,
                entry_sha256=entry, transport_sha256=files,
                collector_sha256=_hash(record.get("collector_sha256")),
                workers=dict(planned=planned, missing=missing, failed=failures["failed"], timeout=failures["timeout"]))


def _positive(value):
    return type(value) in (int, float) and isfinite(value) and value > 0


def _latency(peer, title, scenario, row, cases):
    """Join each finalized independent observation to the same round's complete batch."""
    pairs, reasons = [], set()
    count = len(cases)
    samples = row.get("rounds", [])
    by_round = {s["round"]: s for s in samples if type(s.get("round")) is int}
    numbers = Counter(s["round"] for s in samples if type(s.get("round")) is int)
    declared = row.get("eligible_pairs", 0)
    expected_status = "complete" if declared == count else "partial" if declared else "unavailable"
    consistent = (row.get("status") == expected_status and type(declared) is int and 0 <= declared <= count
                  and len(samples) <= count and all(1 <= n <= count for n in by_round)
                  and declared == sum("right_over_left" in s and "withheld_reason" not in s for s in samples))
    for index, case in enumerate(cases, 1):
        sample = by_round.get(index, {})
        batch, reason = case["batch"], case["reason"]
        if not reason:
            if not sample:
                reason = "missing-evidence"
            elif not consistent or numbers[index] != 1:
                reason = "evidence-mismatch"
            elif "withheld_reason" in sample:
                reason = "collector-withheld"
            elif any(batch[k] != sample.get(k) for k in ("left", "right", "right_over_left")):
                reason = "evidence-mismatch"
        if reason:
            case["reason"] = reason
            reasons.add(reason)
        else:
            pairs.append(dict(round=index, merman_ms=batch["left"], peer_ms=batch["right"], ratio=batch["right_over_left"]))

    merman_ms = peer_ms = ratio = None
    if len(pairs) == count:
        merman_ms = median(p["merman_ms"] for p in pairs)
        peer_ms = median(p["peer_ms"] for p in pairs)
        ratios = [p["ratio"] for p in pairs]
        ratio = dict(min=min(ratios), median=median(ratios), max=max(ratios))
    return dict(peer=peer, scenario=title, requests_per_batch=scenario["requests"], distinct_sources=scenario["distinct_sources"],
                visual_checks=dict(planned=scenario.get("with_drawing_facts", {}).get("requests", 0),
                                   passed_pairs=sum(case["visual_passed"] for case in cases)),
                status="complete" if len(pairs) == count else "partial" if pairs else "unavailable",
                planned_pairs=count, eligible_pairs=len(pairs), merman_ms=merman_ms, peer_ms=peer_ms, ratio=ratio,
                pairs=pairs, reasons=sorted(reasons))


def _coverage(peer, scenarios, observations):
    """Count scenario/round occurrences. Completion and stability count request occurrences."""
    counts = dict(planned_scenarios=len(scenarios) * len(observations), reported_scenarios=0,
                  planned_requests=sum(s["requests"] for s in scenarios) * len(observations), reported_requests=0,
                  common_completed=0, common_stable=0, common_expected_rejections=0,
                  eligible_scenarios=0, failed_scenarios=0, unchecked_scenarios=0, withheld_scenarios=0)
    drawing = dict(planned=0, passed=0, failed=0, unchecked=0)
    families, reasons = {}, set()
    for cases in observations:
        for position, scenario in enumerate(scenarios, 1):
            case = cases[peer, scenario["id"]]
            row, reason = case["observation"], case["reason"]
            issues = row.get("non_stable_requests", [])
            counts["reported_scenarios"] += bool(row)
            counts["reported_requests"] += row.get("planned_requests", 0)
            for field in ("common_completed", "common_stable"):
                counts[field] += row.get(field, {}).get("requests", 0)
            counts["common_expected_rejections"] += row.get("common_expected_rejections", 0)

            if scenario.get("with_drawing_facts"):
                checks = row.get("drawing_checks", {})
                sides = (checks.get("left"), checks.get("right"))
                verdict = "failed" if "failed" in sides else "passed" if sides == ("passed", "passed") else "unchecked"
                drawing["planned"] += 1
                drawing[verdict] += 1
            if not reason:
                counts["eligible_scenarios"] += 1
            elif reason in ("worker-failure", "output-not-stable", "drawing-failed"):
                counts["failed_scenarios"] += 1
            elif reason in INCOMPLETE or reason == "drawing-unchecked":
                counts["unchecked_scenarios"] += 1
            else:
                counts["withheld_scenarios"] += 1
            if reason:
                reasons.add(reason)

            if "family" in scenario:
                family = _text(scenario["family"]) or f"family #{position}"
                entry = families.setdefault(family, dict(planned=0, reported=0, merman_stable=0, peer_stable=0, both_stable=0))
                entry["planned"] += 1
                entry["reported"] += bool(row)
                both = row.get("common_stable", {}).get("requests", 0)
                entry["both_stable"] += both
                # Each family baseline is one request, including when a worker was unmeasured.
                for side, field in (("left", "merman_stable"), ("right", "peer_stable")):
                    entry[field] += both or sum(issue[side]["state"] == "stable" for issue in issues)
    return dict(**counts, drawing_checks=drawing, reasons=sorted(reasons),
                families={key: sum(f[key] for f in families.values())
                          for key in ("planned", "reported", "merman_stable", "peer_stable", "both_stable")},
                family_gaps=[dict(family=name, **f) for name, f in families.items() if f["both_stable"] < f["planned"]])


def _round(directory, rounds, index, digest):
    if not rounds:
        try:
            return directory, _read(directory / "coverage.json", digest), None, None
        except FileNotFoundError:
            raise ValueError("missing finalized Node matrix coverage") from None
    relative = f"rounds/{index:03d}/coverage.json"
    folder = (directory / relative).parent
    history = [r for r in rounds.get("rounds", []) if type(r.get("round")) is int and r["round"] == index]
    if len(history) != 1 or history[0].get("coverage") != relative:
        return folder, {}, None, "evidence-mismatch" if history else "missing-evidence"
    coverage, issue = _sidecar(folder / "coverage.json", digest)
    if issue or coverage.get("collection", {}).get("status") != history[0].get("status"):
        return folder, {}, None, issue or "evidence-mismatch"
    order = history[0].get("order")
    return (folder, coverage, order, None) if isinstance(order, list) else (folder, {}, None, "missing-evidence")


def _frozen_issue(product, frozen):
    observed = (product.get("node_project") or {}).get("digests", {})
    for name, fingerprint in frozen.items():
        if name == "package-lock.json" or name in observed:
            if fingerprint is None:
                return "missing-frozen-input"
            if name in observed and observed[name] != fingerprint:
                return "frozen-input-mismatch"
    return None


def _comparison(folder, peer, summary, digest, frozen, records, rounds, runs):
    data, issue = _sidecar(folder / f"merman-vs-{peer}.json", digest)
    if issue:
        return {}, issue
    samples = rounds.get("warm_batches_per_worker") if rounds else summary.get("samples")
    if (any(report.get("surface") != "node" or report.get("format") != "svg" for report in (summary, data))
            or data.get("classification") != "diagnostic"
            or type(samples) is not int or samples < 1 or data.get("samples") != samples):
        return {}, "evidence-mismatch"
    for side, engine in (("left", "merman"), ("right", peer)):
        product = data[side]
        if (product.get("engine") != engine or product != summary[side] or data["host"] != summary["host"]
                or any(product.get(k) != runs[engine].get(k) for k in ("package", "version"))):
            return {}, "evidence-mismatch"
        identity = dict(product=product, host=data["host"], collector_sha256=data["collector_sha256"][side])
        records.setdefault(engine, identity)
        if (product.get("artifact_identity") or {}).get("status") != "recorded":
            return {}, "collector-withheld"
        if rounds and rounds.get("reference_identities", {}).get(engine) != identity:
            return {}, "evidence-mismatch"
        issue = _frozen_issue(product, frozen)
        if issue:
            return {}, issue
    collectors = data["collector_sha256"]
    if data.get("ratio_withheld_reason") or not _hash(collectors.get("left")) or collectors["left"] != collectors.get("right"):
        return {}, "collector-withheld"
    return {r["id"]: r for r in data["scenarios"]}, None


def _case(scenario, observation, compared, issue, failures, audit_issue):
    """Use the published comparison's eligibility; distinguish its structured failure outcomes."""
    if scenario["id"] in failures or observation.get("worker_failures"):
        issue = issue or "worker-failure"
        if not observation.get("worker_failures"):
            observation = {}
    if audit_issue and scenario.get("with_drawing_facts") and observation:
        observation = dict(observation, drawing_checks={})
        issue = issue or audit_issue
    batch = compared.get("complete_batch_ms") or {}
    if not issue:
        checks = observation.get("drawing_checks", {})
        if not observation or not compared:
            issue = "missing-evidence"
        elif compared.get("planned_requests") != scenario["requests"] or observation.get("planned_requests") != scenario["requests"]:
            issue = "evidence-mismatch"
            observation = {}
        elif observation.get("non_stable_requests") or compared.get("worker_failures"):
            issue = "output-not-stable"
        elif "failed" in checks.values():
            issue = "drawing-failed"
        elif scenario.get("with_drawing_facts") and (checks.get("left"), checks.get("right")) != ("passed", "passed"):
            issue = "drawing-unchecked"
        elif observation.get("common_expected_rejections"):
            issue = "expected-rejection"
        elif compared.get("common_stable") != scenario["requests"] or observation["common_stable"]["requests"] != scenario["requests"]:
            issue = "output-not-stable"
        elif batch.get("right_over_left") is None:
            issue = "collector-withheld"
        elif not all(_positive(batch.get(k)) for k in ("left", "right", "right_over_left")):
            issue = "evidence-mismatch"
    return dict(observation=observation, batch=batch, reason=issue)


def _worker_report(folder, engine, run, rounds, frozen, plan):
    """Read the scheduled product's local sidecar against its finalized round identity."""
    filename = f"{engine}.json"
    reference = rounds.get("reference_identities", {}).get(engine, {})
    samples = rounds.get("warm_batches_per_worker")
    if (run.get("report") != filename or "error" in run or not _hash(reference.get("collector_sha256"))
            or type(samples) is not int or samples < 1):
        return {}
    try:
        raw = _read(folder / filename, rounds["plan_sha256"])
        host = raw.get("host")
        if (raw.get("engine") != engine or raw.get("surface") != "node" or raw.get("output_format") != "svg"
                or raw.get("process_lifecycle") != "fresh-per-scenario" or type(raw.get("samples")) is not int
                or raw["samples"] != samples or not isinstance(host, dict) or host != reference.get("host")
                or not all(host.get(k) for k in ("hostname", "platform", "arch"))
                or raw.get("collector_sha256") != reference["collector_sha256"]):
            return {}
        product = reference_product(raw, node=True)
        if (product != reference.get("product") or product["artifact_identity"]["status"] != "recorded"
                or any(product[k] != run.get(k) for k in ("package", "version")) or _frozen_issue(product, frozen)
                or node_worker_failures(raw) != run.get("worker_failures", [])
                or [row["id"] for row in raw["scenarios"]] != list(plan)):
            return {}
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return {}
    return raw


def _matches_requests(observed, planned, *, raw=False):
    return isinstance(observed, list) and len(observed) == len(planned) and all(
        isinstance(got, dict) and all(got.get(k) == want[k] for k in ("fixture", "diagram_id", "source_sha256"))
        and got.get("expected_labels", []) == want.get("expected_labels", [])
        and got.get("expected_outcome", "draw") == want.get("expected_outcome", "draw")
        and got.get("drawing_required", raw and "drawing_facts" in want) is ("drawing_facts" in want)
        for got, want in zip(observed, planned))


def _measured_worker(row, index, requests, raw):
    warm = row.get("warm_samples")
    return ("status" not in row and type(row.get("index")) is int and row["index"] == index
            and row.get("artifact_identity", raw["artifact_identity"]) == raw["artifact_identity"]
            and _matches_requests(row.get("requests"), requests, raw=True) and isinstance(warm, list) and len(warm) == raw["samples"]
            and all(isinstance(batch, dict) and isinstance(batch.get("results"), list)
                    and len(batch["results"]) == len(requests)
                    and all(isinstance(r, dict) and isinstance(r.get("status"), str) and r["status"] for r in batch["results"])
                    for batch in [row.get("cold_batch"), *warm]))


def _duration(value):
    return value if type(value) in (int, float) and 0 <= value <= float_info.max else None


def _worker_measurements(row, init_scope):
    import_scoped = init_scope == "package-import-and-engine-creation"
    init = _duration(row.get("init_ms")) if import_scoped else None
    cold = row["cold_batch"]
    first = cold["results"][0]
    outcome = row["requests"][0].get("expected_outcome", "draw")
    elapsed = through = None
    if (first.get("status") == "ok" and outcome != "reject" and first.get("expected_outcome", outcome) == outcome
            and first.get("output_class", "draw" if outcome == "draw" else None) == "draw"
            and type(first.get("output_bytes")) is int and first["output_bytes"] > 0 and _hash(first.get("output_sha256"))):
        call = _duration(first.get("elapsed_ms"))
        if call == _duration(row.get("first_render_ms")):
            elapsed = call
        if import_scoped:
            through = _duration(cold.get("init_through_first_svg_ms"))
            if through is not None and any(v is not None and v > through for v in (init, call)):
                through = None
    rss = row.get("peak_rss_kib")
    return dict(initialize_ms=init, first_svg_ms=elapsed, init_through_first_svg_ms=through,
                peak_rss_mib=rss / 1024 if type(rss) is int and 0 < rss <= float_info.max else None)


def _drawing_rows(audit, reference, samples):
    product = reference["product"]
    if not isinstance(audit.get("products"), list):
        return {}
    matching = [p for p in audit["products"] if isinstance(p, dict) and p.get("engine") == product["engine"]]
    if len(matching) == 1:
        proof = matching[0]
        if (all(proof.get(k) == v for k, v in product.items()) and proof.get("collector_sha256") == reference["collector_sha256"]
                and proof.get("surface") == "node" and proof.get("format") == "svg"
                and type(proof.get("samples")) is int and proof["samples"] == samples
                and isinstance(proof.get("scenarios"), dict)):
            return proof["scenarios"]
    return {}


def _passed_facts(declared, facts):
    """The auditor stores verdicts in declaration order within each request."""
    if not isinstance(declared, dict) or not isinstance(facts, list) or not facts:
        return False
    if declared.keys() - {"arrows", "groups", "label_widths", "sequence_messages"}:
        return False
    try:
        expected = [dict(kind="arrow", **a) for a in declared.get("arrows", [])]
        expected.extend(dict(kind="membership", group=g, node=n) for g, nodes in declared.get("groups", {}).items() for n in nodes)
        expected.extend(dict(kind="label-width", node=n, label=label) for n, label in declared.get("label_widths", {}).items())
        expected.extend(dict(kind="sequence-message-count", label=label, expected=n, observed=n)
                        for label, n in declared.get("sequence_messages", {}).items())
    except (TypeError, AttributeError):
        return False
    return len(expected) == len(facts) and all(
        isinstance(got, dict) and got.get("status") == "passed"
        and all(k in got and got[k] == v and type(got[k]) is type(v) for k, v in want.items())
        for want, got in zip(expected, facts))


def _passed_drawing(proof, row, planned):
    """Count the auditor's verdict once, bound to each declared occurrence and retained warm output."""
    if not isinstance(proof, dict) or proof.get("status") != "passed":
        return False
    positions = [i for i, r in enumerate(planned) if "drawing_facts" in r]
    cases = proof.get("cases")
    if (not positions or type(proof.get("occurrences")) is not int or proof["occurrences"] != len(positions) or not isinstance(cases, list)
            or len(cases) != len(positions)):
        return False
    warm = row["warm_samples"]
    fact_count = 0
    for index, case in zip(positions, cases):
        if (not isinstance(case, dict) or case.get("status") != "passed" or type(case.get("request_index")) is not int
                or case["request_index"] != index or any(case.get(k) != planned[index][k] for k in ("fixture", "diagram_id", "source_sha256"))):
            return False
        artifact, facts = case.get("artifact"), case.get("facts")
        result = warm[-1]["results"][index]
        if (not isinstance(artifact, dict) or type(artifact.get("sample_index")) is not int or artifact["sample_index"] != len(warm) - 1
                or result.get("status") != "ok" or type(result.get("output_bytes")) is not int
                or not _hash(artifact.get("sha256")) or artifact["sha256"] != result.get("output_sha256")
                or type(artifact.get("bytes")) is not int or artifact["bytes"] <= 0 or artifact["bytes"] != result.get("output_bytes")
                or not _passed_facts(planned[index]["drawing_facts"], facts)):
            return False
        fact_count += len(facts)
    return type(proof.get("facts")) is int and fact_count == proof["facts"]


def _round_workers(folder, order, collection, rounds, frozen, plan, audit, measurements):
    """Visit each fresh worker once even when Merman is paired with multiple peers."""
    passed = set()
    for engine, run in collection.get("runs", {}).items():
        if (engine not in NODE_PRODUCTS or order.count(engine) != 1
                or (engine in collection.get("errors", {}) and not run.get("worker_failures"))):
            continue
        raw = _worker_report(folder, engine, run, rounds, frozen, plan)
        if not raw:
            continue
        drawings = _drawing_rows(audit, rounds["reference_identities"][engine], raw["samples"])
        for index, row in enumerate(raw["scenarios"]):
            name = row["id"]
            if _measured_worker(row, index, plan[name], raw):
                measurements[name, engine].append(_worker_measurements(row, raw.get("init_scope")))
                if _passed_drawing(drawings.get(name), row, plan[name]):
                    passed.add((name, engine))
    return passed


def _startup(title, engine, workers, count):
    result = dict(scenario=title, engine=engine, workers=len(workers), planned_workers=count)
    for field in ("initialize_ms", "first_svg_ms", "init_through_first_svg_ms", "peak_rss_mib"):
        values = [w[field] for w in workers]
        result[field] = _duration(median(values)) if len(values) == count and None not in values else None
    return result


def _load(directory, *, independent):
    try:
        raw_plan = (directory / "plan.json").read_bytes()
        inventory = summarize_coverage(raw_plan)
    except (OSError, ValueError, KeyError, TypeError):
        raise ValueError("missing or invalid frozen Node plan") from None
    digest, scenarios = inventory["plan_sha256"], inventory["scenarios"]
    try:
        rounds = _read(directory / "process-rounds.json", digest) if independent else {}
    except FileNotFoundError:
        raise ValueError("missing finalized Node process rounds") from None
    if "variants" in rounds:
        raise ValueError("Node variant bundles are diagnostic. See their bundled summary.md.")
    count = rounds.get("process_rounds") if independent else 1
    if independent and (rounds.get("left") != "merman" or rounds.get("classification") != "diagnostic"
                        or type(count) is not int or count < 2 or count % 2):
        raise ValueError("expected a balanced Merman Node process-round bundle")
    frames = [_round(directory, rounds, index, digest) for index in range(1, count + 1)]
    if any("variants" in coverage for _, coverage, _, _ in frames):
        raise ValueError("Node variant bundles are diagnostic. See their bundled summary.md.")
    selected = {"merman", *rounds.get("comparisons", {}), *rounds.get("reference_identities", {})}
    for r in rounds.get("rounds", []):
        selected.update(r.get("order", []))
    for _, coverage, _, _ in frames:
        collection = coverage.get("collection", {})
        selected.update(collection.get("runs", {}))
        selected.update(name for name in NODE_PRODUCTS if name in collection.get("errors", {})
                        or f"merman-vs-{name}.json" in collection.get("errors", {}))
        selected.update(c["right"]["engine"] for c in coverage.get("comparisons", []))
    peers = [name for name in NODE_PRODUCTS if name in selected and name != "merman"]
    if not peers:
        raise ValueError("expected at least one scheduled Node peer")
    frozen = {}
    for name in ("package.json", "package-lock.json"):
        path = directory / "consumer" / name
        frozen[name] = sha256(path.read_bytes()).hexdigest() if path.is_file() else None

    records = dict(rounds.get("reference_identities", {}))
    warm_batches = rounds.get("warm_batches_per_worker")
    plan = {}
    if independent:
        for s in json.loads(raw_plan)["scenarios"]:
            plan[s["id"]] = s["requests"]
            for request in s["requests"]:
                if "source_sha256" not in request:
                    request["source_sha256"] = _source_digest(request)
    measurements = defaultdict(list)
    reference_observer = None
    observations = []
    status = rounds.get("collection_status") if independent else "complete"
    incomplete = status != "complete"
    unexplained = status not in ("complete", "incomplete") or (
        incomplete and all(c.get("collection", {}).get("status") == "complete" for _, c, _, _ in frames))
    for folder, coverage, order, issue in frames:
        collection = coverage.get("collection", {})
        runs, errors = collection.get("runs", {}), collection.get("errors", {})
        incomplete |= bool(errors) or collection.get("status") != "complete"
        known_failure = errors or selected - runs.keys() or any("error" in r or r.get("worker_failures") for r in runs.values())
        if collection.get("status") not in ("complete", "incomplete") or (collection["status"] == "incomplete" and not known_failure):
            issue = issue or "collection-incomplete"
        if unexplained or errors.keys() - {"drawing-audit.json", *selected, *(f"merman-vs-{p}.json" for p in peers)}:
            issue = issue or "collection-incomplete"
        audit_issue = "drawing-unchecked" if "drawing-audit.json" in errors else None
        audit = {}
        if "drawing_audit" in coverage and not audit_issue:
            if coverage["drawing_audit"] == "drawing-audit.json":
                audit, audit_issue = _sidecar(folder / "drawing-audit.json", digest)
            else:
                audit_issue = "evidence-mismatch"
        passed_drawings = set()
        if independent and not issue:
            observer = audit.get("observer")
            if (isinstance(observer, dict) and isinstance(observer.get("browser"), str) and observer["browser"].strip()
                    and _hash(observer.get("observer_sha256"))):
                reference_observer = reference_observer or observer
            if not reference_observer or observer != reference_observer:
                audit = {}
            passed_drawings = _round_workers(folder, order, collection, rounds, frozen, plan, audit, measurements)
        summaries = {s["right"]["engine"]: s for s in coverage.get("comparisons", [])}
        cases = {}
        for peer in peers:
            problem = issue
            failures = set()
            for engine in ("merman", peer):
                run = runs.get(engine, {})
                if "package" not in run or "error" in run or (order is not None and order.count(engine) != 1):
                    problem = problem or "missing-product"
                if engine in errors and not run.get("worker_failures"):
                    problem = problem or "collection-incomplete"
                failures.update(f["id"] for f in run.get("worker_failures", []))
            if f"merman-vs-{peer}.json" in errors:
                problem = problem or "collection-incomplete"
            summary = summaries.get(peer, {}) if not problem else {}
            compared = {}
            if summary:
                warm_batches = warm_batches or summary["samples"]
                compared, problem = _comparison(folder, peer, summary, digest, frozen, records, rounds, runs)
            else:
                problem = problem or "missing-evidence"
            rows = {r["id"]: r for r in summary.get("scenarios", [])} if problem != "evidence-mismatch" else {}
            for s in scenarios:
                name = s["id"]
                comparison = compared.get(name, {})
                case = _case(s, rows.get(name, {}), comparison, problem, failures, audit_issue)
                if independent:
                    checks = {"left": "passed", "right": "passed"}
                    case["visual_passed"] = (
                        (name, "merman") in passed_drawings and (name, peer) in passed_drawings
                        and case["observation"].get("drawing_checks") == checks == comparison.get("drawing_checks")
                        and comparison.get("planned_requests") == s["requests"]
                        and not comparison.get("worker_failures") and not case["observation"].get("worker_failures")
                        and _matches_requests(comparison.get("requests"), plan[name]))
                cases[peer, name] = case
        observations.append(cases)

    products = {}
    for engine in ("merman", *peers):
        runs = [c.get("collection", {}).get("runs", {}).get(engine, {}) for _, c, _, _ in frames]
        if engine not in records:
            for (folder, _, _, _), run in zip(frames, runs):
                if "package" in run and "error" not in run:
                    raw, _ = _sidecar(folder / f"{engine}.json", digest)
                    if raw.get("engine") != engine:
                        raw = {}
                    records[engine] = dict(product=raw or run, host=raw.get("host", {}), collector_sha256=raw.get("collector_sha256"))
                    warm_batches = warm_batches or raw.get("samples")
                    break
        products[engine] = _product(engine, records.get(engine, {}), runs, scenarios)
    latency = []
    startup = []
    if independent:
        for peer in peers:
            for position, scenario in enumerate(scenarios, 1):
                name = scenario["id"]
                row = rounds.get("comparisons", {}).get(peer, {}).get(name, {})
                latency.append(_latency(peer, _text(name) or f"scenario #{position}", scenario, row,
                                        [cases[peer, name] for cases in observations]))
        startup = [_startup(_text(s["id"]) or f"scenario #{i}", engine, measurements[s["id"], engine], count)
                   for i, s in enumerate(scenarios, 1) for engine in ("merman", *peers)]
    incomplete |= any(case["reason"] in INCOMPLETE for cases in observations for case in cases.values())
    host = next((records[e]["host"] for e in products if records.get(e, {}).get("host")), {})
    planned = inventory["planned"]
    return dict(kind="node-rounds" if independent else "node-matrix", suite=_text(inventory["suite"]),
                status="incomplete" if incomplete else "complete",
                planned=dict(scenarios=planned["scenarios"], requests=planned["requests"], distinct_sources=planned["distinct_sources"],
                             families=planned.get("family_baselines", 0)),
                host=dict(os=_text(host.get("platform")), arch=_text(host.get("arch")),
                          cpu=_text(host.get("cpu"), r"[A-Za-z0-9 ().,@_+-]{1,160}"), runtime=_text(host.get("node"))),
                hashes=dict(plan_sha256=digest, lock_sha256=frozen["package-lock.json"]),
                products=products, coverage={p: _coverage(p, scenarios, observations) for p in peers},
                process_rounds=count, warm_batches_per_worker=warm_batches if type(warm_batches) is int else None, latency=latency,
                **({"startup": startup} if independent else {}))


def load_matrix(path: Path) -> dict:
    """Read a bundle directory or coverage.json. One-pass warm batches contribute coverage only."""
    return _load(path.parent if path.name == "coverage.json" else path, independent=False)


def load_rounds(path: Path) -> dict:
    """Read a bundle directory or process-rounds.json. Coverage denominators include each round.

    Each latency pair is one fresh worker's warm-batch median per product. A complete series
    releases the two medians in milliseconds and the range and median of within-pair ratios.
    Partial series retain only their individually supported pairs. Reasons are public codes.
    Visual coverage counts declared request positions per batch and independently audited rounds.

    Startup counts identified, measured fresh workers per scenario and engine. Each timing is a
    median requiring a value from every scheduled worker. Initialization times package import and
    Merman engine creation. First SVG times only the cold batch's first call when it returned a
    drawable SVG. Import through first SVG uses the recorded interval from before import until
    that call returns, including intervening setup. Both SVG timers end before validation.
    Peak RSS is the worker's own high-water mark after cold/warm batches and output checks and
    before artifact hashing, converted from KiB to MiB. Older collectors omit these latter two
    observations. Process launch is outside all three timers.
    """
    return _load(path.parent if path.name == "process-rounds.json" else path, independent=True)
