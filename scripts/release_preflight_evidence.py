#!/usr/bin/env python3
"""Reuse bounded, immutable release preflight evidence; otherwise run every check."""

from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import urllib.parse
import urllib.request
import zipfile

if __package__:
    from .release_version import parse_release_version
else:
    from release_version import parse_release_version

WORKFLOW_PATH = ".github/workflows/release-preflight.yml"
RECEIPT_NAME = "preflight-evidence.json"
REQUIRED_JOBS = frozenset({
    "validate-inputs", "versions-and-packages", "mermaid-reference-materialized", "cli-and-lsp-archives",
    "python-wheel", "android-aar", "apple-xcframework", "web-npm-dry-run",
    "node-loader-package", "node-wasm-package", "node-platform-package",
    "node-npm-dry-run", "vscode-extension-dry-run", "flutter-dry-run",
})
MAX_RUNS = 20
MAX_ARTIFACTS = 100
MAX_API_BYTES = 2 * 1024 * 1024
MAX_ARCHIVE_BYTES = 64 * 1024
MAX_RECEIPT_BYTES = 16 * 1024
GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")


class EvidenceError(ValueError):
    """An evidence boundary did not match the current release."""


def positive_integer(value: object) -> int:
    if type(value) is not int or value <= 0:
        raise EvidenceError("expected a positive integer identity")
    return value


@dataclass(frozen=True)
class ReleaseIdentity:
    repository: str
    repository_id: int
    workflow_sha: str
    source_sha: str
    source_tree: str
    version: str

    def __post_init__(self) -> None:
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", self.repository):
            raise EvidenceError("invalid repository identity")
        positive_integer(self.repository_id)
        for value in (self.workflow_sha, self.source_sha, self.source_tree):
            if not GIT_SHA.fullmatch(value):
                raise EvidenceError("source and workflow identities must be full lowercase SHAs")
        try:
            parse_release_version(self.version, allow_v_prefix=False)
        except ValueError as error:
            raise EvidenceError("invalid release version") from error


def artifact_name(identity: ReleaseIdentity, attempt: int) -> str:
    return f"release-preflight-{identity.source_sha}-attempt-{attempt}"


def receipt(identity: ReleaseIdentity, run_id: int, attempt: int) -> dict:
    return {
        "schema_version": 1,
        "workflow_path": WORKFLOW_PATH,
        **asdict(identity),
        "run_id": positive_integer(run_id),
        "run_attempt": positive_integer(attempt),
    }


def create_receipt(
    identity: ReleaseIdentity, source_ref: str, needs: dict, run_id: int, attempt: int,
) -> dict | None:
    # Branch/tag preflights remain useful, but only frozen inputs can be reused.
    if not GIT_SHA.fullmatch(source_ref):
        return None
    if source_ref != identity.source_sha:
        raise EvidenceError("immutable input differs from the resolved source")
    if not isinstance(needs, dict) or set(needs) != REQUIRED_JOBS:
        raise EvidenceError("preflight evidence requires every owner job")
    if any(not isinstance(job, dict) or job.get("result") != "success" for job in needs.values()):
        raise EvidenceError("preflight evidence requires successful owner jobs")
    return receipt(identity, run_id, attempt)


class SafeRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise EvidenceError("artifact redirect must use HTTPS")
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is not None:
            # GitHub redirects artifact downloads to signed storage URLs.
            redirected.remove_header("Authorization")
        return redirected


class GitHubApi:
    def __init__(self, repository: str, token: str):
        if not token:
            raise EvidenceError("GitHub read token is unavailable")
        self.root = f"https://api.github.com/repos/{repository}"
        self.token = token
        self.opener = urllib.request.build_opener(SafeRedirectHandler())

    def read(self, path: str, limit: int) -> bytes:
        request = urllib.request.Request(self.root + path, headers={
            "Authorization": f"Bearer {self.token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "merman-release-preflight",
        })
        with self.opener.open(request, timeout=15) as response:
            data = response.read(limit + 1)
        if len(data) > limit:
            raise EvidenceError("GitHub evidence response exceeds its size limit")
        return data

    def get(self, path: str) -> dict:
        value = json.loads(self.read(path, MAX_API_BYTES))
        if not isinstance(value, dict):
            raise EvidenceError("GitHub evidence metadata must be an object")
        return value

    def download(self, artifact_id: int) -> bytes:
        return self.read(f"/actions/artifacts/{artifact_id}/zip", MAX_ARCHIVE_BYTES)


