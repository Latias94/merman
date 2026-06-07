# HPD-050 - Production Panic Surface Reclassification

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the theme numeric-default and Gantt datetime fallback cleanups, the broad
`unwrap/expect/panic!` search still produced many hits from `src/tests`, `tests.rs`, and same-file
`#[cfg(test)]` modules. `docs/quality/PANIC_SURFACE.md` still carried a vague renderer-internals
triage note that no longer matched a filtered production scan.

## Changes

- Reclassified the known remaining panic candidates in `docs/quality/PANIC_SURFACE.md`.
- Removed the stale renderer-internals `unwrap/expect` triage note.
- Recorded that the current filtered production scan reports only generated/static core JSON
  validity checks and the source-backed Graphlib named-edge panic.

## Verification

- Filtered production scan across `merman-core`, `merman-render`, `dugong`, `dugong-graphlib`, and
  `manatee`, excluding `src/tests`, `tests.rs`, `test.rs`, comments, and lines after same-file
  `#[cfg(test)]`, reported only:
  - `crates/dugong-graphlib/src/graph/core.rs:398`
  - `crates/merman-core/src/theme.rs:324`
  - `crates/merman-core/src/theme.rs:327`
  - `crates/merman-core/src/generated/mod.rs:13`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `880`
  lines parsed.
- `git diff --check` - passed.

## Boundary

This is a documentation and triage reclassification slice only. It does not change parser,
renderer, layout, Graphlib, generated config, generated theme snapshot, SVG baseline, root
viewport, or Mermaid parity behavior.
