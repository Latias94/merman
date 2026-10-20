# v0.9.0 C4, Packet and Kanban confirmation (macOS, 2026-10-19)

Packet and Kanban still exceed the registered joint distance of 10% and 1 us from v0.9.0.
C4 remains inconclusive because the historical executable failed absolute A/A calibration.
The all-family convergence goal remains open.

This observation compares clean product `cd7b236741ae0640f24384d3ad595bfa70ffaf50` against
`v0.9.0` (`f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`). Both use the byte-identical pipeline
benchmark and corpus, the same fixture bytes and features, a reused engine, deterministic
measurement and the complete public SVG render operation. All three preflight SVG identities match;
timed invocations verify their postflight identities. Each checkout keeps its own locked dependency
graph. This historical range does not isolate the theme refactor or prove equivalent release error
and resource policies.

| Public E2E fixture | v0.9.0 (us) | Current (us) | Paired difference (us) | Paired relative estimate | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| Packet medium | 8.03646 | 10.23294 | +2.19648 | +27.33% | Confirmed regression |
| Kanban medium | 44.78075 | 56.49238 | +11.71163 | +26.15% | Confirmed regression |
| C4 medium | — | — | — | — | Inconclusive; no confirmation samples |

The simultaneous one-sided lower bounds remain above both thresholds: Packet is at least
+26.94% and +2.16663 us; Kanban is at least +25.74% and +11.54900 us. Base/head columns are
means of the independent confirmation estimates; relative movement uses the existing paired
log-ratio estimator rather than the ratio of those means.

The existing `compare_self.py` confirmation lane independently builds locked executables with one
Cargo job and distinct target directories. Each runner receives eight balanced A/A pairs per row.
The frozen recipe uses 30 Criterion samples, 2 s warm-up, 3 s measurement, 10,000 paired bootstrap
resamples and 95% simultaneous coverage over six relative/absolute components in this three-row
cohort. Packet and Kanban pass calibration and each receive 32 fresh balanced AB/BA pairs.

C4's historical absolute identity/order intervals fail the 1 us margin, with a power requirement
of 44 pairs above the preregistered cap of 32. It therefore receives no A/B samples and has no
admitted latency ratio. The harness derives its common fixed schedule from the cohort's maximum
requirement and caps it at 32, so the two stable rows receive 32 pairs despite individually requiring
only eight. The top-level minimum pair count is zero because C4 is retained; use the explicit
per-row counts. There are no evidence-contract failures. Exit code 1 reports the two confirmed
regressions.

Registration, commands, runner provenance, A/A observations, every independent confirmation
estimate, intervals and the complete report are under
`target/bench/experiments/theme-v090-confirm-c4-packet-kanban-20261019/`. Both executables were
copied and hashed after completion so later builds cannot replace this evidence. The host is
Apple M4 Pro/48 GiB, macOS aarch64, Rust 1.95.0. Cargo, allocation probes and profiling did not
overlap timing. Read-only source audits and a separate source-development checkout ran while
the measured checkout stayed unchanged; host noise remains subject to the recorded A/A gate.

The other sixteen previously selected same-SVG scenes and all changed-output lanes remain in
scope, as do 35 logical families and ten compiled presets. This cohort does not admit those rows
or measure compiled-preset latency. Exact allocation reductions from the preceding changes remain
separate evidence; they do not establish release convergence. Next work must attribute the remaining
Packet/Kanban gap and independently validate the newly identified TreeView, Radar, State and
Sequence ownership candidates.
