"""Policy-free helpers for strict JSON contracts and canonical JSON digests."""

from __future__ import annotations

from collections.abc import Callable, Collection
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
from typing import Any, TypeAlias


ErrorFactory: TypeAlias = Callable[[str], Exception]


class _DuplicateJsonKeyError(ValueError):
    def __init__(self, key: str) -> None:
        super().__init__(key)
        self.key = key


@dataclass(frozen=True)
class StrictJsonContract:
    error_factory: ErrorFactory
    read_error_prefix: str
    include_path_in_duplicate_key: bool = True

    def load(self, path: Path) -> Any:
        """Load UTF-8 JSON while rejecting duplicate object keys at every depth."""

        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise self.error_factory(
                f"{self.read_error_prefix} {path}: {error}"
            ) from error
        return self.loads(source, path=path)

    def load_bytes(self, path: Path) -> tuple[bytes, Any]:
        """Read and parse one regular-file byte snapshot without following symlinks."""
        raw = read_regular_file_bytes(
            path,
            error_factory=self.error_factory,
            read_error_prefix=self.read_error_prefix,
        )
        return raw, self.loads(raw, path=path)

    def loads(self, source: str | bytes, *, path: Path) -> Any:
        """Parse one already captured JSON value with duplicate-key rejection."""

        def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
            result: dict[str, Any] = {}
            for key, value in pairs:
                if key in result:
                    raise _DuplicateJsonKeyError(key)
                result[key] = value
            return result

        try:
            return json.loads(
                source,
                object_pairs_hook=reject_duplicate_keys,
            )
        except _DuplicateJsonKeyError as error:
            location = f" in {path}" if self.include_path_in_duplicate_key else ""
            raise self.error_factory(
                f"duplicate JSON key{location}: {error.key}"
            ) from None
        except (UnicodeError, json.JSONDecodeError) as error:
            raise self.error_factory(
                f"{self.read_error_prefix} {path}: {error}"
            ) from error

    def object(self, value: Any, context: str) -> dict[str, Any]:
        if not isinstance(value, dict):
            raise self.error_factory(f"{context} must be an object")
        return value

    def array(self, value: Any, context: str) -> list[Any]:
        if not isinstance(value, list):
            raise self.error_factory(f"{context} must be an array")
        return value

    def exact_fields(
        self,
        value: dict[str, Any],
        required: Collection[str],
        context: str,
        *,
        optional: Collection[str] = (),
    ) -> None:
        required_fields = set(required)
        optional_fields = set(optional)
        missing = required_fields - value.keys()
        unknown = value.keys() - required_fields - optional_fields
        if missing:
            raise self.error_factory(
                f"{context} is missing fields: {', '.join(sorted(missing))}"
            )
        if unknown:
            raise self.error_factory(
                f"{context} has unknown fields: {', '.join(sorted(unknown))}"
            )

    def string(self, value: Any, context: str) -> str:
        if not isinstance(value, str) or not value or value.strip() != value:
            raise self.error_factory(
                f"{context} must be a non-empty, trimmed string"
            )
        return value


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def bytes_sha256(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def read_regular_file_bytes(
    path: Path,
    *,
    error_factory: Callable[[str], Exception],
    read_error_prefix: str,
) -> bytes:
    """Read one non-symlink regular file through a single descriptor snapshot."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode):
                raise OSError("not a regular file")
            with os.fdopen(descriptor, "rb", closefd=False) as source:
                return source.read()
        finally:
            os.close(descriptor)
    except OSError as error:
        raise error_factory(f"{read_error_prefix} {path}: {error}") from error
