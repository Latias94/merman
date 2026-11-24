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


FORKS = ("merman-krilla", "merman-krilla-svg", "merman-resvg", "merman-usvg")
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


def _manifest(versions: dict[str, str], candidate: str, candidate_path: Path) -> str:
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
    for name in FORKS:
        alias = name.removeprefix("merman-")
        lines.append(
            f'{alias} = {{ package = "{name}", version = "={versions[name]}", default-features = false }}'
        )
    lines.extend([
        "",
        "[patch.crates-io]",
        f'{candidate} = {{ path = {toml_string(str(candidate_path))} }}',
    ])
    return "\n".join(lines) + "\n"


def _verify_resolution(
    metadata: dict, versions: dict[str, str], candidate: str, candidate_path: Path
) -> None:
    resolved_ids = {node["id"] for node in metadata["resolve"]["nodes"]}
    for name in FORKS:
        selected = [
            package for package in metadata["packages"]
            if package["name"] == name and package["id"] in resolved_ids
        ]
        expected_source = None if name == candidate else CRATES_IO_SOURCE
        valid = (
            len(selected) == 1
            and selected[0]["version"] == versions[name]
            and selected[0]["source"] == expected_source
        )
        if valid and name == candidate:
            valid = Path(selected[0]["manifest_path"]).resolve() == candidate_path / "Cargo.toml"
        if not valid:
            expected = "the extracted candidate" if name == candidate else "crates.io"
            observed = [(package["version"], package["source"]) for package in selected]
            raise RegistryDependentError(
                f"expected exactly one {name} {versions[name]} from {expected}; resolved {observed}"
            )


def verify(
    archive: Path,
    *,
    candidate: str,
    versions: dict[str, str],
    repo_root: Path,
    target_directory: Path,
    run_check=run_cargo_check,
) -> None:
    spec = PackageSpec(candidate, versions[candidate])
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
        (project / "Cargo.toml").write_text(_manifest(versions, candidate, candidate_path), encoding="utf-8")
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
        _verify_resolution(json.loads(resolved.stdout), versions, candidate, candidate_path)

        checked = run_check(["cargo", "check", "--locked"], cwd=project, env=env)
        if checked.returncode:
            detail = (checked.stderr or checked.stdout).strip()
            raise RegistryDependentError(f"fork consumer failed with {archive.name}: {detail}")

    siblings = ", ".join(f"{name} {versions[name]}" for name in FORKS if name != candidate)
    print(f"Compiled {archive.name} with registry siblings {siblings}")
