"""Inventory frozen workload inputs and the intersections reported by compare_reference."""

import hashlib
import json

if __package__:
    from .workload_svg import valid_expected_labels
else:
    from workload_svg import valid_expected_labels


def _schema_v1(data):
    version = data.get("schema_version") if isinstance(data, dict) else None
    return type(version) is int and version == 1


def _indexed(rows, key):
    if not isinstance(rows, list):
        raise ValueError(f"expected a list of rows with {key}")
    result = {}
    for row in rows:
        name = row.get(key) if isinstance(row, dict) else None
        if not isinstance(name, str) or not name or name in result:
            raise ValueError(f"missing or duplicate {key}: {name!r}")
        result[name] = row
    return result


def _source_digest(row):
    if not isinstance(row.get("source"), str):
        raise ValueError("expected a UTF-8 workload source")
    digest = hashlib.sha256(row["source"].encode("utf-8")).hexdigest()
    if row.get("source_sha256", digest) != digest:
        raise ValueError(f"source_sha256 mismatch for {row.get('diagram_id', row.get('id'))!r}")
    return digest


def _counts(requests, *, verified=False):
    labeled = [r["source_sha256"] for r in requests if r.get("expected_labels")]
    sources = {r["source_sha256"] for r in requests}
    counts = dict(requests=len(requests), distinct_sources=len(sources))
    counts["label_verified" if verified else "with_expected_labels"] = dict(
        requests=len(labeled), distinct_sources=len(set(labeled))
    )
    if verified:
        counts["mechanical_only"] = len(requests) - len(labeled)
    else:
        declared = [r for r in requests if "drawing_facts" in r]
        if declared:
            counts["with_drawing_facts"] = dict(requests=len(declared), distinct_sources=len({r["source_sha256"] for r in declared}))
    return counts


def _intersection(row, planned, *, svg):
    name = row["id"]
    observed = row["requests"]
    if not isinstance(observed, list) or len(observed) != len(planned):
        raise ValueError(f"different request count in {name!r}")

    completed, stable, issues = [], [], []
    for position, (request, result) in enumerate(zip(planned, observed), 1):
        # Older comparisons identify the source via the plan digest and ordered occurrence.
        if (
            not isinstance(result, dict)
            or result.get("fixture") != request["fixture"]
            or result.get("diagram_id") != request["diagram_id"]
            or result.get("source_sha256", request["source_sha256"]) != request["source_sha256"]
            or result.get("expected_labels", []) != request.get("expected_labels", [])
            or result.get("drawing_required", False) is not (svg and "drawing_facts" in request)
        ):
            raise ValueError(f"different request identity in {name!r} at position {position}")
        sides = (result.get("left"), result.get("right"))
        for side in sides:
            if (
                not isinstance(side, dict)
                or not isinstance(side.get("state"), str)
                or not side["state"]
                or not isinstance(side.get("statuses"), list)
                or not side["statuses"]
                or (side["state"] == "stable" and side["statuses"] != ["ok"])
            ):
                raise ValueError(f"invalid comparison outcome in {name!r} at position {position}")
        if all(side["statuses"] == ["ok"] for side in sides):
            completed.append(request)
        if all(side["state"] == "stable" for side in sides):
            stable.append(request)
        else:
            issues.append(result)

    counts = dict(
        planned_requests=len(planned), common_completed=len(completed), common_stable=len(stable)
    )
    for key, count in counts.items():
        if type(row.get(key)) is not int or row[key] != count:
            raise ValueError(f"comparison {key} disagrees with requests in {name!r}")
    return dict(
        id=name,
        planned_requests=len(planned),
        common_completed=_counts(completed, verified=True),
        common_stable=_counts(stable, verified=True),
        non_stable_requests=issues,
        **({"drawing_checks": row["drawing_checks"]} if "drawing_checks" in row else {}),
    )


