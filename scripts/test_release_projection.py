#!/usr/bin/env python3
"""Unit tests for release version projections."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Callable
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import release_projection, release_version_owners


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
    entries = [surface["root"], surface["wasm"], *surface["targets"]]
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


def git(root: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), *args], check=True, capture_output=True
    ).stdout


@contextmanager
def linked_worktree_fixture():
    with tempfile.TemporaryDirectory() as temp_dir:
        repository = Path(temp_dir) / "repository"
        release_worktree = Path(temp_dir) / "release"
        repository.mkdir()
        subprocess.run(["git", "init", "--quiet", str(repository)], check=True)
        for name in ("sentinel.txt", "concurrent.txt"):
            (repository / name).write_text("original\n", encoding="utf-8")
        git(repository, "add", "sentinel.txt", "concurrent.txt")
        git(
            repository,
            "-c",
            "user.name=Merman Tests",
            "-c",
            "user.email=merman-tests@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        )
        git(repository, "worktree", "add", "--quiet", "-b", "release-test", str(release_worktree))
        yield repository, release_worktree


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

    def test_workspace_dependency_requirement_is_exact_only_for_prereleases(self) -> None:
        self.assertEqual(
            release_projection.workspace_dependency_requirement(
                release_projection.parse_release_version("0.8.0-alpha.6")
            ),
            "=0.8.0-alpha.6",
        )
        self.assertEqual(
            release_projection.workspace_dependency_requirement(
                release_projection.parse_release_version("0.8.0")
            ),
            "0.8.0",
        )

    def test_no_argument_verifier_covers_the_complete_current_projection(self) -> None:
        result = release_projection.verify_repository(self.ROOT)

        self.assertTrue(result.ok)
        labels = {observation.label for observation in result.observations}
        self.assertIn("Cargo workspace dependency merman-core", labels)
        self.assertIn("Cargo workspace independent dependency roughr", labels)
        self.assertIn("Cargo.lock package merman-lsp", labels)
        self.assertIn("Cargo.lock independent package roughr-merman", labels)
        self.assertIn("fuzz/Cargo.lock package merman-ffi", labels)
        self.assertIn("fuzz/Cargo.lock independent package roughr-merman", labels)
        self.assertIn("Web workspace", labels)
        self.assertIn("Web workspace lock", labels)
        self.assertIn("Web workspace lock package", labels)
        self.assertIn("Web package @mermanjs/web", labels)
        self.assertIn("Web lock package @mermanjs/web", labels)
        self.assertIn("Playground application", labels)
        self.assertIn("Playground application lock", labels)
        self.assertIn("Playground application lock package", labels)
        self.assertIn("Playground local Web workspace lock", labels)
        self.assertIn("Playground local Web lock @mermanjs/web", labels)
        self.assertIn("Playground license lock digest", labels)
        self.assertIn("Playground license local Web package", labels)
        self.assertIn("Node candidate Cargo package", labels)
        self.assertIn("Node candidate Cargo.lock package merman-bindings-core", labels)
        self.assertIn(
            "Node candidate Cargo.lock independent package roughr-merman",
            labels,
        )
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
        catalog = release_projection.load_workspace_catalog(
            release_projection.RepositoryView(self.ROOT)
        )
        roughr_version = catalog.independent_packages["roughr-merman"][1]
        roughr_dependency = (
            'roughr = { package = "roughr-merman", path = "crates/roughr", '
            f'version = "{roughr_version}"'
        )
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
                    roughr_dependency,
                    'roughr = { package = "roughr-merman", path = "crates/roughr", '
                    'version = "9.9.9"',
                ),
            ),
            (
                Path("Cargo.toml"),
                lambda text: replace_once(
                    text,
                    f'merman-core = {{ path = "crates/merman-core", version = "={canonical}"',
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
                Path("Cargo.lock"),
                lambda text: replace_once(
                    text,
                    f'name = "roughr-merman"\nversion = "{roughr_version}"',
                    'name = "roughr-merman"\nversion = "9.9.9"',
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
                release_projection.FUZZ_LOCK,
                lambda text: replace_once(
                    text,
                    f'name = "roughr-merman"\nversion = "{roughr_version}"',
                    'name = "roughr-merman"\nversion = "9.9.9"',
                ),
            ),
            (
                release_projection.NODE_CARGO_LOCK,
                lambda text: replace_once(
                    text,
                    f'name = "roughr-merman"\nversion = "{roughr_version}"',
                    'name = "roughr-merman"\nversion = "9.9.9"',
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
                release_projection.PLAYGROUND_PACKAGE,
                lambda text: replace_json_path(text, ("version",), "9.9.9"),
            ),
            (
                release_projection.PLAYGROUND_LOCK,
                lambda text: replace_json_path(text, ("version",), "9.9.9"),
            ),
            (
                release_projection.PLAYGROUND_LOCK,
                lambda text: replace_json_path(
                    text,
                    ("packages", "", "version"),
                    "9.9.9",
                ),
            ),
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
                release_projection.PLAYGROUND_LICENSE_REPORT,
                lambda text: replace_once(
                    text,
                    f" - @mermanjs/web@{canonical}",
                    " - @mermanjs/web@9.9.9",
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

    def test_node_candidate_verification_fails_closed_on_each_version_projection(
        self,
    ) -> None:
        version = release_projection.verify_repository(self.ROOT).authority.canonical
        catalog = release_projection.load_workspace_catalog(
            release_projection.RepositoryView(self.ROOT)
        )
        descriptor = node_package_surface()
        local_lock_packages = [
            (package["name"], package["version"])
            for package in release_projection.RepositoryView(self.ROOT)
            .toml(release_projection.NODE_CARGO_LOCK)["package"]
            if "source" not in package
            and (
                package["name"] == release_projection.NODE_CARGO_PACKAGE
                or package["name"] in catalog.coupled_packages
                or package["name"] in catalog.independent_packages
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
                    lambda text,
                    package_name=package_name,
                    package_version=package_version: replace_once(
                        text,
                        f'name = "{package_name}"\nversion = "{package_version}"',
                        f'name = "{package_name}"\nversion = "9.9.9"',
                    ),
                )
                for package_name, package_version in local_lock_packages
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


class ReleaseVersionOwnerTests(unittest.TestCase):
    FIXTURES = {
        release_version_owners.PYTHON_MANIFEST: (
            '[project]\nversion = "0.8.0a5"\n'
            '[tool.fixture]\nversion = "keep-python"\n'
        ),
        release_version_owners.ANDROID_MANIFEST: (
            'version = "0.8.0-alpha.5"\n'
            'val dependencyVersion = "keep-android"\n'
        ),
        release_version_owners.FLUTTER_MANIFEST: (
            "version: 0.8.0-alpha.5\ndependencies:\n  fixture: keep-flutter\n"
        ),
        release_version_owners.FLUTTER_PACKAGE_VERSION: (
            "const String mermanPackageVersion = '0.8.0-alpha.5';\n"
        ),
    }

    def write_fixtures(self, root: Path, fixtures: dict[Path, str]) -> None:
        for path, text in fixtures.items():
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")

    def test_owner_editors_update_every_owned_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            self.write_fixtures(root, self.FIXTURES)
            release = release_projection.parse_release_version("0.9.0-beta.2")

            release_version_owners.prepare_python_version(root, release)
            release_version_owners.prepare_android_version(root, release)
            release_version_owners.prepare_flutter_version(root, release)

            checks = (
                (release_version_owners.PYTHON_MANIFEST, 'version = "0.9.0b2"', "keep-python", 1),
                (release_version_owners.ANDROID_MANIFEST, 'version = "0.9.0-beta.2"', "keep-android", 1),
                (release_version_owners.FLUTTER_MANIFEST, "version: 0.9.0-beta.2", "keep-flutter", 1),
                (release_version_owners.FLUTTER_PACKAGE_VERSION, "'0.9.0-beta.2'", None, 1),
            )
            for path, version_text, retained_text, count in checks:
                text = (root / path).read_text(encoding="utf-8")
                self.assertEqual(text.count(version_text), count)
                if retained_text is not None:
                    self.assertIn(retained_text, text)


class ReleaseProjectionWriteTests(unittest.TestCase):
    def test_set_requires_a_clean_linked_worktree(self) -> None:
        with linked_worktree_fixture() as (repository, release_worktree):
            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "linked release worktree"):
                release_projection._capture_release_preimage(repository)

            release_projection._capture_release_preimage(release_worktree)

            (release_worktree / "untracked.txt").write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "clean release worktree"):
                release_projection._capture_release_preimage(release_worktree)

    def test_owner_preparers_are_explicit_and_failure_leaves_caller_byte_identical(self) -> None:
        with linked_worktree_fixture() as (_repository, release_worktree):
            before = (release_worktree / "sentinel.txt").read_bytes()
            preimage = release_projection._capture_release_preimage(release_worktree)
            release = release_projection.parse_release_version("0.9.0-alpha.1")
            calls: list[tuple[str, Path]] = []

            def prepare_cargo(root: Path, _release) -> None:  # noqa: ANN001
                calls.append(("cargo", root))
                (root / "sentinel.txt").write_text("prepared\n", encoding="utf-8")

            def prepare(owner: str) -> Callable[[Path, object], None]:
                def record(root: Path, _release: object) -> None:
                    calls.append((owner, root))

                return record

            def fail_flutter(root: Path, _release) -> None:  # noqa: ANN001
                calls.append(("flutter", root))
                raise release_version_owners.ReleaseOwnerError(
                    "injected Flutter owner failure"
                )

            with mock.patch.object(
                release_projection, "_prepare_cargo_versions", side_effect=prepare_cargo
            ), mock.patch.object(
                release_projection,
                "_prepare_npm_versions",
                side_effect=prepare("npm"),
            ), mock.patch.object(
                release_version_owners,
                "prepare_python_version",
                side_effect=prepare("python"),
            ), mock.patch.object(
                release_version_owners,
                "prepare_android_version",
                side_effect=prepare("android"),
            ), mock.patch.object(
                release_version_owners,
                "prepare_flutter_version",
                side_effect=fail_flutter,
            ):
                with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "Flutter owner failure"):
                    release_projection._prepare_release_patch(release_worktree, release, preimage)

            self.assertEqual(
                [owner for owner, _root in calls],
                ["cargo", "npm", "python", "android", "flutter"],
            )
            self.assertEqual(len({root for _owner, root in calls}), 1)
            self.assertEqual((release_worktree / "sentinel.txt").read_bytes(), before)
            release_projection._capture_release_preimage(release_worktree)

    def test_concurrent_preimage_change_aborts_before_git_apply(self) -> None:
        with linked_worktree_fixture() as (_repository, release_worktree):
            sentinel = release_worktree / "sentinel.txt"
            sentinel.write_text("projected\n", encoding="utf-8")
            patch = git(release_worktree, "diff", "--binary", "--full-index", "HEAD", "--")
            sentinel.write_text("original\n", encoding="utf-8")
            preimage = release_projection._capture_release_preimage(release_worktree)
            (release_worktree / "concurrent.txt").write_text("concurrent edit\n", encoding="utf-8")

            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "source preimage changed"):
                release_projection._apply_release_patch(release_worktree, patch, preimage)

            self.assertEqual(sentinel.read_text(encoding="utf-8"), "original\n")

    def test_unrelated_npm_lock_drift_is_rejected(self) -> None:
        before = '{"version":"old","packages":{"":{"version":"old"},"node_modules/example":{"version":"1.0.0"}}}'
        after = '{"version":"new","packages":{"":{"version":"new"},"node_modules/example":{"version":"1.1.0"}}}'

        with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "unrelated dependency drift"):
            release_projection._assert_npm_lock_dependency_state(
                Path("package-lock.json"), before, after, local_package_keys={""}
            )

    def test_pinned_tool_missing_or_drifted_is_reported(self) -> None:
        with mock.patch.object(release_projection.shutil, "which", return_value=None):
            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "required pinned tool npm"):
                release_projection._require_exact_tool_version("npm", "11.17.0")
        with mock.patch.object(release_projection.shutil, "which", return_value="/npm"), mock.patch.object(
            release_projection, "_run_command", return_value=b"11.18.0\n"
        ):
            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "found 11.18.0"):
                release_projection._require_exact_tool_version("npm", "11.17.0")

    def test_patch_is_checked_then_applied_once_and_malformed_input_is_rejected(self) -> None:
        with linked_worktree_fixture() as (_repository, release_worktree):
            sentinel = release_worktree / "sentinel.txt"
            before = sentinel.read_bytes()
            preimage = release_projection._capture_release_preimage(release_worktree)

            with self.assertRaisesRegex(release_projection.ReleaseProjectionError, "cannot validate release patch"):
                release_projection._apply_release_patch(release_worktree, b"not a patch\n", preimage)
            self.assertEqual(sentinel.read_bytes(), before)

            sentinel.write_text("projected\n", encoding="utf-8")
            patch = git(release_worktree, "diff", "--binary", "--full-index", "HEAD", "--")
            sentinel.write_bytes(before)
            with mock.patch.object(
                release_projection, "_run_command", wraps=release_projection._run_command
            ) as run_command:
                release_projection._apply_release_patch(release_worktree, patch, preimage)
            apply_calls = [call.args[0] for call in run_command.call_args_list if "apply" in call.args[0]]
            self.assertEqual(["--check" in args for args in apply_calls], [True, False])
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "projected\n")
