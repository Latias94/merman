"""Shared data builders for focused performance contract tests."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"


def minimal_corpus(*, schema_version: int, default_group: str) -> dict[str, object]:
    corpus = json.loads(CORPUS_PATH.read_text(encoding="utf-8"))
    fixture = next(
        item for item in corpus["fixtures"] if item["name"] == "flowchart_medium"
    )
    corpus["schema_version"] = schema_version
    corpus["default_group"] = default_group
    corpus["fixtures"] = [fixture]
    if schema_version == 1:
        corpus.pop("lanes", None)
    return corpus


def preflight_receipt(
    benchmark: str = "end_to_end/flowchart_medium",
    *,
    output_sha256: str = "a" * 64,
    output_bytes: int = 123,
    svg_elements: int | None = 7,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "benchmark": benchmark,
        "output_kind": "svg",
        "output_bytes": output_bytes,
        "output_sha256": output_sha256,
        "svg_elements": svg_elements,
    }
