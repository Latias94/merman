# Mermaid 11.15 Complete Adaptation - Milestones

Status: Active
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

Exit criteria:

- The umbrella lane exists and names the exact baseline claim.
- Current green and red evidence is captured.
- The first executable task is unambiguous.

Status: complete.

## M1 - Baseline Evidence And Tooling

Exit criteria:

- Current `compare-all-svgs` failures are classified by diagram and failure kind.
- Active compare reports and 11.15 parity docs no longer mislabel the current baseline as 11.12.3.
- Marker-ID impacted diagrams have a clear baseline-refresh plan or completed regenerated baselines.

Status: complete.

## M2 - Residual Existing-Matrix Parity

Exit criteria:

- Sankey 11.15 stroke-width/layout deltas are closed or split with evidence.
- Class hierarchical namespace deltas are closed against 11.15 baselines.
- XYChart and Flowchart Math single-fixture deltas are closed or split.
- ER 11.15 renderer-envelope deltas are closed against refreshed 11.15 baselines.

Status: complete. Sequence, C4, Journey, Timeline, Sankey, XYChart, Flowchart supported fixtures,
ER, and Class are green against Mermaid 11.15 stored baselines. `upstream_parser_class_spec`
remains a documented Class upstream render artifact skip.

## M3 - Full Implemented-Matrix Gates

Exit criteria:

- Full `parity` DOM compare is green for the implemented matrix.
- `parity-root` is green or any residual root-only work is explicitly split.
- Package/workspace tests relevant to changed code are green.

Status: in progress. Full `parity` DOM compare is green for the implemented matrix. `parity-root`
is still red for root/viewBox/max-width residuals across the implemented matrix; current largest
buckets are Flowchart, Sequence, Architecture, Class, and C4.

## M4 - Upstream Family Decisions

Exit criteria:

- `eventmodeling`, `wardley`, `treeView`, `venn`, `ishikawa`, `cynefin`, and `railroad` have final
  11.15 support decisions.
- Promoted families have child workstreams.
- Deferred or out-of-scope families are visible in `docs/alignment/STATUS.md`.

Status: not started.

## M5 - Closeout

Exit criteria:

- All prior milestones are complete or split.
- Final evidence gates are recorded.
- `WORKSTREAM.json` and `HANDOFF.md` reflect closed or split status.

Status: not started.
