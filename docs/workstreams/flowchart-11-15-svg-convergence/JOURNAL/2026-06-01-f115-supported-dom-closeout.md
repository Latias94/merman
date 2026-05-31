# 2026-06-01 - F115 Supported Flowchart DOM Closeout

## Summary

Closed the remaining supported Mermaid 11.15 Flowchart canonical XML drift. The fresh Flowchart
gate now reports zero canonical XML mismatches and one remaining unsupported `flowchart-elk` local
layout failure.

## Changes

- Matched non-markdown subgraph title behavior with Mermaid 11.15 `createLabel(... width=Infinity)`.
- Scoped empty subgraph node ids by diagram id.
- Matched `nonMarkdownToHTML(...)` for literal `\n` line breaks and non-markdown edge label
  paragraph wrappers.
- Removed the stale text/string markdown heuristic for `*` and `_`; Flowchart 11.15 only parses
  markdown labels when `labelType=markdown`.

## Evidence

- `cargo nextest run -p merman-render flowchart`: passed, 87 tests.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
  failed only for unsupported `flowchart-elk`; report showed `canonical XML mismatches: 0`.

## Next

Finish F115-070 by deciding whether `flowchart-elk` should be supported now, skipped narrowly with
policy, or split into a separate ELK layout workstream before stored Flowchart baseline refresh.
