# Rendering Refactor TODO (Fearless, Parity-First)

This is a tactical TODO list for refactoring `crates/merman-render/src/svg/parity/*` while keeping
all release gates green.

Related docs:

- Design: `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
- Milestones: `docs/rendering/REFACTOR_MILESTONES.md`

## Guardrails (must stay green)

- `cargo fmt --check`
- `cargo nextest run`
- SVG DOM gates:
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`

## TODO (prioritized)

### P0: Reduce duplication with zero behavior change

- [x] Introduce a shared root `<svg>` writer (open tag) and adopt it in a single low-risk diagram
      (ER) first, then expand adoption diagram-by-diagram.
- [x] Centralize root viewport override application (parse viewBox → w/h + style max-width) into a
      helper and use it across diagrams.
- [x] Support root attribute ordering quirks (e.g. `viewBox` before `style`) in the shared writer
      so strict XML diffs can remain stable while refactoring.
- [ ] Consolidate common SVG escaping + number formatting usage so diagram renderers don’t reach for
      ad-hoc `format!` / `write!` patterns.
- [x] Extend the shared writer to support root attribute placement quirks beyond `viewBox`/`style`
      ordering (e.g. `style` after aria, fixed-size `height` placement) and adopt it in diagrams that
      depend on that ordering.
- [x] Migrate the remaining Stage B root `<svg>` emitters (`sequence`, `state`) to the shared writer.

### P1: Diagram module structure

- [ ] Split `flowchart.rs` into submodules by concern:
      - `layout_to_svg/*` (nodes/edges/clusters)
      - `root/*` (viewport + acc metadata)
      - `defs/*` (markers + filters)
      - `css/*`
- [ ] Split the class renderer into submodules (in progress: moved to `svg/parity/class/*`, extracted `debug_svg`, `defs`, `label`, `rough`).
- [ ] Create a consistent naming convention for “Stage B” parity render entry points across diagrams.

### P2: Overrides and tooling ergonomics

- [ ] Add an `xtask` command to update root viewport overrides from a report, e.g.
      `xtask update-root-overrides --diagram <name> --from-report <path>`.
- [ ] Add a “coverage sanity” report for root viewport overrides:
      - list mismatch stems that lack overrides
      - list overrides that no longer affect any fixture
- [ ] Make `xtask verify-generated` not fail on missing optional `repo-ref/*` build artifacts, or add
      a dedicated `xtask bootstrap` that materializes them.

### P3: Output stability and debug UX

- [ ] Provide a per-diagram “debug bundle” emitter that can write:
      - semantic JSON
      - layout JSON
      - local SVG
      - diff report (if any)
- [ ] Add a tiny helper that prints “root viewport deltas” (viewBox/max-width) for a single fixture
      without generating full reports.
