# ASCII Reference Implementation Expansion — Evidence And Gates

Status: Complete
Last updated: 2026-05-30

## Smallest Current Repro

`merman-ascii` now has bounded class, ER, and xychart renderers in addition to the existing
flowchart and sequence renderers. The graph delta triage against `beautiful-mermaid` is recorded in
`FLOWCHART_SUPPORT.md`; shipped renderer public integration is wired through `merman::ascii` and
`merman-cli`. The reference expansion lane is closed; remaining work is split as follow-on
candidates rather than kept in this lane.

Relevant interface:

```text
crates/merman-ascii/src/lib.rs
crates/merman/src/ascii.rs
crates/merman-cli/src/main.rs
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
| 2026-05-29 | ARI-040 | Added `RenderSemanticModel::Er` dispatch, `render_er`, `crates/merman-ascii/src/er/`, and `crates/merman-ascii/tests/er_model.rs`. | First ER ASCII/Unicode slice implemented: entity boxes, attributes, relationship labels, identifying/non-identifying lines, common cardinality markers, and explicit diagnostics for multiple-relationship layouts. |
| 2026-05-29 | ARI-050 | Added `RenderSemanticModel::XyChart` dispatch, `render_xychart`, `crates/merman-ascii/src/xychart/`, `crates/merman-ascii/tests/xychart_model.rs`, and the README scaling contract. | XYChart ASCII/Unicode slice implemented: compact vertical bars, stair-step lines, mixed overlays, horizontal bars, title/axis text, inferred numeric x labels, and empty-chart handling. |
| 2026-05-29 | ARI-060 | Compared `crates/merman-ascii` graph support with `repo-ref/beautiful-mermaid/src/ascii/` and updated `crates/merman-ascii/FLOWCHART_SUPPORT.md`. | Delta matrix recorded: thick edges ported; `BT`, true `RL`, subgraph direction overrides, multiline subgraph labels, color/style roles, state graph rendering, and uncommon shapes deferred or rejected with rationale. |
| 2026-05-30 | ARI-070 | Re-exported `render_class`, `render_er`, and `render_xychart` from `merman::ascii`; added `merman` and CLI public-path tests for classDiagram, erDiagram, and xychart; updated README support text and the `merman-ascii` shipped diagram matrix. | Public APIs and CLI text output now advertise and test the shipped flowchart, sequence, class, ER, and XYChart ASCII/Unicode families without changing feature gates. |
| 2026-05-30 | ARI-080 | Reviewed the closeout condition, marked the workstream complete, and recorded remaining work as follow-on candidates instead of extending this reference-expansion lane. | Lane closed with model-driven boundary intact, reference-source obligations recorded, public support docs current, and final verification gates passing. |

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
| 2026-05-29 | ARI-040 | `cargo nextest run -p merman-ascii --test er_model` | Focused ER renderer tests | PASS, 7 tests | `render_model` accepts `RenderSemanticModel::Er` for the supported subset and covers entity boxes, attributes, identifying/non-identifying relationships, cardinalities, Unicode borders, and multiple-relationship diagnostics. |
| 2026-05-29 | ARI-040 | `cargo nextest run -p merman-ascii er` | Task ledger ER filter | PASS, 65 tests | The workstream-listed focused gate passes; the broad filter also exercises existing tests with `render` in their names. |
| 2026-05-29 | ARI-040 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 97 tests | The ER slice does not regress existing flowchart, fixture, sequence, or class behavior. |
| 2026-05-29 | ARI-040 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable after ER implementation. |
| 2026-05-29 | ARI-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | New ER renderer and tests compile cleanly under deny-warnings clippy for this package. |
| 2026-05-29 | ARI-050 | `cargo nextest run -p merman-ascii xychart` | Focused XYChart renderer tests | PASS, 7 tests | `render_model` accepts `RenderSemanticModel::XyChart` for the supported subset and covers vertical bars, lines, mixed plots, horizontal orientation, titles, axes, Unicode output, inferred numeric labels, and empty charts. |
| 2026-05-29 | ARI-050 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 104 tests | The XYChart slice does not regress existing flowchart, fixture, sequence, class, or ER behavior. |
| 2026-05-29 | ARI-050 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable after XYChart implementation and workstream doc updates. |
| 2026-05-29 | ARI-050 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | New XYChart renderer and tests compile cleanly under deny-warnings clippy for this package. |
| 2026-05-29 | ARI-050 | `git diff --check` | Patch hygiene | PASS | No whitespace errors in implementation, tests, or workstream docs. |
| 2026-05-29 | ARI-060 | `cargo nextest run -p merman-ascii --test flowchart_model flowchart_parser_thick_edges_render_with_heavy_ascii_line` | Focused thick-edge tracer test | PASS, 1 test | The shipped `beautiful-mermaid` graph delta is proven through the public `render_model` flowchart path for ASCII. |
| 2026-05-29 | ARI-060 | `cargo nextest run -p merman-ascii --test flowchart_model flowchart_parser_thick_edges_render_with_heavy_unicode_line` | Focused thick-edge Unicode test | PASS, 1 test | Thick edge rendering also maps to Unicode heavy line glyphs. |
| 2026-05-29 | ARI-060 | `cargo nextest run -p merman-ascii --test flowchart_model flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line` | Focused thick-edge TD test | PASS, 1 test | Thick edge rendering maps vertical ASCII routes to a visually distinct heavy line approximation. |
| 2026-05-29 | ARI-060 | `cargo nextest run -p merman-ascii flowchart` | Flowchart focused gate | PASS, 25 tests | Thick edge support and the updated unsupported-stroke diagnostic do not regress existing flowchart behavior. |
| 2026-05-29 | ARI-060 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 107 tests | The graph delta triage implementation does not regress sequence, class, ER, XYChart, fixture, or graph behavior. |
| 2026-05-29 | ARI-060 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable after graph delta implementation and docs. |
| 2026-05-29 | ARI-060 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | Thick edge stroke mapping and tests compile cleanly under deny-warnings clippy for this package. |
| 2026-05-29 | ARI-060 | `git diff --check` | Patch hygiene | PASS | No whitespace errors in implementation, tests, or workstream docs. |
| 2026-05-30 | ARI-070 | `cargo nextest run -p merman --features ascii --test ascii_api` | Focused public `merman::ascii` API tests | PASS, 6 tests | The top-level ASCII wrapper renders flowchart, sequence, class, ER, and XYChart text and re-exports direct typed helpers for shipped model families. |
| 2026-05-30 | ARI-070 | `cargo nextest run -p merman-cli --features ascii --test ascii_smoke` | Focused CLI ASCII smoke tests | PASS, 3 tests | CLI `render --format ascii|unicode` covers existing sequence/flowchart paths plus classDiagram, erDiagram, and xychart through stdin/stdout or file output. |
| 2026-05-30 | ARI-070 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package | PASS, 107 tests | Public integration did not regress the underlying flowchart, sequence, class, ER, XYChart, fixture, or graph behavior. |
| 2026-05-30 | ARI-070 | `cargo nextest run -p merman --features ascii` | Public library ASCII feature gate | PASS, 6 tests | The top-level `merman` ASCII feature exposes shipped renderer families through text and typed public paths. |
| 2026-05-30 | ARI-070 | `cargo nextest run -p merman-cli --features ascii` | CLI ASCII feature gate | PASS, 11 tests | The CLI ASCII feature coexists with existing SVG/raster smoke tests and renders the shipped terminal-text families. |
| 2026-05-30 | ARI-070 | `cargo fmt --all --check` | Workspace formatting | FAIL, unrelated dirty files | The command only reported rustfmt diffs in unrelated `crates/merman-render` worktree changes outside ARI-070 scope; those files were not reverted or staged. |
| 2026-05-30 | ARI-070 | `cargo fmt -p merman-ascii -p merman -p merman-cli --check` | Scoped formatting for ARI-070 packages | PASS | The packages touched by ARI-070 are rustfmt-clean. |
| 2026-05-30 | ARI-070 | `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings` | ASCII library lint gate | PASS | `merman-ascii` and the top-level `merman` ASCII API compile cleanly under deny-warnings clippy. |
| 2026-05-30 | ARI-070 | `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings` | CLI ASCII lint gate | PASS | CLI ASCII integration compiles cleanly under deny-warnings clippy with its existing raster dependency stack. |
| 2026-05-30 | ARI-070 | `git diff --check` | Patch hygiene | PASS | No whitespace errors in current dirty worktree diffs. |
| 2026-05-30 | ARI-080 | `cargo nextest run -p merman-ascii` | Final `merman-ascii` package gate | PASS, 107 tests | The closed lane's underlying flowchart, sequence, class, ER, XYChart, fixture, and graph behavior remains green. |
| 2026-05-30 | ARI-080 | `cargo nextest run -p merman --features ascii` | Final public library ASCII feature gate | PASS, 6 tests | `merman::ascii` public API integration remains green after closeout. |
| 2026-05-30 | ARI-080 | `cargo nextest run -p merman-cli --features ascii` | Final CLI ASCII feature gate | PASS, 11 tests | CLI ASCII integration remains green alongside existing raster smoke tests. |
| 2026-05-30 | ARI-080 | `cargo fmt --all --check` | Workspace formatting | PASS | Workspace rustfmt check is clean after concurrent `merman-render` fixes. |
| 2026-05-30 | ARI-080 | `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings` | ASCII library lint gate | PASS | `merman-ascii` and top-level `merman` ASCII APIs remain clean under deny-warnings clippy. |
| 2026-05-30 | ARI-080 | `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings` | CLI ASCII lint gate | PASS | CLI ASCII/raster dependency stack compiles cleanly under deny-warnings clippy. |
| 2026-05-30 | ARI-080 | `git diff --check` | Patch hygiene | PASS | Current worktree diffs have no whitespace errors. |

Broader public feature gates (`cargo nextest run -p merman --features ascii`,
`cargo nextest run -p merman-cli --features ascii`) were not run for ARI-020 because the existing
public `render_model` path is already used by the top-level wrappers and no `merman` or CLI files
changed in this task.

The same broader public feature gates were not rerun for ARI-030 because the task only changes
`merman-ascii` class relationship behavior and docs; no `merman` or CLI integration files changed.
They were also not rerun for ARI-040 because the task only changes `merman-ascii` ER behavior and
docs; no `merman` or CLI integration files changed. They were not rerun for ARI-050 for the same
reason: the task only changes `merman-ascii` XYChart behavior and docs; no `merman` or CLI
integration files changed.

ARI-070 reran the broader public feature gates after updating `merman::ascii` re-exports, public API
tests, CLI smoke coverage, and support documentation. ARI-080 reran the final closeout gates and
closed the lane.

## Review And Closeout

Closeout review on 2026-05-30 found no blocking findings. The target state from `DESIGN.md` is met:
provenance is tracked, new ASCII renderers consume `merman-core` typed models, class/ER/XYChart
slices shipped with tests, useful `beautiful-mermaid` deltas were either ported or classified, and
public docs reflect shipped support. Remaining work is outside this lane's boundary and should start
as smaller follow-ons when prioritized.
