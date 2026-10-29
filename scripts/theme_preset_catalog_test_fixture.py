"""Load the shared production catalog for synthetic theme contract tests."""

import json
from pathlib import Path


def canonical_theme_preset_catalog() -> dict[str, object]:
    fixture = (
        Path(__file__).resolve().parents[1]
        / "crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json"
    )
    return {
        "schema_version": 1,
        "presets": json.loads(fixture.read_text(encoding="utf-8")),
    }
