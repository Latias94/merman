"""Canonical release-version parsing and registry projections."""

from __future__ import annotations

import re
from dataclasses import dataclass


_RELEASE_VERSION = re.compile(
    r"^(?:v)?"
    r"(?P<major>0|[1-9][0-9]*)\."
    r"(?P<minor>0|[1-9][0-9]*)\."
    r"(?P<patch>0|[1-9][0-9]*)"
    r"(?:-(?P<channel>alpha|beta|rc)\.(?P<number>0|[1-9][0-9]*))?"
    r"(?:\+(?P<build>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


@dataclass(frozen=True)
class ReleaseVersion:
    """A release version accepted by every Merman publishing workflow."""

    major: int
    minor: int
    patch: int
    prerelease_channel: str | None = None
    prerelease_number: int | None = None
    build_metadata: str | None = None

    @property
    def base(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"

    @property
    def canonical(self) -> str:
        version = self.base
        if self.prerelease_channel is not None:
            version += f"-{self.prerelease_channel}.{self.prerelease_number}"
        if self.build_metadata is not None:
            version += f"+{self.build_metadata}"
        return version

    @property
    def tag(self) -> str:
        return f"v{self.canonical}"

    @property
    def kind(self) -> str:
        return "prerelease" if self.prerelease_channel is not None else "stable"

    @property
    def channel(self) -> str:
        return self.prerelease_channel or "stable"

    def to_pep440(self) -> str:
        version = self.base
        if self.prerelease_channel is not None:
            label = {"alpha": "a", "beta": "b", "rc": "rc"}[self.prerelease_channel]
            version += f"{label}{self.prerelease_number}"
        if self.build_metadata is not None:
            local = re.sub(r"[-_]+", ".", self.build_metadata).lower()
            version += f"+{local}"
        return version

    def to_vscode_manifest(self) -> str:
        return self.base


def parse_release_version(value: str, *, allow_v_prefix: bool = True) -> ReleaseVersion:
    if not isinstance(value, str) or not value:
        raise ValueError("release version must be a non-empty string")
    if not allow_v_prefix and value.startswith("v"):
        raise ValueError(f"release version must not use a v prefix: {value!r}")

    match = _RELEASE_VERSION.fullmatch(value)
    if match is None:
        raise ValueError(
            "unsupported release version "
            f"{value!r}; expected X.Y.Z, optionally followed by -alpha.N, -beta.N, "
            "or -rc.N and SemVer build metadata"
        )

    channel = match.group("channel")
    number = match.group("number")
    return ReleaseVersion(
        major=int(match.group("major")),
        minor=int(match.group("minor")),
        patch=int(match.group("patch")),
        prerelease_channel=channel,
        prerelease_number=int(number) if number is not None else None,
        build_metadata=match.group("build"),
    )
