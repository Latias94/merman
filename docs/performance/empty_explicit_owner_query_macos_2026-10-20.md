# Empty explicit-owner query — macOS, October 20, 2026

Returning false immediately for an empty explicit configuration owner set removes temporary
path storage from the public render path. All old true branches require a set member, so an
empty set cannot own any string, including empty, dotted or Unicode paths. The surrounding
Tracking/Frozen compatibility-owner checks still execute; nonempty explicit-owner lookup is
unchanged. No public API, cache or configuration representation is added.

The preregistered no-theme Flowchart primary saves **40 allocations and 1344 allocated bytes**
per warmed public render, with unchanged peak and retained growth. All 418 catalog cells are
stable: **371 improve and 47 retain all four metrics**, with no increases. This is exact
allocation/work removal, not a latency improvement or proof of v0.9.0 convergence.

## Frozen experiment

The clean adjacent baseline is `a6742779c80252e40f535a3de087f842a61ba94c`. Registration and
independent review precede baseline capture and production application. All six baseline steps
complete successfully, and four actual baseline probe executables are SHA-frozen and made
read-only before applying the candidate. The preceding full19 release confirmation reaches its
real terminal exit and archives both selected executables before any baseline native work.

The Apple M4 Pro / native aarch64 macOS host uses Rust 1.95.0, optimized release, retained defaults
and the full public feature set. Library and probe builds use one Cargo job. Focused and full
nextest reuse the same feature graph, with four compile jobs and four test threads. Native command
stages run serially. Each allocation row has one warmup and five saved samples; all original
variable rows remain present without resampling. The SVG allocation probes use deterministic
measurement and default Renderer settings. Ordered custom-measurer callbacks are a separate
independent public control.

The preset is prepared outside warmed render measurements. All 645 saved control SVG/report
identities, independent ordered callback traces and 26 exact public errors must match. Catalog
admission preserves each warmup's result metadata and all five saved sample SVG/report identities,
configuration, diagnostics and family/theme metadata. Warmup SVG bytes are not captured. Jointly
stable rows require nonincreasing allocation count, allocated bytes, peak growth and retained
growth; the Flowchart / None allocation count must strictly decrease.

## Source and result

`MermaidConfig::explicit_config_owns_path` previously queried root, exact and ancestor members,
then built a descendant prefix and queried its lexical range even for an empty set. Its new entry
guard skips that entire unsuccessful query. It does not short-circuit the outer compatibility
predicate or skip theme calculation. There is no epoch load, journal mutation, copy-on-write,
callback, resource charge or error in the removed work.

The test change removes only a measurement wrapper and positive query-count assertion for the
empty explicit set with a real compatibility owner. Both Tracking/Frozen semantic branches,
derived-path authority, complete raw value equality and actual nonempty explicit-owner work
assertions remain. Existing boundary, Unicode, host/source and detector tests cover the behavior;
no implementation-mirroring guard test or new harness is introduced.

| Flowchart / None metric | Baseline | Candidate | Difference |
| --- | ---: | ---: | ---: |
| Allocations | 23770 | 23730 | -40 |
| Allocated bytes | 3210192 | 3208848 | -1344 |
| Peak growth bytes | 608311 | 608311 | 0 |
| Retained growth bytes | 123248 | 123248 | 0 |

Of the 645 controls, **504 are jointly stable**: 108 improve and 396 remain unchanged; 141 are
variable and remain inconclusive for allocation claims. The baseline has 140 variable rows and
the candidate 141. All stable improvements reduce allocations by 2–94 and allocated bytes by
72–3267, with unchanged peak and retained growth. Forty-seven improving controls use None and
61 use Some theme. Output and callback equivalence include the variable controls.

All 418 catalog cells are stable and exact. Eleven None cells improve: Flowchart, Class, State,
Sequence, Mindmap, Requirement, Sankey, Block, Treemap, Swimlane and Agentflow. Each of ten typed
presets improves 36 fixture aliases and leaves two unchanged, for 360 themed improvements.
The union covers 33 logical families and 36 aliases; alias count is not logical-family count.
ZenUML and Usecase retain all four metrics in every mode. No-theme Kanban and Packet also remain
unchanged. Every catalog cell retains the same peak and retained growth; there is no new cache
or persistent owner. Absolute process-live counters are not an admission claim in this experiment.

## Verification and limits

All 13 candidate command stages complete and the real runner exits 0:

- 930 focused and 7651 full release tests pass, with seven full-suite skips.
- Strict matching all-features release Clippy passes for core, facade and renderer.
- The full 37-entry SVG DOM matrix passes its original structure, parity and parity-root policies;
  reviewed browser-text-layout and parser-diagnostic residual boundaries remain unchanged.
- The optimized full WASM build completes, including `wasm-opt`, in transaction `f74f89003622`.
  This proves the build, not a browser runtime smoke.
- Format, diff and path-length checks pass; source bytes and diff match the frozen candidate.

Independent raw review reconstructs both sides' five samples and confirms all four gates, exact
outputs, callbacks, errors, inputs, frozen probes and linked optimized libraries. The terminal
review verifies actual successful checks rather than treating started stages as completed.
The formal release result remains the [separate full19 confirmation](theme_v090_post_initialization_confirmation_macos_2026-10-20.md)
at its own clean revision. This query reduction does not claim to fix the measured default
ZenUML, Packet or Kanban regressions.

## Evidence identities

- Registration SHA-256: `818146bfd8601de7843c5ae5531fecff7e9ff9f1318b9c84049745606dc53e14`.
- Frozen candidate source SHA-256: `54b9d91180e594b4fe3dcbdf9b9e642dc0afe823bc85b64c58fcd65c5ec8e787`.
- Independent raw review SHA-256: `6ca6b44c8a8a945ff47a38d79b64ef5b2416429eb51a166136ab97db8b2809f6`.
- Terminal receipt SHA-256: `2c58b4bab01468b3691d7dc82abf783d5c129cf70419c91bbd5a8144553281f6`.

The ignored directory `target/bench/experiments/empty-explicit-owner-query-20261020/` retains the
registration and reviews, unchanged probes and drivers, raw controls/catalog/error records,
source snapshots, successful command logs and the baseline's frozen probe executables.
