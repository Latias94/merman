# ASCII Color Role API - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Design Evidence

- `crates/merman-ascii/src/options.rs`: current public `AsciiRenderOptions` fields and validation.
- `docs/adr/0067-ascii-color-role-api.md`: accepted public API and options migration decision.
- `crates/merman-ascii/src/canvas.rs`: current character-only canvas and final string assembly.
- `crates/merman-ascii/src/lib.rs`: public render entry points and option validation.
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`: existing deferred color/style row.
- `repo-ref/beautiful-mermaid/src/ascii/types.ts`: reference role names and `AsciiTheme`.
- `repo-ref/beautiful-mermaid/src/ascii/ansi.ts`: reference ANSI/HTML encoder and theme bridge.
- `repo-ref/beautiful-mermaid/src/ascii/canvas.ts`: reference parallel role canvas.

## Initial Gate Set

```bash
git diff --check -- docs/workstreams/ascii-color-role-api
cargo fmt --all --check
```

## Implementation Gate Set

```bash
cargo nextest run -p merman-ascii color
cargo nextest run -p merman-ascii canvas
cargo nextest run -p merman-ascii flowchart_color
cargo nextest run -p merman-ascii flowchart
cargo nextest run -p merman-ascii
cargo fmt --all --check
git diff --check
cargo clippy -p merman-ascii --all-targets -- -D warnings
```

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACR-010 | Drafted the color role API workstream and public API sketch. | Lane is draft; implementation waits on ADR/public API decision. |
| 2026-05-30 | ACR-020 | Accepted ADR 0067 for the color role API and options migration. | Lane is active; implementation can start with role-aware canvas and encoders. |
| 2026-05-30 | ACR-030 | Added public color types, color options, role-aware canvas storage, and forced ANSI/HTML encoders. | M1 infrastructure is complete; ACR-040 can assign flowchart roles. |
| 2026-05-30 | ACR-040 | Assigned flowchart semantic roles for node text/borders, group borders/titles, edge lines, labels, arrowheads, and routed junctions. | M2 vertical slice is complete; broader diagram-family adoption can be planned. |
| 2026-05-30 | ACR-050 | Accepted a split family adoption plan for class/ER, XYChart, and sequence. | M3 is decomposed into ACR-051 through ACR-054; ACR-060 remains style mapping. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACR-010 | `git diff --check -- docs/workstreams/ascii-color-role-api` | Workstream docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACR-010 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Draft docs did not disturb Rust formatting. |
| 2026-05-30 | ACR-020 | `git diff --check -- docs/adr/0067-ascii-color-role-api.md docs/workstreams/ascii-color-role-api` | ADR and workstream docs | PASS | ADR/workstream update has no whitespace errors. |
| 2026-05-30 | ACR-020 | `cargo fmt --all --check` | Workspace formatting gate | PASS | ADR-only task did not disturb Rust formatting. |
| 2026-05-30 | ACR-030 | `cargo nextest run -p merman-ascii color canvas` | Color API and canvas encoder slice | PASS | Plain, truecolor, ANSI 256, ANSI 16, and HTML encoder tests pass. |
| 2026-05-30 | ACR-030 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Rust formatting is stable after implementation. |
| 2026-05-30 | ACR-030 | `cargo nextest run -p merman-ascii` | Full ascii crate regression suite | PASS | Default diagram snapshots still pass after graph finalization uses color-aware output. |
| 2026-05-30 | ACR-030 | `git diff --check` | Full worktree diff | PASS | Implementation and docs have no whitespace errors. |
| 2026-05-30 | ACR-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII crate lint gate | PASS | New public color API and canvas encoder code are warning-free under clippy. |
| 2026-05-30 | ACR-040 | `cargo nextest run -p merman-ascii flowchart_color` | Flowchart forced-color slice | PASS | TrueColor, HTML, and transformed-direction role snapshots pass. |
| 2026-05-30 | ACR-040 | `cargo nextest run -p merman-ascii flowchart` | Flowchart regression suite | PASS | Plain flowchart snapshots remain unchanged while colored output is opt-in. |
| 2026-05-30 | ACR-040 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Rust formatting is stable after flowchart role assignment. |
| 2026-05-30 | ACR-040 | `cargo nextest run -p merman-ascii` | Full ascii crate regression suite | PASS | Flowchart color roles do not regress other ascii diagram families. |
| 2026-05-30 | ACR-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII crate lint gate | PASS | Flowchart role helpers and tests are warning-free under clippy. |
| 2026-05-30 | ACR-040 | `git diff --check` | Full worktree diff | PASS | Implementation and docs have no whitespace errors. |
| 2026-05-30 | ACR-050 | `git diff --check -- docs/workstreams/ascii-color-role-api crates/merman-ascii/FLOWCHART_SUPPORT.md crates/merman-ascii/README.md` | Adoption plan and support docs | PASS | Decision docs and support updates have no whitespace errors. |
| 2026-05-30 | ACR-051 | `cargo nextest run -p merman-ascii color canvas relation_graph` | Shared trim substrate and relation graph line roles | PASS | Trimmed plain, TrueColor, HTML, and role-aware relation graph line tests pass. |
| 2026-05-30 | ACR-051 | `cargo nextest run -p merman-ascii class er` | Class and ER plain regression suite | PASS | Relation graph substrate changes preserve existing class and ER output. |
| 2026-05-30 | ACR-051 | `cargo nextest run -p merman-ascii` | Full ascii crate regression suite | PASS | Shared substrate changes do not regress any ascii family. |
| 2026-05-30 | ACR-051 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Rust formatting is stable after the shared substrate slice. |
| 2026-05-30 | ACR-051 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII crate lint gate | PASS | Shared trim substrate and relation graph line types are warning-free under clippy. |
| 2026-05-30 | ACR-051 | `git diff --check` | Full worktree diff | PASS | Implementation and docs have no whitespace errors. |
