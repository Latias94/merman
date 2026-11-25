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
from urllib.parse import urlencode


OWNERS = {
    "crates": ("release-crates.yml", "publish", "Upload crates to crates.io"),
    "web": ("release-web.yml", "publish", "Publish npm package group"),
    "node": ("release-node.yml", "publish npm package group", "Publish npm package group"),
    "flutter": ("release-flutter.yml", "publish", "Publish to pub.dev"),
    "grammar-npm": ("release-tree-sitter-mermaid.yml", "publish-npm", "Publish npm package"),
    "grammar-crates": ("release-tree-sitter-mermaid.yml", "publish-crates", "Publish crate"),
}


def read_api(path: str, *, paginate: bool = False, gh_env: dict[str, str] | None = None):
    command = ["gh", "api", path]
    if paginate:
        command.extend(["--paginate", "--slurp"])
    completed = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60, env=gh_env)
    return json.loads(completed.stdout)


def paginated_records(path: str, field: str, *, gh_env: dict[str, str] | None = None) -> list[dict]:
    """Reject incomplete or duplicated GitHub job/artifact listings."""

    pages = read_api(path, paginate=True, gh_env=gh_env)
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
class IndependentCrateReceipt:
    sha256: str
    source_sha: str
    state: str


INDEPENDENT_WORKFLOW = "release-independent-crate.yml"
INDEPENDENT_RECEIPT_RUN = "Release independent crate [receipts] "
INDEPENDENT_RECEIPT_ENV = "crates.io-receipts"


def _independent_receipt_endpoint(repository: str, name: str, version: str) -> tuple[str, str]:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Invalid independent crate receipt repository")
    return f"repos/{repository}/deployments", f"independent-crate:{name}:{version}"


def _independent_receipt(
    deployment: dict, *, task: str, name: str, version: str,
) -> IndependentCrateReceipt:
    payload = deployment.get("payload")
    if isinstance(payload, str):
        payload = json.loads(payload)
    if (deployment.get("task") != task or deployment.get("environment") != INDEPENDENT_RECEIPT_ENV
            or not isinstance(payload, dict) or payload.get("schema_version") != 1
            or (payload.get("name"), payload.get("version")) != (name, version)
            or payload.get("state") not in ("upload-intent", "registry-confirmed")):
        raise ValueError("Invalid independent crate deployment receipt")
    sha, digest = deployment.get("sha"), payload.get("archive_sha256")
    if (not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha)
            or not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest)):
        raise ValueError("Independent crate receipt does not bind a source and archive digest")
    return IndependentCrateReceipt(digest, sha, payload["state"])


def read_independent_receipt(
    repository: str, name: str, version: str, *, gh_env: dict[str, str] | None = None,
) -> IndependentCrateReceipt | None:
    endpoint, task = _independent_receipt_endpoint(repository, name, version)
    query = urlencode({"task": task, "environment": INDEPENDENT_RECEIPT_ENV, "per_page": 100})
    try:
        pages = read_api(f"{endpoint}?{query}", paginate=True, gh_env=gh_env)
    except subprocess.SubprocessError as exc:
        raise OSError("Cannot read independent crate receipts") from exc
    if not isinstance(pages, list) or not pages or any(not isinstance(page, list) for page in pages):
        raise ValueError("Invalid independent crate deployment listing")
    result = None
    for page in pages:
        for deployment in page:
            if not isinstance(deployment, dict):
                raise ValueError("Invalid independent crate deployment")
            receipt = _independent_receipt(deployment, task=task, name=name, version=version)
            if result is not None and (receipt.sha256, receipt.source_sha) != (result.sha256, result.source_sha):
                raise ValueError("Independent crate receipts disagree on the archive digest or source")
            if result is None or (result.state == "upload-intent" and receipt.state == "registry-confirmed"):
                result = receipt
    return result


def record_independent_receipt(
    repository: str, name: str, version: str, receipt: IndependentCrateReceipt,
    *, gh_env: dict[str, str] | None = None,
) -> None:
    endpoint, task = _independent_receipt_endpoint(repository, name, version)
    request = {
        "ref": receipt.source_sha,
        "task": task,
        "environment": INDEPENDENT_RECEIPT_ENV,
        "auto_merge": False,
        "required_contexts": [],
        "production_environment": False,
        "payload": {"schema_version": 1, "name": name, "version": version,
                    "state": receipt.state, "archive_sha256": receipt.sha256},
    }
    try:
        completed = subprocess.run(
            ["gh", "api", "--method", "POST", endpoint, "--input", "-"],
            input=json.dumps(request), check=True, capture_output=True, text=True, timeout=60, env=gh_env,
        )
    except subprocess.SubprocessError as exc:
        raise OSError(f"Cannot retain independent crate {receipt.state} receipt") from exc
    response = json.loads(completed.stdout)
    if (not isinstance(response, dict)
            or _independent_receipt(response, task=task, name=name, version=version) != receipt):
        raise ValueError(f"Cannot verify retained independent crate {receipt.state} receipt")


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
    source_sha: str, repo_root: Path, manifest: str, gh_env: dict[str, str] | None = None,
) -> IndependentCrateReceipt | None:
    """Read operation receipts and detect versions predating the receipt protocol."""
    recorded = read_independent_receipt(repository, name, version, gh_env=gh_env)
    if recorded is not None:
        return recorded
    title = f"Release independent crate {name} {version}"
    try:
        runs = paginated_records(
            f"repos/{repository}/actions/workflows/{INDEPENDENT_WORKFLOW}/runs?per_page=100", "workflow_runs",
            gh_env=gh_env,
        )
    except subprocess.SubprocessError as exc:
        raise OSError("Cannot inspect legacy independent crate runs") from exc
    current = next((run for run in runs if run["id"] == run_id), None)
    if (current is None or current.get("run_attempt") != before_attempt
            or current.get("display_title") != INDEPENDENT_RECEIPT_RUN + f"{name} {version}"
            or current.get("head_sha") != source_sha):
        raise ValueError("Current independent crate run identity is missing or changed")
    numbers = [run.get("run_number") for run in runs]
    if (any(type(number) is not int or number < 1 for number in numbers)
            or len(set(numbers)) != len(runs)):
        raise ValueError("Invalid independent crate run numbers")
    first_receipt_run = current["run_number"]
    for run in runs:
        run_title, sha = run.get("display_title"), run.get("head_sha")
        if not isinstance(run_title, str) or not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
            raise ValueError("Invalid legacy independent crate workflow run")
        if run_title.startswith(INDEPENDENT_RECEIPT_RUN):
            first_receipt_run = min(first_receipt_run, run["run_number"])
            continue
        if run_title == title or (
            not re.fullmatch(r"Release independent crate [a-z0-9_-]+ [0-9A-Za-z.+-]+", run_title)
            and _legacy_independent_version(repo_root, sha, manifest, name) == version
        ):
            raise ValueError(f"{name} {version} has legacy release history without a receipt; reconcile it manually")
    if sum(number < first_receipt_run for number in numbers) != first_receipt_run - 1:
        raise ValueError("Independent crate history before receipts is incomplete; reconcile legacy runs manually")
    return None


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