def verify_run(run: dict, identity: ReleaseIdentity, workflow_id: int) -> tuple[int, int]:
    if not isinstance(run, dict):
        raise EvidenceError("invalid workflow run metadata")
    for key, expected in {
        "path": WORKFLOW_PATH, "workflow_id": workflow_id,
        "event": "workflow_dispatch", "status": "completed", "conclusion": "success",
        # For this non-reusable dispatch workflow, head_sha is the workflow ref,
        # not the separately checked-out inputs.source_ref.
        "head_sha": identity.workflow_sha,
    }.items():
        if type(run.get(key)) is not type(expected) or run[key] != expected:
            raise EvidenceError(f"workflow run {key} does not match")
    for key in ("repository", "head_repository"):
        repository = run.get(key)
        if not isinstance(repository, dict) or (
            repository.get("full_name"), repository.get("id")
        ) != (identity.repository, identity.repository_id):
            raise EvidenceError("workflow run repository does not match")
    return positive_integer(run.get("id")), positive_integer(run.get("run_attempt"))


def verify_archive(data: bytes, artifact: dict, expected: dict) -> None:
    if len(data) > MAX_ARCHIVE_BYTES:
        raise EvidenceError("preflight artifact exceeds its size limit")
    digest = "sha256:" + hashlib.sha256(data).hexdigest()
    if artifact.get("digest") != digest:
        raise EvidenceError("preflight artifact digest does not match GitHub metadata")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        members = archive.infolist()
        if len(members) != 1:
            raise EvidenceError("preflight artifact must contain one JSON receipt")
        member = members[0]
        mode = stat.S_IFMT(member.external_attr >> 16)
        if (member.filename != RECEIPT_NAME or member.file_size > MAX_RECEIPT_BYTES
                or member.flag_bits & 1 or mode not in (0, stat.S_IFREG)):
            raise EvidenceError("invalid preflight artifact member")
        with archive.open(member) as handle:
            content = handle.read(MAX_RECEIPT_BYTES + 1)
        if len(content) > MAX_RECEIPT_BYTES:
            raise EvidenceError("preflight receipt exceeds its size limit")
    actual = json.loads(content)
    if not isinstance(actual, dict) or set(actual) != set(expected) or any(
        type(actual[key]) is not type(value) or actual[key] != value
        for key, value in expected.items()
    ):
        raise EvidenceError("preflight receipt identity does not match")


