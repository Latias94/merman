---
title: Unify measured typography and repair browser probes
type: fix
status: in-progress
date: 2026-10-26
---

# Measured typography

## Scope

Repair the font review findings on the main and 0.9 maintenance PRs. Preserve
unrelated Flutter changes in the root worktree. Diagnose the current CI failures
from their first causal errors rather than weakening checks.

## Design

Use one internal typography cascade for the supported font longhands. Select
valid declaration winners before resolving relative values against an immutable
inherited style. Keep parent and root font-size contexts distinct. Preserve
explicit unsupported or environment-dependent typography as an honest residual;
do not claim that a browser callback recovers information absent from its request.

Layout and SVG row positioning must consume equivalent resolved typography.
For Class SVG rows, preserve the pinned inner tspan normal resets and project
those resets into measurement; outer row italics do not override an explicitly
normal child. Keep Markdown emphasis and literal fallback behavior distinct.
Preserve Mermaid's distinct wrapping-budget probes. Align Flowchart HTML
measurement with the font-size CSS actually emitted by this release line.

Keep the existing synchronous host measurement protocol and browser opt-in.
Browser probes must detect rejected CSS assignments and recover from detached
measurement elements. Unsupported requests return unhandled, never successful
measurements using stale font state.

## Work units

1. Fix Web CSS assignment admission, probe lifecycle, and host option types.
2. Fix Class SVG member and method measurement and row placement.
3. Share supported typography resolution between Class and Flowchart, align
   HTML measurement with emitted root sizes, and settle root-relative admission.
4. Repair CI failures on each branch and update stale font integration guidance.
5. Run focused Rust nextest suites serially, Web/browser tests, formatting and
   the full SVG structure owner. Inspect independent review findings.
6. Commit focused changes, adapt to 0.9 without importing main-only contracts,
   push both existing PR branches, and inspect their new CI results.

## Acceptance

- Equivalent measured and emitted font size for numeric/string Flowchart config.
- Repeated relative declarations and inheritance use CSS winner semantics.
- Class SVG row styles affect both layout and final positioning.
- Invalid browser styles cannot reuse old probe typography; detached probes do
  not report successful zero-width measurements for nonempty text.
- No new ABI layout, operation-code meaning, automatic font discovery, or
  asynchronous callback behavior.
- Existing CI failures have concrete fixes or reproducible external attribution.
- Main and maintenance line validation are recorded independently.

## Implementation outcome

The implementation and local validation are complete on both release lines;
replacement remote CI runs remain owner evidence, not an assumed green result.
Main additionally fixes unthemed host portability enforcement and records
root-dependent source sizes in its existing strict evidence framework. The
maintenance backport retains its older public and theme contracts.

See `docs/research/2026-10-26-font-validation.md` for the tested scope and CI causes.
Known unsupported descendant HTML typography, arbitrary spacing/direction, and
document-root discovery are not silently promoted to supported features.
