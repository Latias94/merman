# Fearless Refactoring Design (Rendering / SVG Parity)

This document describes a “fearless refactoring” plan for the Stage B SVG parity renderers under:

- `crates/merman-render/src/svg/parity/*`

The key principle: **refactor without changing behavior**, backed by strong release gates.

## Status (as of 2026-03-02)

- A shared root viewport override helper exists (`apply_root_viewport_override`).
- A shared root `<svg>` open-tag writer exists (`push_svg_root_open` / `push_svg_root_open_ex`),
  including support for:
  - additional root attributes (e.g. `preserveAspectRatio`, `height`)
  - `viewBox`/`style` ordering quirks (to keep strict XML diffs stable)
- Diagrams already migrated to the shared root writer: `er`, `requirement`, `journey`, `timeline`,
  `kanban`, `gitgraph`, `gantt`, `packet`, `pie`, `xychart`, `block`, `error`, `treemap`, `info`,
  `quadrantchart`.

## Goals

- Reduce duplication in root `<svg>` emission (viewport + accessibility + style).
- Make `parity-root` behavior explicit and centralized.
- Make large diagram renderers easier to navigate and modify safely.
- Improve tooling ergonomics for maintaining fixture-scoped overrides.

## Non-goals

- Changing visual output (unless explicitly planned as a separate, justified parity fix).
- Relaxing release gates.
- “Rewrite everything” or large-scale reorganizations without incremental checkpoints.

## Correctness guardrails (must stay green)

- Formatting: `cargo fmt --check`
- Tests: `cargo nextest run`
- SVG DOM gates:
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`

Notes:

- `strict` SVG XML comparisons are useful but are intentionally not release gates.

## Current pain points (observed)

- Root `<svg>` emission is duplicated across diagrams:
  - `style` / `viewBox` / `width` / `height`
  - aria attributes and `<title>` / `<desc>`
  - `parity-root` overrides are applied with per-diagram ad-hoc parsing
- Some diagram renderers are very large single files (Flowchart/Class/State), which increases:
  - change risk
  - review time
  - accidental drift across diagrams
- `xtask verify-generated` can fail due to missing `repo-ref/*` build artifacts, which is confusing
  when the actual release gates are green.

## Proposed architecture (incremental)

### 1) Root viewport override helper

Introduce a small helper that:

- takes a `diagram_id`
- takes default `viewBox` / `max-width` / `width` / `height` strings
- applies a lookup from `crates/merman-render/src/generated/*_root_overrides_11_12_2.rs`
- returns the final strings used by the root `<svg>` emission

This prevents:

- duplicated parsing logic
- inconsistent edge cases (e.g. missing/placeholder `max-width`)

### 2) Shared root `<svg>` writer

Introduce a shared writer that can emit a root `<svg>` tag with:

- fixed-size mode (`width`/`height`)
- max-width mode (`width="100%"` + `style="max-width: ...px; ..."`)
- aria attributes (role, roledescription, describedby, labelledby)
- optional `<title>` and `<desc>`
- a `<style>` node wrapping diagram CSS

Design constraints:

- Avoid changing attribute names/values.
- Keep attribute order stable where practical (helps strict-mode diffs).
- Avoid allocations where easy, but correctness takes priority.

### 3) Module normalization for large diagrams

Adopt a consistent submodule split for large renderers:

- `root.rs` (viewport + accessibility)
- `css.rs`
- `defs.rs` (markers/filters)
- `render.rs` (main emission)
- `debug.rs` (debug-only SVG helpers)

Start with ER (smallest blast radius), then Flowchart/Class.

### 4) Tooling: update overrides from reports

Extend `xtask` with a command that can take a compare report and update the corresponding override
module, ideally by:

- parsing upstream root `viewBox` / `style max-width`
- generating match arms in sorted order
- updating the file while preserving existing entries

This reduces manual drift and makes override maintenance cheap.

## Rollout plan (safe increments)

1. Add documentation + TODO + milestones.
2. Add root viewport helper; adopt in ER.
3. Add shared root `<svg>` writer; adopt in ER.
4. Expand adoption diagram-by-diagram.
5. Split large modules once the shared root writer is stable.
6. Add `xtask` automation for overrides.

## Validation strategy

Every commit should keep gates green. For focused validation, use:

- `cargo run --release -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

before running the full `compare-all-svgs` gates.
