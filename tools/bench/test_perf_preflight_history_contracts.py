#!/usr/bin/env python3
"""Keep historical runner contracts local to their own checkout."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

import compare_self
from perf_contract_test_support import minimal_corpus


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = Path(compare_self._NATIVE_CRITERION_PREFLIGHT_CONTRACT)


class PreflightHistoryContractsTest(unittest.TestCase):
    def _checkout(
        self,
        checkout: Path,
        *,
        corpus_has_editor: bool,
        contract_has_editor: bool,
    ) -> tuple[compare_self.RunnerRecipe, Path, Path]:
        corpus = minimal_corpus(schema_version=2, default_group="end_to_end")
        if not corpus_has_editor:
            corpus["lanes"] = [
                lane for lane in corpus["lanes"]
                if lane["selector"] != "editor_snapshot/{fixture}"
            ]
        contract = json.loads((ROOT / CONTRACT).read_text(encoding="utf-8"))
        if not contract_has_editor:
            contract["output_kinds"].pop("editor_snapshot")
        corpus_path = checkout / "tools/bench/corpus.json"
        contract_path = checkout / CONTRACT
        for path, value in ((corpus_path, corpus), (contract_path, contract)):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value) + "\n", encoding="utf-8")
        recipe = compare_self.RunnerRecipe(
            label=checkout.name,
            checkout=checkout,
            package="merman",
            bench="pipeline",
            features=("all-diagrams", "svg", "compat-json", "editor-facts")
            if corpus_has_editor else ("svg",),
            default_features=True,
            toolchain=None,
            target_dir=checkout / "target",
            locked=True,
            corpus=Path("tools/bench/corpus.json"),
        )
        return recipe, corpus_path, contract_path

    def test_each_runner_uses_its_checkout_preflight_groups(self) -> None:
        # Run 40294905358 compared a seven-group base with an eight-group head.
        with tempfile.TemporaryDirectory() as temp:
            for has_editor in (False, True):
                with self.subTest(editor_snapshot=has_editor):
                    recipe, corpus, contract = self._checkout(
                        Path(temp) / str(has_editor),
                        corpus_has_editor=has_editor,
                        contract_has_editor=has_editor,
                    )
                    description = compare_self._describe_corpus(corpus, recipe=recipe)
                    self.assertTrue(description["preflight_receipts_required"])
                    self.assertEqual(
                        description["preflight_contract"]["sha256"],
                        hashlib.sha256(contract.read_bytes()).hexdigest(),
                    )
                    self.assertEqual(
                        len(description["preflight_contract"]["output_kinds"]),
                        8 if has_editor else 7,
                    )

    def test_corpus_and_contract_groups_must_agree_within_each_checkout(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            for corpus_has_editor in (False, True):
                with self.subTest(corpus_has_editor=corpus_has_editor):
                    recipe, corpus, _contract = self._checkout(
                        Path(temp) / str(corpus_has_editor),
                        corpus_has_editor=corpus_has_editor,
                        contract_has_editor=not corpus_has_editor,
                    )
                    with self.assertRaisesRegex(
                        compare_self.ContractViolation, "lane groups differ"
                    ):
                        compare_self._describe_corpus(corpus, recipe=recipe)

    def test_historical_contract_acceptance_is_limited_to_known_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            for has_editor in (False, True):
                recipe, corpus, contract = self._checkout(
                    Path(temp) / str(has_editor),
                    corpus_has_editor=has_editor,
                    contract_has_editor=has_editor,
                )
                original = json.loads(contract.read_text(encoding="utf-8"))
                for mutation in ("missing_group", "wrong_kind", "extra_group", "prefix"):
                    value = copy.deepcopy(original)
                    if mutation == "missing_group":
                        value["output_kinds"].pop("render")
                    elif mutation == "wrong_kind":
                        value["output_kinds"]["render"] = "prepared_layout"
                    elif mutation == "extra_group":
                        value["output_kinds"]["unregistered"] = "svg"
                    else:
                        value["line_prefix"] = "unregistered"
                    contract.write_text(json.dumps(value), encoding="utf-8")
                    with self.subTest(editor_snapshot=has_editor, mutation=mutation):
                        with self.assertRaisesRegex(
                            compare_self.ContractViolation, "content differs"
                        ):
                            compare_self._describe_corpus(corpus, recipe=recipe)

    def test_different_historical_harnesses_still_fail_confirmation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            runners = []
            for side, has_editor in (("base", False), ("head", True)):
                recipe, corpus, _contract = self._checkout(
                    Path(temp) / side,
                    corpus_has_editor=has_editor,
                    contract_has_editor=has_editor,
                )
                runners.append(SimpleNamespace(provenance={
                    "bench_target": {"sha256": "a" * 64},
                    "bench_source": {"sha256": "b" * 64},
                    "corpus": compare_self._describe_corpus(corpus, recipe=recipe),
                }))
            self.assertEqual(
                compare_self._confirmation_harness_identity_errors(*runners),
                [
                    "confirmation requires byte-identical corpus manifest",
                    "confirmation requires a byte-identical preflight contract",
                ],
            )


if __name__ == "__main__":
    unittest.main()