def _family_coverage(families, rows):
    observed = [(family, rows[name]["requests"][0]) for name, family in families.items() if name in rows]
    result = dict(planned=len(families), reported=len(observed),
                  unreported=[family for name, family in families.items() if name not in rows])
    for side in ("left", "right"):
        gaps = [{"family": family, **request[side]} for family, request in observed if request[side]["state"] != "stable"]
        result[side] = dict(completed=sum(request[side]["statuses"] == ["ok"] for _, request in observed),
                            stable=len(observed) - len(gaps), gaps=gaps)
    result["both_stable"] = sum(request["left"]["state"] == request["right"]["state"] == "stable" for _, request in observed)
    return result


def _comparison(report, plan_digest, scenarios, families):
    if (
        not _schema_v1(report)
        or report.get("classification") != "diagnostic"
        or report.get("surface") not in ("node", "cli")
    ):
        raise ValueError("expected a schema_version 1 external workload comparison")
    if report.get("plan_sha256") != plan_digest:
        raise ValueError("comparison plan_sha256 differs from the supplied plan bytes")
    for side in ("left", "right"):
        product = report.get(side)
        if not isinstance(product, dict) or any(
            not isinstance(product.get(key), str) or not product[key] for key in ("engine", "version")
        ):
            raise ValueError(f"missing {side} product identity")
    rows = _indexed(report["scenarios"], "id")
    if not rows or list(rows) != [name for name in scenarios if name in rows]:
        raise ValueError("comparison must select known scenarios in plan order")
    fields = ("surface", "format", "left", "right", "host", "samples")
    summary = {key: report[key] for key in fields}
    summary["scenarios"] = [
        _intersection(row, scenarios[name]["requests"], svg=report["format"] == "svg") for name, row in rows.items()
    ]
    summary["unreported_scenarios"] = [name for name in scenarios if name not in rows]
    if families:
        summary["family_baselines"] = _family_coverage(families, rows)
    return summary


def summarize_coverage(raw_plan: bytes, comparisons=()) -> dict:
    """Count occurrences in plan order and deduplicate sources by their exact UTF-8 bytes."""
    plan = json.loads(raw_plan)
    if not _schema_v1(plan):
        raise ValueError("expected workload plan schema_version 1")
    plan_digest = hashlib.sha256(raw_plan).hexdigest()
    scenarios = _indexed(plan["scenarios"], "id")
    inventory, all_requests, families = [], [], {}
    for name, scenario in scenarios.items():
        requests = scenario["requests"]
        if not requests:
            raise ValueError(f"empty request sequence in {name!r}")
        missing_labels = []
        for diagram_id, request in _indexed(requests, "diagram_id").items():
            if not isinstance(request.get("fixture"), str) or not request["fixture"]:
                raise ValueError(f"expected a fixture in {name!r}")
            labels = request.get("expected_labels", [])
            if not valid_expected_labels(labels):
                raise ValueError(f"invalid expected_labels for {diagram_id!r}")
            if not labels:
                missing_labels.append(dict(fixture=request["fixture"], diagram_id=diagram_id))
            request["source_sha256"] = _source_digest(request)
        item = dict(id=name, origin=scenario.get("origin"), **_counts(requests))
        if "family" in scenario:
            family = scenario["family"]
            if (not isinstance(family, str) or not family.strip() or family != family.strip()
                    or family in families.values() or len(requests) != 1):
                raise ValueError(f"expected one uniquely identified family baseline and request in {name!r}")
            families[name] = item["family"] = family
        if missing_labels:
            item["requests_without_expected_labels"] = missing_labels
        inventory.append(item)
        all_requests.extend(requests)
    topologies = _indexed(plan.get("topologies", []), "id")
    for topology in topologies.values():
        _source_digest(topology)
    planned = dict(scenarios=len(scenarios), topology_rows=len(topologies), **_counts(all_requests))
    if families:
        planned["family_baselines"] = len(families)
    return dict(
        schema_version=1,
        plan_sha256=plan_digest,
        suite=plan.get("suite"),
        planned=planned,
        scenarios=inventory,
        comparisons=[_comparison(report, plan_digest, scenarios, families) for report in comparisons],
    )
