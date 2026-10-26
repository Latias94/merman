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
