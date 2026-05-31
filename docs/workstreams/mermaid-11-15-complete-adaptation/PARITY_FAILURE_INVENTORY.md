# Mermaid 11.15 Parity Failure Inventory

Status: Draft
Last updated: 2026-05-31

Source command:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

Result on 2026-05-31: failed with 525 DOM mismatches across 8 diagram groups.

## Summary

| Diagram | Mismatches | Dominant observed failure | Current classification | Next action |
| --- | ---: | --- | --- | --- |
| Sequence | 322 | Stored upstream markers use bare IDs such as `arrowhead`; local output uses `<svg-id>-arrowhead`; fresh 11.15 probes also exposed central-connection model drift. | Mostly stale upstream SVG baseline; central-connection parser/model gap fixed in M15C-040. | Bulk-refresh/check sequence 11.15 upstream baselines after the code fix, then recalculate residuals. |
| Timeline | 91 | Stored-baseline marker drift plus fresh 11.15 renderer/model deltas: scoped node ids, wrapper class/DOM shape, and multiline/tspan differences. | Real Timeline convergence gap after baseline freshness was checked. | Split or continue a Timeline-specific 11.15 convergence slice before refreshing stored Timeline baselines. |
| C4 | 51 | Stored upstream marker/base symbol drift; fresh 11.15 also changed scoped base symbol ids and selected type-label text lengths. | Fresh 11.15 C4 target is green after M15C-040 renderer fixes; stored baselines still need refresh. | Refresh stored C4 upstream SVG baselines as a separate fixture-churn commit. |
| Journey | 26 | Stored upstream marker/task-line id drift; fresh 11.15 scopes task-line ids by SVG id. | Fresh 11.15 Journey target is green after M15C-040 renderer fix; stored baselines still need refresh. | Refresh stored Journey upstream SVG baselines as a separate fixture-churn commit. |
| Sankey | 24 | Link `stroke-width` differs by fixture. | Likely 11.15 baseline/config drift or d3-sankey math delta. | Refresh/check Sankey 11.15 upstream baselines, then inspect layout math only if still red. |
| Class | 9 | Hierarchical namespace DOM differs: local nested groups versus old upstream namespace structure. | Likely expected 11.15 behavior compared against stale baseline. | Regenerate/check Class 11.15 baselines; keep local hierarchy unless fresh upstream disproves it. |
| Flowchart | 1 | MathML `columnalign` extra attribute under KaTeX/MathML output. | Targeted renderer or normalizer gap after baseline freshness is confirmed. | Reproduce with fresh 11.15 upstream output for `upstream_docs_math_flowcharts_001`. |
| XYChart | 1 | Data label `fill` differs: upstream `black`, local configured theme color. | Likely 11.15 config/baseline drift or theme precedence bug. | Reproduce with fresh 11.15 upstream output for the config fixture. |

## Immediate Split

### Stale-baseline dominated

These should be handled first because they account for 490 of 525 current mismatches:

- sequence
- timeline
- c4
- journey

The local output already follows the 11.14/11.15 internal SVG ID prefix direction implemented in
the baseline-upgrade lane. The stored upstream SVG files still contain old bare IDs, and active
compare reports still label their baselines as Mermaid 11.12.3.

M15C-040 update: fresh Mermaid 11.15 `sequence/basic` and `sequence/central` SVG probes now compare
green in `parity` mode after implementing upstream central connections and 11.15 sequence SVG
metadata. Fresh Mermaid 11.15 C4 and Journey full-diagram probes are also green after scoped-id and
C4 type-label fixes. Timeline fresh 11.15 still fails broadly, so it is no longer grouped as a
simple stale-baseline marker-id batch. Stored upstream SVG directories are still not bulk-refreshed
in this slice.

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
4. M15C-050/M15C-060: address only the residual mismatches that survive fresh 11.15 baselines.
