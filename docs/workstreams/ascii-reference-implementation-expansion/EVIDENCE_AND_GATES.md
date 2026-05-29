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
| 2026-05-29 | ARI-020 | Added `RenderSemanticModel::Class` dispatch, `render_class`, `crates/merman-ascii/src/class/`, and `crates/merman-ascii/tests/class_model.rs`. | First classDiagram ASCII/Unicode slice implemented: class boxes, members, methods, one solid extension relationship, and explicit diagnostics for unsupported relationship labels and unrelated-class relationship layouts. |
| 2026-05-29 | ARI-030 | Expanded the class relation layout mapper in `crates/merman-ascii/src/class/render.rs` and relationship snapshots in `crates/merman-ascii/tests/class_model.rs`. | Single-relationship class layouts now cover extension labels, reverse extension orientation, aggregation, composition, dependency dotted arrows, and Unicode composition markers from typed `RelationShape` constants. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-29 | ARI-020 | `cargo nextest run -p merman-ascii class` | Focused class renderer tests | PASS, 6 tests | `render_model` accepts `RenderSemanticModel::Class` for the supported subset and rejects unsupported relationship labels and unrelated-class relationship layouts explicitly. |
| 2026-05-29 | ARI-020 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 85 tests | The class slice does not regress existing flowchart, fixture, or sequence behavior. |
| 2026-05-29 | ARI-020 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable after the implementation. |
| 2026-05-29 | ARI-020 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | New class renderer and tests compile cleanly under deny-warnings clippy for this package. |
| 2026-05-29 | ARI-030 | `cargo nextest run -p merman-ascii class` | Focused class relationship tests | PASS, 11 tests | Class relationship rendering supports labels, extension orientation, dependency, aggregation, composition, and Unicode marker coverage for the supported single-relation layout. |
| 2026-05-29 | ARI-030 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 90 tests | Relationship expansion does not regress existing flowchart, fixture, sequence, or class behavior. |
| 2026-05-29 | ARI-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable after relationship expansion. |
| 2026-05-29 | ARI-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | Expanded class relationship renderer and tests compile cleanly under deny-warnings clippy for this package. |

Broader public feature gates (`cargo nextest run -p merman --features ascii`,
`cargo nextest run -p merman-cli --features ascii`) were not run for ARI-020 because the existing
public `render_model` path is already used by the top-level wrappers and no `merman` or CLI files
changed in this task.

The same broader public feature gates were not rerun for ARI-030 because the task only changes
`merman-ascii` class relationship behavior and docs; no `merman` or CLI integration files changed.

Fresh verification is required before marking implementation tasks complete.
