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

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACR-010 | `git diff --check -- docs/workstreams/ascii-color-role-api` | Workstream docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACR-010 | `cargo fmt --all --check` | Workspace formatting gate | PASS | Draft docs did not disturb Rust formatting. |
| 2026-05-30 | ACR-020 | `git diff --check -- docs/adr/0067-ascii-color-role-api.md docs/workstreams/ascii-color-role-api` | ADR and workstream docs | PASS | ADR/workstream update has no whitespace errors. |
| 2026-05-30 | ACR-020 | `cargo fmt --all --check` | Workspace formatting gate | PASS | ADR-only task did not disturb Rust formatting. |
