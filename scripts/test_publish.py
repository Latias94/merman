#!/usr/bin/env python3
"""Unit tests for crates.io publish helper metadata handling."""

import contextlib
import io
import json
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

from tools import publish as publish_tool


ROOT = Path(__file__).resolve().parents[1]


class PublishMetadataTests(unittest.TestCase):
    def test_publish_field_allows_default_and_crates_io_registry(self) -> None:
        self.assertTrue(publish_tool.publish_field_allows_crates_io(None))
        self.assertTrue(publish_tool.publish_field_allows_crates_io(True))
        self.assertTrue(publish_tool.publish_field_allows_crates_io(["crates-io"]))

    def test_publish_field_rejects_publish_false_and_other_registries(self) -> None:
        self.assertFalse(publish_tool.publish_field_allows_crates_io([]))
        self.assertFalse(publish_tool.publish_field_allows_crates_io(False))
        self.assertFalse(publish_tool.publish_field_allows_crates_io(["internal"]))

    def test_workspace_packages_exclude_publish_false_metadata(self) -> None:
        metadata = workspace_metadata(
            package("xtask", publish=[]),
            package("merman-core"),
        )

        packages = publish_tool.crates_io_publish_plan(metadata).packages

        self.assertNotIn("xtask", packages)
        self.assertEqual(packages["merman-core"].version, "1.0.0")

    def test_list_mode_returns_before_package_info_projection(self) -> None:
        original_argv = sys.argv
        original_cargo_metadata = publish_tool.cargo_metadata
        original_publish_plan = publish_tool.crates_io_publish_plan
        original_require_tool = publish_tool.require_tool
        try:
            sys.argv = ["publish.py", "--list-crates-io-packages"]
            publish_tool.cargo_metadata = lambda _repo_root, **_kwargs: workspace_metadata(
                package("merman-core")
            )
            publish_tool.crates_io_publish_plan = lambda _metadata: self.fail(
                "list mode must not construct package info"
            )
            publish_tool.require_tool = lambda _name: None

            stdout = io.StringIO()
            stderr = io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(publish_tool.main(), 0)
        finally:
            sys.argv = original_argv
            publish_tool.cargo_metadata = original_cargo_metadata
            publish_tool.crates_io_publish_plan = original_publish_plan
            publish_tool.require_tool = original_require_tool

        self.assertEqual(stdout.getvalue(), "merman-core\n")
        self.assertEqual(stderr.getvalue(), "")

    def test_crates_io_package_order_rejects_internal_registry_packages(self) -> None:
        metadata = workspace_metadata(
            package("default-publish"),
            package("explicit-crates-io", publish=["crates-io"]),
            package("internal-only", publish=["internal"]),
            package("publish-false", publish=[]),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_order(metadata),
            ("default-publish", "explicit-crates-io"),
        )

    def test_independent_packages_are_excluded_from_lockstep_graph(self) -> None:
        metadata = workspace_metadata(
            package("tree-sitter-mermaid"),
            package("merman-lsp", dependency("tree-sitter-mermaid")),
            package("merman-core"),
            independent=("tree-sitter-mermaid",),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("merman-core", "merman-lsp"),),
        )

    def test_independent_package_declarations_must_name_publishable_members(self) -> None:
        metadata = workspace_metadata(
            package("private", publish=[]),
            independent=("private",),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "independent packages must allow crates.io publication.*private",
        ):
            publish_tool.crates_io_publish_batches(metadata)

    def test_unknown_independent_package_is_rejected(self) -> None:
        metadata = workspace_metadata(
            package("merman-core"),
            independent=("missing",),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "independent packages are not workspace members.*missing",
        ):
            publish_tool.crates_io_publish_batches(metadata)


class WorkspacePublishPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.sources = (
            "merman-bindings-core", "merman-ffi", "merman-uniffi", "merman-wasm",
        )
        self.metadata = workspace_metadata(
            *(package(name) for name in self.sources),
            package("merman-android-jni", publish=[]),
        )

    def load_metadata(self) -> dict:
        result = subprocess.CompletedProcess(
            ["cargo", "metadata"], 0, json.dumps(self.metadata), "",
        )
        with mock.patch.object(publish_tool, "run_command", return_value=result):
            return publish_tool.cargo_metadata(ROOT, quiet=True)

    def test_metadata_admits_public_sources_but_not_the_jni_implementation(self) -> None:
        metadata = self.load_metadata()

        self.assertEqual(metadata, self.metadata)
        self.assertEqual(set(publish_tool.crates_io_publish_order(metadata)), set(self.sources))

    def test_jni_manifest_must_explicitly_disable_publication(self) -> None:
        for declaration in ("", "publish = true\n", "publish = []\n", 'publish = ["internal"]\n'):
            manifest = '[package]\nname = "merman-android-jni"\n' + declaration
            with (
                self.subTest(declaration=declaration),
                mock.patch.object(Path, "read_text", return_value=manifest),
                self.assertRaisesRegex(publish_tool.PublishGraphError, "merman-android-jni.*publish = false"),
            ):
                self.load_metadata()

    def test_jni_cannot_enter_the_crates_io_graph(self) -> None:
        self.metadata["packages"][-1]["publish"] = None

        with self.assertRaisesRegex(publish_tool.PublishGraphError, "merman-android-jni"):
            self.load_metadata()

    def test_required_source_packages_cannot_leave_the_coupled_graph(self) -> None:
        for name in self.sources:
            for mutation in ("missing", "private", "independent"):
                metadata = json.loads(json.dumps(self.metadata))
                selected = next(item for item in metadata["packages"] if item["name"] == name)
                if mutation == "missing":
                    metadata["workspace_members"].remove(selected["id"])
                elif mutation == "private":
                    selected["publish"] = []
                else:
                    metadata["metadata"] = {"merman-release": {"independent-packages": [name]}}
                result = subprocess.CompletedProcess(["cargo", "metadata"], 0, json.dumps(metadata), "")
                with (
                    self.subTest(package=name, mutation=mutation),
                    mock.patch.object(publish_tool, "run_command", return_value=result),
                    self.assertRaisesRegex(publish_tool.PublishGraphError, name),
                ):
                    publish_tool.cargo_metadata(ROOT, quiet=True)


