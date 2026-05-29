# ASCII Reference Implementation Expansion — Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Smallest Current Repro

`merman-ascii` currently routes only flowchart and sequence typed models. Class, ER, and xychart
typed models exist in `merman-core`, but `render_model` returns `UnsupportedDiagram` for them.

Relevant interface:

```text
crates/merman-ascii/src/lib.rs
```

## Gate Set

### Documentation And Provenance Gate

```bash
git diff --check
```

This catches trailing whitespace and patch hygiene for the intake task.

### Targeted Iteration Gates

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
cargo nextest run -p merman-ascii xychart
cargo nextest run -p merman-ascii graph
```

Use the relevant focused gate for the active slice. New tests should be named so the filter remains
stable.

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

This proves the new renderer did not regress existing flowchart and sequence text output.

### Public Feature Gates

```bash
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
```

Run these once a new renderer is wired through public convenience APIs or CLI behavior.

### Formatting And Lint Gates

```bash
cargo fmt --all --check
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings
```

Use clippy before closeout or before committing a broad API slice.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/ascii-reference-implementation-expansion/DESIGN.md`
- `docs/workstreams/ascii-reference-implementation-expansion/TODO.md`
- `docs/workstreams/ascii-reference-implementation-expansion/MILESTONES.md`
- `crates/merman-ascii/README.md`
- `crates/merman-ascii/LICENSES/mermaid-ascii-MIT.txt`
- `crates/merman-ascii/LICENSES/beautiful-mermaid-MIT.txt`
- `tools/upstreams/REPOS.lock.json`

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-29 | ARI-010 | Reference source inspection: `mermaid-ascii@6fffb8e2714acab2c4cb41c78894fabbc62cee56`, `beautiful-mermaid@2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`; both local license files are MIT. | Provenance task opened and docs updated. |

Fresh verification is required before marking implementation tasks complete.
