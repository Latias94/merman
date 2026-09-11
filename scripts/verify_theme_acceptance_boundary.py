#!/usr/bin/env python3
"""Verify package exclusions and compile an independent consumer of production crates."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_MODULES = {
    "merman": {"src/theme_acceptance.rs"},
    "merman-render": {"src/svg_artifact_receipts.rs", "src/theme_raster_paint.rs"},
    "merman-export": {"src/raster_paint_cutover.rs"},
}
PRIVATE_IMPORTS = (
    "merman::__theme_acceptance::TargetArtifactView",
    "merman_render::__private::ThemeRouteCutoverReceipt",
    "merman_render::__private::ThemeRasterPaintBindingReceipt",
    "merman_export::RasterPaintCutoverReceipt",
)


def main() -> None:
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=ROOT, text=True,
    ))
    packages = {package["name"]: package for package in metadata["packages"]}
    for name, excluded in EXCLUDED_MODULES.items():
        if "internal-theme-acceptance" in packages[name]["features"]:
            raise RuntimeError(f"{name} exposes the retired acceptance Cargo feature")
        listing = subprocess.check_output(
            ["cargo", "package", "--list", "--allow-dirty", "--offline", "-p", name],
            cwd=ROOT, text=True,
        ).splitlines()
        leaked = excluded.intersection(path.replace("\\", "/") for path in listing)
        if leaked:
            raise RuntimeError(f"{name} packages private acceptance modules: {sorted(leaked)}")
        print(f"{name}: private feature and acceptance modules excluded", flush=True)

    env = dict(os.environ)
    if any("merman_internal_theme_acceptance" in env.get(key, "") for key in
           ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")):
        raise RuntimeError("Run this production consumer check without the acceptance cfg")
    env.setdefault("CARGO_TARGET_DIR", metadata["target_directory"])
    env.setdefault("CARGO_BUILD_JOBS", "2")
    with tempfile.TemporaryDirectory(prefix="merman-theme-public-consumer-") as temporary:
        project = Path(temporary)
        dependencies = []
        for name in EXCLUDED_MODULES:
            features = sorted(packages[name]["features"])
            path = str(Path(packages[name]["manifest_path"]).parent)
            dependencies.append(
                f'{name} = {{ path = {json.dumps(path)}, default-features = false, '
                f'features = {json.dumps(features)} }}'
            )
        (project / "Cargo.toml").write_text(
            '[package]\nname = "theme-public-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
            '[workspace]\n[dependencies]\n' + "\n".join(dependencies) + "\n", encoding="utf-8",
        )
        source = project / "src" / "main.rs"
        source.parent.mkdir()
        source.write_text(
            'use merman::RenderedDocument;\nuse merman_render::svg::StandaloneSvgArtifact;\n'
            'use merman_export::RasterOptions;\nfn main() {\n'
            'let _ = core::mem::size_of::<(RenderedDocument, StandaloneSvgArtifact, RasterOptions)>();\n}\n',
            encoding="utf-8",
        )
        command = ["cargo", "check", "--offline", "--quiet"]
        production = subprocess.run(command, cwd=project, env=env, capture_output=True, text=True)
        if production.returncode != 0:
            raise RuntimeError(f"Production consumer failed to compile:\n{production.stderr}")
        print("external consumer resolves production APIs with every producer feature enabled", flush=True)
        for private_import in PRIVATE_IMPORTS:
            source.write_text(f"use {private_import};\nfn main() {{}}\n", encoding="utf-8")
            result = subprocess.run(command, cwd=project, env=env, capture_output=True, text=True)
            if result.returncode == 0 or "error[E0432]" not in result.stderr:
                raise RuntimeError(
                    f"Expected an unresolved private import for {private_import}:\n{result.stderr}"
                )
            print(f"external consumer cannot import {private_import}", flush=True)


if __name__ == "__main__":
    main()
