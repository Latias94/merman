# Panic Surface Policy

This repo is parity-focused, but it is also intended to be used as a library in headless contexts.
Library code should not panic on user-controlled input.

## Policy

- **No panics in library code on user input.**
  - Avoid `unwrap()` / `expect()` in production code paths that can be reached by parsing or
    rendering untrusted Mermaid text, or by calling public APIs with arbitrary data.
- **Panics are acceptable** in:
  - tests, examples, and `xtask`
  - generated code (e.g. parser generator output)
  - “impossible states” guarded by prior checks (prefer `debug_assert!` if it helps)
- When an invariant is violated, prefer:
  - returning an error when the caller can act on it
  - degrading gracefully (best-effort output) when strictness would be counterproductive (e.g.
    layout on disconnected graphs)

## Current status (2026-06-06)

- `dugong` (Dagre port):
  - No `unwrap/expect/panic!` usage in `crates/dugong/src` (production code).
  - Layout-related helpers are now defensive against:
    - empty graphs
    - disconnected graphs (build a forest instead of panicking)
    - missing node/rank metadata (treat as defaults where possible)
- `merman-core`:
  - `MermaidConfig::set_value` no longer panics if the config was constructed from a non-object
    JSON value (it coerces to an object).
- `merman-render`:
  - Class namespace edge bucketing no longer unwraps the optional namespace root after a separate
    guard. Edges without complete same-root attribution degrade to outer-edge rendering instead of
    depending on that invariant staying panic-safe.
  - State edge segment merging no longer unwraps the last accumulated point after a separate
    non-empty guard. Duplicate segment-boundary points are still skipped when present; an unexpected
    empty accumulator now falls through to normal point insertion.
  - Verification: `cargo fmt --check -p merman-render`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter namespace`
    passed for the Class namespace cleanup.
  - Verification: `cargo nextest run -p merman-render state` and
    `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the State edge segment cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo nextest run -p merman-render state` passed.
- `manatee`:
  - FCoSE relative-placement DAG construction no longer inserts keys and immediately unwraps
    mutable map lookups for source/destination adjacency, reverse edges, or indegree updates. The
    code now uses entry-based buckets so malformed or future-expanded relative-placement input does
    not depend on that local construction invariant staying panic-safe.
  - Verification: `cargo fmt --check -p manatee -p merman-render`,
    `cargo nextest run -p manatee`, `cargo nextest run -p merman-render architecture`, and
    `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the FCoSE relative-placement cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman` and
    `cargo nextest run -p manatee` passed.

## Known remaining panic candidates (triage)

The following patterns are intentionally tolerated for now but should be tracked:

- Regex compilation via `Regex::new("...").unwrap()` in detector initialization:
  - input is a static literal; failures indicate a programming error, not user input.
- A small number of `unwrap/expect` in renderer internals:
  - most are on index/iterator operations that are guarded by bounds checks, but they are worth
    auditing because they can become input-reachable if assumptions drift.

## Suggested workflow

- When adding new code, prefer `Option`/`Result` over `unwrap/expect` unless it is in tests/examples.
- When porting upstream JS, treat “throw” sites as `Result` boundaries in Rust, unless upstream
  behavior explicitly crashes (rare).
