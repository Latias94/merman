"""Select a measured weekly ancestor and qualify the next paired workload artifact."""

import argparse
from collections import Counter
from hashlib import sha256
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


WORKFLOW = ".github/workflows/performance.yml"
SENTINEL = "mixed-families"
ARTIFACT = re.compile(r"perf-public-workloads-weekly-([1-9][0-9]*)-(v[0-9]+-[0-9a-f]{64})")


def sentinel_bytes(plan):
    rows = [row for row in plan["scenarios"] if row["id"] == SENTINEL]
    if len(rows) != 1 or not rows[0]["requests"]:
        raise ValueError(f"expected one nonempty {SENTINEL} scenario")
    # Match the portable one-scenario plan emitted by bench.py plan --scenario.
    frozen = {"schema_version": plan["schema_version"], "suite": "quick", "scenarios": rows, "topologies": []}
    return (json.dumps(frozen, ensure_ascii=False, indent=2, allow_nan=False) + "\n").encode()


def protocol(plan, env):
    sources = env["WORKLOAD_SOURCES"]
    if not re.fullmatch(r"[0-9a-f]{64}", sources):
        raise ValueError("missing weekly measurement source fingerprint")
    recipe = dict(sentinel_sha256=sha256(sentinel_bytes(plan)).hexdigest(), sources_sha256=sources,
                  recipe_sha256=sha256(env["WORKLOAD_RECIPE"].encode()).hexdigest(),
                  runner=env["WORKLOAD_RUNNER"], arch=env["RUNNER_ARCH"],
                  toolchain=env["WORKLOAD_TOOLCHAIN"])
    fingerprint = sha256(json.dumps(recipe, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {**recipe, "protocol": "v1-" + fingerprint}


def github_get(path):
    request = Request(f"{os.environ['GITHUB_API_URL']}/repos/{os.environ['GITHUB_REPOSITORY']}{path}",
                      headers={"Accept": "application/vnd.github+json", "User-Agent": "merman-weekly-workload",
                               "X-GitHub-Api-Version": "2022-11-28"})
    request.add_unredirected_header("Authorization", "Bearer " + os.environ["GH_TOKEN"])
    with urlopen(request, timeout=30) as response:
        return json.load(response)


def pages(get, path, field, **query):
    page = 1
    while True:
        rows = get(path + "?" + urlencode({**query, "per_page": 100, "page": page}))[field]
        yield from rows
        if len(rows) < 100:
            return
        page += 1


def is_ancestor(base, head):
    # A force-pushed commit may already be absent from the full branch checkout.
    if subprocess.run(["git", "cat-file", "-e", base + "^{commit}"], stderr=subprocess.DEVNULL).returncode:
        return False
    check = subprocess.run(["git", "merge-base", "--is-ancestor", base, head])
    if check.returncode == 1:
        return False
    check.check_returncode()
    return True


def available(get, path):
    try:
        return get(path)
    except HTTPError as error:
        if error.code in (404, 410):
            return None
        raise


def measured_point(get, run, skipped):
    path = f"/actions/runs/{run['id']}"
    artifacts = list(pages(get, path + "/artifacts", "artifacts"))
    receipts = {}
    for artifact in artifacts:
        match = ARTIFACT.fullmatch(artifact["name"])
        if match and int(match[1]) <= run["run_attempt"]:
            receipts.setdefault(int(match[1]), []).append((match[2], artifact))
    if not receipts:
        partial = f"perf-public-workloads-incomplete-{run['run_attempt']}"
        reason = "incomplete-measurement" if any(a["name"] == partial for a in artifacts) else "no-receipt"
        skipped[reason] += 1
    for attempt in sorted(receipts, reverse=True):
        if len(receipts[attempt]) != 1:
            skipped["ambiguous-receipt"] += 1
            continue
        protocol_id, listed = receipts[attempt][0]
        if listed["expired"]:
            skipped["artifact-unavailable"] += 1
            continue
        # Run-scoped artifact listings include artifacts from earlier attempts.
        producer = available(get, f"{path}/attempts/{attempt}")
        identity = ("id", "run_number", "workflow_id", "path", "event", "head_sha", "head_branch")
        if (not producer or any(producer[k] != run[k] for k in identity)
                or producer["status"] != "completed" or producer["run_attempt"] != attempt):
            skipped["attempt-mismatch"] += 1
            continue
        artifact = available(get, f"/actions/artifacts/{listed['id']}")
        if (not artifact or artifact["expired"] or not artifact["size_in_bytes"]
                or any(artifact[k] != listed[k] for k in ("id", "name", "digest"))
                or not re.fullmatch(r"sha256:[0-9a-f]{64}", artifact.get("digest") or "")):
            skipped["artifact-unavailable"] += 1
            continue
        origin = artifact.get("workflow_run")
        if origin and any(origin[k] != run[k] for k in ("id", "head_sha", "head_branch")):
            skipped["artifact-origin-mismatch"] += 1
            continue
        return dict(run_id=run["id"], run_attempt=attempt, head_sha=run["head_sha"],
                    artifact_id=artifact["id"], digest=artifact["digest"], protocol=protocol_id)
    return None


def select(env, recipe, get=github_get, ancestor=is_ancestor):
    repo, branch, head = (env[key] for key in ("GITHUB_REPOSITORY", "GITHUB_REF_NAME", "GITHUB_SHA"))
    current_number, current_id = int(env["GITHUB_RUN_NUMBER"]), int(env["GITHUB_RUN_ID"])
    skipped = Counter()
    interval = dict(repository=repo, branch=branch, head_sha=head, base_sha=head, kind="bootstrap",
                    previous=None, skipped=skipped, **recipe)
    runs = pages(get, "/actions/workflows/performance.yml/runs", "workflow_runs",
                 event="schedule", branch=branch, status="completed")
    # run_number orders occurrences even when an old check is polled or rerun much later.
    candidates = sorted((r for r in runs if r["run_number"] < current_number and r["id"] != current_id),
                        key=lambda r: r["run_number"], reverse=True)
    for run in candidates:
        sha = run["head_sha"]
        if (run["path"] != WORKFLOW or run["event"] != "schedule" or run["status"] != "completed"
                or run["head_branch"] != branch or run["repository"]["full_name"] != repo
                or (run.get("head_repository") or {}).get("full_name") != repo
                or not re.fullmatch(r"[0-9a-f]{40}", sha)):
            continue
        try:
            point = measured_point(get, run, skipped)
        except HTTPError as error:
            if error.code not in (404, 410):
                raise
            skipped["history-unavailable"] += 1
            continue
        if not point:
            continue
        if not ancestor(sha, head):
            skipped["non-ancestor"] += 1
            continue
        interval["previous"] = point
        if point["protocol"] != recipe["protocol"]:
            interval["kind"] = "protocol-change"
        else:
            interval.update(base_sha=sha, kind="same-head" if sha == head else "interval")
        return interval
    return interval


def memory_outcomes(row, requests, sample_count):
    outputs = row["outputs"]
    if (len(outputs) != len(requests) or row["expected_outcomes"] != [r.get("expected_outcome", "draw") for r in requests]):
        raise ValueError("incomplete request sequence or different expected outcomes")
    statuses = Counter()
    for output, request in zip(outputs, requests):
        status = output.get("status", "ok")
        if status == "ok":
            if (not re.fullmatch(r"[0-9a-f]{64}", output.get("sha256", ""))
                    or output.get("bytes", 0) <= 0 or output.get("observed_size", 0) <= 0):
                raise ValueError("missing verified output identity")
            if request.get("expected_outcome") == "reject":
                raise ValueError("unclassified output for a request expected to be rejected")
        elif status not in ("expected-rejection", "render-error", "invalid-svg", "empty-svg", "error-svg",
                            "missing-labels", "unexpected-success") or not output.get("message"):
            raise ValueError("missing or unrecognized request outcome")
        elif status in ("expected-rejection", "unexpected-success") and request.get("expected_outcome") != "reject":
            raise ValueError("rejection classification belongs to a different request")
        statuses[status] += 1
    if row["completed"] != statuses["ok"] or row.get("expected_rejections", 0) != statuses["expected-rejection"]:
        raise ValueError("request counts disagree with observed outputs")
    if statuses["ok"] == len(requests):
        if (row.get("status") or len(row["samples"]) != sample_count
                or any(type(s.get("allocated_bytes")) is not int or s["allocated_bytes"] < 0 for s in row["samples"])):
            raise ValueError("incomplete or inconsistent allocation samples")
    else:
        expected = "expected-rejection" if set(statuses) <= {"ok", "expected-rejection"} else "failed"
        if row.get("status") != expected or row["samples"] != []:
            raise ValueError("inconsistent outcome for an unsampled batch")
    return statuses


def work_outcomes(row):
    outcomes = {}
    for prefix, probes in (("layout", row), ("svg", row["svg"])):
        for budget in ("default", "high"):
            observation = probes[budget]
            status = observation["status"]
            if status == "accepted":
                valid = bool(re.fullmatch(r"[0-9a-f]{64}", observation.get("sha256", ""))) and observation.get("bytes", 0) > 0
            elif status == "source_max_edges_rejected":
                valid = bool(observation.get("code") and observation.get("message"))
            elif status == "layout_work_rejected":
                valid = (observation.get("id") == "max_layout_work_units" and observation.get("phase")
                         and observation["actual"] > observation["maximum"] == row[budget + "_budget"])
            else:
                valid = False
            if not valid:
                raise ValueError(f"missing or unrecognized {budget}-budget {prefix} observation")
            outcomes[f"{prefix}_{budget}"] = status
    if outcomes["layout_high"] == "accepted" and not (row["boundary"] or {}).get("minimum_passing_budget"):
        raise ValueError("missing layout-work boundary")
    return outcomes


def qualify(directory, interval):
    """Qualify both measured witnesses and account for every public coverage observation."""
    from .workloads import compare

    raw_plan = (directory / "plan.json").read_bytes()
    plan_sha = sha256(raw_plan).hexdigest()
    plan = json.loads(raw_plan)
    frozen = sentinel_bytes(plan)
    if plan["suite"] != "full" or sha256(frozen).hexdigest() != interval["sentinel_sha256"]:
        raise ValueError("weekly sentinel or suite differs from the selected protocol")
    (directory / "weekly-sentinel.json").write_bytes(frozen)
    requests = {case["id"]: case["requests"] for case in plan["scenarios"]}
    selections = {"memory": list(requests), "work": [r["id"] for r in plan["topologies"]], "retained": ["retained-configs"]}
    coverage = {}
    host = None
    for metric, expected in selections.items():
        base, head, comparison = (json.loads((directory / f"{side}-{metric}.json").read_bytes())
                                  for side in ("base", "head", "compare"))
        if comparison["classification"] != "diagnostic":
            raise ValueError(f"{metric} classified the pair as {comparison['classification']}")
        if not expected or comparison["plan_sha256"] != plan_sha or list(comparison["rows"]) != expected:
            raise ValueError(f"{metric} does not cover the frozen plan selection")
        options = comparison["options"]
        recipe_metric = "memory" if metric == "retained" else metric
        operation = {"memory": "svg-data", "work": "layout-work", "retained": "prepare-retained"}[metric]
        if (options["metric"] != recipe_metric or options["operation"] != operation
                or type(options["samples"]) is not int or options["samples"] <= 0):
            raise ValueError(f"invalid {metric} comparison recipe")
        coverage[metric] = {"comparisons": Counter(r["status"] for r in comparison["rows"].values())}
        for side, report in (("base", base), ("head", head)):
            if report["revision"] != interval[f"{side}_sha"] or report["surface"] != "rust" or report["schema_version"] != 1:
                raise ValueError(f"{side}-{metric} does not measure the selected Rust revision")
            if host is None:
                host = report["host"]
            if not host or report["host"] != host:
                raise ValueError(f"{side}-{metric} belongs to a different measurement host")
            observed = Counter()
            exceptions = {}
            for scenario, row in report["rows"].items():
                try:
                    if metric == "work":
                        outcomes = work_outcomes(row)
                        observed.update(outcomes.values())
                        if set(outcomes.values()) != {"accepted"}:
                            exceptions[scenario] = outcomes
                    else:
                        statuses = memory_outcomes(row, requests[scenario], options["samples"])
                        observed.update(statuses)
                        if row.get("status"):
                            exceptions[scenario] = statuses
                except (ValueError, KeyError, TypeError, AttributeError) as error:
                    raise ValueError(f"{side}-{metric}/{scenario}: {error}") from error
            coverage[metric][side] = dict(observed=observed, exceptions=exceptions)
        # The existing comparator binds plan, options, host, harness, executables and row outcomes.
        if comparison != compare(base, head):
            raise ValueError(f"{metric} comparison differs from the paired raw reports")
        if metric != "work":
            witness = SENTINEL if metric == "memory" else "retained-configs"
            row = comparison["rows"][witness]
            required_metrics = {"allocated_bytes"} if metric == "memory" else {"allocated_bytes", "live_delta_bytes", "after_release_delta_bytes"}
            if (row["status"] not in ("matching-verification-outputs", "same-output")
                    or row.get("completed") != len(requests[witness]) or not required_metrics.issubset(row.get("metrics", {}))):
                raise ValueError(f"{witness} has no comparable paired allocation measurement")
    return dict(plan_sha256=plan_sha, host=host, coverage=coverage)


def append(path, text):
    with Path(path).open("a", encoding="utf-8") as output:
        print(text, file=output)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("select", "record"))
    parser.add_argument("path", type=Path, help="sentinel plan to select, or raw workload directory to record")
    args = parser.parse_args(argv)
    env = os.environ
    try:
        if args.command == "select":
            interval = select(env, protocol(json.loads(args.path.read_bytes()), env))
            summary = f"Weekly workload `{interval['kind']}`: `{interval['base_sha']}` → `{interval['head_sha']}`."
            if interval["previous"]:
                previous = interval["previous"]
                summary += f" Previous receipt: run {previous['run_id']}, attempt {previous['run_attempt']}, artifact {previous['artifact_id']}."
            if interval["skipped"]:
                summary += f" Skipped: {dict(interval['skipped'])}."
            append(env["GITHUB_STEP_SUMMARY"], summary)
            append(env["GITHUB_OUTPUT"], f"base_sha={interval['base_sha']}\ninterval=" + json.dumps(interval, separators=(",", ":")))
            return 0

        receipt = json.loads(env["WEEKLY_INTERVAL"])
        attempt = int(env["GITHUB_RUN_ATTEMPT"])
        receipt.update(workflow=WORKFLOW, run_id=int(env["GITHUB_RUN_ID"]), run_attempt=attempt, outcome="incomplete")
        try:
            if env["WORKLOAD_RESULT"] != "success":
                raise ValueError(f"ordered workload step: {env['WORKLOAD_RESULT']}")
            receipt.update(qualify(args.path, receipt))
            receipt["outcome"] = "complete"
        except (ValueError, KeyError, OSError, TypeError, AttributeError) as error:
            receipt["reason"] = str(error)
        args.path.mkdir(parents=True, exist_ok=True)
        (args.path / "weekly-interval.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        if receipt["outcome"] == "incomplete":
            raise ValueError(receipt["reason"])
        summary = ["### Weekly workload coverage", "",
                   "| Revision | Verified outputs | Expected rejections | Other observed outcomes | Affected scenarios |",
                   "| --- | ---: | ---: | ---: | --- |"]
        for side in ("base", "head"):
            memory = receipt["coverage"]["memory"][side]
            counts = memory["observed"]
            other = sum(counts.values()) - counts["ok"] - counts["expected-rejection"]
            affected = ", ".join(f"`{name}`" for name, statuses in memory["exceptions"].items()
                                 if statuses.keys() - {"ok", "expected-rejection"})
            summary.append(f"| {side} | {counts['ok']} | {counts['expected-rejection']} | {other} | {affected or '—'} |")
        append(env["GITHUB_STEP_SUMMARY"], "\n".join(summary))
        append(env["GITHUB_OUTPUT"], f"artifact=perf-public-workloads-weekly-{attempt}-{receipt['protocol']}")
        return 0
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        message = f"Weekly workload {args.command} failed: {error}"
        print(message, file=sys.stderr)
        append(env["GITHUB_STEP_SUMMARY"], message)
        return 1


if __name__ == "__main__":
    sys.exit(main())
