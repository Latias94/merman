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
FLUTTER_ANDROID_MANIFEST = Path("platforms/flutter/android/build.gradle")
FLUTTER_IOS_PODSPEC = Path("platforms/flutter/ios/merman.podspec")
FLUTTER_MACOS_PODSPEC = Path("platforms/flutter/macos/merman.podspec")
FLUTTER_IOS_BUILD = Path("platforms/flutter/build-ios.sh")
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
    """Update the Flutter owner's Dart, Gradle, CocoaPods, and plist surfaces."""

    updates = {
        FLUTTER_MANIFEST: _replace_assignment(
            _read(root, FLUTTER_MANIFEST),
            r"^(version:\s*)[^\s#]+(\s*)$",
            release.canonical,
            FLUTTER_MANIFEST,
            "Flutter version",
        ),
        FLUTTER_PACKAGE_VERSION: _replace_assignment(
            _read(root, FLUTTER_PACKAGE_VERSION),
            r"^(const String mermanPackageVersion = ')[^']+(';\s*)$",
            release.canonical,
            FLUTTER_PACKAGE_VERSION,
            "Flutter bundled native package version",
        ),
        FLUTTER_ANDROID_MANIFEST: _replace_assignment(
            _read(root, FLUTTER_ANDROID_MANIFEST),
            r"^(version\s*=\s*')[^']+('\s*)$",
            release.canonical,
            FLUTTER_ANDROID_MANIFEST,
            "Flutter Android version",
        ),
        FLUTTER_IOS_PODSPEC: _replace_assignment(
            _read(root, FLUTTER_IOS_PODSPEC),
            r"^(\s*s\.version\s*=\s*')[^']+('\s*)$",
            release.canonical,
            FLUTTER_IOS_PODSPEC,
            "Flutter iOS Podspec version",
        ),
        FLUTTER_MACOS_PODSPEC: _replace_assignment(
            _read(root, FLUTTER_MACOS_PODSPEC),
            r"^(\s*s\.version\s*=\s*')[^']+('\s*)$",
            release.canonical,
            FLUTTER_MACOS_PODSPEC,
            "Flutter macOS Podspec version",
        ),
    }

    build_text = _read(root, FLUTTER_IOS_BUILD)
    for plist_key in ("CFBundleShortVersionString", "CFBundleVersion"):
        build_text = _replace_assignment(
            build_text,
            rf"(<key>{plist_key}</key>\s*<string>)[^<]+(</string>)",
            release.base,
            FLUTTER_IOS_BUILD,
            plist_key,
            flags=0,
        )
    updates[FLUTTER_IOS_BUILD] = build_text

    for path, content in updates.items():
        _write_if_changed(root, path, content)


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
    expression = re.compile(pattern, flags=flags)
    matches = list(expression.finditer(text))
    if len(matches) != 1:
        raise ReleaseOwnerError(
            f"{path} must contain exactly one {label}; found {len(matches)}"
        )
    return expression.sub(
        lambda match: f"{match.group(1)}{version}{match.group(2)}",
        text,
        count=1,
    )
