# ADR 0046: Determinism and Numerics for `dugong`

## Status

Accepted

## Context

Upstream Dagre runs in JavaScript and uses `Number` (IEEE-754 double) for all numeric layout
computations. The final layout is sensitive to:

- stable ordering and tie-breaking in sorting steps,
- graph traversal order (node/edge iteration),
- floating-point rounding and accumulation.

For parity-driven development we need deterministic behavior across platforms and runs, while still
matching upstream numeric behavior closely enough to pass ported tests.

## Decision

### Numeric type

- Use `f64` for all coordinates and layout intermediate values.
- Avoid `f32` to reduce drift and to match JavaScript `Number`.

### Stable ordering rules

To match upstream behavior (stable JS sorts + insertion-ordered containers):

- use stable sorts in all steps where ordering affects layout outcomes.
- when two items compare equal under the primary key, preserve input order (stable sort) or apply
  an explicit deterministic tie-breaker derived from stable IDs.
- never rely on hash map iteration order; node and edge iteration must be deterministic.

### Deterministic data structures

Implementation should ensure:

- `nodes()` and `edges()` iteration order is deterministic (prefer insertion order semantics).
- internal maps used for algorithm steps either:
  - iterate over sorted keys, or
  - are explicitly order-preserving containers.

Exact container choice is an implementation detail, but determinism is a hard requirement.

### Floating-point comparisons in tests

Parity tests ported from JS often compare exact values for simple graphs, but some algorithms can
produce non-trivial floating results.

Rules:

- prefer exact assertions when expected values are derived directly from input sizes and spacing.
- allow epsilon comparisons for derived values that may differ by tiny floating error.
- define a shared epsilon constant for `dugong` tests (e.g. `1e-9`), and tighten/loosen only when
  an upstream test demonstrates the need.

### Rounding and normalization

- Do not round coordinates during computation.
- Only normalize/round for display/export (debug JSON), and keep raw `f64` values for algorithm
  invariants and downstream rendering.

### Platform consistency

- Avoid parallelism or non-deterministic iteration in layout computations.
- Keep any randomization explicitly disabled; Dagre algorithms should be deterministic.

### Error handling

For invalid inputs (missing width/height, NaNs, etc.):

- match upstream defaults where possible (commonly treat missing sizes as `0`).
- reject NaN/Infinity inputs with explicit errors only if upstream would fail; otherwise coerce to
  a safe default for parity.

## Consequences

- Ported upstream tests become reproducible and stable in CI.
- `dugong` numeric behavior stays close to JS Dagre by using `f64` and stable ordering.
- Rendering parity can focus on text measurement and Mermaid-specific semantics rather than layout
  nondeterminism.
