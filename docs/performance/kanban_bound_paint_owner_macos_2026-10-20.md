# Kanban bound paint owner — macOS, October 20, 2026

Kanban now retains one task-paint owner. Palette CSS is finalized in the existing item array at
the original binding phase; the renderer borrows the final strings into per-item stack views.
The second decision array and copied typed strings are removed. Each of ten themed catalog
cells saves five allocations and 556 bytes per complete public render. Removing the inline
optional-array field also saves 16 allocated bytes and 16 bytes of peak growth on the registered
no-theme medium cases, without changing their allocation count. This is allocation/work removal,
not a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `a991d441a6a7dfe16850d7b678ca3596e7f54626`, after the
[Pie CSS facts checkpoint](pie_css_facts_macos_2026-10-20.md) and the historical warmup-evidence
wording correction. The host is Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Registration,
source freeze, read-only baseline probes, build/link identities and raw observations remain in
`target/bench/experiments/kanban-bound-paint-owner-20261020/`. Both sides use release opt3 and the
same public feature set. The 645 success controls, 26 errors and 418 catalog cells are inherited
unchanged. Each complete public render has one warmup and five samples and retains its rich
result at the memory checkpoint. Loading, theme compilation and serialization are outside it.
Library/probe compilation uses one job; focused and full all-feature nextest share a feature
graph with four compile jobs and four test threads. Native tasks remain serial. The baseline
completes and freezes four probe identities before the production edit.

## One bound owner

`KanbanTaskTheme.items` already owns the resolved per-task fill, stroke, label and provenance.
Preparation formerly created another owned decision array, copied the typed strings and stored
it in `KanbanPreparedArtifact`. The original array remained alive. The second representation was
consumed only after all preparation had finished.

The private binder now finalizes eligible palette fills in place after the original text-paint
preparation and independent 12-slot CSS binding. Task order, typed-fill precedence and the one
lighten/darken operation per eligible palette fill remain unchanged. No later consumer needs the
raw palette string. Typed fill, stroke and label owners stay intact. The writer obtains a
borrowed decision view from each item, retaining its exact optional values, capability and rule
indices. It never repeats palette conversion. Preparation owns the only binding call and exposes
immutable completed data to rendering; no additional state flag or general tracker is introduced.

A later color error can leave earlier local items bound, but the existing error propagation drops
that unpublished preparation result. No public retry or partially prepared artifact escapes.
The first error, prior callbacks, work charges and cancellation order remain unchanged.
Some empty item arrays retain their original reconciliation, zero-work charge/checkpoint,
unsupported settlement and ordinal/effect handling. The separate 12-slot CSS resolution remains
complete even for unused slots. Radius/layout and label-owner selection happen before binding
and do not depend on the changed palette values.

The second array's length assertion disappears with that representation. The sole item owner is
constructed over the complete occurrence domain, and the existing layout/item-count assertions
remain. Active access uses direct indexing and never silently falls back. Only a retired
test-only array helper and its two None assertions are removed; all 25 test names in the three
changed files and independent output, ownership, radius and resource assertions remain.

## Complete public-operation observations

The source-selected primary is the existing `kanban_medium/editor-light` catalog cell. Both
allocation count and allocated bytes must independently decrease. Counts include allocation and
reallocation observations. Before sampling, the registration also notes that `FamilyPair` boxes
the complete artifact: removing an inline optional-array field may reduce no-theme bytes even
though that path never creates a heap decision array. No-theme secondaries require nonincrease,
without a strict saving or a promise that all metrics remain identical.

| Operation | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Kanban, editor-light catalog | 3,750 → 3,745 | 769,683 → 769,127 | 311,625 → 311,625 | 31,225 |
| Kanban, no-theme control | 615 → 615 | 115,415 → 115,399 | 45,838 → 45,822 | 30,319 |
| Kanban, no-theme catalog | 615 → 615 | 117,059 → 117,043 | 46,624 → 46,608 | 31,087 |

All 645 success controls preserve exact SVG/report identities and ordered custom-measurer
traces; all 26 errors match exactly. Of 503 jointly stable controls, both no-theme Kanban aliases
reduce allocated bytes and peak growth by 16, with allocation count and retained growth unchanged.
The other 501 retain all four metrics. The 142 variable rows remain inconclusive without retry.
Baseline has 140 and candidate 141: only baseline has backedges, while direction-conflict and
TreeView-02 vary only in the candidate. Relative to Pie's union, long-edge-label Flowchart is the
only addition and none is removed. No input or observation is filtered or resampled.

All 418 catalog cells are stable and preserve exact warmup result metadata, all five saved sample
SVG/report identities and configuration, diagnostics, theme status and family metadata. Warmup
SVG bytes are not captured and their byte identity is not separately observed. The ten themed
Kanban cells each save five allocations and 556 bytes. Brutalist, Spotless and Cyberpunk lower
peak growth by 584 bytes; the other seven typed peaks stay unchanged. The no-theme cell saves
16 allocated bytes and 16 peak bytes without changing its count. The other 407 cells retain all
four metrics. Retained growth is unchanged everywhere. The catalog covers 35 logical families
and 38 fixture aliases across ten presets plus no theme. Control and catalog totals stay within
their separate request/result recipes.

## Validation and continuation

Independent source and raw-evidence reviews find no blocker. Focused Kanban all-feature release
nextest passes 87 tests. Full public/renderer release nextest passes 5,613 tests with seven skipped,
and strict matching release Clippy passes. All 37 entries pass structure, parity and parity-root
checks with existing browser-text-layout and parser-diagnostic residuals retained; diagnostic
residuals are not DOM equivalence. The full optimized WASM package builds, including wasm-opt;
WASM runtime smoke is not repeated. Formatting, diff and changed-path checks pass. The driver
exits zero with all 13 registered steps successful. Logs, source/probe identities, independent
reviews and 37 current DOM reports remain in the ignored directory. The measured source patch
SHA-256 is
`98ef17deccd0899baf60db1eb9165eccd4fe35656fb1c2b1c72f4d24a130d729`.

The all-family convergence goal remains active. The earlier release observation still has
ZenUML, Packet and Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive; this
allocation result does not reclassify it. Next freeze the latest clean revision and refresh the
complete 19-scene same-SVG release cohort with fresh A/A and fixed balanced A/B. Use the existing
pipeline stages and bounded system CPU sampling to attribute the remaining release gaps. Keep
unimplemented receipt and ZenUML finite-fusion drafts separate until that evidence supports them.
