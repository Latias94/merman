# Strict SVG Canonical XML Backlog (Mermaid@11.12.2)

This note tracks the current gaps for byte-level **canonical SVG XML** parity when running:

- `cargo run -p xtask -- compare-svg-xml --dom-mode strict --dom-decimals 3`

Unlike DOM parity mode (used for day-to-day regression checks), `strict` canonical XML compares include:

- `<style>` contents
- full text contents
- all geometry attributes (subject to `--dom-decimals`)

Notes:

- Strict canonicalization keeps identifier-like attributes byte-for-byte (e.g. `id="flowchart-A-0"` is not rewritten).
- Strict canonicalization preserves mixed-content text segments (e.g. `foo<br />bar` keeps the `bar` tail text).

## Current status (as of 2026-01-30)

Total strict mismatches: **197**

Mismatch counts by diagram:

- `sequence`: 26
- `state`: 36
- `architecture`: 25
- `block`: 22
- `class`: 16
- `kanban`: 15
- `gitgraph`: 14
- `mindmap`: 11
- `pie`: 11
- `xychart`: 11
- `c4`: 10
- `gantt`: 0
- `flowchart`: 0

Recently resolved:

- `gantt`: 0 (was 65)
- `er`: 0 (was 5)
- `flowchart`: 0 (was 7)
- `journey`: 0 (was 8)
- `requirement`: 0 (was 9)
- `timeline`: 0 (was 1)

### Flowchart notes

- Strict parity for flowchart text metrics relies on a small set of vendored per-string overrides in
  `crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs` to exactly match
  upstream `getBoundingClientRect()` / `getBBox()` lattice values (1/64px / binary fractions).
- `<strong>/<b>` HTML runs use a full bold delta model in `crates/merman-render/src/text.rs` to
  match Mermaid@11.12.2 upstream fixtures.
- When text metrics change, some layout goldens may need regeneration via
  `cargo run -p xtask -- update-layout-snapshots --filter <fixture>`.

### Sequence notes

- Sequence strict XML parity is particularly sensitive to `calculateTextDimensions(...)` width for
  message labels (it affects `actor.margin` and thus most x coordinates).
- `xtask gen-svg-overrides --mode sequence` infers upstream `calculateTextDimensions.width` for
  message labels by rendering a minimal 2-participant diagram under Puppeteer headless shell and
  inverting Mermaid's `actor.margin` formula.
  - IMPORTANT: the diagram source must use real newlines (`\n`) in the Mermaid definition; joining
    lines with a literal `\\n` changes Mermaid parsing and yields a different layout baseline.

## Workflow

1. Generate the report:
   - `cargo run -p xtask -- compare-svg-xml --dom-mode strict --dom-decimals 3`
2. Inspect the mismatch list:
   - `target/compare/xml/xml_report.md`
3. Diff a single fixture:
   - `git diff --no-index target/compare/xml/<diagram>/<fixture>.upstream.xml target/compare/xml/<diagram>/<fixture>.local.xml`
