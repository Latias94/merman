# ADR-0062: Fixture-Derived Overrides (Parity Stabilization Without Weakening the Contract)

## Status

Accepted

## Context

`merman` is a 1:1, headless re-implementation of Mermaid pinned to a specific upstream tag (see ADR-0014).
Upstream Mermaid renders diagrams in a browser pipeline, which means the authoritative SVG baselines encode:

- browser-derived float lattices (`getBBox()`, `getComputedTextLength()`, serialization)
- platform font fallback behavior (especially for non-Latin glyphs)
- renderer quirks that are not representable in a “pure” semantic model (e.g. `NaN` coordinates)

For regression safety we gate on DOM parity against official Mermaid CLI SVG baselines.
However, byte-identical SVG is not always attainable early, and even DOM parity can become unstable if
tiny browser-specific viewport numbers change across otherwise-correct renders.

We need a mechanism that:

- keeps release gates stable (ADR-0050)
- remains auditable and version-pinned
- does **not** weaken the semantic/structural parity contract

## Decision

We adopt **fixture-derived overrides** as a first-class, explicitly-scoped mechanism to stabilize parity.

### What “override” means in this repository

An override is a small, deterministic adjustment that is:

- **derived from upstream SVG baselines** for the pinned Mermaid version
- **scoped** (root viewport surface, text bbox, or a documented upstream oddity)
- **keyed** (by `diagram_id` fixture stem, or by exact label string + font key)
- applied only where required to keep parity checks stable

Overrides are not “make it look right” knobs. They are *traceable* parity shims to model upstream behavior
that is currently impractical to reproduce algorithmically in a pure Rust pipeline.

### Override categories (allowed)

1. **Root viewport overrides (`parity-root` only)**
   - Scope: root `<svg>` `viewBox` and `style="max-width: …px"`.
   - Key: `diagram_id` (fixture stem).
   - Source: exact values extracted from `fixtures/upstream-svgs/**`.
   - Rationale: upstream uses browser `getBBox()`; a headless pipeline can match structure while still
     drifting in viewport numbers due to float lattice differences.

2. **Text / bbox overrides (string-keyed)**
   - Scope: text measurement results for specific label strings and font keys.
   - Key: `(font_key, text)` (exact string match).
   - Source: generated from upstream SVG baselines when vendored font tables are insufficient.
   - Rationale: browser font fallback (CJK/emoji) can change wrap decisions and thus the SVG DOM.

3. **Upstream-oddity compatibility markers (documented, minimal)**
   - Scope: rare cases where upstream emits behavior that is not representable directly in JSON snapshots
     (e.g. `NaN` values).
   - Policy: encode the semantic intent with an explicit marker, and re-materialize the upstream oddity
     only in the SVG parity surface.

### Governance rules

- Overrides must be **version-pinned** to the upstream baseline (`11.12.2` today).
- Overrides must be **traceable** to an upstream fixture and reproducible from baselines.
- Prefer **general fixes** first (layout/text algorithms); add overrides only when the remaining delta is
  primarily browser/font lattice behavior.
- Every override footprint must stay **auditable**:
  - summary: `cargo run -p xtask -- report-overrides`
  - files: `crates/merman-render/src/generated/*_overrides_11_12_2.rs`

### Paydown strategy (avoid “overfitting debt”)

Overrides are acceptable, but they should not grow without control. We treat them like debt with a plan:

- Expand fixtures in diverse batches so overrides are forced to generalize.
- Track removal candidates in `docs/alignment/GAP_BACKLOG.md`.
- Prefer replacing fixture-scoped tweaks with:
  - better deterministic text measurement (ADR-0049 / ADR-0051)
  - renderer/layout algorithm alignment
  - more accurate bbox modeling

## Consequences

- Release gates remain stable while the fixture corpus grows (ADR-0050).
- Overrides become explicit, reviewable artifacts rather than hidden “magic constants”.
- Some parity improvements will land as “generated override deltas” before they can be fully generalized.
- The project gains a measurable convergence metric: override footprint should trend down over time as
  algorithms converge.

