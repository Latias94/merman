# Maintenance terminal text verification

Date: 2026-10-26. Main companion PR: #246; maintenance PR: #247.
Maintenance starts at `96fc586d5` and targets pinned Mermaid 12.1.0.

## Adaptation

The backport fixes Sequence title root-font measurement and start anchoring,
Mindmap configured HTML font size, Block source typography, shared SVG Markdown
normal resets/code filtering, Class note row/entity provenance and Flowchart raw
declaration-winner selection. Usecase retains this release's inline role fonts
while measuring the SVG inner-word reset. Main's newer State geometry/evidence
architecture is not imported: maintenance's existing literal SVG Markdown State
behavior remains an explicit capability boundary.

HTML retains its measurement primitives, paragraph gaps and host-provided
heights. A final independent review finding is reproduced before repair:
``x `c` [visible label](u)`` renders fully but was measured as only ``x `c` ``.
`0fd54d886` preserves visible inline syntax and source adjacency, including
empty links and code next to ordinary text. It preserves maintenance's existing
escape handling rather than importing main's pinned-parser differences.

## Verification

- Affected-family integration before the final HTML fix: 1,880 passed, one
  existing skip. Final renderer library plus Usecase: 1,678 passed, no skips.
  These runs overlap and are not a distinct-test total.
- Strict renderer-library Clippy passes with
  `all-diagrams,layout-elk,layout-cytoscape,math` and `-D warnings`.
- The exact complete 37-family release-mode SVG structure owner passes again
  after the HTML fix. No upstream SVG, comparator policy or tolerance changes.
- Current-source Edge checks confirm the 40px Sequence title is contained by
  `-50 -62.777777776 805 287.777777776`; title ink is approximately
  `(75, -61.965, 481.571, 46.206)`. Mindmap computes to 32px. The probe uses this
  branch's consuming `Engine` API and current maintenance rlibs.
- The main formal review completes with one P2, repaired on both lines. A
  focused independent inspection of both final parser patches finds no blocker.

No performance qualification, pixel-perfect font guarantee, package release or
remote CI success is claimed. Separate old-CI failures in Playground accessible
naming and an environment-bound exact viewport receipt require their own repair
and validation; the successful structure check does not prove those browser
owners green.

## Maintenance CI repairs

The old CI run `40288400669` has two independent browser-owner failures:

- Desktop Playground's visible `Presentation` button was overridden by the
  accessible name `Theme`, failing Axe's `label-content-name-mismatch`. Removing
  that redundant desktop override lets visible text name the button. Mobile's
  icon-only button retains its explicit name. The existing desktop interaction
  test now locates the visible name and verifies its text; no Axe rule is disabled.
- The title/filter exact residual for
  `flowchart/stress_flowchart_title_padding_subgraph_029` still binds Playwright
  1.64.0 although the dependency upgrade selects 1.64.1. Chromium remains
  `156.0.8078.4`. Recomputing the original paired CI audit with only Playwright
  changed back reproduces the old exact SHA
  `176bfd8735db49d993cb3145c6de399639d7858cbfae4b8307b18ff69b660dbc`.
  Current-source release-mode fixture generation passes the focused structure
  owner and reproduces both recorded SVG byte hashes exactly. All saved paired
  geometry, paint, structural pixels, capture and other environment fields are
  unchanged. The receipt is refreshed to
  `1908aca6c4fb2104cbfae63d54c9f4ff12c891b5c024bb5038a2cc1ed0131181`;
  the existing matcher accepts that exact saved evidence. Eligibility checks,
  environment binding and unused-receipt enforcement remain intact.

The saved Linux audit comes from CI's `svg-parity-diagnostics-1` artifact
`12637249469`; it is evidence for this exact fixture, not a fresh local Linux
oracle execution. New CI must still run the full browser oracle on the pushed
source. A host macOS browser cannot substitute for that environment-bound owner.

Current-source production verification rebuilds and assembles all five Web WASM
packages, checks the TypeScript contracts (37 exports, 49 runtime bindings and
five package entries), and builds Playground with its production artifact-graph
verification. Both focused desktop Chromium tests pass: production WASM/Axe
baseline and automatic/explicit presentation controls. These use the freshly
built source, not the previous WASM artifacts.
Both focused mobile Chromium interaction checks pass (320px and mid-width
controls), all 17 root-viewport oracle contracts pass, and browser-test TypeScript
checking passes after package assembly. These oracle contracts do not replace
the complete Linux corpus containment run.

The initial build aborts in the host Homebrew Binaryen 133 optimizer assertion
`constraints.provesEverything()`. Repeating the unchanged build with the normal
wasm-pack-managed optimizer succeeds for all five packages. Optimization is not
disabled, no product/build-script workaround is added, and CPU concurrency is
bounded to two jobs/optimizer cores. This is a local toolchain distinction, not
evidence of a successful Linux production-size budget check.

The separate focused formal review `20261026-195803-f2b454a7` completes on
`4035b631d..3fb2fdc97` with no actionable findings. It independently rechecks
accessible-name ownership and the saved paired-audit fingerprints; it does not
claim to have executed the browser tests listed above. The review uses Codex
contexts only and makes no cross-provider independence claim.
