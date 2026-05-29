# ASCII Graph Parser Padding - Handoff

Status: Complete
Last updated: 2026-05-29

## Current State

The lane is complete. Exact graph fixture count is now 60: 37 ASCII and 23 Unicode.
Comments, explicit-label precedence, preserve-order routing, copied padding directives, and short-Y
backlink spacing are in the exact allowlist.

## Active Task

- Task ID: AGP-040
- Owner: codex
- Files: `docs/workstreams/ascii-graph-parser-padding`, `CHANGELOG.md`,
  `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`;
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`;
  `git diff --check`
- Status: COMPLETE
- Review: Remaining graph gaps are ASCII-only multiline/subgraph-heavy layouts.
- Evidence: Broad verification passed.

## Decisions Since Last Update

- Keep complex subgraph parity out of this lane.
- Prefer parser/model semantics before padding/layout work.
- Treat `paddingX/Y` as `mermaid-ascii` source directives in the ASCII render entry point, not as
  Mermaid flowchart syntax.
- Same-row reverse edges into a node with a self-loop reuse the self-loop junction and push the
  loop's return lane down by one row.

## Blockers

- None.

## Next Recommended Action

- Open a dedicated lane for multiline labels or subgraph-heavy graph parity.
