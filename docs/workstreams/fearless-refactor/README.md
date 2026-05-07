# Fearless Refactor Workstream

This workstream tracks the cleanup plan for the next merman version. The goal is to make the
project cleaner, faster, and easier to extend while preserving Mermaid parity.

Baseline upstream remains Mermaid `@11.12.3`. Parity work still lives in
`docs/workstreams/TODO.md`; this workstream is about internal architecture, maintainability,
feature-gate health, and performance-oriented simplification.

## Mission

Ship the next version with a simpler render pipeline, fewer redundant code paths, clearer module
boundaries, and stronger verification gates.

The target state is:

- A typed render pipeline for high-impact diagrams.
- One authoritative dispatch point for each pipeline stage.
- Large renderer/text modules split by responsibility.
- Feature-gated code that compiles under `--all-features`.
- Override tables treated as generated compatibility data, not as unchecked permanent debt.
- Benchmarks and parity gates that make refactoring safe.

## Non-goals

- Do not chase new Mermaid syntax unless it blocks cleanup.
- Do not relax semantic/layout/SVG parity to simplify implementation.
- Do not rewrite the whole renderer at once.
- Do not delete fixtures or upstream baselines just to reduce test time.
- Do not make public APIs unstable without a migration path or a clear pre-1.0 rationale.

## Refactor Rules

- Keep changes reviewable: one architectural concern per commit.
- Prefer deleting obsolete code over adding compatibility shims.
- Prefer typed models over `serde_json::Value` in render-critical paths.
- Preserve the public parse APIs until a replacement is documented.
- Add or reuse tests before changing behavior-sensitive code.
- Run the smallest relevant gate first, then a broader gate before committing.
- Use generated override data only when the underlying upstream behavior is genuinely browser/font
  dependent or intentionally pinned.

## Standard Gates

Minimum gate for any refactor touching `merman-core` or `merman-render`:

```sh
cargo fmt
cargo check -p merman-core -p merman-render
cargo clippy -p merman-core -p merman-render --all-targets -- -D warnings
cargo nextest run -p merman-core -p merman-render
```

Feature and public-surface gate:

```sh
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Equivalent xtask release gate:

```sh
cargo run -p xtask -- verify --strict
```

Parity gate for layout/SVG-affecting work:

```sh
cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3
cargo run -p xtask -- check-alignment
```

Benchmark gate for performance-sensitive work:

```sh
cargo bench -p merman --features render
```

Use narrower `xtask compare-*` commands when working on one diagram family.

## Priority Model

Use this order when choosing work:

1. Remove duplicated orchestration that can cause behavior drift.
2. Replace JSON render paths with typed render models for hot diagrams.
3. Split oversized modules without changing behavior.
4. Remove stale experimental or feature-gated code.
5. Reduce clone/allocation cost where benchmarks or profiles show impact.
6. Improve docs only after the code path is clean enough to describe.

## Workstream Documents

- `TODO.md`: prioritized task backlog.
- `MILESTONES.md`: staged roadmap and exit criteria.
- `RENDER_MODEL_INVENTORY.md`: current typed-vs-JSON render pipeline inventory and API decision.
