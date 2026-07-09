#!/usr/bin/env python3
"""Report declared and observed release surface status."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SURFACES_PATH = ROOT / "docs" / "release" / "SURFACES.json"
STATE_ORDER = {
    "published": 0,
    "artifact-only": 1,
    "manual-registry": 2,
    "credential-blocked": 3,
    "registry-blocked": 4,
    "not-built": 5,
    "not-applicable": 6,
}


class SurfaceError(Exception):
    pass


def load_contract(path: Path = SURFACES_PATH) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise SurfaceError(f"surface contract not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SurfaceError(f"invalid JSON in {path}: {exc}") from exc
    validate_contract(data)
    return data


def validate_contract(data: dict[str, Any]) -> None:
    states = set(data.get("states", []))
    missing_states = set(STATE_ORDER) - states
    if missing_states:
        raise SurfaceError(f"SURFACES.json missing states: {', '.join(sorted(missing_states))}")
    seen_ids: set[str] = set()
    for surface in data.get("surfaces", []):
        surface_id = require_str(surface, "id", "surface")
        if surface_id in seen_ids:
            raise SurfaceError(f"duplicate surface id: {surface_id}")
        seen_ids.add(surface_id)
        require_str(surface, "name", surface_id)
        require_str(surface, "entry_point", surface_id)
        require_str(surface, "dependency_weight", surface_id)
        require_list(surface, "capabilities", surface_id)
        require_list(surface, "docs", surface_id)
        channels = require_list(surface, "channels", surface_id)
        for channel in channels:
            channel_id = require_str(channel, "id", surface_id)
            state = require_str(channel, "declared_state", f"{surface_id}/{channel_id}")
            if state not in states:
                raise SurfaceError(f"{surface_id}/{channel_id}: unknown declared_state {state!r}")
            release_kinds = channel.get("release_kinds", [])
            if not isinstance(release_kinds, list) or not release_kinds:
                raise SurfaceError(f"{surface_id}/{channel_id}: release_kinds must be a non-empty list")


def require_str(item: dict[str, Any], key: str, owner: str) -> str:
    value = item.get(key)
    if not isinstance(value, str) or not value.strip():
        raise SurfaceError(f"{owner}: missing string field {key}")
    return value


def require_list(item: dict[str, Any], key: str, owner: str) -> list[Any]:
    value = item.get(key)
    if not isinstance(value, list) or not value:
        raise SurfaceError(f"{owner}: missing non-empty list field {key}")
    return value


def release_kind(version: str | None) -> str | None:
    if not version:
        return None
    normalized = version.removeprefix("v")
    if "-" in normalized:
        return "prerelease"
    if re.search(r"(a|b|rc)\d+$", normalized):
        return "prerelease"
    return "stable"


def effective_declared_state(channel: dict[str, Any], version: str | None) -> str:
    kind = release_kind(version)
    if kind and kind not in channel.get("release_kinds", []):
        return "not-applicable"
    return channel["declared_state"]


def summarize_surface_state(surface: dict[str, Any], version: str | None) -> str:
    states = [effective_declared_state(channel, version) for channel in surface["channels"]]
    return min(states, key=lambda state: STATE_ORDER[state])


def channel_probe(channel: dict[str, Any], surface: dict[str, Any], version: str) -> dict[str, str]:
    kind = channel.get("kind")
    if kind == "npm":
        package = first_package_name(surface, "npm")
        if package:
            return probe_npm(package, version)
    if kind == "pub.dev":
        package = first_package_name(surface, "flutter")
        if package:
            return probe_pub_dev(package, version)
    if kind == "pypi":
        package = first_package_name(surface, "python")
        if package:
            return probe_pypi(package, python_version(version))
    if kind == "crates.io":
        packages = package_names(surface, "crate")
        if packages:
            return probe_many(packages, lambda package: probe_crates_io(package, version))
    if kind == "github-release-assets":
        return probe_github_release(version)
    return {"state": "unknown", "reason": f"no probe implemented for {kind}"}


def first_package_name(surface: dict[str, Any], kind: str) -> str | None:
    names = package_names(surface, kind)
    return names[0] if names else None


def package_names(surface: dict[str, Any], kind: str) -> list[str]:
    return [
        package["name"]
        for package in surface.get("packages", [])
        if package.get("kind") == kind and package.get("name")
    ]


def probe_many(packages: list[str], probe: Any) -> dict[str, str]:
    results = [(package, probe(package)) for package in packages]
    missing = [(package, result) for package, result in results if result["state"] == "missing"]
    unknown = [(package, result) for package, result in results if result["state"] == "unknown"]
    if missing:
        return {
            "state": "missing",
            "reason": "; ".join(f"{package}: {result['reason']}" for package, result in missing),
        }
    if unknown:
        return {
            "state": "unknown",
            "reason": "; ".join(f"{package}: {result['reason']}" for package, result in unknown),
        }
    return {
        "state": "found",
        "reason": "all package versions exist: " + ", ".join(package for package, _result in results),
    }


def python_version(version: str) -> str:
    normalized = version.removeprefix("v")
    return re.sub(r"-alpha\.(\d+)$", r"a\1", normalized)


def probe_npm(package: str, version: str) -> dict[str, str]:
    encoded_package = urllib.parse.quote(package, safe="")
    encoded_version = urllib.parse.quote(version, safe="")
    url = f"https://registry.npmjs.org/{encoded_package}/{encoded_version}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            data = json.load(response)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "npm package version not found"}
        return {"state": "unknown", "reason": f"npm HTTP {exc.code}"}
    except (OSError, json.JSONDecodeError) as exc:
        return {"state": "unknown", "reason": str(exc)}
    if data.get("version") == version:
        return {"state": "found", "reason": "npm package version exists"}
    return {"state": "unknown", "reason": "npm response did not confirm version"}


def probe_pub_dev(package: str, version: str) -> dict[str, str]:
    encoded_package = urllib.parse.quote(package, safe="")
    url = f"https://pub.dev/api/packages/{encoded_package}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            data = json.load(response)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "pub.dev package not found"}
        return {"state": "unknown", "reason": f"pub.dev HTTP {exc.code}"}
    except (OSError, json.JSONDecodeError) as exc:
        return {"state": "unknown", "reason": str(exc)}
    versions = data.get("versions")
    if isinstance(versions, list) and any(
        item.get("version") == version for item in versions if isinstance(item, dict)
    ):
        return {"state": "found", "reason": "pub.dev package version exists"}
    return {"state": "missing", "reason": "pub.dev package version not found"}


def probe_pypi(package: str, version: str) -> dict[str, str]:
    url = f"https://pypi.org/pypi/{package}/{version}/json"
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            if response.status == 200:
                return {"state": "found", "reason": "PyPI package version exists"}
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "PyPI package version not found"}
        return {"state": "unknown", "reason": f"PyPI HTTP {exc.code}"}
    except OSError as exc:
        return {"state": "unknown", "reason": str(exc)}
    return {"state": "unknown", "reason": "PyPI response did not confirm version"}


def probe_crates_io(package: str, version: str) -> dict[str, str]:
    url = f"https://crates.io/api/v1/crates/{package}/{version}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            if response.status == 200:
                return {"state": "found", "reason": "crates.io package version exists"}
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "crates.io package version not found"}
        return {"state": "unknown", "reason": f"crates.io HTTP {exc.code}"}
    except OSError as exc:
        return {"state": "unknown", "reason": str(exc)}
    return {"state": "unknown", "reason": "crates.io response did not confirm version"}


def probe_github_release(version: str) -> dict[str, str]:
    gh = shutil.which("gh")
    if not gh:
        return {"state": "unknown", "reason": "gh not found"}
    tag = version if version.startswith("v") else f"v{version}"
    result = subprocess.run(
        [gh, "release", "view", tag, "--json", "tagName"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    if result.returncode == 0:
        return {"state": "found", "reason": "GitHub Release exists"}
    return {"state": "missing", "reason": stderr_or_stdout(result)}


def stderr_or_stdout(result: subprocess.CompletedProcess[str]) -> str:
    return (result.stderr or result.stdout or "command failed").strip().splitlines()[0]


def build_rows(data: dict[str, Any], *, version: str | None, probe: bool) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for surface in data["surfaces"]:
        surface_state = summarize_surface_state(surface, version)
        channel_rows: list[dict[str, Any]] = []
        for channel in surface["channels"]:
            channel_state = effective_declared_state(channel, version)
            row: dict[str, Any] = {
                "id": channel["id"],
                "kind": channel["kind"],
                "declared_state": channel_state,
                "workflow": channel.get("workflow"),
                "credential": channel.get("credential"),
                "blocker": channel.get("blocker"),
            }
            if probe:
                if channel_state == "not-applicable":
                    row["observed_status"] = {
                        "state": "not-applicable",
                        "reason": channel.get("not_applicable_reason", "channel does not apply"),
                    }
                else:
                    row["observed_status"] = channel_probe(channel, surface, version or "")
            channel_rows.append(row)
        rows.append(
            {
                "id": surface["id"],
                "name": surface["name"],
                "entry_point": surface["entry_point"],
                "support_level": surface["support_level"],
                "dependency_weight": surface["dependency_weight"],
                "capabilities": surface["capabilities"],
                "declared_state": surface_state,
                "install": surface.get("install"),
                "channels": channel_rows,
            }
        )
    return rows


def render_public(rows: list[dict[str, Any]]) -> str:
    lines = ["Surface | Entry point | Availability | Weight | Key capabilities", "--- | --- | --- | --- | ---"]
    for row in rows:
        lines.append(
            " | ".join(
                [
                    row["name"],
                    f"`{row['entry_point']}`",
                    row["declared_state"],
                    row["dependency_weight"],
                    ", ".join(row["capabilities"][:5]),
                ]
            )
        )
    return "\n".join(lines)


def render_maintainer(rows: list[dict[str, Any]]) -> str:
    lines = ["Surface | Channel | State | Workflow | Blocker", "--- | --- | --- | --- | ---"]
    for row in rows:
        for channel in row["channels"]:
            observed = channel.get("observed_status")
            state = channel["declared_state"]
            if observed:
                state = f"{state} ({observed['state']})"
            lines.append(
                " | ".join(
                    [
                        row["id"],
                        channel["id"],
                        state,
                        channel.get("workflow") or "",
                        channel.get("blocker") or "",
                    ]
                )
            )
    return "\n".join(lines)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=SURFACES_PATH)
    parser.add_argument("--version", help="Target release version, for example 0.8.0-alpha.3")
    parser.add_argument("--probe", action="store_true", help="Best-effort network/tool probes for the target version.")
    parser.add_argument("--view", choices=["maintainer", "public"], default="maintainer")
    parser.add_argument("--format", choices=["table", "json"], default="table")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.probe and not args.version:
        print("error: --probe requires --version", file=sys.stderr)
        return 2
    try:
        data = load_contract(args.contract)
        rows = build_rows(data, version=args.version, probe=args.probe)
    except SurfaceError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.format == "json":
        payload = {
            "schema_version": data["schema_version"],
            "version": args.version,
            "release_kind": release_kind(args.version),
            "view": args.view,
            "surfaces": rows,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if args.view == "public":
        print(render_public(rows))
    else:
        print(render_maintainer(rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
