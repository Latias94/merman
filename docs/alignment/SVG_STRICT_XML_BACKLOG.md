# Strict SVG Canonical XML Backlog (Mermaid@11.12.2)

This note tracks the current gaps for byte-level **canonical SVG XML** parity when running:

- `cargo run -p xtask -- compare-svg-xml --dom-mode strict --dom-decimals 3`

Unlike DOM parity mode (used for day-to-day regression checks), `strict` canonical XML compares include:

- `<style>` contents
- full text contents
- all geometry attributes (subject to `--dom-decimals`)

## Current status (as of 2026-01-28)

Total strict mismatches: **311**

Mismatch counts by diagram:

- `gantt`: 65
- `sequence`: 40
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
- `er`: 10
- `requirement`: 9
- `journey`: 8
- `timeline`: 8

## Workflow

1. Generate the report:
   - `cargo run -p xtask -- compare-svg-xml --dom-mode strict --dom-decimals 3`
2. Inspect the mismatch list:
   - `target/compare/xml/xml_report.md`
3. Diff a single fixture:
   - `git diff --no-index target/compare/xml/<diagram>/<fixture>.upstream.xml target/compare/xml/<diagram>/<fixture>.local.xml`
