#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import json
import sys
import unittest
from dataclasses import replace
from pathlib import Path

import compare_self
import verify_pipeline_bench_list
from corpus_utils import (
    fixture_names_for_suite,
    load_corpus,
    resolve_lane_group,
    select_corpus_fixtures,
)


ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"
ASCII_CORPUS_PATH = ROOT / "tools" / "bench" / "ascii_corpus.json"
BINDING_REQUEST_CORPUS_PATH = ROOT / "tools" / "bench" / "binding_request_corpus.json"


class CorpusContractsTest(unittest.TestCase):
    def test_cross_family_has_one_fixture_per_declared_family(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        fixtures = select_corpus_fixtures(corpus, "cross_family")

        self.assertEqual(len(fixtures), len({fixture.family for fixture in fixtures}))
        self.assertTrue(all((ROOT / fixture.source).is_file() for fixture in fixtures))

    def test_branch_start_controls_use_a_revision_stable_exact_filter(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        expected = {
            "flowchart_medium",
            "flowchart_large",
            "flowchart_ports_heavy",
            "class_medium",
            "mindmap_medium",
            "requirement_medium",
            "architecture_medium",
        }

        self.assertNotIn("branch_start", corpus.suites)
        exact = compare_self.expand_filter_to_exact_benches(
            "end_to_end/(flowchart_medium|flowchart_large|flowchart_ports_heavy|"
            "class_medium|mindmap_medium|requirement_medium|architecture_medium)"
        )
        self.assertEqual(
            {compare_self.split_exact_bench(item)[1] for item in exact}, expected
        )

    def test_pipeline_groups_are_owned_by_registered_lanes(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        expected_groups = {
            "parse",
            "compatibility_json_parse",
            "parse_cold_engine",
            "frontmatter_preprocess",
            "layout",
            "render",
            "end_to_end",
        }

        self.assertEqual(
            {group: resolve_lane_group(corpus, group).id for group in expected_groups},
            {
                "parse": "typed-render-model-parse",
                "compatibility_json_parse": "compatibility-json-parse",
                "parse_cold_engine": "typed-render-model-parse-cold",
                "frontmatter_preprocess": "frontmatter-preprocess-known-type",
                "layout": "prepare-layout",
                "render": "emit-svg",
                "end_to_end": "render-svg",
            },
        )

        with self.assertRaisesRegex(compare_self.ContractViolation, "no registered lane"):
            compare_self._optional_lane_for_group(corpus, "unregistered_group")

    def test_compiled_pipeline_list_requires_current_groups_and_rejects_history(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        current = dict(
            verify_pipeline_bench_list._pipeline_lane_groups(
                corpus,
                enabled_features=frozenset({"svg"}),
            )[0]
        )
        current_groups = sorted(current)
        expected_benches = sorted(
            verify_pipeline_bench_list._expected_pipeline_benches(
                corpus,
                current_groups=current,
            )
        )
        expected_set = set(expected_benches)
        self.assertIn("frontmatter_preprocess/frontmatter_basic", expected_set)
        self.assertNotIn("end_to_end/frontmatter_basic", expected_set)
        self.assertIn("end_to_end/error_basic", expected_set)
        self.assertNotIn("frontmatter_preprocess/error_basic", expected_set)
        fixture = corpus.fixtures[0].name
        list_output = "\n".join(f"{bench}: benchmark" for bench in expected_benches)
        receipt_output = "\n".join(
            "[bench][preflight] "
            + json.dumps(
                {
                    "schema_version": 1,
                    "benchmark": bench,
                    "output_kind": compare_self._PREFLIGHT_OUTPUT_KIND_BY_GROUP[group],
                    "output_bytes": 123,
                    "output_sha256": "a" * 64,
                    "svg_elements": 7 if group in {"render", "end_to_end"} else None,
                },
                separators=(",", ":"),
            )
            for bench in expected_benches
            for group, _fixture in (bench.rsplit("/", 1),)
        )
        output = f"{list_output}\n{receipt_output}"

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(corpus, output)

        self.assertEqual(result["groups"], tuple(current_groups))
        self.assertEqual(result["receipt_count"], len(expected_benches))
        historical = output.replace(
            f"compatibility_json_parse/{fixture}",
            f"parse_known_type/{fixture}",
            1,
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "historical lane aliases",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(corpus, historical)

        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            r"unknown=\['unregistered'\]",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus,
                output + f"\nunregistered/{fixture}: benchmark\n",
            )

        missing_benchmark = "end_to_end/error_basic"
        without_benchmark = "\n".join(
            line
            for line in output.splitlines()
            if line != f"{missing_benchmark}: benchmark"
            and f'"benchmark":"{missing_benchmark}"' not in line
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "corpus/lane product.*missing",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus, without_benchmark
            )

        without_receipt = output.replace(
            next(
                line
                for line in output.splitlines()
                if line.startswith("[bench][preflight]")
            ),
            "",
            1,
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "preflight receipts differ",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus, without_receipt
            )

        for contract in (
            None,
            compare_self._NATIVE_ASCII_CRITERION_PREFLIGHT_CONTRACT,
            "docs/performance/contracts/unknown.json",
        ):
            mixed = replace(
                corpus,
                lanes=tuple(
                    replace(lane, evidence_contract=contract)
                    if lane.id == "render-svg"
                    else lane
                    for lane in corpus.lanes
                ),
            )
            with self.subTest(contract=contract), self.assertRaisesRegex(
                verify_pipeline_bench_list.PipelineBenchListError,
                "uniformly declare",
            ):
                verify_pipeline_bench_list.validate_pipeline_bench_list(
                    mixed, output
                )

    def test_compiled_ascii_pipeline_list_requires_plain_ascii_receipts(self) -> None:
        corpus = load_corpus(ASCII_CORPUS_PATH)
        current = dict(
            verify_pipeline_bench_list._pipeline_lane_groups(
                corpus,
                enabled_features=frozenset({"ascii"}),
            )[0]
        )
        expected_benches = sorted(
            verify_pipeline_bench_list._expected_pipeline_benches(
                corpus,
                current_groups=current,
            )
        )
        output = "\n".join(
            [*(f"{bench}: benchmark" for bench in expected_benches)]
            + [
                "[bench][preflight] "
                + json.dumps(
                    {
                        "schema_version": 1,
                        "benchmark": bench,
                        "output_kind": "plain_ascii",
                        "output_bytes": 123,
                        "output_sha256": "a" * 64,
                        "svg_elements": None,
                    },
                    separators=(",", ":"),
                )
                for bench in expected_benches
            ]
        )

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(
            corpus,
            output,
            enabled_features=("ascii",),
        )

        self.assertEqual(result["groups"], ("ascii_end_to_end",))
        self.assertEqual(result["bench_count"], len(corpus.fixtures))
        self.assertEqual(result["receipt_count"], len(corpus.fixtures))

        recipe = compare_self.RunnerRecipe(
            label="ascii-pipeline",
            checkout=ROOT,
            package="merman",
            bench="ascii_pipeline",
            features=("ascii",),
            default_features=False,
            toolchain=None,
            target_dir=ROOT / "target",
            locked=True,
            corpus=ASCII_CORPUS_PATH,
        )
        description = compare_self._describe_corpus(
            ASCII_CORPUS_PATH,
            recipe=recipe,
        )
        self.assertTrue(description["preflight_receipts_required"])
        self.assertEqual(
            description["preflight_contract"]["id"],
            "native-ascii-criterion-preflight-v1",
        )

        without_receipt = "\n".join(
            line
            for line in output.splitlines()
            if not line.startswith("[bench][preflight]")
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "preflight receipts differ",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus,
                without_receipt,
                enabled_features=("ascii",),
            )

        legacy_contract = replace(
            corpus,
            lanes=tuple(
                replace(
                    lane,
                    evidence_contract=compare_self._NATIVE_CRITERION_PREFLIGHT_CONTRACT,
                )
                for lane in corpus.lanes
            ),
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "correct preflight contract",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                legacy_contract,
                output,
                enabled_features=("ascii",),
            )

    def test_binding_request_corpus_owns_one_complete_benchmark_list(self) -> None:
        corpus = load_corpus(BINDING_REQUEST_CORPUS_PATH)
        fixture = corpus.fixtures[0]
        expected_benches = (
            "binding_request_empty_analysis_ascii_svg/info_fixed_cost",
            "binding_request_resource_override_analysis_ascii_svg/info_fixed_cost",
            "binding_request_version_only_analysis_ascii_svg/info_fixed_cost",
        )
        output = "\n".join(f"{bench}: benchmark" for bench in expected_benches)

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(
            corpus,
            output,
            enabled_features=("analysis", "ascii", "svg"),
        )

        self.assertEqual(result["bench_count"], 3)
        self.assertEqual(
            result["lane_ids"],
            (
                "binding-analysis-ascii-svg-request-empty",
                "binding-analysis-ascii-svg-request-resource-override",
                "binding-analysis-ascii-svg-request-version-only",
            ),
        )
        self.assertEqual(
            corpus.default_group,
            "binding_request_version_only_analysis_ascii_svg",
        )
        self.assertEqual(
            resolve_lane_group(corpus, corpus.default_group).id,
            "binding-analysis-ascii-svg-request-version-only",
        )
        self.assertEqual(
            fixture.source,
            "crates/merman-bindings-core/benches/fixtures/info_fixed_cost.mmd",
        )
        lanes = {lane.id: lane for lane in corpus.lanes}
        self.assertEqual(
            {
                lane_id: (
                    lane.kind,
                    lane.owner,
                    lane.public_operation,
                    lane.transport,
                    lane.process_lifecycle,
                    lane.engine_lifecycle,
                    lane.required_features,
                    lane.logical_operations_per_estimate,
                    lane.measurement_metrics,
                    lane.size_vector,
                    lane.workload,
                )
                for lane_id, lane in lanes.items()
            },
            {
                "binding-analysis-ascii-svg-request-empty": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-empty-trusted-native-v1",
                ),
                "binding-analysis-ascii-svg-request-version-only": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-version-only-trusted-native-v1",
                ),
                "binding-request-version-only-memory": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-system-allocator-subprocess",
                    "fresh-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("allocation_count", "allocated_bytes", "peak_growth_bytes"),
                    (1, 2, 4, 10, 32, 100),
                    "binding-semantic-info-analysis-ascii-svg-version-only-operation-calls-v1",
                ),
                "binding-analysis-ascii-svg-request-resource-override": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-resource-max-source-4096-trusted-native-v1",
                ),
            },
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "missing=",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus,
                "\n".join(
                    f"{bench}: benchmark" for bench in expected_benches[:-1]
                ),
                enabled_features=("analysis", "ascii", "svg"),
            )

    def test_canary_suite_is_standard_hotspot_set(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(
            fixture_names_for_suite(corpus, "canary"),
            (
                "flowchart_medium",
                "class_medium",
                "mindmap_medium",
                "architecture_medium",
            ),
        )

    def test_frontmatter_suite_covers_preprocess_fixtures(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(
            fixture_names_for_suite(corpus, "frontmatter"),
            (
                "frontmatter_basic",
                "frontmatter_indented",
                "frontmatter_deep_config",
            ),
        )

    def test_full_suite_uses_all_fixtures_in_corpus_order(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(select_corpus_fixtures(corpus, "full"), list(corpus.fixtures))

if __name__ == "__main__":
    sys.exit(unittest.main())
