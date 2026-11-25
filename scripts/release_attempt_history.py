#!/usr/bin/env python3
"""Read GitHub job history to distinguish an unattempted upload from an unknown attempt."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
import tomllib


OWNERS = {
    "crates": ("release-crates.yml", "publish", "Upload crates to crates.io"),
    "independent-crate": ("release-independent-crate.yml", "publish", "Upload independent crate to crates.io"),
    "web": ("release-web.yml", "publish", "Publish npm package group"),
    "node": ("release-node.yml", "publish npm package group", "Publish npm package group"),
    "flutter": ("release-flutter.yml", "publish", "Publish to pub.dev"),
    "grammar-npm": ("release-tree-sitter-mermaid.yml", "publish-npm", "Publish npm package"),
    "grammar-crates": ("release-tree-sitter-mermaid.yml", "publish-crates", "Publish crate"),
}


def read_api(path: str, *, paginate: bool = False):
    command = ["gh", "api", path]
    if paginate:
        command.extend(["--paginate", "--slurp"])
    completed = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60)
    return json.loads(completed.stdout)


def paginated_records(path: str, field: str) -> list[dict]:
    """Reject incomplete or duplicated GitHub job/artifact listings."""

    pages = read_api(path, paginate=True)
    if not isinstance(pages, list) or not pages or not isinstance(pages[0], dict):
        raise ValueError(f"Invalid GitHub {field} pages")
    total = pages[0].get("total_count")
    if type(total) is not int or total < 0:
        raise ValueError(f"Invalid GitHub {field} total_count")
    records = []
    for page in pages:
        if (not isinstance(page, dict) or type(page.get("total_count")) is not int
                or page["total_count"] != total or not isinstance(page.get(field), list)):
            raise ValueError(f"Inconsistent GitHub {field} pages")
        records.extend(page[field])
    if len(records) != total or any(not isinstance(record, dict) for record in records):
        raise ValueError(f"Incomplete GitHub {field} listing")
    ids = [record.get("id") for record in records]
    if any(type(value) is not int or value < 1 for value in ids) or len(set(ids)) != len(ids):
        raise ValueError(f"Invalid or duplicate GitHub {field} ids")
    return records


def upload_was_skipped(repository: str, run_id: int, attempt: int, owner: str) -> bool:
    """Check a complete attempt listing for evidence that the upload was skipped."""
    return _upload_was_skipped(_attempt_jobs(repository, run_id, attempt), owner)


def _attempt_jobs(repository: str, run_id: int, attempt: int) -> list[dict]:
    return paginated_records(
        f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100",
        "jobs",
    )


def _upload_was_skipped(jobs: list[dict], owner: str) -> bool:
    _, job_name, step_name = OWNERS[owner]
    matches = [job for job in jobs if job.get("name") == job_name]
    if len(matches) != 1 or matches[0].get("status") != "completed":
        return False
    job = matches[0]
    if job.get("conclusion") == "skipped":
        return True
    if job.get("conclusion") == "cancelled":
        return ("runner_id" in job and job["runner_id"] is None
                and "runner_name" in job and job["runner_name"] is None
                and job.get("steps") == [])
    if job.get("conclusion") not in {"success", "failure"}:
        return False
    steps = job.get("steps")
    if not isinstance(steps, list) or any(not isinstance(step, dict) for step in steps):
        return False
    matches = [step for step in steps if step.get("name") == step_name]
    return (len(matches) == 1 and matches[0].get("status") == "completed"
            and matches[0].get("conclusion") == "skipped")


def prior_publish_state(repository: str, run_id: int, before_attempt: int, owner: str) -> str:
    """Return never-attempted only when every prior upload is proven not to have run."""

    if before_attempt == 1:
        return "never-attempted"
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) or run_id < 1:
        return "unknown"
    if before_attempt < 1 or before_attempt > 100:
        return "unknown"
    workflow, _, _ = OWNERS[owner]
    try:
        run = read_api(f"repos/{repository}/actions/runs/{run_id}")
        if not isinstance(run, dict) or run.get("path") != f".github/workflows/{workflow}":
            return "unknown"
        for attempt in range(1, before_attempt):
            if not upload_was_skipped(repository, run_id, attempt, owner):
                return "unknown"
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"Cannot prove that the previous upload was skipped: {exc}", file=sys.stderr)
        return "unknown"
    return "never-attempted"


@dataclass(frozen=True)
class IndependentCrateHistory:
    observe_only: bool = True
    artifact_id: int | None = None
    run_id: int | None = None
    attempt: int | None = None
    source_sha: str | None = None


def _independent_evidence(
    repository: str, run_id: int, sha: str, names: dict[str, int],
) -> dict[str, IndependentCrateHistory]:
    evidence = {}
    for artifact in paginated_records(
        f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100", "artifacts",
    ):
        name = artifact.get("name")
        if not isinstance(name, str):
            raise ValueError("Invalid independent workflow artifact name")
        if name not in names:
            continue
        if name in evidence:
            raise ValueError(f"Duplicate independent workflow artifact: {name}")
        producing_run = artifact.get("workflow_run")
        if producing_run is not None and (
            not isinstance(producing_run, dict)
            or producing_run.get("id") != run_id or producing_run.get("head_sha") != sha
        ):
            raise ValueError("Independent evidence has a different producing run or source")
        evidence[name] = (
            IndependentCrateHistory(
                artifact_id=artifact["id"], run_id=run_id, attempt=names[name], source_sha=sha,
            ) if artifact.get("expired") is False else IndependentCrateHistory()
        )
    return evidence


def _independent_confirmation_recorded(jobs: list[dict]) -> bool:
    for job in jobs:
        name = job.get("name")
        steps = job.get("steps")
        if not isinstance(steps, list):
            continue
        conclusions = {}
        for step in steps:
            if not isinstance(step, dict):
                continue
            label = step.get("name")
            if isinstance(label, str):
                conclusions[label] = step.get("conclusion")
            if (name == "verify-published" and label == "Verify exact published archive and registry consumers"
                    and step.get("conclusion") == "success"):
                return True
            if ((name, label) in (
                ("preflight", "Retain the independent preflight registry confirmation"),
                ("publish", "Retain the independent publisher registry confirmation"),
            ) and (step.get("conclusion") in {"failure", "success"}
                   or (step.get("started_at") and step.get("conclusion") != "skipped"))):
                return True
        if name == "preflight" and conclusions.get(
            "Verify registry bytes or dry-run independent crate publication"
        ) == "success":
            candidate = conclusions.get("Retain the frozen independent candidate")
            if candidate == "skipped" or (
                candidate not in {"success", "failure"} and job.get("conclusion") in {"cancelled", "timed_out"}
            ):
                return True
        if (name == "publish" and conclusions.get("Inspect independent crate publication state") == "success"
                and conclusions.get("Download the frozen independent candidate") == "skipped"):
            return True
    return False


def _legacy_independent_version(repo_root: Path, sha: str, manifest: str, package: str) -> str | None:
    # Earlier dispatches did not include their inputs in the title. Their source
    # manifest limits the version the old workflow could have published.
    listed = subprocess.run(
        ["git", "ls-tree", sha, "--", manifest], cwd=repo_root,
        check=True, capture_output=True, text=True, timeout=30,
    )
    if not listed.stdout:
        return None
    contents = subprocess.run(
        ["git", "show", f"{sha}:{manifest}"], cwd=repo_root,
        check=True, capture_output=True, encoding="utf-8", timeout=30,
    )
    entry = tomllib.loads(contents.stdout).get("package", {})
    if not isinstance(entry, dict) or entry.get("name") != package or not isinstance(entry.get("version"), str):
        raise ValueError("Cannot establish the independent package version at a legacy run's source")
    return entry["version"]


def independent_crate_history(
    repository: str, run_id: int, before_attempt: int, *, name: str, version: str,
    source_sha: str, repo_root: Path, manifest: str,
) -> IndependentCrateHistory:
    """Prove a first send, or select the original archive for observation only."""
    owner = "independent-crate"
    workflow, _, _ = OWNERS[owner]
    title = f"Release independent crate {name} {version}"
    try:
        if (not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)
                or run_id < 1 or not 1 <= before_attempt <= 100):
            raise ValueError("Invalid independent crate publication identity")
        runs = paginated_records(
            f"repos/{repository}/actions/workflows/{workflow}/runs?per_page=100", "workflow_runs",
        )
        current = next((run for run in runs if run["id"] == run_id), None)
        if (current is None or current.get("run_attempt") != before_attempt
                or current.get("display_title") != title or current.get("head_sha") != source_sha):
            raise ValueError("Current independent crate run identity is missing or changed")

        numbers = [run.get("run_number") for run in runs]
        if any(type(number) is not int or number < 1 for number in numbers) or len(set(numbers)) != len(numbers):
            raise ValueError("Invalid or duplicate independent workflow run numbers")
        incomplete_runs = max(numbers) != len(numbers)
        if incomplete_runs:
            print("Independent crate run history has missing run numbers", file=sys.stderr)

        possibly_sent = False
        previously_confirmed = False
        intents = []
        confirmations = []
        for run in sorted(runs, key=lambda run: run["run_number"]):
            sha = run.get("head_sha")
            run_title = run.get("display_title")
            attempt = run.get("run_attempt")
            if (run.get("path") != f".github/workflows/{workflow}"
                    or not isinstance(run.get("repository"), dict)
                    or run["repository"].get("full_name") != repository
                    or run.get("event") != "workflow_dispatch"
                    or not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha)
                    or not isinstance(run_title, str)
                    or type(attempt) is not int or not 1 <= attempt <= 100):
                raise ValueError("Invalid independent crate workflow history")
            if run_title != title:
                if re.fullmatch(r"Release independent crate [a-z0-9_-]+ [0-9A-Za-z.+-]+", run_title):
                    continue
                if _legacy_independent_version(repo_root, sha, manifest, name) != version:
                    continue

            if run["id"] == run_id:
                attempt -= 1
            elif run.get("status") != "completed":
                incomplete_runs = True
                attempt -= 1

            intent_names = {}
            confirmation_names = {}
            for past_attempt in range(1, attempt + 1):
                jobs = _attempt_jobs(repository, run["id"], past_attempt)
                if not jobs and run.get("run_attempt") == 1 and run.get("conclusion") == "cancelled":
                    continue
                if not _upload_was_skipped(jobs, owner):
                    intent_names[f"crates-io-independent-upload-intent-attempt-{past_attempt}"] = past_attempt
                for stage in ("preflight", "publish"):
                    confirmation_names[f"crates-io-independent-confirmation-{stage}-attempt-{past_attempt}"] = past_attempt
                previously_confirmed |= _independent_confirmation_recorded(jobs)
            if not confirmation_names:
                continue
            evidence = _independent_evidence(repository, run["id"], sha, intent_names | confirmation_names)
            possibly_sent |= bool(intent_names)
            intents.extend(evidence[name] for name in intent_names if name in evidence)
            confirmations.extend(evidence[name] for name in confirmation_names if name in evidence)

        if intents:
            return intents[0] if len(intents) == 1 else IndependentCrateHistory()
        if confirmations:
            return confirmations[0]
        return IndependentCrateHistory(observe_only=incomplete_runs or possibly_sent or previously_confirmed)
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"Cannot establish independent crate publication history: {exc}", file=sys.stderr)
    return IndependentCrateHistory()


@dataclass(frozen=True)
class NpmRecovery:
    artifact_id: int | None = None
    run_id: int | None = None
    attempt: int | None = None
    observe_only: bool = True


def npm_run(repository: str, run_id: int, owner: str) -> dict:
    run = read_api(f"repos/{repository}/actions/runs/{run_id}")
    workflow = OWNERS[owner][0]
    if (not isinstance(run, dict) or type(run.get("id")) is not int or run["id"] != run_id
            or run.get("path") != f".github/workflows/{workflow}"
            or not isinstance(run.get("repository"), dict)
            or run["repository"].get("full_name") != repository
            or run.get("event") != "workflow_dispatch"
            or type(run.get("run_attempt")) is not int or not 1 <= run["run_attempt"] <= 100):
        raise ValueError("Invalid npm recovery run identity")
    return run


def report_before(repository: str, run_id: int, last_attempt: int, owner: str) -> NpmRecovery:
    """Stop at the nearest report, or at any upload not proven to have been unattempted."""

    artifacts = paginated_records(
        f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100", "artifacts",
    )
    for attempt in range(last_attempt, 0, -1):
        name = f"merman-{owner}-npm-reconciliation-report-attempt-{attempt}"
        matches = [artifact for artifact in artifacts if artifact.get("name") == name]
        if len(matches) > 1:
            raise ValueError(f"Duplicate npm reconciliation report for {run_id}/{attempt}")
        if matches:
            artifact = matches[0]
            if type(artifact.get("expired")) is not bool:
                raise ValueError("Invalid npm reconciliation report expiry")
            if not artifact["expired"]:
                return NpmRecovery(artifact["id"], run_id, attempt, observe_only=False)
        if not upload_was_skipped(repository, run_id, attempt, owner):
            return NpmRecovery()
    return NpmRecovery(observe_only=False)


def select_npm_recovery(
    repository: str, run_id: int, before_attempt: int, owner: str,
    *, ancestor_run_id: int | None = None,
) -> NpmRecovery:
    """Select an exact report only across later uploads proven not to have run.

    Report contents remain untrusted data. The package-group reconciler must bind
    their publication identity and manifest before allowing any missing upload.
    """

    try:
        if (owner not in {"node", "web"}
                or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)
                or type(run_id) is not int or run_id < 1
                or type(before_attempt) is not int or not 1 <= before_attempt <= 100
                or (ancestor_run_id is not None and
                    (type(ancestor_run_id) is not int or ancestor_run_id < 1 or ancestor_run_id == run_id))):
            raise ValueError("Invalid npm recovery selection inputs")
        if before_attempt > 1:
            run = npm_run(repository, run_id, owner)
            if run["run_attempt"] != before_attempt:
                raise ValueError("Current npm run attempt changed")
            selected = report_before(repository, run_id, before_attempt - 1, owner)
            if selected.artifact_id is not None or selected.observe_only:
                return selected
        if ancestor_run_id is None:
            return NpmRecovery(observe_only=False)
        ancestor = npm_run(repository, ancestor_run_id, owner)
        if ancestor.get("status") != "completed":
            raise ValueError("Ancestor npm recovery run is not completed")
        selected = report_before(repository, ancestor_run_id, ancestor["run_attempt"], owner)
        # A report-less cross-run recovery never authorizes a first upload.
        return selected if selected.artifact_id is not None else NpmRecovery()
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"Cannot safely select an npm recovery report: {exc}", file=sys.stderr)
        return NpmRecovery()


def flutter_publish_state(
    repository: str, run_id: int, before_attempt: int, authentication_tag: str,
    authentication_sha: str,
) -> str:
    """Permit a send only when this tag has no earlier attempted pub.dev upload."""

    try:
        if (not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository)
                or not re.fullmatch(r"flutter-v[0-9A-Za-z.+-]+", authentication_tag)
                or not re.fullmatch(r"[0-9a-f]{40}", authentication_sha)
                or not 1 <= before_attempt <= 100 or run_id < 1):
            raise ValueError("Invalid Flutter publication identity")
        runs = paginated_records(
            f"repos/{repository}/actions/workflows/release-flutter.yml/runs"
            f"?head_sha={authentication_sha}&per_page=100", "workflow_runs",
        )
        current = [run for run in runs if run["id"] == run_id]
        if len(current) != 1 or current[0].get("run_attempt") != before_attempt:
            raise ValueError("Current Flutter run identity is missing or changed")
        for run in runs:
            if run.get("head_branch") != authentication_tag:
                continue
            if (run.get("path") != ".github/workflows/release-flutter.yml"
                    or run.get("head_sha") != authentication_sha
                    or run.get("event") not in {"push", "workflow_dispatch"}
                    or not isinstance(run.get("repository"), dict)
                    or run["repository"].get("full_name") != repository
                    or type(run.get("run_attempt")) is not int
                    or not 1 <= run["run_attempt"] <= 100):
                raise ValueError("Invalid earlier Flutter publication run")
            if run["id"] == run_id:
                last_attempt = before_attempt - 1
            else:
                if run.get("status") != "completed":
                    raise ValueError("Another Flutter publication run is unresolved")
                last_attempt = run["run_attempt"]
            for attempt in range(1, last_attempt + 1):
                if not upload_was_skipped(repository, run["id"], attempt, "flutter"):
                    return "unknown"
        if current[0].get("head_branch") != authentication_tag:
            raise ValueError("Current Flutter authentication tag differs")
        return "never-attempted"
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"Cannot establish Flutter publication history: {exc}", file=sys.stderr)
        return "unknown"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner", choices=OWNERS, required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--before-attempt", type=int, required=True)
    parser.add_argument("--select-npm-report", action="store_true")
    parser.add_argument("--ancestor-run-id", type=int, default=0)
    parser.add_argument("--authentication-tag")
    parser.add_argument("--authentication-sha")
    args = parser.parse_args(argv)
    if args.select_npm_report:
        selected = select_npm_recovery(
            args.repository, args.run_id, args.before_attempt, args.owner,
            ancestor_run_id=args.ancestor_run_id or None,
        )
        for field in ("artifact_id", "run_id", "attempt"):
            value = getattr(selected, field)
            print(f"{field}={value if value is not None else ''}")
        print(f"observe_only={str(selected.observe_only).lower()}")
        return 0
    if args.owner == "flutter":
        if not args.authentication_tag or not args.authentication_sha:
            print("unknown")
            return 0
        print(flutter_publish_state(
            args.repository, args.run_id, args.before_attempt,
            args.authentication_tag, args.authentication_sha,
        ))
        return 0
    print(prior_publish_state(args.repository, args.run_id, args.before_attempt, args.owner))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
