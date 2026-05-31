# Mermaid 11.15 Parity Failure Inventory

Status: Draft
Last updated: 2026-06-01

Source command:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

Initial result on 2026-05-31: failed with 525 DOM mismatches across 8 diagram groups.

Current result after the M15C-040 Sequence/C4/Journey/Timeline convergence on 2026-06-01: failed
with 35 DOM mismatches across 4 diagram groups: sankey=24, class=9, flowchart=1, xychart=1.

## Summary

| Diagram | Mismatches | Dominant observed failure | Current classification | Next action |
| --- | ---: | --- | --- | --- |
| Sequence | 0 | Stored baselines were stale and fresh 11.15 probes exposed central-connection plus DOM metadata drift. | Green after M15C-040 renderer fixes and stored baseline refresh. `stress_end_keyword_016` is skipped because Mermaid 11.15 rejects `(end)` participant ids. | None for current full gate; keep the skipped fixture as local parser coverage. |
| Timeline | 0 | Stored-baseline marker drift plus a real 11.15 scoped node id delta. | Green after M15C-040 renderer fix and stored baseline refresh. | None for M15C-040; keep Timeline in future full-gate regression checks. |
| C4 | 0 | Stored upstream marker/base symbol drift; fresh 11.15 also changed scoped base symbol ids and selected type-label text lengths. | Green after M15C-040 renderer fixes and stored baseline refresh. | None for M15C-040; keep C4 in future full-gate regression checks. |
| Journey | 0 | Stored upstream marker/task-line id drift; fresh 11.15 scopes task-line ids by SVG id. | Green after M15C-040 renderer fix and stored baseline refresh. | None for M15C-040; keep Journey in future full-gate regression checks. |
| Sankey | 24 | Link `stroke-width` differs by fixture. | Likely 11.15 baseline/config drift or d3-sankey math delta. | Refresh/check Sankey 11.15 upstream baselines, then inspect layout math only if still red. |
| Class | 9 | Hierarchical namespace DOM differs: local nested groups versus old upstream namespace structure. | Likely expected 11.15 behavior compared against stale baseline. | Regenerate/check Class 11.15 baselines; keep local hierarchy unless fresh upstream disproves it. |
| Flowchart | 1 | MathML `columnalign` extra attribute under KaTeX/MathML output. | Targeted renderer or normalizer gap after baseline freshness is confirmed. | Reproduce with fresh 11.15 upstream output for `upstream_docs_math_flowcharts_001`. |
| XYChart | 1 | Data label `fill` differs: upstream `black`, local configured theme color. | Likely 11.15 config/baseline drift or theme precedence bug. | Reproduce with fresh 11.15 upstream output for the config fixture. |

## Immediate Split

### Stale-baseline dominated

These were handled first because they accounted for 490 of the initial 525 mismatches:

- sequence
- timeline
- c4
- journey

The local output already followed the 11.14/11.15 internal SVG ID prefix direction implemented in
the baseline-upgrade lane. M15C-030 removed stale active report labels, and M15C-040 refreshed the
stored baselines for the affected implemented diagrams.

M15C-040 update: fresh Mermaid 11.15 `sequence/basic` and `sequence/central` SVG probes now compare
green in `parity` mode after implementing upstream central connections and 11.15 sequence SVG
metadata. A later full Sequence convergence pass closed the 121 fresh-corpus residuals and refreshed
stored Sequence baselines. `stress_end_keyword_016` remains excluded from upstream SVG gates because
Mermaid 11.15 rejects its `(end)` participant id. Fresh Mermaid 11.15 C4 and Journey full-diagram
probes are green after scoped-id and C4 type-label fixes, and their stored baselines have been
refreshed. Timeline is also green after matching Mermaid 11.15 scoped node ids and refreshing stored
Timeline baselines.

Current `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` split after
Sequence/C4/Journey/Timeline refresh: sankey=24, class=9, flowchart=1, xychart=1.

### Needs fresh 11.15 baseline before code changes

- sankey
- class
- xychart

These diagrams already had targeted 11.15 behavior changes. Treat the current mismatches as
unproven until fresh upstream 11.15 baselines are generated or checked.

### Likely targeted code/normalizer issue

- flowchart MathML `columnalign`

This is a single fixture and may be either renderer output drift or DOM compare normalization drift.
Do not batch it with marker-ID baseline refresh.

## Recommended Batch Order

1. M15C-030: fix active compare/report metadata so generated reports no longer present 11.12.3 as
   the current baseline.
2. M15C-040 batch A: regenerate/check sequence, timeline, C4, and Journey 11.15 upstream SVGs.
3. M15C-040 batch B: rerun full `parity` and recalculate mismatch counts.
4. M15C-050/M15C-060: address the remaining Sankey, Class, Flowchart, and XYChart mismatches.
