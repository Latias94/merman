"""Generate the package-local, compile-time Criterion inputs from the native corpus."""

from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path

from .corpus import load_corpus


ROOT = Path(__file__).resolve().parents[3]
PIPELINE = Path("crates/merman/benches/pipeline.rs")
PIPELINE_STAGE_MODULE = Path("pipeline/stages.rs")
BEGIN = b"// Generated from tools/bench/corpus.json by tools/bench/bench.py sync-fixtures --write."
LEGACY_BEGIN = b"// Generated from tools/bench/corpus.json by tools/bench/pipeline_fixtures.py --write."
END = b"// End of generated pipeline fixtures."


def sync_pipeline_fixtures(root: Path, source: Path, *, write: bool = False) -> tuple[Path, ...]:
    """Return the verified generated paths, or an empty tuple for a hand-written harness."""
    benches = source.resolve().parent
    stages = benches / PIPELINE_STAGE_MODULE
    raw = source.read_bytes()
    if (
        source.name == PIPELINE.name
        and BEGIN not in raw
        and LEGACY_BEGIN not in raw
        and stages.parent.is_dir()
    ):
        source = stages
        raw = source.read_bytes()

    before, marker, rest = raw.partition(BEGIN)
    if not marker:
        before, marker, rest = raw.partition(LEGACY_BEGIN)
    if not marker:
        return ()
    _, end, after = rest.partition(END)
    if not end:
        raise ValueError(f"incomplete generated fixture registry in {source}")

    newline = b"\r\n" if rest.startswith(b"\r\n") else b"\n"
    root = root.resolve()
    corpus = load_corpus(root / "tools/bench/corpus.json")
    # Ranked controls ran first in Criterion. All unranked rows retain corpus order.
    fixtures = sorted(corpus.fixtures, key=lambda f: (f.criterion_order is None, f.criterion_order))
    groups: dict[str, list[str]] = {"DIAGRAM_FIXTURES": [], "FRONTMATTER_FIXTURES": []}
    expected: dict[Path, bytes] = {}
    for fixture in fixtures:
        if not re.fullmatch(r"[A-Za-z0-9_.-]+", fixture.name):
            raise ValueError(f"invalid Criterion fixture name: {fixture.name!r}")
        origin = (root / fixture.source).resolve(strict=True)
        origin.relative_to(root)
        local = origin
        if not origin.is_relative_to(benches.parent):
            # Cargo packages contain only crate-local files. Keep upstream bytes verbatim.
            local = benches / "support/corpus" / f"{fixture.name}.mmd"
            expected[local] = origin.read_bytes()
        args = [fixture.name]
        if local != benches / "fixtures" / f"{fixture.name}.mmd":
            args.append(Path(os.path.relpath(local, benches)).as_posix())
        literals = ", ".join(json.dumps(arg, ensure_ascii=False) for arg in args)
        group = "FRONTMATTER_FIXTURES" if fixture.family == "frontmatter" else "DIAGRAM_FIXTURES"
        groups[group].append(f"    fixture!({literals}),\n")

    block = "\n".join(
        f"#[rustfmt::skip]\nconst {name}: &[(&str, &str)] = &[\n{''.join(rows)}];\n"
        for name, rows in groups.items()
    )
    registry = block.encode("utf-8").replace(b"\n", newline)
    expected[source] = before + marker + newline + registry + end + after
    for path, data in expected.items():
        if not path.is_file() or path.read_bytes() != data:
            if not write:
                command = (
                    "tools/bench/pipeline_fixtures.py"
                    if marker == LEGACY_BEGIN
                    else "tools/bench/bench.py sync-fixtures"
                )
                raise ValueError(f"stale {path}: run python {command} --write")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    return tuple(expected)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh the registry and package copies")
    try:
        if not sync_pipeline_fixtures(ROOT, ROOT / PIPELINE, write=parser.parse_args(argv).write):
            raise ValueError("missing generated pipeline fixture registry")
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")

    return 0
