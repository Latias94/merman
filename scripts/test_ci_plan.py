#!/usr/bin/env python3
"""Tests for the pull-request CI planner and same-run gate."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

try:
    from scripts.ci_plan import (
        OWNER_NAMES,
        GateError,
        _write_github_outputs,
        evaluate_gate,
        parse_name_status_z,
        plan_all,
        plan_changes,
        plan_repository_diff,
        plan_selected,
    )
except ModuleNotFoundError:
    from ci_plan import (
        OWNER_NAMES,
        GateError,
        _write_github_outputs,
        evaluate_gate,
        parse_name_status_z,
        plan_all,
        plan_changes,
        plan_repository_diff,
        plan_selected,
    )


class NameStatusParserTests(unittest.TestCase):
    def test_parses_modified_deleted_and_renamed_paths_without_line_splitting(self) -> None:
        changes = parse_name_status_z(
            b"M\0docs/README.md\0D\0old.txt\0R100\0old name.txt\0new name.txt\0"
        )

        self.assertEqual(
            [(change.status, change.paths) for change in changes],
            [
                ("M", ("docs/README.md",)),
                ("D", ("old.txt",)),
                ("R100", ("old name.txt", "new name.txt")),
            ],
        )

    def test_rejects_malformed_or_unsafe_records(self) -> None:
        fixtures = (
            b"M\0missing-terminator",
            b"R100\0only-old\0",
            b"M\0../escape\0",
            b"Z\0file\0",
            b"M\0/absolute\0",
        )

        for fixture in fixtures:
            with self.subTest(fixture=fixture):
                with self.assertRaises(ValueError):
                    parse_name_status_z(fixture)


class PlannerTests(unittest.TestCase):
    def test_valid_empty_diff_runs_only_the_aggregate(self) -> None:
        plan = plan_changes([], base="a" * 40, head="b" * 40)

        self.assertTrue(plan["empty"])
        self.assertFalse(plan["fallback"])
        self.assertEqual(plan["owners"], {name: False for name in OWNER_NAMES})
        self.assertFalse(plan["svg_parity"])

    def test_docs_only_change_selects_hygiene_without_expensive_owners(self) -> None:
        plan = plan_changes(
            parse_name_status_z(b"M\0docs/development/CI.md\0"),
            base="a" * 40,
            head="b" * 40,
        )

        self.assertTrue(plan["owners"]["hygiene"])
        self.assertFalse(plan["owners"]["core"])
        self.assertFalse(plan["owners"]["platform"])
        self.assertFalse(plan["owners"]["web"])
        self.assertFalse(plan["svg_parity"])

    def test_renderer_change_selects_core_without_unrelated_lifecycle_owners(self) -> None:
        plan = plan_changes(
            parse_name_status_z(b"M\0crates/merman-render/src/lib.rs\0"),
            base="a" * 40,
            head="b" * 40,
        )

        self.assertFalse(plan["fallback"])
        self.assertEqual(
            {name for name, enabled in plan["owners"].items() if enabled},
            {"core", "hygiene"},
        )
        self.assertTrue(plan["svg_parity"])

    def test_svg_parity_selector_uses_explicit_input_boundaries(self) -> None:
        fixtures = {
            "crates/dugong/src/lib.rs": True,
            "crates/merman/src/render.rs": True,
            "crates/merman-core/src/diagram/mod.rs": True,
            "crates/merman-render/src/lib.rs": True,
            "crates/xtask/src/cmd/compare/xml.rs": True,
            "crates/xtask/src/cmd/flowchart_elk_corpus.rs": True,
            "crates/xtask/src/svgdom.rs": True,
            "fixtures/_config/default.json": True,
            "fixtures/_verification/deterministic-root-contracts.json": True,
            "fixtures/flowchart/basic.mmd": True,
            "fixtures/_upstream/flowchart-elk-11.16.1/_manifest.json": True,
            "fixtures/upstream-svgs/flowchart/basic.svg": True,
            "playground/tests/.npmrc": True,
            "playground/tests/package-lock.json": True,
            "playground/tests/package.json": True,
            "playground/tests/root-viewport-oracle.ts": True,
            "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json": True,
            "crates/merman-ascii/src/safe_text/wrapped.rs": False,
            "crates/merman-cli/src/main.rs": False,
            "crates/xtask/src/cmd/native_abi.rs": False,
            "docs/development/CI.md": False,
            "fixtures/_deferred/flowchart/candidate.mmd": False,
            "fixtures/_upstream/mermaid-11.16.0/source.mmd": False,
            "fixtures/bindings/errors.json": False,
        }

        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                self.assertEqual(plan["svg_parity"], expected)
                if expected:
                    self.assertTrue(plan["owners"]["core"])

    def test_crate_changes_use_explicit_owner_boundaries(self) -> None:
        fixtures = {
            "crates/merman-ascii/src/lib.rs": {"core", "hygiene"},
            "crates/merman-cli/src/main.rs": {"cli", "core", "hygiene"},
            "crates/merman-wasm/src/lib.rs": {"core", "hygiene", "npm", "web"},
            "crates/merman-uniffi/src/lib.rs": {
                "core",
                "hygiene",
                "platform",
                "python",
            },
            "crates/merman-core/src/lib.rs": {"core", "fuzz", "hygiene"},
            "crates/merman/benches/ascii_pipeline.rs": {
                "core",
                "hygiene",
                "performance",
            },
        }

        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {name for name, enabled in plan["owners"].items() if enabled}
                self.assertFalse(plan["fallback"])
                self.assertEqual(selected, expected)

    def test_xtask_changes_select_their_artifact_consumers(self) -> None:
        fixtures = {
            "crates/xtask/src/cmd/typst_package.rs": {
                "core",
                "hygiene",
                "typst",
            },
            "crates/xtask/src/cmd/editor_language_contract.rs": {
                "core",
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "crates/xtask/src/cmd/playground_catalog.rs": {
                "core",
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "crates/xtask/src/cmd/native_abi.rs": {
                "core",
                "hygiene",
                "platform",
            },
            "crates/xtask/src/cmd/fixtures.rs": {"core", "hygiene"},
        }

        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {name for name, enabled in plan["owners"].items() if enabled}
                self.assertFalse(plan["fallback"])
                self.assertEqual(selected, expected)

    def test_fixture_upstream_and_script_changes_use_narrow_explicit_owners(self) -> None:
        fixtures = {
            "fixtures/flowchart/basic.mmd": {"core", "hygiene"},
            "fixtures/bindings/errors.json": {
                "core",
                "hygiene",
                "node",
                "npm",
                "platform",
                "python",
                "web",
            },
            "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json": {
                "core",
                "hygiene",
                "npm",
                "web",
            },
            "scripts/build-python-uniffi-wheel.py": {"hygiene", "python"},
            "scripts/audit_plan.py": {"hygiene", "npm", "security"},
            "scripts/strict_json.py": {"hygiene"},
            "scripts/release_projection.py": {"hygiene"},
        }

        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {name for name, enabled in plan["owners"].items() if enabled}
                self.assertFalse(plan["fallback"])
                self.assertEqual(selected, expected)

    def test_owner_local_changes_select_their_narrow_jobs(self) -> None:
        fixtures = {
            "distribution/tree-sitter-mermaid/queries/portable/highlights.scm": {
                "grammar",
                "hygiene",
                "web",
            },
            "platforms/web/src/index.ts": {"hygiene", "npm", "web"},
            "platforms/node/package-lock.json": {"core", "hygiene", "node", "npm", "security"},
            "tools/vscode-extension/src/extension.ts": {"hygiene", "npm", "vscode"},
            "contracts/editor-language/rename-policy-v1.json": {
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "tools/bench/perf_runner.py": {"core", "hygiene", "performance"},
            "platforms/python/pyproject.toml": {"hygiene", "python"},
            "distribution/cli/registry-templates/scoop.template.json": {
                "cli",
                "hygiene",
            },
        }

        for path, selected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                actual = {name for name, enabled in plan["owners"].items() if enabled}
                self.assertEqual(actual, selected)

    def test_tree_sitter_grammar_mechanics_use_the_focused_owner(self) -> None:
        for path in (
            "distribution/tree-sitter-mermaid/grammar.js",
            "distribution/tree-sitter-mermaid/grammar/families/flowchart.js",
            "distribution/tree-sitter-mermaid/src/parser.c",
            "distribution/tree-sitter-mermaid/src/scanner.c",
        ):
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {
                    name for name, enabled in plan["owners"].items() if enabled
                }
                self.assertEqual(selected, {"grammar", "hygiene"})

    def test_browser_tree_sitter_assets_select_the_web_owner(self) -> None:
        for path in (
            "distribution/tree-sitter-mermaid/tree-sitter-mermaid.wasm",
            "distribution/tree-sitter-mermaid/queries/portable/highlights.scm",
        ):
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {
                    name for name, enabled in plan["owners"].items() if enabled
                }
                self.assertEqual(selected, {"grammar", "hygiene", "web"})

    def test_tree_sitter_manifests_select_dependency_owners(self) -> None:
        fixtures = {
            "distribution/tree-sitter-mermaid/Cargo.toml": {
                "grammar",
                "hygiene",
                "security",
            },
            "distribution/tree-sitter-mermaid/package-lock.json": {
                "grammar",
                "hygiene",
                "npm",
                "security",
            },
            "distribution/tree-sitter-mermaid/THIRD_PARTY_LICENSES/mermaid/LICENSE": {
                "grammar",
                "hygiene",
                "security",
            },
        }
        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {
                    name for name, enabled in plan["owners"].items() if enabled
                }
                self.assertEqual(selected, expected)

    def test_tree_sitter_legal_authority_selects_its_consumers(self) -> None:
        plan = plan_changes(
            parse_name_status_z(
                b"M\0docs/release/THIRD_PARTY_COMPONENTS.json\0"
            ),
            base="a" * 40,
            head="b" * 40,
        )

        selected = {name for name, enabled in plan["owners"].items() if enabled}
        self.assertEqual(selected, {"grammar", "hygiene", "security"})

    def test_unknown_grammar_path_fails_broad(self) -> None:
        plan = plan_changes(
            parse_name_status_z(b"M\0distribution/unowned-language/grammar.js\0"),
            base="a" * 40,
            head="b" * 40,
        )

        self.assertTrue(plan["fallback"])
        self.assertEqual(plan["owners"], {name: True for name in OWNER_NAMES})
        self.assertTrue(plan["svg_parity"])

    def test_cross_owner_inputs_select_every_consumer(self) -> None:
        fixtures = {
            "platforms/web/src/svg-safety-policy.ts": {
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "tools/vscode-extension/src/preview-svg-safety-policy.ts": {
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "playground/src/generated/ascii-capabilities.ts": {
                "core",
                "hygiene",
                "npm",
                "web",
            },
            "playground/examples/manifest.json": {
                "hygiene",
                "npm",
                "vscode",
                "web",
            },
            "playground/editor-artifact-receipt-v2.json": {
                "hygiene",
                "web",
            },
        }
        for path, expected in fixtures.items():
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                selected = {name for name, enabled in plan["owners"].items() if enabled}
                self.assertEqual(selected, expected)

    def test_workflow_classifier_and_unknown_paths_fail_broad(self) -> None:
        for path in (
            ".github/workflows/ci.yml",
            "capabilities/feature-surface-v1.json",
            "contracts/abi/merman-v3.json",
            "distribution/typst/merman/lib.typ",
            "scripts/ci_plan.py",
            "scripts/new_unclassified_owner.py",
            "scripts/audit_plan.py.backup",
            "scripts/build-python-uniffi-wheel.py.backup",
            "scripts/release_projection.py.generated",
            "scripts/strict_json.py.generated",
            "scripts/test_publish.py.tmp",
            "crates/new-unclassified-crate/src/lib.rs",
            "unowned/new-surface.txt",
        ):
            with self.subTest(path=path):
                plan = plan_changes(
                    parse_name_status_z(f"M\0{path}\0".encode()),
                    base="a" * 40,
                    head="b" * 40,
                )
                self.assertTrue(plan["fallback"])
                self.assertEqual(plan["owners"], {name: True for name in OWNER_NAMES})
                self.assertTrue(plan["svg_parity"])

    def test_rename_classifies_both_old_and_new_paths(self) -> None:
        plan = plan_changes(
            parse_name_status_z(
                b"R100\0docs/old.md\0platforms/web/src/new-location.ts\0"
            ),
            base="a" * 40,
            head="b" * 40,
        )

        self.assertTrue(plan["owners"]["hygiene"])
        self.assertTrue(plan["owners"]["web"])
        self.assertTrue(plan["owners"]["npm"])

    def test_missing_base_and_diff_errors_select_every_owner(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo = Path(temp_dir)
            subprocess.run(["git", "init", "-q", repo], check=True)
            subprocess.run(["git", "-C", repo, "config", "user.name", "CI Test"], check=True)
            subprocess.run(
                ["git", "-C", repo, "config", "user.email", "ci@example.invalid"],
                check=True,
            )
            (repo / "README.md").write_text("initial\n", encoding="utf-8")
            subprocess.run(["git", "-C", repo, "add", "README.md"], check=True)
            subprocess.run(["git", "-C", repo, "commit", "-qm", "initial"], check=True)
            head = subprocess.run(
                ["git", "-C", repo, "rev-parse", "HEAD"],
                check=True,
                text=True,
                capture_output=True,
            ).stdout.strip()

            plan = plan_repository_diff(repo, "0" * 40, head)

        self.assertTrue(plan["fallback"])
        self.assertEqual(plan["owners"], {name: True for name in OWNER_NAMES})
        self.assertIn("git diff failed", plan["fallback_reason"])
        self.assertTrue(plan["svg_parity"])

    def test_repository_diff_reads_real_nul_delimited_rename_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo = Path(temp_dir)
            subprocess.run(["git", "init", "-q", repo], check=True)
            subprocess.run(["git", "-C", repo, "config", "user.name", "CI Test"], check=True)
            subprocess.run(
                ["git", "-C", repo, "config", "user.email", "ci@example.invalid"],
                check=True,
            )
            (repo / "docs").mkdir()
            (repo / "docs" / "old name.md").write_text("content\n", encoding="utf-8")
            subprocess.run(["git", "-C", repo, "add", "docs/old name.md"], check=True)
            subprocess.run(["git", "-C", repo, "commit", "-qm", "base"], check=True)
            base = subprocess.run(
                ["git", "-C", repo, "rev-parse", "HEAD"],
                check=True,
                text=True,
                capture_output=True,
            ).stdout.strip()

            (repo / "platforms" / "web" / "src").mkdir(parents=True)
            subprocess.run(
                [
                    "git",
                    "-C",
                    repo,
                    "mv",
                    "docs/old name.md",
                    "platforms/web/src/new name.ts",
                ],
                check=True,
            )
            subprocess.run(["git", "-C", repo, "commit", "-qm", "rename"], check=True)
            head = subprocess.run(
                ["git", "-C", repo, "rev-parse", "HEAD"],
                check=True,
                text=True,
                capture_output=True,
            ).stdout.strip()

            plan = plan_repository_diff(repo, base, head)

        self.assertFalse(plan["fallback"])
        self.assertEqual(plan["changes"][0]["status"], "R100")
        self.assertEqual(
            plan["changes"][0]["paths"],
            ["docs/old name.md", "platforms/web/src/new name.ts"],
        )
        self.assertTrue(plan["owners"]["web"])

    def test_plan_is_compact_json_serializable(self) -> None:
        plan = plan_changes(
            parse_name_status_z(b"M\0docs/README.md\0"),
            base="a" * 40,
            head="b" * 40,
        )
        encoded = json.dumps(plan, separators=(",", ":"), sort_keys=True)
        self.assertNotIn("\n", encoded)

    def test_main_push_plan_selects_svg_parity(self) -> None:
        plan = plan_all(
            base="a" * 40,
            head="b" * 40,
            reason="main push full lifecycle",
        )

        self.assertTrue(plan["svg_parity"])

    def test_github_outputs_include_svg_parity_selector(self) -> None:
        plan = plan_changes(
            parse_name_status_z(b"M\0crates/merman-render/src/lib.rs\0"),
            base="a" * 40,
            head="b" * 40,
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            output_path = Path(temp_dir) / "github-output"
            _write_github_outputs(output_path, plan)
            output = output_path.read_text(encoding="utf-8")

        self.assertIn("\nsvg_parity=true\n", output)

    def test_non_pr_host_safety_net_can_select_only_core(self) -> None:
        plan = plan_selected(
            base="a" * 40,
            head="b" * 40,
            selected=["core"],
            reason="scheduled full host safety net",
            svg_parity=True,
        )

        self.assertEqual(
            {name for name, selected in plan["owners"].items() if selected},
            {"core"},
        )
        self.assertTrue(plan["svg_parity"])
        summary = evaluate_gate(
            plan,
            {"build-test": {"owner": "core", "required": True, "result": "success"}},
        )
        self.assertEqual(summary["selected"], ["build-test"])

    def test_explicit_svg_parity_requires_core_owner(self) -> None:
        with self.assertRaises(ValueError):
            plan_selected(
                base="a" * 40,
                head="b" * 40,
                selected=["hygiene"],
                reason="invalid isolated selector",
                svg_parity=True,
            )

class GateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.plan = plan_changes(
            parse_name_status_z(b"M\0docs/README.md\0"),
            base="a" * 40,
            head="b" * 40,
        )

    def test_selected_success_and_unselected_skip_pass(self) -> None:
        jobs = {
            "hygiene": {"owner": "hygiene", "required": True, "result": "success"},
            "core": {"owner": "core", "required": True, "result": "skipped"},
        }

        summary = evaluate_gate(self.plan, jobs)

        self.assertEqual(summary["selected"], ["hygiene"])
        self.assertEqual(summary["skipped"], ["core"])

    def test_failure_cancellation_skip_or_missing_selected_result_fails_closed(self) -> None:
        for result in ("failure", "cancelled", "skipped", ""):
            with self.subTest(result=result):
                jobs = {
                    "hygiene": {
                        "owner": "hygiene",
                        "required": True,
                        "result": result,
                    }
                }
                with self.assertRaises(GateError):
                    evaluate_gate(self.plan, jobs)

    def test_missing_selected_owner_job_fails_closed(self) -> None:
        with self.assertRaises(GateError):
            evaluate_gate(self.plan, {})

    def test_malformed_plan_or_job_shape_fails_closed(self) -> None:
        malformed_plans = (
            {},
            {**self.plan, "owners": {"hygiene": True}},
            {**self.plan, "base": "not-a-sha"},
            {**self.plan, "schema_version": True},
            {**self.plan, "schema_version": 1},
            {**self.plan, "changes": {}},
            {**self.plan, "changes": [{"status": "Z", "paths": ["docs/README.md"]}]},
            {**self.plan, "changes": [{"status": "R100", "paths": ["docs/README.md"]}]},
            {**self.plan, "changes": [{"status": "R101", "paths": ["a", "b"]}]},
            {**self.plan, "fallback": True, "fallback_reason": None},
            {**self.plan, "fallback_reason": "unexpected"},
            {**self.plan, "svg_parity": "false"},
            {**self.plan, "svg_parity": True},
            {**self.plan, "empty": True},
            {
                **self.plan,
                "reasons": {**self.plan["reasons"], "hygiene": []},
            },
            {
                **self.plan,
                "owners": {
                    **self.plan["owners"],
                    "hygiene": False,
                    "core": True,
                },
                "reasons": {
                    **self.plan["reasons"],
                    "hygiene": [],
                    "core": ["forged selection"],
                },
            },
        )
        for plan in malformed_plans:
            with self.subTest(plan=plan):
                with self.assertRaises(GateError):
                    evaluate_gate(plan, {})

        with self.assertRaises(GateError):
            evaluate_gate(
                self.plan,
                {"hygiene": {"owner": "missing", "required": True, "result": "success"}},
            )

    def test_valid_empty_diff_needs_no_owner_job(self) -> None:
        plan = plan_changes([], base="a" * 40, head="b" * 40)

        summary = evaluate_gate(plan, {})

        self.assertEqual(summary["selected"], [])
        self.assertEqual(summary["skipped"], [])


if __name__ == "__main__":
    unittest.main()
