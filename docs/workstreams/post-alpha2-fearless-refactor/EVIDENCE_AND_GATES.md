# Post Alpha.2 Fearless Refactor — Evidence And Gates

Status: Active
Last updated: 2026-06-08

## Planned Gates

- `git diff --check -- docs/workstreams/post-alpha2-fearless-refactor`
- `cargo fmt --all --check`
- `cargo nextest run -p merman-bindings-core`
- `cargo nextest run -p merman-ffi render_svg`
- `cargo run -p xtask -- check-alignment`

## Evidence Log

- 2026-06-08: Created this follow-on lane after `0.7.0-alpha.2` release. The prior `docs/workstreams/merman-0-7-architecture-deepening` lane remains closed and is referenced as history, not reopened.
- 2026-06-08: PA2R-020 moved binding render request construction and execution behind `RenderRequestPlan`. `cargo nextest run -p merman-bindings-core` passed 31/31 tests. `cargo nextest run -p merman-ffi render_svg` passed 2/2 focused tests. `cargo fmt --all --check` passed.
- 2026-06-08: PA2R-030 derived supported diagram metadata from render parser facts. `cargo nextest run -p merman-core registry` passed 10/10 focused tests. `cargo nextest run -p merman-core detect` passed 22/22 focused tests. `cargo run -p xtask -- check-alignment` passed. `cargo fmt --all --check` passed.
