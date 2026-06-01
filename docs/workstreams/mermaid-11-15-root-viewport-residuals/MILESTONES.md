# Mermaid 11.15 Root Viewport Residuals - Milestones

Status: Active
Last updated: 2026-06-01

## M0 - Baseline Split

- `compare-all-svgs --dom-mode parity-root` fails normally with bounded summaries.
- Fresh per-diagram root reports exist under `target/compare/*_report_parity_root.md`.
- The root lane is split from Mermaid 11.15 structural adaptation.

## M1 - Largest Buckets Classified

- Sequence is classified: central-connection source rules match Mermaid 11.15, stale pins were
  removed, and the remaining bucket is root-bounds/text-measurement residual work.
- Flowchart is classified: retained root pins mostly reduce drift, one stale pin was removed, and
  the remaining 61 rows are small SVG text/root BBox measurement tails.
- Architecture, Class, and C4 are classified: C4 is root-green after refreshing existing
  fixture-derived pins; Architecture and Class remain layout/root-bound residual buckets.
- Source-derived rules are either implemented or split further.
- Browser/font lattice tails are not converted into hand-written renderer constants.

## M2 - Smaller Buckets Classified

- ER, Sankey, Timeline, and Journey residuals are classified.
- Low-risk source-derived or stale-pin rows are closed.
- Remaining rows have explicit diagnostic status.

## M3 - Root Policy Closeout

- Full `parity` remains green.
- Full `parity-root` is green or fails only with accepted diagnostic residual policy entries.
- Override no-growth passes.
