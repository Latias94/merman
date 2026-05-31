# Pie 11.15 Parity - Evidence And Gates

Status: Active
Last updated: 2026-05-31

## Smallest Current Repro

```bash
cargo nextest run -p merman-render pie
```

The first failing proof should demonstrate that local Pie slice layout still sorts by value while
Mermaid 11.15 preserves input order.

## Gate Set

### Targeted Iteration Gates

```bash
cargo nextest run -p merman-render pie
cargo nextest run -p merman-core pie
cargo run -p xtask -- verify-default-config
```

### Generated Config Gates

```bash
cargo run -p xtask -- gen-default-config
cargo run -p xtask -- verify-default-config
cargo nextest run -p merman-core config
```

### SVG Parity Gates

```bash
cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Use the full Pie compare gate after fixture/baseline changes. For isolated unit/SVG public tests,
record why targeted gates are sufficient.

### Formatting And Diff Gates

```bash
cargo fmt --check
git diff --check
```

## Evidence Log

- 2026-05-31 PIE-010 scope:
  - Result: lane opened from the generated-default-config closeout follow-on.
  - Upstream evidence: `pieRenderer.ts` uses `d3pie().sort(null)` and reads `textPosition`,
    `donutHole`, `legendPosition`, and `highlightSlice`.
  - Local evidence: `crates/merman-render/src/pie.rs` still sorts visible slices by descending
    value and ignores the effective Pie config.
- 2026-05-31 PIE-020 red:
  - `cargo nextest run -p merman-render pie_slices_follow_input_order_in_mermaid_11_15`: failed
    with local slice order `B, C, A` instead of input order `A, B, C`.
  - `cargo nextest run -p merman-render pie_hidden_slices_still_reserve_color_domain_slots`:
    failed because a hidden `<1%` slice did not reserve its upstream color-domain slot.
- 2026-05-31 PIE-020 green:
  - Result: removed descending value sorting and pre-reserved the color scale domain from all Pie
    sections before hidden-slice filtering.
  - `cargo nextest run -p merman-render pie_slices_follow_input_order_in_mermaid_11_15`: passed.
  - `cargo nextest run -p merman-render pie_hidden_slices_still_reserve_color_domain_slots`:
    passed.
  - `cargo nextest run -p merman-render pie`: passed.
  - `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`:
    passed after refreshing the two affected upstream SVG baselines.
  - `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
    passed.
  - `cargo nextest run -p merman-render`: passed after refreshing affected Pie layout goldens.
  - `cargo nextest run -p merman-core pie`: passed.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
- 2026-05-31 PIE-030 red:
  - `cargo nextest run -p merman-core parse_pie_exposes_11_15_config_defaults_and_overrides`:
    failed because `pie.donutHole` was absent from the generated default config.
- 2026-05-31 PIE-030 green:
  - Result: removed Pie key removals from `default_config_overrides.json`, regenerated
    `default_config.json`, and added a regression test for default/override exposure.
  - Result: stabilized `gen-default-config` output with recursive key sorting and a trailing
    newline so the generator itself preserves the committed artifact shape.
  - `cargo run -p xtask -- gen-default-config`: passed.
  - `cargo nextest run -p merman-core parse_pie_exposes_11_15_config_defaults_and_overrides`:
    passed.
  - `cargo nextest run -p xtask default_config`: passed.
  - `cargo run -p xtask -- verify-default-config`: passed.
  - `cargo nextest run -p merman-core config`: passed.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.

## Evidence Anchors

- `docs/workstreams/pie-11-15-parity/DESIGN.md`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieRenderer.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieStyles.ts`
- `repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml`
- `crates/merman-render/src/pie.rs`
- `crates/merman-render/src/svg/parity/pie.rs`
- `crates/xtask/default_config_overrides.json`
