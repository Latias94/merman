#!/usr/bin/env python3
"""Contracts for the non-admission U1 parser allocation observations."""

from __future__ import annotations

import hashlib
import json
import sys
import unittest
from pathlib import Path


BENCH_DIR = Path(__file__).resolve().parent
ROOT = BENCH_DIR.parents[1]
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

from corpus_utils import load_corpus
from native_memory import MEMORY_SCALES, MemoryContractError, validate_response
from run_native_memory import load_owner_contract, memory_recipe


CORPUS_PATH = ROOT / "tools/bench/corpus.json"
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()


def response_for(lane, contract, *, mode="operation", scale=100):
    semantic = contract["semantic_response"]
    zero = mode == "zero"
    return {
        "schema_version": 3,
        "lane_id": lane.id,
        "public_operation": lane.public_operation,
        "process_lifecycle": lane.process_lifecycle,
        "engine_lifecycle": lane.engine_lifecycle,
        "logical_operations_per_estimate": 1,
        "mode": mode,
        "scale": scale,
        "seed": 101,
        "repeat": 0,
        "pid": 1234,
        "executable_sha256": "a" * 64,
        "invocation_id": f"{lane.id}:{mode}:{scale}",
        "nonce": "b" * 32,
        "output_sha256": semantic[f"{mode}_output_sha256"],
        "workload_units": scale,
        "semantic_output": {
            **semantic[mode],
            "generated_units": 0 if zero else scale,
        },
        "snapshot_live_bytes": 100,
        "allocation_count": 0 if zero else scale * 10,
        "allocated_bytes": 0 if zero else scale * 100,
        "live_bytes_after": 100 if zero else 100 + scale * 10,
        "peak_live_bytes": 100 if zero else 100 + scale * 100,
        "peak_growth_bytes": 0 if zero else scale * 100,
        "live_bytes_after_drop": 100,
        "counter_overflowed": False,
        "counter_underflowed": False,
    }


def validate(payload, contract):
    echo_fields = (
        "lane_id", "public_operation", "process_lifecycle", "engine_lifecycle",
        "logical_operations_per_estimate", "mode", "scale", "seed", "repeat",
        "executable_sha256", "invocation_id", "nonce",
    )
    return validate_response(
        json.dumps(payload, separators=(",", ":")) + "\n",
        "",
        expected={field: payload[field] for field in echo_fields},
        semantic_contract=contract["semantic_response"],
        workload_units_per_scale=1,
    )


class ParseMemoryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = load_corpus(CORPUS_PATH)
        cls.parser_lanes = tuple(
            lane for lane in cls.corpus.lanes if lane.id.startswith("parser-")
        )
        cls.contracts = {
            lane.id: load_owner_contract(ROOT / lane.evidence_contract, lane=lane)
            for lane in cls.parser_lanes
        }

    def test_main_corpus_registers_all_nine_generated_allocator_routes(self):
        self.assertEqual(
            {lane.id for lane in self.parser_lanes},
            {
                f"parser-{family}-{route}-memory"
                for family in ("flowchart", "sequence", "state")
                for route in ("typed", "json", "snapshot")
            },
        )
        self.assertTrue(self.corpus.fixtures)
        self.assertTrue(
            any(lane.transport == "native-criterion" for lane in self.corpus.lanes)
        )
        for lane in self.parser_lanes:
            with self.subTest(lane=lane.id):
                contract = self.contracts[lane.id]
                recipe = memory_recipe(ROOT, target_dir=ROOT / "target", toolchain=None, corpus=CORPUS_PATH, contract=contract)
                self.assertEqual(lane.size_vector, MEMORY_SCALES)
                self.assertEqual(lane.transport, "native-system-allocator-subprocess")
                self.assertEqual(lane.process_lifecycle, "fresh-process")
                self.assertEqual(lane.engine_lifecycle, "reused-engine")
                self.assertEqual(contract["probe"]["bench"], "parse_memory")
                self.assertEqual(contract["probe"]["protocol_schema_version"], 3)
                self.assertEqual(recipe.features, lane.required_features)
                self.assertIn("retained_growth_bytes", lane.measurement_metrics)
                candidate_bound = lane.id in {
                    "parser-flowchart-typed-memory",
                    "parser-sequence-typed-memory",
                    "parser-sequence-json-memory",
                    "parser-sequence-snapshot-memory",
                    "parser-state-typed-memory",
                    "parser-state-json-memory",
                    "parser-state-snapshot-memory",
                }
                expected_class = "candidate-bound" if candidate_bound else "infrastructure-smoke"
                self.assertEqual(contract["evidence_class"], expected_class)
                self.assertIs(contract["candidate_admission"], candidate_bound)

    def test_probe_inputs_and_verified_projection_digests_match(self):
        for lane in self.parser_lanes:
            with self.subTest(lane=lane.id):
                contract = self.contracts[lane.id]
                for item in contract["probe"]["inputs"]:
                    self.assertEqual(item["sha256"], hashlib.sha256((ROOT / item["path"]).read_bytes()).hexdigest())
                semantic = contract["semantic_response"]
                digest = hashlib.sha256(json.dumps(semantic["operation"], separators=(",", ":")).encode()).hexdigest()
                self.assertEqual(semantic["operation_output_sha256"], digest)
                self.assertEqual(semantic["zero_output_sha256"], EMPTY_SHA256)

    def test_operation_and_zero_accept_all_registered_family_routes(self):
        for lane in self.parser_lanes:
            contract = self.contracts[lane.id]
            for mode in ("operation", "zero"):
                with self.subTest(lane=lane.id, mode=mode):
                    payload = response_for(lane, contract, mode=mode)
                    self.assertEqual(validate(payload, contract), payload)

    def test_snapshot_requires_full_success_and_requested_editor_facts(self):
        required = (
            "metadata_matches", "model_matches", "editor_facts_requested",
            "editor_facts_complete", "editor_symbols_match", "editor_spans_match_source",
            "result_alive_at_checkpoint", "recovery_characterized",
        )
        for lane in self.parser_lanes:
            if not lane.id.endswith("-snapshot-memory"):
                continue
            contract = self.contracts[lane.id]
            for field in required:
                with self.subTest(lane=lane.id, field=field):
                    self.assertIs(contract["semantic_response"]["operation"][field], True)
                    payload = response_for(lane, contract)
                    payload["semantic_output"][field] = False
                    with self.assertRaises(MemoryContractError):
                        validate(payload, contract)


if __name__ == "__main__":
    unittest.main()
