# Mermaid 11.15 Parity Failure Inventory

Status: Draft
Last updated: 2026-06-01

Source command:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

Initial result on 2026-05-31: failed with 525 DOM mismatches across 8 diagram groups.

Current result after the M15C-060 Class convergence and baseline refresh on 2026-06-01: passed for
the implemented matrix in `parity` mode. The active residual set has moved to `parity-root`.

## Summary

| Diagram | Mismatches | Dominant observed failure | Current classification | Next action |
| --- | ---: | --- | --- | --- |
| Sequence | 0 | Stored baselines were stale and fresh 11.15 probes exposed central-connection plus DOM metadata drift. | Green after M15C-040 renderer fixes and stored baseline refresh. `stress_end_keyword_016` is skipped because Mermaid 11.15 rejects `(end)` participant ids. | None for current full gate; keep the skipped fixture as local parser coverage. |
| Timeline | 0 | Stored-baseline marker drift plus a real 11.15 scoped node id delta. | Green after M15C-040 renderer fix and stored baseline refresh. | None for M15C-040; keep Timeline in future full-gate regression checks. |
| C4 | 0 | Stored upstream marker/base symbol drift; fresh 11.15 also changed scoped base symbol ids and selected type-label text lengths. | Green after M15C-040 renderer fixes and stored baseline refresh. | None for M15C-040; keep C4 in future full-gate regression checks. |
| Journey | 0 | Stored upstream marker/task-line id drift; fresh 11.15 scopes task-line ids by SVG id. | Green after M15C-040 renderer fix and stored baseline refresh. | None for M15C-040; keep Journey in future full-gate regression checks. |
| Sankey | 0 | Link `stroke-width` differed in stale stored baselines. | Green after M15C-050 fresh 11.15 check and stored baseline refresh. | None for M15C-050; keep Sankey in future full-gate regression checks. |
| Class | 0 | Stored baselines were stale and fresh 11.15 exposed a full unified-renderer envelope convergence slice. | Green after renderer fixes and stored baseline refresh. `upstream_parser_class_spec` is skipped as an upstream prototype-key render artifact. | None for structural parity; keep in future full-gate regression checks. |
| Flowchart | 0 | The stored single MathML mismatch masked a broader 11.15 Flowchart envelope refresh. | Green for supported fixtures after child-lane convergence and stored baseline refresh. `flowchart-elk` is out of the current headless matrix. | None for structural parity; root-only residuals are split to `docs/workstreams/mermaid-11-15-root-viewport-residuals`. |
| XYChart | 0 | Data-label color mismatch was stale baseline drift. | Green after targeted 11.15 baseline refresh. | None for structural parity; keep in future full-gate regression checks. |

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

Current `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` result after
Sequence/C4/Journey/Timeline/Sankey/XYChart/Flowchart/ER/Class refresh and convergence: passed.

### Needs fresh 11.15 baseline before code changes

- class
- xychart

Class and XYChart already had targeted 11.15 behavior changes. This bucket proved why apparent
stored-fixture mismatches must be checked against fresh upstream 11.15 output before being treated
as renderer defects. Sankey, XYChart, and Class were in this bucket and are now green after fresh
11.15 baseline evidence and targeted refreshes or renderer convergence.

### Likely targeted code/normalizer issue

- flowchart MathML `columnalign`

This initially looked like a single fixture, but fresh Mermaid 11.15 output exposed a broader
Flowchart convergence lane. The supported Flowchart matrix is now structurally green; remaining
Flowchart work in this umbrella lane is root-only.

## Recommended Batch Order

1. M15C-030: fix active compare/report metadata so generated reports no longer present 11.12.3 as
   the current baseline.
2. M15C-040 batch A: regenerate/check sequence, timeline, C4, and Journey 11.15 upstream SVGs.
3. M15C-040 batch B: rerun full `parity` and recalculate mismatch counts.
4. M15C-050: refresh Sankey after fresh 11.15 output proves local renderer parity.
5. M15C-060: address the remaining Class, Flowchart, ER, and XYChart mismatches.
6. M15C-070: closed by splitting the remaining root/viewBox/max-width residual set exposed by
   `parity-root` to `docs/workstreams/mermaid-11-15-root-viewport-residuals`.
