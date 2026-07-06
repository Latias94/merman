#!/usr/bin/env python3
"""Unit tests for release crate publish-order verification helpers."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify-release-crate-order.py")
SPEC = importlib.util.spec_from_file_location("verify_release_crate_order", MODULE_PATH)
assert SPEC is not None
verify_release_crate_order = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verify_release_crate_order)


class ReleaseCrateOrderTopologyTests(unittest.TestCase):
    def test_extract_workflow_publish_orders_reads_every_crates_array(self) -> None:
        orders = verify_release_crate_order.extract_workflow_publish_orders(
            """
          crates=(
            alpha
            beta
          )
          other=ignored
          crates=(
            alpha
            beta
          )
            """
        )

        self.assertEqual(orders, [["alpha", "beta"], ["alpha", "beta"]])

    def test_workflow_publish_order_rejects_mismatched_crates_arrays(self) -> None:
        original_extract = verify_release_crate_order.extract_workflow_publish_orders
        try:
            verify_release_crate_order.extract_workflow_publish_orders = lambda _text: [
                ["alpha", "beta"],
                ["alpha", "gamma"],
            ]

            with self.assertRaisesRegex(ValueError, "does not match array #1"):
                verify_release_crate_order.workflow_publish_order()
        finally:
            verify_release_crate_order.extract_workflow_publish_orders = original_extract

    def test_publishable_workspace_packages_excludes_publish_false_metadata(self) -> None:
        packages = verify_release_crate_order.publishable_workspace_packages(
            {
                "packages": [
                    {"name": "merman-core", "publish": None},
                    {"name": "merman-cli", "publish": ["crates-io"]},
                    {"name": "xtask", "publish": []},
                    {"name": "internal-only", "publish": ["internal"]},
                ],
            }
        )

        self.assertEqual(set(packages), {"merman-core", "merman-cli"})

    def test_dependency_before_dependent_passes(self) -> None:
        failed, stdout, stderr = check_topology(
            ["merman-core", "merman-analysis"],
            {
                "merman-core": package(),
                "merman-analysis": package(
                    dependency("merman-core"),
                ),
            },
        )

        self.assertFalse(failed)
        self.assertIn("1 workspace dependency edges", stdout)
        self.assertEqual(stderr, "")

    def test_swapped_dependency_order_fails(self) -> None:
        failed, _stdout, stderr = check_topology(
            ["merman-analysis", "merman-core"],
            {
                "merman-core": package(),
                "merman-analysis": package(
                    dependency("merman-core"),
                ),
            },
        )

        self.assertTrue(failed)
        self.assertIn(
            "release order publishes merman-analysis before its normal workspace dependency merman-core",
            stderr,
        )

    def test_dev_only_dependency_does_not_block_publish_order(self) -> None:
        failed, stdout, stderr = check_topology(
            ["merman-analysis", "merman-core"],
            {
                "merman-core": package(),
                "merman-analysis": package(
                    dependency("merman-core", kind="dev"),
                ),
            },
        )

        self.assertFalse(failed)
        self.assertIn("0 workspace dependency edges", stdout)
        self.assertEqual(stderr, "")

    def test_build_dependency_still_blocks_publish_order(self) -> None:
        failed, _stdout, stderr = check_topology(
            ["merman-analysis", "merman-core"],
            {
                "merman-core": package(),
                "merman-analysis": package(
                    dependency("merman-core", kind="build"),
                ),
            },
        )

        self.assertTrue(failed)
        self.assertIn(
            "release order publishes merman-analysis before its build workspace dependency merman-core",
            stderr,
        )


def check_topology(order: list[str], packages: dict[str, dict]) -> tuple[bool, str, str]:
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        failed = verify_release_crate_order.report_topology_violations(order, packages)
    return failed, stdout.getvalue(), stderr.getvalue()


def package(*dependencies: dict) -> dict:
    return {"dependencies": list(dependencies)}


def dependency(name: str, *, kind: str | None = None) -> dict:
    return {"name": name, "kind": kind}


if __name__ == "__main__":
    unittest.main()
