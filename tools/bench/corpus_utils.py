#!/usr/bin/env python3
"""
Shared helpers for the corpus-driven benchmark scripts.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class CorpusFixture:
    name: str
    family: str
    size: str
    category: str
    source: str
    suites: tuple[str, ...]
    features: tuple[str, ...]
    quality: tuple[str, ...]


@dataclass(frozen=True)
class Corpus:
    schema_version: int
    default_group: str
    suites: dict[str, str]
    fixtures: tuple[CorpusFixture, ...]


def load_corpus(path: Path) -> Corpus:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"corpus must be a JSON object: {path}")
    schema_version = int(data.get("schema_version", 0))
    if schema_version != 1:
        raise ValueError(f"unsupported corpus schema_version: {schema_version}")

    suites_raw = data.get("suites") or {}
    if not isinstance(suites_raw, dict):
        raise ValueError("corpus.suites must be an object")
    suites = {str(k): str(v) for k, v in suites_raw.items()}
    suites.setdefault("full", "All fixtures in corpus order.")

    fixtures_raw = data.get("fixtures") or []
    if not isinstance(fixtures_raw, list):
        raise ValueError("corpus.fixtures must be a list")

    seen: set[str] = set()
    fixtures: list[CorpusFixture] = []
    for idx, item in enumerate(fixtures_raw):
        if not isinstance(item, dict):
            raise ValueError(f"fixture entry {idx} must be an object")
        name = str(item.get("name") or "").strip()
        if not name:
            raise ValueError(f"fixture entry {idx} is missing name")
        if name in seen:
            raise ValueError(f"duplicate fixture in corpus: {name}")
        seen.add(name)

        def str_tuple(key: str) -> tuple[str, ...]:
            value = item.get(key) or []
            if isinstance(value, str):
                return (value,)
            if not isinstance(value, list):
                raise ValueError(f"fixture {name}.{key} must be a string or list")
            return tuple(str(v) for v in value)

        fixtures.append(
            CorpusFixture(
                name=name,
                family=str(item.get("family") or "unknown"),
                size=str(item.get("size") or "unknown"),
                category=str(item.get("category") or "standard"),
                source=str(item.get("source") or f"crates/merman/benches/fixtures/{name}.mmd"),
                suites=str_tuple("suites"),
                features=str_tuple("features"),
                quality=str_tuple("quality"),
            )
        )

    return Corpus(
        schema_version=schema_version,
        default_group=str(data.get("default_group") or "end_to_end"),
        suites=suites,
        fixtures=tuple(fixtures),
    )


def select_corpus_fixtures(corpus: Corpus, suite: str) -> list[CorpusFixture]:
    if suite == "full":
        return list(corpus.fixtures)
    fixtures = [f for f in corpus.fixtures if suite in f.suites]
    if not fixtures:
        available = ", ".join(sorted(corpus.suites))
        raise ValueError(f"unknown or empty suite {suite!r}; available suites: {available}")
    return fixtures


def fixture_names_for_suite(corpus: Corpus, suite: str) -> tuple[str, ...]:
    return tuple(f.name for f in select_corpus_fixtures(corpus, suite))