def find_evidence(identity: ReleaseIdentity, api: GitHubApi, log=print) -> dict | None:
    workflow = api.get(f"/actions/workflows/{WORKFLOW_PATH.rsplit('/', 1)[1]}")
    if workflow.get("path") != WORKFLOW_PATH:
        raise EvidenceError("preflight workflow path does not match")
    workflow_id = positive_integer(workflow.get("id"))
    page = api.get(f"/actions/workflows/{workflow_id}/runs?event=workflow_dispatch&per_page={MAX_RUNS}")
    runs = page.get("workflow_runs")
    if not isinstance(runs, list):
        raise EvidenceError("workflow run list is unavailable")
    for candidate in runs[:MAX_RUNS]:
        try:
            run_id, attempt = verify_run(candidate, identity, workflow_id)
            attempt_run = api.get(f"/actions/runs/{run_id}/attempts/{attempt}")
            if verify_run(attempt_run, identity, workflow_id) != (run_id, attempt):
                raise EvidenceError("workflow attempt identity changed")
            page = api.get(f"/actions/runs/{run_id}/artifacts?per_page={MAX_ARTIFACTS}")
            artifacts = page.get("artifacts")
            if (not isinstance(artifacts, list) or type(page.get("total_count")) is not int
                    or page["total_count"] != len(artifacts) or len(artifacts) > MAX_ARTIFACTS):
                raise EvidenceError("artifact listing exceeds the bounded evidence search")
            matches = [item for item in artifacts if isinstance(item, dict)
                       and item.get("name") == artifact_name(identity, attempt)]
            if len(matches) != 1:
                raise EvidenceError("one matching attempt artifact is required")
            artifact = matches[0]
            artifact_id = positive_integer(artifact.get("id"))
            if (artifact.get("expired") is not False
                    or type(artifact.get("size_in_bytes")) is not int
                    or not 0 < artifact["size_in_bytes"] <= MAX_ARCHIVE_BYTES):
                raise EvidenceError("preflight artifact is expired or exceeds its size limit")
            origin = artifact.get("workflow_run")
            if not isinstance(origin, dict) or any(origin.get(key) != value for key, value in {
                "id": run_id, "repository_id": identity.repository_id,
                "head_repository_id": identity.repository_id, "head_sha": identity.workflow_sha,
            }.items()):
                raise EvidenceError("preflight artifact producer does not match")
            verify_archive(api.download(artifact_id), artifact, receipt(identity, run_id, attempt))
            latest = api.get(f"/actions/runs/{run_id}")
            if verify_run(latest, identity, workflow_id) != (run_id, attempt):
                raise EvidenceError("workflow attempt changed while downloading evidence")
            return {"run_id": run_id, "run_attempt": attempt, "artifact_id": artifact_id,
                    "artifact_digest": artifact["digest"]}
        except (ValueError, RuntimeError, zipfile.BadZipFile) as error:
            log(f"Preflight candidate rejected: {error}")
    return None


def identity_from_environment() -> ReleaseIdentity:
    return ReleaseIdentity(
        repository=os.environ["GITHUB_REPOSITORY"],
        repository_id=int(os.environ["GITHUB_REPOSITORY_ID"]),
        workflow_sha=os.environ["PREFLIGHT_WORKFLOW_SHA"],
        source_sha=os.environ["SOURCE_SHA"], source_tree=os.environ["SOURCE_TREE"],
        version=os.environ["VERSION"],
    )


def write_outputs(values: dict) -> None:
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as handle:
        for key, value in values.items():
            handle.write(f"{key}={value}\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("create")
    create.add_argument("--output", type=Path, required=True)
    commands.add_parser("find")
    args = parser.parse_args(argv)
    if args.command == "create":
        evidence = create_receipt(
            identity_from_environment(), os.environ["PREFLIGHT_SOURCE_REF"],
            json.loads(os.environ["PREFLIGHT_NEEDS"]),
            int(os.environ["GITHUB_RUN_ID"]), int(os.environ["GITHUB_RUN_ATTEMPT"]),
        )
        if evidence is None:
            print("Preflight evidence not issued: source_ref is not an immutable commit SHA")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(evidence, sort_keys=True) + "\n", encoding="utf-8")
        write_outputs({"created": str(evidence is not None).lower()})
        return 0

    evidence = None
    try:
        identity = identity_from_environment()
        evidence = find_evidence(identity, GitHubApi(identity.repository, os.environ.get("GH_TOKEN", "")))
    except Exception as error:
        # Reuse is optional: even an unexpected lookup failure must run full gates.
        print(f"Preflight evidence unavailable: {error}")
    write_outputs({"reusable": str(evidence is not None).lower(), **(evidence or {})})
    if evidence:
        print(f"Reusing strict feature matrix and Web build/smoke from run {evidence['run_id']} "
              f"attempt {evidence['run_attempt']}, artifact {evidence['artifact_id']}")
    else:
        print("No reusable preflight evidence; running the complete release checks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
