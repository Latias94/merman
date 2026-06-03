# HPD-080 - C4 Headless-Shell Text Measurement

Date: 2026-06-03

## Context

The C4 parity investigation narrowed the drift to browser-backed SVG text measurement rather than
Mermaid syntax or renderer structure. The stored upstream C4 baselines correspond to
`mmdc + chrome-headless-shell`, while the local vendored fallback and Edge-backed browser probes
measured the same C4 description strings wider.

The highest-signal sample was
`fixtures/c4/upstream_docs_c4_c4_diagrams_001.mmd`: the `SystemAA` description
`Allows customers to view information about their bank accounts, and make payments.` measures
`532.484375px` in the headless-shell baseline. C4 box sizing then rounds that to `532px` and adds
the C4 padding, yielding the expected `552px` box width and `1059px` root width.

## Outcome

- Added a generated C4 text width table keyed by normalized font family, font size, font weight,
  and exact text.
- Added `xtask gen-c4-text-overrides` to regenerate those C4 measurements from upstream C4 SVG text
  nodes through a browser `getBBox()` probe.
- C4 layout now defaults its measurement font family to Mermaid's emitted C4 default,
  `"Open Sans", sans-serif`, instead of letting `None` fall through to the generic vendored default
  stack.
- `measure_c4_text(...)` now uses the generated headless-shell text width when available, falling
  back to deterministic SVG text bbox measurement for uncaptured text.
- C4 layout goldens were refreshed because the internal layout model now reflects the pinned
  upstream measurement environment.
- `report-overrides --check-no-growth` now classifies the C4 generated table as text lookup data
  instead of a hand-curated helper.

## Touched Surfaces

- `crates/merman-render/src/c4.rs`
- `crates/merman-render/src/generated/c4_text_overrides_11_12_2.rs`
- `crates/merman-render/src/generated/mod.rs`
- `crates/xtask/src/cmd/overrides/c4.rs`
- `crates/xtask/src/cmd/overrides/mod.rs`
- `crates/xtask/src/cmd/overrides/report.rs`
- `crates/xtask/src/main.rs`
- `fixtures/c4/*.layout.golden.json`

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render c4` - passed, `5` tests run.
- `cargo nextest run -p merman --features render c4` - passed, `2` tests run.
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='c4'; cargo nextest run -p merman --features render --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit` -
  passed, `1` filtered C4 fixture audit run.
- `cargo nextest run -p merman-render fixtures_match_layout_golden_snapshots_when_present` -
  passed, full layout snapshot gate run.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-decimals 3 --out target\compare\c4_report_parity_after_hpd080_text_measurement.md` -
  passed, all C4 fixtures matched structurally.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\c4_report_parity_root_after_hpd080_text_measurement.md` -
  passed, all C4 fixtures matched in root mode.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed with C4 counted as `201`
  text lookup entries.
- `cargo nextest run -p xtask` - passed, `84` tests run.
- `git diff --check` - passed.

## Residual Boundary

This is a C4 measurement-environment seam, not a broad font-model replacement. The generated table
is intentionally scoped to C4 exact text contexts because changing the shared `sans-serif` or
Open Sans metrics would risk unrelated diagram families. Future removal should happen only when
the shared text backend can reproduce the pinned headless-shell C4 widths without exact text
lookup rows.
