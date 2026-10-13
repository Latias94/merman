---
title: "CI Runtime Audit"
type: "research"
date: "2026-10-13"
---

# CI Runtime Audit

## Observed costs

The GitHub jobs API and step logs were inspected for successful main runs
[38336588276](https://github.com/Latias94/merman/actions/runs/38336588276),
[38279518132](https://github.com/Latias94/merman/actions/runs/38279518132), and
[38242866257](https://github.com/Latias94/merman/actions/runs/38242866257), plus PR run
[38233519599](https://github.com/Latias94/merman/actions/runs/38233519599). Durations are observed
elapsed step/job time, not forecasts or controlled compiler benchmarks.

| Path | Observed cost | Attribution and change |
| --- | --- | --- |
| Main Linux workspace job | 64–67 min | Move Node builds and SVG parity to independently gated jobs |
| Node-WASM plus N-API | About 18 min serial | Parallel matrix members; both remain required |
| SVG DOM plus Chromium oracle | About 16 min serial | Separate selected SVG job, preserving all three DOM policies and oracle |
| Pages | 35m47s–37m33s across four runs | One Web producer, separate budget-tool producer, parallel budget/browser consumers |
| Web size budget | 11m19s–13m08s | Latest 12m43s includes 67s native compile and 696s compression; use optimized xtask |
| VS Code Intel package | 21.7–32 min | Independent of editor-contract job; runner queues also affect end-to-end time |
| Editor-contract job | 1.8–2.6 min | Keep the assertion without serializing package builds behind it |

The Web budget command already consumes the existing packaged WASM through `--web-package-root`.
Its cost is not a second WASM compilation. The slow debug compression covered five artifacts;
retaining the existing budget implementation and compiling it in release mode avoids a duplicate
measurement script. The single uploaded Web package includes the freshness manifests used by
both downstream consumers.

## Verification boundary

The refactor keeps every selected owner in the same CI run and aggregates failures, skips and
cancellations through the existing fail-closed `pr-gate`. It retains Linux workspace coverage,
three-host font/platform regressions, weekly/manual full host coverage, all Web budgets, all
Playground browsers, Node install smoke and SVG provenance/parity assertions.

GNU release, CLI CI and VS Code runtime builds use Ubuntu 22.04 with an explicit GLIBC 2.35
version-needs ceiling. Setup checks do not wait for compilation; container checks depend only on
the GNU archives they consume; the stable-source Arch package has an independent owner.

Cold optimized-tool compilation, native artifact handoff and complete elapsed PR time require
GitHub runner evidence. Historical costs justify the dependency changes but do not prove a new
runtime. No less-than-30-minute claim is made before that run completes. Raw API responses and
logs are local ignored experiment artifacts rather than tracked copies of GitHub logs.

## First runner observation after the refactor

PR run [38353444277](https://github.com/Latias94/merman/actions/runs/38353444277), at commit
`47fd6806ee0a5e6c6f470bad02186bdb73438310`, ran from 05:50:12 to 06:33:41 UTC on October 13:
43m29s including runner queues. It completed with two failures: Arch compiler flags overrode
AWS-LC jitterentropy's required `-O0`, and the separated Playground consumer lacked Web checker
dependencies and original compiled-package evidence. Fixes require a subsequent runner result;
this run is timing evidence for successful paths, not a successful PR gate.

| Path | Execution time | Result |
| --- | --- | --- |
| Linux workspace job | 29m42s | Passed, including 9,420 workspace tests and feature/documentation contracts |
| macOS / Windows workspace jobs | 11m38s / 13m41s | Passed focused host coverage |
| SVG parity and oracle | 13m14s | Passed independently |
| Web WASM producer | 14m15s | Passed; one build feeds both consumers |
| Optimized Web budget-tool producer | 6m37s | Passed, including 6m09s cold compilation |
| Web size-budget consumer | 1m52s | Passed; budget command itself took 1m31s |
| Intel VS Code package | 29m55s | Passed; started 13m19s after the workflow began |

The Linux job's observed execution fell from 66m45s to 29m42s. The longest remaining path was
Intel VS Code packaging plus its runner queue. More parallel jobs would not automatically shorten
that queue. Feature-matrix checks include standalone consumers and intentional compile failures;
ordinary workspace tests cannot replace them. A further split would duplicate cold compilation
and needs separate measurement. This change does not guarantee a sub-30-minute complete run.
