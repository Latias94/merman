#!/usr/bin/env python3
"""
Shared helpers for the corpus-driven benchmark scripts.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


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


def resolve_merman_fixture_path(
    repo_root: Path,
    name: str,
    fixture: CorpusFixture | None = None,
) -> Path:
    candidates: list[Path] = []
    if fixture is not None:
        candidates.append(repo_root / fixture.source)
    candidates.append(repo_root / "crates" / "merman" / "benches" / "fixtures" / f"{name}.mmd")
    return next((path for path in candidates if path.exists()), candidates[0])


def compare_mmdr_fixture_inputs(
    *,
    repo_root: Path,
    mmdr_dir: Path,
    fixture_names: Iterable[str],
    fixtures_by_name: dict[str, CorpusFixture] | None = None,
) -> dict[str, dict[str, object]]:
    comparisons: dict[str, dict[str, object]] = {}
    metadata = fixtures_by_name or {}

    for name in dict.fromkeys(fixture_names):
        merman_path = resolve_merman_fixture_path(repo_root, name, metadata.get(name))
        mmdr_path = mmdr_dir / "benches" / "fixtures" / f"{name}.mmd"

        def describe(path: Path, root: Path) -> dict[str, object]:
            relative = str(path.relative_to(root)).replace("\\", "/")
            if not path.exists():
                return {"path": relative, "bytes": None, "sha256": None}
            data = path.read_bytes()
            return {
                "path": relative,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }

        merman = describe(merman_path, repo_root)
        mmdr = describe(mmdr_path, mmdr_dir)
        if merman["sha256"] is None and mmdr["sha256"] is None:
            status = "missing_both"
        elif merman["sha256"] is None:
            status = "missing_merman"
        elif mmdr["sha256"] is None:
            status = "missing_mmdr"
        elif merman["sha256"] == mmdr["sha256"]:
            status = "identical"
        else:
            status = "different"

        comparisons[name] = {
            "status": status,
            "merman": merman,
            "mermaid_rs_renderer": mmdr,
        }

    return comparisons
