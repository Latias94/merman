"""Narrow owner-local editors for non-package-manager release versions."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

try:
    from scripts.release_version import ReleaseVersion
except ModuleNotFoundError:
    from release_version import ReleaseVersion


PYTHON_MANIFEST = Path("platforms/python/merman/pyproject.toml")
ANDROID_MANIFEST = Path("platforms/android/build.gradle.kts")
FLUTTER_MANIFEST = Path("platforms/flutter/pubspec.yaml")
FLUTTER_PACKAGE_VERSION = Path(
    "platforms/flutter/lib/src/generated/package_version.dart"
)


class ReleaseOwnerError(ValueError):
    """An owner-local release version surface cannot be updated safely."""


def prepare_python_version(root: Path, release: ReleaseVersion) -> None:
    """Update the Python project version using its PEP 440 projection."""

    text = _read(root, PYTHON_MANIFEST)
    _write_if_changed(
        root,
        PYTHON_MANIFEST,
        _replace_python_project_version(text, release.to_pep440()),
    )


def prepare_android_version(root: Path, release: ReleaseVersion) -> None:
    """Update the Android package's single top-level Gradle version."""

    text = _read(root, ANDROID_MANIFEST)
    _write_if_changed(
        root,
        ANDROID_MANIFEST,
        _replace_assignment(
            text,
            r'^(version\s*=\s*")[^"]+("\s*)$',
            release.canonical,
            ANDROID_MANIFEST,
            "Android version",
        ),
    )


def prepare_flutter_version(root: Path, release: ReleaseVersion) -> None:
    """Update the Flutter package and bundled native contract version."""

    assignments = (
        (FLUTTER_MANIFEST, r"^(version:\s*)[^\s#]+(\s*)$", "Flutter version"),
        (
            FLUTTER_PACKAGE_VERSION,
            r"^(const String mermanPackageVersion = ')[^']+(';\s*)$",
            "Flutter bundled native package version",
        ),
    )
    for path, pattern, label in assignments:
        _write_if_changed(
            root,
            path,
            _replace_assignment(
                _read(root, path), pattern, release.canonical, path, label
            ),
        )


def _read(root: Path, path: Path) -> str:
    return (root / path).read_text(encoding="utf-8")


def _write_if_changed(root: Path, path: Path, content: str) -> None:
    target = root / path
    if target.read_text(encoding="utf-8") != content:
        target.write_text(content, encoding="utf-8")


def _replace_python_project_version(text: str, version: str) -> str:
    lines = text.splitlines(keepends=True)
    in_project = False
    matches: list[int] = []
    assignment = re.compile(
        r'^(\s*version\s*=\s*)"[^"]*"(\s*(?:#.*)?(?:\r?\n)?)$'
    )
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_project = stripped == "[project]"
            continue
        if in_project and assignment.match(line):
            matches.append(index)
    if len(matches) != 1:
        raise ReleaseOwnerError(
            "expected one project.version string assignment in "
            f"{PYTHON_MANIFEST}; found {len(matches)}"
        )

    index = matches[0]
    lines[index] = assignment.sub(rf'\g<1>"{version}"\g<2>', lines[index])
    candidate = "".join(lines)
    try:
        actual = tomllib.loads(candidate)["project"]["version"]
    except (KeyError, TypeError, tomllib.TOMLDecodeError) as exc:
        raise ReleaseOwnerError(
            f"failed to structurally update {PYTHON_MANIFEST} project.version: {exc}"
        ) from exc
    if actual != version:
        raise ReleaseOwnerError(
            f"failed to update {PYTHON_MANIFEST} project.version"
        )
    return candidate


def _replace_assignment(
    text: str,
    pattern: str,
    version: str,
    path: Path,
    label: str,
    *,
    flags: int = re.MULTILINE,
) -> str:
    candidate, count = re.subn(
        pattern,
        lambda match: f"{match.group(1)}{version}{match.group(2)}",
        text,
        count=1,
        flags=flags,
    )
    if count != 1:
        raise ReleaseOwnerError(
            f"{path} must contain exactly one {label}; found {count}"
        )
    return candidate
