# ASCII Architecture Deepening — Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Smallest Current Repro

```bash
cargo nextest run -p merman-ascii canvas color
```

This is the first gate because the styled text/cell module should preserve role-aware rendering
before higher-level families migrate to it.

## Gate Set

### Workstream Document Gate

```bash
git diff --check -- docs/workstreams/ascii-architecture-deepening
```

Proves the lane docs are syntactically clean before implementation starts.

### Styled Text/Cell Gate

```bash
cargo nextest run -p merman-ascii canvas color
```

Proves shared role-aware text behavior, trimming, and ANSI/HTML finalization.

### Graph Gate

```bash
cargo nextest run -p merman-ascii flowchart
```

Proves graph route planning and painting preserve supported flowchart behavior.

### Relation Graph Gate

```bash
cargo nextest run -p merman-ascii class er
```

Proves class and ER relation adapters preserve current rendering behavior.

### Sequence Gate

```bash
cargo nextest run -p merman-ascii sequence
```

Proves event planning preserves lifecycle, activation, visibility, and control-frame behavior.

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

Proves all ASCII family regressions still pass after the architectural seams are introduced.

### Closeout Gates

```bash
cargo fmt --all --check
cargo clippy -p merman-ascii --all-targets -- -D warnings
git diff --check
```

Proves formatting, lint cleanliness, and whitespace hygiene.

### Review Gate

Run `review-workstream` before accepting lane completion. Record blocking findings, missing gates,
and residual risks in this file or a journal note.

## Evidence Anchors

- `docs/workstreams/ascii-architecture-deepening/DESIGN.md`
- `docs/workstreams/ascii-architecture-deepening/TODO.md`
- `docs/workstreams/ascii-architecture-deepening/MILESTONES.md`
- `docs/workstreams/ascii-architecture-deepening/HANDOFF.md`
- `docs/workstreams/ascii-architecture-deepening/WORKSTREAM.json`

## Evidence Log

- 2026-05-30 — AAD-010 started. Workstream opened for the five ASCII architecture deepening targets.
- 2026-05-30 — AAD-010 passed `git diff --check -- docs/workstreams/ascii-architecture-deepening`.
- 2026-05-30 — AAD-020 introduced shared `StyledCell`/`StyledLine` substrate and migrated sequence
  and XYChart line buffers. Passed `cargo nextest run -p merman-ascii canvas color`,
  `cargo nextest run -p merman-ascii text`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii xychart`, `cargo nextest run -p merman-ascii`,
  `cargo fmt --all --check`, and `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-030 introduced a graph route-plan seam for top-down direct routes. Passed
  `cargo nextest run -p merman-ascii top_down_direct`, `cargo nextest run -p merman-ascii flowchart`,
  `cargo nextest run -p merman-ascii`, `cargo fmt --all --check`, and
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-040 deepened relation graph adapters by moving `RelationGraphLine` onto
  `StyledLine`, centralizing box row construction, relation line merging, and centered relation text
  writing. Passed `cargo nextest run -p merman-ascii relation_graph`,
  `cargo nextest run -p merman-ascii class er`, `cargo nextest run -p merman-ascii`,
  `cargo fmt --all --check`, and `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-050 introduced `SequenceEventPlan` for activation counts, actor visibility,
  lifecycle visibility transitions, and control frame ordering state. Passed
  `cargo nextest run -p merman-ascii event_plan`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii`, `cargo fmt --all --check`, and
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`.
- 2026-05-30 — AAD-060 added `crates/merman-ascii/ASCII_GAP_REGISTRY.md` and linked it from the
  ASCII README. Passed `git diff --check -- crates/merman-ascii docs/workstreams/ascii-architecture-deepening`.
- 2026-05-30 — AAD-070 final verification passed `cargo nextest run -p merman-ascii` with 163
  tests, `cargo fmt --all --check`, `cargo clippy -p merman-ascii --all-targets -- -D warnings`,
  and `git diff --check`.
- 2026-05-30 — AAD-070 review found no blocking workstream compliance or code-quality findings.
  Residual route/feature expansion risk is intentionally tracked in `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.
