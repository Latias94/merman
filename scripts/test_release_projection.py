#!/usr/bin/env python3
"""Unit tests for release version projections."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import release_projection


VERSION_SCRIPT_PATH = Path(__file__).with_name("release-version.py")
VERSION_SCRIPT_SPEC = importlib.util.spec_from_file_location(
    "release_version_script",
    VERSION_SCRIPT_PATH,
)
assert VERSION_SCRIPT_SPEC is not None
release_version_script = importlib.util.module_from_spec(VERSION_SCRIPT_SPEC)
assert VERSION_SCRIPT_SPEC.loader is not None
VERSION_SCRIPT_SPEC.loader.exec_module(release_version_script)

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


def replace_once(text: str, old: str, new: str) -> str:
    return replace_nth(text, old, new, 0)


def replace_nth(text: str, old: str, new: str, occurrence: int) -> str:
    parts = text.split(old)
    if len(parts) <= occurrence + 1:
        raise AssertionError(f"test fixture does not contain occurrence {occurrence} of {old!r}")
    return old.join(parts[: occurrence + 1]) + new + old.join(parts[occurrence + 1 :])


def web_package_entries() -> list[dict]:
    descriptor_path = REPOSITORY_ROOT / release_projection.WEB_DESCRIPTOR
    descriptor = json.loads(descriptor_path.read_text(encoding="utf-8"))
    packages = descriptor.get("packages")
    if not isinstance(packages, list):
        raise AssertionError("Web package descriptor must declare packages")
    return packages


def web_package_manifest(entry: dict) -> Path:
    return release_projection.WEB_DESCRIPTOR.parent / entry["package_dir"] / "package.json"


def node_package_surface() -> dict:
    descriptor_path = REPOSITORY_ROOT / release_projection.NODE_DESCRIPTOR
    return json.loads(descriptor_path.read_text(encoding="utf-8"))


def node_package_manifests(descriptor: dict | None = None) -> list[Path]:
    surface = descriptor or node_package_surface()
    entries = [surface["root"], *surface["targets"]]
    return [
        release_projection.NODE_ROOT / entry["directory"] / "package.json"
        for entry in entries
    ]


def replace_json_path(text: str, path: tuple[str, ...], value: object) -> str:
    data = json.loads(text)
    target = data
    for component in path[:-1]:
        target = target[component]
    target[path[-1]] = value
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


class RepositoryViewTests(unittest.TestCase):
    def test_parsed_documents_are_cached_without_exposing_cached_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["crates/merman"]\n',
                encoding="utf-8",
            )
            (root / "surface.json").write_text(
                '{"package": {"name": "merman"}}\n',
                encoding="utf-8",
            )
            view = release_projection.RepositoryView(root)

            with mock.patch.object(
                release_projection.tomllib,
                "loads",
                wraps=release_projection.tomllib.loads,
            ) as toml_loads, mock.patch.object(
                release_projection.json,
                "loads",
                wraps=release_projection.json.loads,
            ) as json_loads:
                first_toml = view.toml("Cargo.toml")
                first_json = view.json("surface.json")
                first_toml["workspace"]["members"].append("mutated")
                first_json["package"]["name"] = "mutated"

                second_toml = view.toml("./Cargo.toml")
                second_json = view.json("./surface.json")

            self.assertEqual(toml_loads.call_count, 1)
            self.assertEqual(json_loads.call_count, 1)
            self.assertEqual(
                second_toml["workspace"]["members"],
                ["crates/merman"],
            )
            self.assertEqual(second_json["package"]["name"], "merman")


class ReleaseProjectionTests(unittest.TestCase):
    ROOT = Path(__file__).resolve().parents[1]

    def test_no_argument_verifier_covers_the_complete_current_projection(self) -> None:
        result = release_projection.verify_repository(self.ROOT)

        self.assertTrue(result.ok)
        labels = {observation.label for observation in result.observations}
        self.assertIn("Cargo workspace dependency merman-core", labels)
        self.assertIn("Cargo.lock package merman-lsp", labels)
        self.assertIn("fuzz/Cargo.lock package merman-ffi", labels)
        self.assertIn("Web workspace", labels)
        self.assertIn("Web workspace lock", labels)
        self.assertIn("Web workspace lock package", labels)
        self.assertIn("Web package @mermanjs/web", labels)
        self.assertIn("Web lock package @mermanjs/web", labels)
        self.assertIn("Playground local Web workspace lock", labels)
        self.assertIn("Playground local Web lock @mermanjs/web", labels)
        self.assertIn("Playground license lock digest", labels)
        self.assertIn("Node candidate Cargo package", labels)
        self.assertIn("Node candidate Cargo.lock package merman-bindings-core", labels)
        self.assertIn("Node candidate package surface", labels)
        self.assertIn("Node candidate workspace", labels)
        self.assertIn("Node candidate workspace lock", labels)
        self.assertIn("Node candidate package @mermanjs/node", labels)
        self.assertIn(
            "Node candidate package @mermanjs/node-darwin-arm64",
            labels,
        )
        self.assertIn("Python package", labels)
        self.assertIn("Flutter bundled native package version", labels)
        self.assertIn("Flutter Android package", labels)
        self.assertIn("Flutter iOS Podspec", labels)
        self.assertIn("Flutter macOS Podspec", labels)
        self.assertIn("Flutter iOS framework bundle version", labels)

    def test_cli_without_arguments_runs_the_authority_verifier(self) -> None:
        authority = release_projection.verify_repository(self.ROOT).authority.canonical
        stdout = io.StringIO()
        with mock.patch.object(sys, "argv", ["release-version.py"]), contextlib.redirect_stdout(
            stdout
        ):
            exit_code = release_version_script.main()

        self.assertEqual(exit_code, 0)
        self.assertIn(f"Cargo workspace authority: {authority}", stdout.getvalue())

    def test_every_release_projection_category_fails_closed_on_drift(self) -> None:
        version = release_projection.verify_repository(self.ROOT).authority
        canonical = version.canonical
        entries = web_package_entries()
        default_entry = next(entry for entry in entries if entry["id"] == "full")
        playground_lock_digest = hashlib.sha256(
            (self.ROOT / release_projection.PLAYGROUND_LOCK).read_text(encoding="utf-8").encode("utf-8")
        ).hexdigest()
        mutations = [
            (
                Path("Cargo.toml"),
                lambda text: replace_once(
                    text,
                    f'version = "{canonical}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                Path("Cargo.toml"),
                lambda text: replace_once(
                    text,
                    f'merman-core = {{ path = "crates/merman-core", version = "{canonical}"',
                    'merman-core = { path = "crates/merman-core", version = "9.9.9"',
                ),
            ),
            (
                Path("crates/merman-bindings-core/Cargo.toml"),
                lambda text: replace_once(
                    text,
                    "version.workspace = true",
                    'version = "9.9.9"',
                ),
            ),
            (
                Path("crates/merman-bindings-core/Cargo.toml"),
                lambda text: replace_once(
                    text,
                    "merman.workspace = true",
                    'merman = { path = "../merman", version = "9.9.9", '
                    "default-features = false }",
                ),
            ),
            (
                Path("Cargo.lock"),
                lambda text: replace_once(
                    text,
                    f'name = "merman"\nversion = "{canonical}"',
                    'name = "merman"\nversion = "9.9.9"',
                ),
            ),
            (
                release_projection.FUZZ_LOCK,
                lambda text: replace_once(
                    text,
                    f'name = "merman"\nversion = "{canonical}"',
                    'name = "merman"\nversion = "9.9.9"',
                ),
            ),
            (
                release_projection.WEB_WORKSPACE_PACKAGE,
                lambda text: replace_once(
                    text,
                    f'"version": "{canonical}"',
                    '"version": "9.9.9"',
                ),
            ),
            (
                release_projection.WEB_LOCK,
                lambda text: replace_once(
                    text,
                    f'"version": "{canonical}"',
                    '"version": "9.9.9"',
                ),
            ),
            (
                release_projection.WEB_LOCK,
                lambda text: replace_once(
                    text,
                    (
                        '"": {\n'
                        '      "name": "merman-web-workspace",\n'
                        f'      "version": "{canonical}"'
                    ),
                    (
                        '"": {\n'
                        '      "name": "merman-web-workspace",\n'
                        '      "version": "9.9.9"'
                    ),
                ),
            ),
            *[
                (
                    web_package_manifest(entry),
                    lambda text, expected=canonical: replace_once(
                        text,
                        f'"version": "{expected}"',
                        '"version": "9.9.9"',
                    ),
                )
                for entry in entries
            ],
            *[
                (
                    release_projection.WEB_LOCK,
                    lambda text, entry=entry, expected=canonical: replace_once(
                        text,
                        (
                            f'"{entry["package_dir"]}": {{\n'
                            f'      "name": "{entry["name"]}",\n'
                            f'      "version": "{expected}"'
                        ),
                        (
                            f'"{entry["package_dir"]}": {{\n'
                            f'      "name": "{entry["name"]}",\n'
                            '      "version": "9.9.9"'
                        ),
                    ),
                )
                for entry in entries
            ],
            (
                release_projection.PLAYGROUND_LOCK,
                lambda text: replace_once(
                    text,
                    (
                        f'"../platforms/web/{default_entry["package_dir"]}": {{\n'
                        '      "name": "@mermanjs/web",\n'
                        f'      "version": "{canonical}"'
                    ),
                    (
                        f'"../platforms/web/{default_entry["package_dir"]}": {{\n'
                        '      "name": "@mermanjs/web",\n'
                        '      "version": "9.9.9"'
                    ),
                ),
            ),
            (
                release_projection.PLAYGROUND_LOCK,
                lambda text: replace_once(
                    text,
                    (
                        '"../platforms/web": {\n'
                        '      "name": "merman-web-workspace",\n'
                        f'      "version": "{canonical}"'
                    ),
                    (
                        '"../platforms/web": {\n'
                        '      "name": "merman-web-workspace",\n'
                        '      "version": "9.9.9"'
                    ),
                ),
            ),
            (
                release_projection.PLAYGROUND_LICENSE_REPORT,
                lambda text: replace_once(
                    text,
                    f"package-lock.json SHA-256: {playground_lock_digest}",
                    "package-lock.json SHA-256: " + "0" * 64,
                ),
            ),
            (
                release_projection.PYTHON_MANIFEST,
                lambda text: replace_once(
                    text,
                    f'version = "{version.to_pep440()}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                release_projection.ANDROID_MANIFEST,
                lambda text: replace_once(
                    text,
                    f'version = "{canonical}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                release_projection.FLUTTER_MANIFEST,
                lambda text: replace_once(
                    text,
                    f"version: {canonical}",
                    "version: 9.9.9",
                ),
            ),
            (
                release_projection.FLUTTER_PACKAGE_VERSION,
                lambda text: replace_once(
                    text,
                    f"const String mermanPackageVersion = '{canonical}';",
                    "const String mermanPackageVersion = '9.9.9';",
                ),
            ),
            (
                release_projection.FLUTTER_ANDROID_MANIFEST,
                lambda text: replace_once(
                    text,
                    f"version = '{canonical}'",
                    "version = '9.9.9'",
                ),
            ),
            *[
                (
                    podspec,
                    lambda text, expected=canonical: replace_once(
                        text,
                        f"s.version          = '{expected}'",
                        "s.version          = '9.9.9'",
                    ),
                )
                for podspec in (
                    release_projection.FLUTTER_IOS_PODSPEC,
                    release_projection.FLUTTER_MACOS_PODSPEC,
                )
            ],
            (
                release_projection.FLUTTER_IOS_BUILD,
                lambda text: replace_once(
                    text,
                    (
                        "<key>CFBundleShortVersionString</key>\n"
                        f"  <string>{version.base}</string>"
                    ),
                    (
                        "<key>CFBundleShortVersionString</key>\n"
                        "  <string>9.9.9</string>"
                    ),
                ),
            ),
            (
                release_projection.FLUTTER_IOS_BUILD,
                lambda text: replace_once(
                    text,
                    f"<key>CFBundleVersion</key>\n  <string>{version.base}</string>",
                    "<key>CFBundleVersion</key>\n  <string>9.9.9</string>",
                ),
            ),
        ]

        for path, mutate in mutations:
            with self.subTest(path=path):
                original = (self.ROOT / path).read_text(encoding="utf-8")
                drifted = mutate(original)
                try:
                    result = release_projection.verify_repository(
                        self.ROOT,
                        overrides={path: drifted},
                    )
                except release_projection.ReleaseProjectionError:
                    continue
                self.assertFalse(result.ok)

    def test_update_plan_projects_one_authority_without_touching_independent_axes(self) -> None:
        current = release_projection.verify_repository(self.ROOT).authority
        next_version = f"{current.major}.{current.minor + 1}.0-alpha.1"

        updates = release_projection.plan_version_update(self.ROOT, next_version)
        result = release_projection.verify_repository(
            self.ROOT,
            expected_version=next_version,
            overrides=updates,
        )

        self.assertTrue(result.ok)
        self.assertIn(Path("Cargo.toml"), updates)
        self.assertIn(release_projection.FUZZ_LOCK, updates)
        self.assertIn(release_projection.WEB_WORKSPACE_PACKAGE, updates)
        for entry in web_package_entries():
            self.assertIn(web_package_manifest(entry), updates)
        self.assertIn(release_projection.NODE_CARGO_MANIFEST, updates)
        self.assertIn(release_projection.NODE_CARGO_LOCK, updates)
        self.assertIn(release_projection.NODE_DESCRIPTOR, updates)
        self.assertIn(release_projection.NODE_WORKSPACE_PACKAGE, updates)
        self.assertIn(release_projection.NODE_WORKSPACE_LOCK, updates)
        for manifest_path in node_package_manifests():
            self.assertIn(manifest_path, updates)
        self.assertIn(release_projection.PLAYGROUND_LICENSE_REPORT, updates)
        self.assertIn(release_projection.FLUTTER_PACKAGE_VERSION, updates)
        self.assertIn(release_projection.FLUTTER_IOS_BUILD, updates)
        self.assertNotIn(Path("README.md"), updates)
        self.assertNotIn(Path("docs/rendering/RASTER_OUTPUT.md"), updates)
        self.assertFalse(any(path.suffix == ".md" for path in updates))
        self.assertNotIn(Path("tools/vscode-extension/package.json"), updates)
        self.assertNotIn(Path("packages/typst/merman/typst.toml"), updates)

    def test_node_candidate_verification_fails_closed_on_each_version_projection(
        self,
    ) -> None:
        version = release_projection.verify_repository(self.ROOT).authority.canonical
        catalog = release_projection.load_workspace_catalog(
            release_projection.RepositoryView(self.ROOT)
        )
        descriptor = node_package_surface()
        local_lock_packages = [
            package["name"]
            for package in release_projection.RepositoryView(self.ROOT)
            .toml(release_projection.NODE_CARGO_LOCK)["package"]
            if "source" not in package
            and (
                package["name"] == release_projection.NODE_CARGO_PACKAGE
                or package["name"] in catalog.coupled_packages
            )
        ]
        mutations = [
            (
                "Cargo manifest",
                release_projection.NODE_CARGO_MANIFEST,
                lambda text: replace_once(
                    text,
                    (
                        '[package]\n'
                        'name = "merman-node-candidate"\n'
                        f'version = "{version}"'
                    ),
                    (
                        '[package]\n'
                        'name = "merman-node-candidate"\n'
                        'version = "9.9.9"'
                    ),
                ),
            ),
            *[
                (
                    f"Cargo lock {package_name}",
                    release_projection.NODE_CARGO_LOCK,
                    lambda text, package_name=package_name: replace_once(
                        text,
                        f'name = "{package_name}"\nversion = "{version}"',
                        f'name = "{package_name}"\nversion = "9.9.9"',
                    ),
                )
                for package_name in local_lock_packages
            ],
            (
                "package surface",
                release_projection.NODE_DESCRIPTOR,
                lambda text: replace_json_path(text, ("version",), "9.9.9"),
            ),
            (
                "workspace manifest",
                release_projection.NODE_WORKSPACE_PACKAGE,
                lambda text: replace_json_path(text, ("version",), "9.9.9"),
            ),
            (
                "workspace lock authority",
                release_projection.NODE_WORKSPACE_LOCK,
                lambda text: replace_json_path(text, ("version",), "9.9.9"),
            ),
            (
                "workspace lock package",
                release_projection.NODE_WORKSPACE_LOCK,
                lambda text: replace_json_path(
                    text,
                    ("packages", "", "version"),
                    "9.9.9",
                ),
            ),
            *[
                (
                    f"package manifest {entry['name']}",
                    release_projection.NODE_ROOT
                    / entry["directory"]
                    / "package.json",
                    lambda text: replace_json_path(text, ("version",), "9.9.9"),
                )
                for entry in [descriptor["root"], *descriptor["targets"]]
            ],
        ]

        for label, path, mutate in mutations:
            with self.subTest(label=label, path=path):
                original = (self.ROOT / path).read_text(encoding="utf-8")
                result = release_projection.verify_repository(
                    self.ROOT,
                    overrides={path: mutate(original)},
                )
                self.assertFalse(result.ok)

    def test_node_candidate_package_structure_is_owned_by_the_node_gate(self) -> None:
        descriptor = (
            self.ROOT / release_projection.NODE_DESCRIPTOR
        ).read_text(encoding="utf-8")
        manifest_path = node_package_manifests()[0]
        manifest = (self.ROOT / manifest_path).read_text(encoding="utf-8")
        descriptor_data = node_package_surface()
        target_name = descriptor_data["targets"][0]["name"]
        optional_dependency_manifest = replace_json_path(
            manifest,
            ("optionalDependencies", target_name),
            f"^{release_projection.verify_repository(self.ROOT).authority.canonical}",
        )
        result = release_projection.verify_repository(
            self.ROOT,
            overrides={
                release_projection.NODE_DESCRIPTOR: replace_json_path(
                    descriptor,
                    ("admission_status",),
                    "public",
                ),
                manifest_path: replace_json_path(
                    optional_dependency_manifest,
                    ("private",),
                    False,
                ),
            },
        )
        self.assertTrue(result.ok)

    def test_node_candidate_cargo_projection_preserves_detached_workspace(self) -> None:
        cargo_manifest = (
            self.ROOT / release_projection.NODE_CARGO_MANIFEST
        ).read_text(encoding="utf-8")
        with self.assertRaisesRegex(
            release_projection.ReleaseProjectionError,
            "detached private workspace",
        ):
            release_projection.verify_repository(
                self.ROOT,
                overrides={
                    release_projection.NODE_CARGO_MANIFEST: replace_once(
                        cargo_manifest,
                        "\n[workspace]\nresolver = \"2\"\n",
                        "\n",
                    )
                },
            )

class ReleaseProjectionWriteTests(unittest.TestCase):
    def test_installs_workspace_authority_last_and_preserves_modes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            nested = root / "nested"
            nested.mkdir()
            manifest = root / release_projection.ROOT_MANIFEST
            projection = nested / "projection.txt"
            manifest.write_text("old authority", encoding="utf-8")
            projection.write_text("old projection", encoding="utf-8")
            projection.chmod(0o640)
            destinations: list[Path] = []
            real_replace = release_projection.os.replace

            def record_replace(source, destination):  # noqa: ANN001
                destinations.append(Path(destination))
                return real_replace(source, destination)

            with mock.patch.object(
                release_projection.os,
                "replace",
                side_effect=record_replace,
            ):
                release_projection._replace_projection_files(
                    root,
                    {
                        release_projection.ROOT_MANIFEST: "new authority",
                        Path("nested/projection.txt"): "new projection",
                    },
                    expected={
                        release_projection.ROOT_MANIFEST: "old authority",
                        Path("nested/projection.txt"): "old projection",
                    },
                )

            self.assertEqual(destinations, [projection.resolve(), manifest.resolve()])
            self.assertEqual(manifest.read_text(encoding="utf-8"), "new authority")
            self.assertEqual(
                projection.read_text(encoding="utf-8"),
                "new projection",
            )
            self.assertEqual(projection.stat().st_mode & 0o777, 0o640)
            self.assertEqual(list(root.rglob(".*.release-version-*")), [])
    def test_edit_during_preparation_is_preserved_before_any_replace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            first = root / "first.txt"
            second = root / "second.txt"
            first.write_text("first-old", encoding="utf-8")
            second.write_text("second-old", encoding="utf-8")
            real_write_temp = release_projection._write_projection_temp
            write_count = 0

            def edit_after_preparing_temp(target, content, mode):  # noqa: ANN001
                nonlocal write_count
                temp_path = real_write_temp(target, content, mode)
                write_count += 1
                if write_count == 2:
                    first.write_text("external edit", encoding="utf-8")
                return temp_path

            with mock.patch.object(
                release_projection,
                "_write_projection_temp",
                side_effect=edit_after_preparing_temp,
            ), self.assertRaisesRegex(
                release_projection.ReleaseProjectionError,
                "changed while preparing",
            ):
                release_projection._replace_projection_files(
                    root,
                    {
                        Path("first.txt"): "first-new",
                        Path("second.txt"): "second-new",
                    },
                    expected={
                        Path("first.txt"): "first-old",
                        Path("second.txt"): "second-old",
                    },
                )

            self.assertEqual(first.read_text(encoding="utf-8"), "external edit")
            self.assertEqual(second.read_text(encoding="utf-8"), "second-old")
            self.assertEqual(list(root.rglob(".*.release-version-*")), [])

    def test_interrupted_group_is_completed_by_rerunning(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            manifest = root / release_projection.ROOT_MANIFEST
            projection = root / "projection.txt"
            manifest.write_text("old authority", encoding="utf-8")
            projection.write_text("old projection", encoding="utf-8")
            real_replace = release_projection.os.replace

            def fail_authority_replace(source, destination):  # noqa: ANN001
                if Path(destination) == manifest.resolve():
                    raise OSError("injected replace failure")
                return real_replace(source, destination)

            with mock.patch.object(
                release_projection.os,
                "replace",
                side_effect=fail_authority_replace,
            ), self.assertRaisesRegex(
                release_projection.ReleaseProjectionError,
                "rerun the same command",
            ):
                release_projection._replace_projection_files(
                    root,
                    {
                        release_projection.ROOT_MANIFEST: "new authority",
                        Path("projection.txt"): "new projection",
                    },
                    expected={
                        release_projection.ROOT_MANIFEST: "old authority",
                        Path("projection.txt"): "old projection",
                    },
                )

            self.assertEqual(manifest.read_text(encoding="utf-8"), "old authority")
            self.assertEqual(projection.read_text(encoding="utf-8"), "new projection")
            self.assertEqual(list(root.rglob(".*.release-version-*")), [])

            release_projection._replace_projection_files(
                root,
                {release_projection.ROOT_MANIFEST: "new authority"},
                expected={release_projection.ROOT_MANIFEST: "old authority"},
            )
            self.assertEqual(manifest.read_text(encoding="utf-8"), "new authority")

    def test_rejects_escape_and_symlink_targets(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "target.txt"
            target.write_text("old", encoding="utf-8")
            link = root / "link.txt"
            link.symlink_to(target.name)

            with self.assertRaisesRegex(
                release_projection.ReleaseProjectionError,
                "escapes repository root",
            ):
                release_projection._replace_projection_files(
                    root,
                    {Path("../outside.txt"): "new"},
                    expected={Path("../outside.txt"): "old"},
                )

            with self.assertRaisesRegex(
                release_projection.ReleaseProjectionError,
                "regular non-symlink",
            ):
                release_projection._replace_projection_files(
                    root,
                    {Path("link.txt"): "new"},
                    expected={Path("link.txt"): "old"},
                )

            self.assertEqual(target.read_text(encoding="utf-8"), "old")
            self.assertEqual(list(root.rglob(".*.release-version-*")), [])
