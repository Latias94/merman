# Canonical SVG XML Comparison

This repository stores upstream Mermaid SVG outputs under `fixtures/upstream-svgs/**` and uses DOM
parity compares (`xtask compare-*-svgs`) for day-to-day regression checks.

For stricter alignment work, `xtask compare-svg-xml` compares **canonicalized** SVG XML between:

- upstream: `fixtures/upstream-svgs/<diagram>/*.svg` (generated via Mermaid CLI pinned to Mermaid `@11.12.2`)
- local: `merman-render` Stage-B SVG output for the corresponding `fixtures/<diagram>/*.mmd`

## What “canonical XML” means here

This is not byte-for-byte SVG parity. Instead, we parse SVG into a tree and re-emit a normalized
representation so comparisons are stable across:

- attribute ordering
- insignificant whitespace differences
- numeric formatting noise (rounded via `--dom-decimals`, including decoded `data-points` in `strict` mode)

Canonical XML is meant as a stepping stone toward stricter parity, while keeping diffs readable and
deterministic.

## Usage

- Generate a report (does not fail the build):
  - `cargo run -p xtask -- compare-svg-xml`

- Fail on mismatches (recommended in CI once stable):
  - `cargo run -p xtask -- compare-svg-xml --check`

- Narrow scope:
  - `cargo run -p xtask -- compare-svg-xml --diagram flowchart --filter titled`

## Options

- `--dom-mode <mode>`
  - Supported: `strict`, `structure`, `parity`, `parity-root`
  - Recommendation for canonical XML: `strict` (default)
- `--dom-decimals <n>`
  - Rounds numeric tokens to reduce float drift (default: `3`)
  - In `strict` mode this also normalizes `data-points` by decoding the Base64 JSON payload, rounding JSON numbers,
    and re-encoding.
- `--text-measurer deterministic|vendored`
  - Default: `vendored` (uses vendored font tables where available, falls back deterministically)

## Outputs

When mismatches are found, the canonical XML files are written to:

- `target/compare/xml/<diagram>/<fixture>.upstream.xml`
- `target/compare/xml/<diagram>/<fixture>.local.xml`

And the summary report is written to:

- `target/compare/xml/xml_report.md`
