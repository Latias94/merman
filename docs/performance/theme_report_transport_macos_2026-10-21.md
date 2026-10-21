# Whole-report Box transport diagnostic

This independent experiment tested whether boxing one frozen `RenderSessionReport` at the
family-completion boundary would make the full public SVG path faster by shrinking report values
carried through the renderer. It did not meet its preregistered screen and is rejected before a
formal latency confirmation. The candidate is separate from the accepted FontStack change and is
not present in this PR's source.

The pre-implementation registration selected eight full public end-to-end fixtures, four paired
runs per fixture in `AB, BA, AB, BA` order, with 20 Criterion samples and one-second warmup and
measurement windows per run. A fixture had to improve by **both** at least 10% and 1000 ns to
continue to a fresh A/A-calibrated confirmation. This was a diagnostic screen, not a formal
confirmation or a v0.9.0 comparison.

All eight SVG outputs matched exactly between the base and candidate, and every pair-side
postflight identity check passed. No fixture crossed the 10% relative threshold. Flowchart,
Sequence, and C4 point estimates exceeded 1000 ns, but their relative point improvements were only
0.110%, 2.079%, and 0.951%, respectively. The other five fixtures also missed the relative
threshold; none proceeds to formal latency testing.

| Fixture | Base → candidate (ns) | Candidate − base (ns) | Improvement (%) | 95% simultaneous relative bounds | Improvement bounds (ns) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Packet | 9,999.975 → 9,909.900 | −90.075 | 0.914 | −2.429% to 0.188% | −18.8 to 242.6 |
| ZenUML | 21,717.5 → 21,572.25 | −145.25 | 0.676 | 0.143% to 1.755% | 31 to 310 |
| Kanban | 52,546 → 52,682.25 | +136.25 | −0.257 | −0.717% to 0.559% | −407 to 292 |
| Flowchart | 1,089,075 → 1,087,900 | −1,175 | 0.110 | −0.992% to 1.130% | −10,900 to 10,425 |
| Journey | 27,952.75 → 27,616.25 | −336.5 | 1.218 | 0.650% to 2.135% | 180 to 589 |
| Sequence | 279,685 → 273,990 | −5,695 | 2.079 | 1.693% to 2.486% | 4,640 to 6,790 |
| Block | 32,060.25 → 32,185.5 | +125.25 | −0.389 | −1.665% to 0.457% | −540 to 147 |
| C4 | 111,230 → 110,177.5 | −1,052.5 | 0.951 | 0.279% to 1.695% | 140 to 2,120 |

The intervals are the diagnostic report's 95% simultaneous one-sided bounds over 16 components
with Bonferroni adjustment. They do not replace the registered materiality screen or establish a
formal result. In particular, the Sequence and C4 point changes exceed 1000 ns but remain far
below 10%; the Flowchart estimate is similarly too small relative to its baseline and its interval
is wide.

The allocation controls explain the tradeoff. In all 504 jointly stable site cells and all 418
stable catalog cells, the candidate adds exactly one allocation, 1416 allocated bytes, zero peak
growth bytes, and 1416 retained-growth bytes. All 645 ordered callback controls and 26 public error
controls match. This is an intentional per-render heap cost, not a memory improvement. The
separate `size_of` probe measured `RenderSessionReport` at 1416 bytes in both builds, while several
larger carrier types became smaller; those type sizes do not establish copied bytes or a latency
benefit.

The candidate commit `21625eda0` is a sibling of FontStack commit `0a2d250dd` and is not an ancestor
of PR head `68f213f1e`. The three candidate source blobs also differ from the PR head blobs. The
initial Clippy-failing variant, its controls, and the lint-corrected candidate evidence are
preserved separately; the repair did not change the registered representation tradeoff or timing
screen.

The [compact evidence archive](evidence/theme-report-transport-2026-10-21/manifest.json) contains
the diagnostic samples, registrations, source patches, control summaries, and the preserved
pre-lint failure. Larger local allocator result files, probe executables, and full raw capture logs
remain in the ignored experiment directory listed by that manifest. The archived evidence records
the diagnostic decision only; it does not admit the candidate or alter the release comparison.