class PublishGraphTests(unittest.TestCase):
    def test_independent_packages_share_lexically_sorted_batches(self) -> None:
        metadata = workspace_metadata(
            package("beta"),
            package("alpha"),
            package("gamma", dependency("alpha")),
            package("delta", dependency("beta")),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("alpha", "beta"), ("delta", "gamma")),
        )

    def test_renamed_optional_target_and_build_dependencies_precede_dependents(self) -> None:
        metadata = workspace_metadata(
            package("core"),
            package(
                "app",
                dependency("core", rename="renamed_core", optional=True),
                dependency("core", kind="build", target="cfg(unix)"),
            ),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("core",), ("app",)),
        )

    def test_coupled_dev_dependencies_precede_consumer_package_preparation(self) -> None:
        metadata = workspace_metadata(
            package("merman-model"),
            package("merman-core", dependency("merman-model")),
            package("merman-ascii", dependency("merman-core", kind="dev")),
            package("merman-render", dependency("merman-core", kind="dev")),
        )

        plan = publish_tool.crates_io_publish_plan(metadata)

        self.assertEqual(
            plan.batches,
            (("merman-model",), ("merman-core",), ("merman-ascii", "merman-render")),
        )
        self.assertEqual(plan.packages["merman-ascii"].internal_deps, ("merman-core",))
        self.assertEqual(plan.packages["merman-render"].internal_deps, ("merman-core",))

    def test_private_path_only_dev_fixtures_do_not_enter_the_release_graph(self) -> None:
        metadata = workspace_metadata(
            package("public", dependency("fixture", kind="dev")),
            package("fixture", dependency("public"), publish=[]),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("public",),),
        )

    def test_coupled_dev_dependency_cycle_is_rejected_before_publication(self) -> None:
        metadata = workspace_metadata(
            package("alpha", dependency("beta", kind="dev")),
            package("beta", dependency("alpha")),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "dependency cycle.*alpha.*beta",
        ):
            publish_tool.crates_io_publish_batches(metadata)

    def test_publishable_package_cannot_depend_on_private_workspace_member(self) -> None:
        metadata = workspace_metadata(
            package("internal", publish=[]),
            package("public", dependency("internal")),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "public.*non-publishable workspace package internal",
        ):
            publish_tool.crates_io_publish_batches(metadata)

    def test_dependency_cycle_is_rejected(self) -> None:
        metadata = workspace_metadata(
            package("alpha", dependency("beta")),
            package("beta", dependency("alpha")),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "dependency cycle.*alpha.*beta",
        ):
            publish_tool.crates_io_publish_batches(metadata)


def workspace_metadata(
    *packages: dict[str, object],
    independent: tuple[str, ...] = (),
) -> dict[str, object]:
    metadata: dict[str, object] = {
        "workspace_members": [package["id"] for package in packages],
        "packages": list(packages),
    }
    if independent:
        metadata["metadata"] = {
            "merman-release": {"independent-packages": list(independent)}
        }
    return metadata


def package(
    name: str,
    *dependencies: dict[str, object],
    publish: object = None,
) -> dict[str, object]:
    manifest_path = ROOT / "crates" / name / "Cargo.toml"
    return {
        "id": f"path+file://{manifest_path.parent}#1.0.0",
        "name": name,
        "version": "1.0.0",
        "publish": publish,
        "manifest_path": str(manifest_path),
        "dependencies": list(dependencies),
    }


def dependency(
    name: str,
    *,
    kind: str | None = None,
    rename: str | None = None,
    optional: bool = False,
    target: str | None = None,
) -> dict[str, object]:
    return {
        "name": name,
        "kind": kind,
        "rename": rename,
        "optional": optional,
        "target": target,
        "path": str(ROOT / "crates" / name),
    }


if __name__ == "__main__":
    unittest.main()
