"""Freeze a Cargo package and send its exact crates.io publish payload."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import http.client
import json
import os
from pathlib import Path
import struct
import tarfile
import tempfile
import tomllib

from scripts.crates_io_auth import USER_AGENT
from tools.publish import run_command


class CrateArchiveError(RuntimeError):
    """The frozen archive cannot be used for publication."""


def publish_metadata(repo_root: Path, archive: Path, name: str, version: str) -> bytes:
    with tempfile.TemporaryDirectory(prefix="merman-crate-metadata-") as temporary:
        root = Path(temporary)
        with tarfile.open(archive, "r:gz") as crate:
            crate.extractall(root, filter="data")
        package_root = root / f"{name}-{version}"
        manifest = package_root / "Cargo.toml"
        completed = run_command(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps",
             "--format-version", "1", "--manifest-path", str(manifest)],
            cwd=repo_root, capture=True, quiet=True,
        )
        if completed.returncode != 0:
            raise CrateArchiveError(f"Cargo could not read {archive.name}: {completed.stderr.strip()}")
        [package] = json.loads(completed.stdout)["packages"]
        if (package["name"], package["version"]) != (name, version):
            raise CrateArchiveError(f"normalized package identity differs from {name} {version}")

        readme_file = package["readme"]
        readme = None
        if readme_file is not None:
            readme_path = (package_root / readme_file).resolve()
            if not readme_path.is_relative_to(package_root.resolve()):
                raise CrateArchiveError(f"README is outside {archive.name}")
            readme = readme_path.read_bytes().decode("utf-8")
        normalized = tomllib.loads(manifest.read_text(encoding="utf-8"))

    dependencies = []
    for dep in package["dependencies"]:
        if dep["source"] != "registry+https://github.com/rust-lang/crates.io-index" or dep["registry"]:
            raise CrateArchiveError(f"normalized dependency {dep['name']} is outside crates.io")
        dependency = {
            "name": dep["name"],
            "version_req": dep["req"],
            "features": dep["features"],
            "optional": dep["optional"],
            "default_features": dep["uses_default_features"],
            "target": dep["target"],
            "kind": dep["kind"] or "normal",
        }
        if dep["rename"] is not None:
            dependency["explicit_name_in_toml"] = dep["rename"]
        dependencies.append(dependency)
    metadata = {
        "name": name, "vers": version, "deps": dependencies,
        "readme": readme, "readme_file": readme_file, "badges": normalized.get("badges", {}),
        "features": normalized.get("features", {}),
        **{key: package[key] for key in (
            "authors", "description", "documentation", "homepage", "keywords",
            "categories", "license", "license_file", "repository", "links", "rust_version",
        )},
    }
    return json.dumps(metadata, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _read_member(directory: Path, name: str) -> bytes:
    path = directory / name
    if path.is_symlink() or not path.is_file():
        raise CrateArchiveError(f"frozen candidate is missing a regular {name}")
    return path.read_bytes()


@dataclass(frozen=True)
class FrozenCrate:
    receipt_bytes: bytes
    archive: bytes
    metadata: bytes

    @property
    def archive_sha256(self) -> str:
        return _digest(self.archive)

    @property
    def receipt_sha256(self) -> str:
        return _digest(self.receipt_bytes)


def freeze_candidate(
    directory: Path, archive: Path, metadata: bytes, *, archive_sha256: str,
    name: str, version: str, source: dict, publication: dict | None,
) -> FrozenCrate:
    archive_bytes = _read_member(archive.parent, archive.name)
    if _digest(archive_bytes) != archive_sha256:
        raise CrateArchiveError("archive changed before freezing the independent candidate")
    receipt = {
        "schema_version": 1,
        "name": name, "version": version, "source": source, "publication": publication,
        "archive_sha256": archive_sha256, "metadata_sha256": _digest(metadata),
    }
    receipt_bytes = (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode("utf-8")
    directory.mkdir(parents=True, exist_ok=False)
    (directory / f"{name}-{version}.crate").write_bytes(archive_bytes)
    (directory / "publish-metadata.json").write_bytes(metadata)
    (directory / "receipt.json").write_bytes(receipt_bytes)
    return FrozenCrate(receipt_bytes, archive_bytes, metadata)


def load_candidate(
    directory: Path, *, name: str, version: str, source: dict, publication: dict | None,
    receipt_sha256: str | None = None, archive_sha256: str | None = None,
) -> FrozenCrate:
    receipt_bytes = _read_member(directory, "receipt.json")
    if receipt_sha256 is not None and _digest(receipt_bytes) != receipt_sha256:
        raise CrateArchiveError("frozen candidate receipt digest differs from preflight")
    receipt = json.loads(receipt_bytes)
    identity = {
        "schema_version": 1, "name": name, "version": version, "source": source,
        "publication": publication,
    }
    if (not isinstance(receipt, dict) or type(receipt.get("schema_version")) is not int
            or any(receipt.get(key) != expected for key, expected in identity.items())
            or (publication is not None and type(receipt["publication"]["attempt"]) is not int)):
        raise CrateArchiveError("frozen candidate identity differs from its producing source or attempt")
    archive = _read_member(directory, f"{name}-{version}.crate")
    metadata = _read_member(directory, "publish-metadata.json")
    digest = _digest(archive)
    if digest != receipt.get("archive_sha256") or (archive_sha256 is not None and digest != archive_sha256):
        raise CrateArchiveError("frozen crate archive digest differs from preflight")
    if _digest(metadata) != receipt.get("metadata_sha256"):
        raise CrateArchiveError("frozen publish metadata digest differs from preflight")
    payload = json.loads(metadata)
    if not isinstance(payload, dict) or (payload.get("name"), payload.get("vers")) != (name, version):
        raise CrateArchiveError("frozen publish metadata identifies a different package or version")
    return FrozenCrate(receipt_bytes, archive, metadata)


def upload(candidate: FrozenCrate, *, token: str, intent_path: Path) -> None:
    if not token or not token.isascii() or not token.isprintable() or any(char.isspace() for char in token):
        raise CrateArchiveError("invalid crates.io upload token")
    # Cargo's Registry Web API frames each original byte string with its u32 length.
    body = (struct.pack("<I", len(candidate.metadata)) + candidate.metadata
            + struct.pack("<I", len(candidate.archive)) + candidate.archive)
    try:
        with intent_path.open("xb") as intent:
            intent.write(candidate.receipt_bytes)
            intent.flush()
            os.fsync(intent.fileno())
    except FileExistsError:
        raise CrateArchiveError("this candidate has an upload intent; observe registry propagation") from None
    connection = http.client.HTTPSConnection("crates.io", timeout=60)
    try:
        connection.request("PUT", "/api/v1/crates/new", body, {
            "Authorization": token,
            "User-Agent": USER_AGENT,
            "Content-Type": "application/octet-stream",
            "Accept": "application/json",
        })
        with connection.getresponse() as response:
            if not 200 <= response.status < 300:
                raise CrateArchiveError(
                    f"crate upload returned HTTP {response.status}; observe the frozen candidate"
                )
    except (OSError, http.client.HTTPException):
        raise CrateArchiveError("crate upload response is uncertain; observe the frozen candidate") from None
    finally:
        connection.close()
