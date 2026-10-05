#!/usr/bin/env python3
"""Read GitHub job history to distinguish a skipped upload from an unknown attempt."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys


OWNERS = {
    "crates": ("release-crates.yml", "publish", "Upload crates to crates.io"),
    "web": ("release-web.yml", "publish", "Publish npm package group"),
    "node": ("release-node.yml", "publish npm package group", "Publish npm package group"),
    "grammar-npm": ("release-tree-sitter-mermaid.yml", "publish-npm", "Publish npm package"),
    "grammar-crates": ("release-tree-sitter-mermaid.yml", "publish-crates", "Publish crate"),
}


def read_api(path: str, *, paginate: bool = False):
    command = ["gh", "api", path]
    if paginate:
        command.extend(["--paginate", "--slurp"])
    completed = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60)
    return json.loads(completed.stdout)


def prior_publish_state(repository: str, run_id: int, before_attempt: int, owner: str) -> str:
    """Return never-attempted only when every prior upload step was explicitly skipped."""

    if before_attempt == 1:
        return "never-attempted"
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) or run_id < 1:
        return "unknown"
    if before_attempt < 1 or before_attempt > 100:
        return "unknown"
    workflow, job_name, step_name = OWNERS[owner]
    try:
        run = read_api(f"repos/{repository}/actions/runs/{run_id}")
        if not isinstance(run, dict) or run.get("path") != f".github/workflows/{workflow}":
            return "unknown"
        for attempt in range(1, before_attempt):
            pages = read_api(
                f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100",
                paginate=True,
            )
            if not isinstance(pages, list) or not pages:
                return "unknown"
            jobs = []
            total = pages[0].get("total_count") if isinstance(pages[0], dict) else None
            for page in pages:
                if (not isinstance(page, dict) or not isinstance(page.get("jobs"), list)
                        or page.get("total_count") != total):
                    return "unknown"
                jobs.extend(page["jobs"])
            if type(total) is not int or len(jobs) != total or any(not isinstance(job, dict) for job in jobs):
                return "unknown"
            ids = [job.get("id") for job in jobs]
            if any(type(value) is not int for value in ids) or len(set(ids)) != len(ids):
                return "unknown"
            matches = [job for job in jobs if job.get("name") == job_name]
            if len(matches) != 1 or matches[0].get("status") != "completed":
                return "unknown"
            job = matches[0]
            if job.get("conclusion") == "skipped":
                continue
            if job.get("conclusion") not in {"success", "failure"}:
                return "unknown"
            steps = job.get("steps")
            if not isinstance(steps, list) or any(not isinstance(step, dict) for step in steps):
                return "unknown"
            matches = [step for step in steps if step.get("name") == step_name]
            if (len(matches) != 1 or matches[0].get("status") != "completed"
                    or matches[0].get("conclusion") != "skipped"):
                return "unknown"
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"Cannot prove that the previous upload was skipped: {exc}", file=sys.stderr)
        return "unknown"
    return "never-attempted"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner", choices=OWNERS, required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--before-attempt", type=int, required=True)
    args = parser.parse_args()
    print(prior_publish_state(args.repository, args.run_id, args.before_attempt, args.owner))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
