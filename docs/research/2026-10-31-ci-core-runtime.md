---
title: "Core CI Runtime Follow-up"
type: "research"
date: "2026-10-31"
---

# Core CI Runtime Follow-up

The final successful [#258 CI run](https://github.com/Latias94/merman/actions/runs/41173324025)
qualified PR head `3edacea4a1af3f78268e56bf26960c190a11c82c`. The
[Linux build-test job](https://github.com/Latias94/merman/actions/runs/41173324025/job/124455735795)
ran for 85.6 minutes. These are GitHub job/step elapsed times, including each step's compilation;
they are not controlled benchmarks or isolated test execution times.

| Existing step | Observed minutes | Placement after this change |
| --- | ---: | --- |
| Full workspace | 19.8 | `build-test` |
| Representative architecture contracts | 12.6 | Feature matrix in `feature-contracts`; generated checks retain pinned DOMPurify/Node bootstrap in `build-test` |
| Preset qualification and family terminal behavior | 13.8 | `theme-contracts` |
| Optimized theme evidence and rejection | 7.1 | `theme-contracts` |
| Theme authoring across SVG/PNG | 3.6 | `theme-contracts` |
| Rustdoc subsets, math and artifact recipe | 7.7 | `feature-contracts` |

Within the architecture step, logs place `verify-generated` at 05:42:34 UTC and
`verify-feature-matrix` at 05:42:44 UTC. Retaining the ten-second generated check with its existing
materialized inputs avoids another DOMPurify checkout and Mermaid npm installation. Feature
tests use the native Rust RaTeX backend; they need no duplicate JavaScript runtime installation.
The complete workspace retains the existing optional Node/KaTeX probes.

The original Rust/Python command recipes and recipe-variable declarations are unchanged: 65
entries across the three resulting jobs, with identical multiplicities. No feature-minimal
command is merged into a broader Cargo invocation. The private acceptance cfg remains confined
to `run_theme_acceptance.py` subprocesses. Its debug and release tests share one owner; the offline
public production consumer remains after actual experimental platform-font builds in
`build-test`, which also compiles math and export dependencies through the workspace suite.

Both new jobs depend directly on `ci-plan`, use its existing `core` selector, and are required by
the same-run `pr-gate`. Workflow contracts check routing and rejection of failed, cancelled,
skipped or empty selected results. An unselected core owner keeps all three jobs deliberately
skipped. No classifier algorithm, release workflow, test recipe or platform coverage is removed.

This topology trades two cold Linux compiler bootstraps and runner slots for independent results
and a shorter serial chain. Registry caches remain enabled and compiled-target caches disabled;
no cross-job target archive is added. Actual critical-path time, queues and runner minutes must
be observed on the new PR before reporting an improvement. If feature cold builds or queues
outweigh the benefit, the bounded fallback is to retain only the separate theme job and return
feature/Rustdoc commands to `build-test`.

The source merge `5fa86a0361419f0d3f23c578b11f0e77d59a4a8a` has the same Git tree as the
qualified PR head. Its main-push run is separate evidence; this record does not claim its outcome
or rebind the architecture refactor's historical measurements to that merge identity.
