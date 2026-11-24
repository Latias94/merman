"""Compile the SVG and PDF fork interfaces with one packaged candidate."""

from __future__ import annotations

import json
import os
from pathlib import Path
import tarfile
import tempfile
import tomllib

from scripts.release_registry_dependents import (
    PackageSpec,
    RegistryDependentError,
    run_cargo_check,
    toml_string,
    validate_candidate,
)


# Compatibility floors for the published series, documented in RELEASING.md.
FORK_REQUIREMENTS = {
    "merman-krilla": "^0.8.2",
    "merman-krilla-svg": "^0.8.1",
    "merman-resvg": "^0.50.0",
    "merman-usvg": "^0.50.0",
}
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
CONSUMER = '''\
use krilla_svg::SurfaceExt;
use resvg::tiny_skia::{Pixmap, Transform};

fn main() {
    let tree = usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
            <rect width="16" height="16"/>
        </svg>"#,
        &usvg::Options::default(),
    )
    .expect("valid smoke SVG");
    let mut pixmap = Pixmap::new(16, 16).expect("valid raster size");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());

    let mut document = krilla::Document::new();
    let size = krilla::geom::Size::from_wh(16.0, 16.0).expect("valid page size");
    let mut page = document.start_page_with(krilla::page::PageSettings::new(size));
    let mut surface = page.surface();
    surface
        .draw_svg(&tree, size, krilla_svg::SvgSettings::default())
        .expect("SVG drawn on the PDF surface");
    surface.finish();
    page.finish();
    document.finish().expect("valid smoke PDF");
}
'''


def _manifest(candidate: PackageSpec, candidate_path: Path) -> str:
    lines = [
        "[workspace]",
        "[package]",
        'name = "merman-renderer-fork-consumer"',
        'version = "0.0.0"',
        'edition = "2024"',
        "publish = false",
        "",
        "[dependencies]",
    ]
    for name, registry_requirement in FORK_REQUIREMENTS.items():
        alias = name.removeprefix("merman-")
        requirement = f"={candidate.version}" if name == candidate.name else registry_requirement
        lines.append(
            f'{alias} = {{ package = "{name}", version = "{requirement}", default-features = false }}'
        )
    lines.extend([
        "",
        "[patch.crates-io]",
        f'{candidate.name} = {{ path = {toml_string(str(candidate_path))} }}',
    ])
    return "\n".join(lines) + "\n"


def _verify_resolution(
    metadata: dict, candidate: PackageSpec, candidate_path: Path
) -> dict[str, str]:
    # Cargo enforces the manifest requirements. Check the identity and source of each fork.
    resolved_ids = {node["id"] for node in metadata["resolve"]["nodes"]}
    resolved_versions = {}
    for name, requirement in FORK_REQUIREMENTS.items():
        selected = [
            package for package in metadata["packages"]
            if package["name"] == name and package["id"] in resolved_ids
        ]
        is_candidate = name == candidate.name
        expected_source = None if is_candidate else CRATES_IO_SOURCE
        valid = len(selected) == 1 and selected[0]["source"] == expected_source
        if valid and is_candidate:
            valid = (
                selected[0]["version"] == candidate.version
                and Path(selected[0]["manifest_path"]).resolve() == candidate_path / "Cargo.toml"
            )
        if not valid:
            expected = (
                f"{candidate.version} from the extracted candidate" if is_candidate
                else f"matching {requirement} from crates.io"
            )
            observed = [(package["version"], package["source"]) for package in selected]
            raise RegistryDependentError(
                f"expected exactly one {name} {expected}; resolved {observed}"
            )
        resolved_versions[name] = selected[0]["version"]
    return resolved_versions


def verify(
    archive: Path,
    *,
    candidate: str,
    version: str,
    repo_root: Path,
    target_directory: Path,
    run_check=run_cargo_check,
) -> None:
    spec = PackageSpec(candidate, version)
    toolchain = tomllib.loads((repo_root / "rust-toolchain.toml").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="merman-renderer-fork-") as temporary:
        root = Path(temporary)
        unpacked = root / "unpacked"
        try:
            with tarfile.open(archive, "r:gz") as package:
                package.extractall(unpacked, filter="data")
        except tarfile.TarError as error:
            raise RegistryDependentError(f"cannot extract candidate {archive.name}: {error}") from error
        candidate_path = validate_candidate(unpacked / f"{spec.name}-{spec.version}", spec)

        project = root / "consumer"
        (project / "src").mkdir(parents=True)
        (project / "src/main.rs").write_text(CONSUMER, encoding="utf-8")
        (project / "Cargo.toml").write_text(_manifest(spec, candidate_path), encoding="utf-8")
        # The consumer lives outside the checkout, so select the repository toolchain explicitly.
        env = {
            **os.environ,
            "RUSTUP_TOOLCHAIN": toolchain["toolchain"]["channel"],
            "CARGO_TARGET_DIR": str(target_directory.resolve()),
            "CARGO_TERM_COLOR": "never",
        }

        resolved = run_check(["cargo", "metadata", "--format-version", "1"], cwd=project, env=env)
        if resolved.returncode:
            detail = (resolved.stderr or resolved.stdout).strip()
            raise RegistryDependentError(f"could not resolve registry siblings for {archive.name}: {detail}")
        resolved_versions = _verify_resolution(json.loads(resolved.stdout), spec, candidate_path)

        checked = run_check(["cargo", "check", "--locked"], cwd=project, env=env)
        if checked.returncode:
            detail = (checked.stderr or checked.stdout).strip()
            raise RegistryDependentError(f"fork consumer failed with {archive.name}: {detail}")

    siblings = ", ".join(
        f"{name} {resolved_version}"
        for name, resolved_version in resolved_versions.items() if name != candidate
    )
    print(f"Compiled {archive.name} with registry siblings {siblings}")
