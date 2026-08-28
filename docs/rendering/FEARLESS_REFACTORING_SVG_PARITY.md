# Fearless Refactoring Contract for SVG Parity

This document records the current refactoring boundary for
`crates/merman-render/src/svg/parity/*`. The migration described by the earlier Stage B plan is
complete; ADR-0073 is the authoritative ownership decision.

## Current Architecture

- Built-in SVG rendering consumes an opaque `FamilyRenderArtifact` whose semantic model and layout
  belong to the same typed family.
- `RenderEnvironment` freezes measurement, math and icon services, time, randomness, resource
  limits, and measurement provenance once per operation.
- Every built-in family routes root sizing and emission through the internal Root Viewport protocol:
  `RootViewportContext`, `RootViewportSpec`, `RootViewportPlan`, `RootChrome`, and, for late-bound
  roots, opaque `RootDocument`.
- Root bounds are always computed from family geometry or deferred emitted-content bounds. There is
  no generated root lookup, fixture-id key, or generated-versus-computed policy split.
- Root Viewport owns fixed/responsive sizing, max-width formatting, finite normalization,
  accessibility chrome, escaping, and DOM-compatible root attribute order. Families retain their
  source-backed content-bounds and root-algorithm inputs.

## Refactoring Rules

- Preserve the pinned Mermaid 11.16 grammar, semantic, layout, and observable SVG behavior.
- Keep family behavior in the owning vertical family module. Shared code must own a genuinely
  cross-family operation, not provide a pass-through wrapper.
- Do not introduce a JSON-first built-in render path or independently pair typed semantics and
  layout.
- Do not instantiate production text measurers inside family code or read process-global render
  policy.
- Do not emit root SVG attributes outside Root Viewport.
- Do not add fixture overrides, complete-label tables, magic geometry constants, model distortion,
  or broad comparator normalization.
- Browser-only text and root residuals remain explicit under ADR-0057 and ADR-0062.

## Verification

Focused family checks should be followed by the shared gates:

```sh
cargo fmt --all --check
cargo nextest run -p merman-render
cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3
cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Parity commands must report `headless-operation-typed` and execute the same canonical operation as
public callers. Compatibility JSON checks are separate projection evidence, not an alternate SVG
oracle.

Architecture guards enforce the ownership boundaries. They should reject forbidden capabilities,
not freeze private helper spelling or prevent family-local source-backed algorithms from evolving.
