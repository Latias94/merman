# XY empty paint owner — macOS, October 20, 2026

XY paint preparation no longer constructs an unused sized empty Arc before it knows whether any
terminal paint proof needs an owner. The plan starts with no owner and captures a real Arc at the
original active phase. A captured empty map remains active and keeps its distinct identity.
Default medium saves one allocation and 40 bytes, including 40 bytes of peak growth. This is
allocation/work removal, not a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `33114519bae2740bf78753def8f0f7a5e045235a`, after the accepted
[GitGraph checkpoint](gitgraph_empty_palette_css_macos_2026-10-20.md). The host is Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. The ignored registration, source freeze, read-only baseline
probes, build/link identities and raw results are in
`target/bench/experiments/xy-empty-paint-owner-20261020/`. Both sides use the same optimized
release opt3 profile and public feature set, with separately identified libraries/executables.
All 645 success controls, 26 errors and 418 catalog cells are inherited unchanged. Each complete
public render has one warmup and five samples and retains its rich result at the memory
checkpoint. Input loading, theme compilation and result serialization remain outside it.
Library/probe builds use one job; same-graph focused/full nextest compilation uses four jobs and
execution four test threads. Native tasks and captures remain serial. All baseline operations
succeed and their four probe identities are frozen before the production edit.

## Absence is different from a captured empty owner

The plan's old `Arc<BTreeMap<...>>` default allocates a sized owner on every construction. Inactive
routes never consume it; active routes replace it with a newly captured owner. The field becomes
`Option<Arc<...>>`, initially None. The existing active capture always stores Some(Arc::new(map)),
even for an empty map. This is not the separate empty Arc-slice case, which can use static storage
on the current compiler.

The public resolver's early-return paths need no terminal receipt. Every other successful path
performs the existing accounting finish, then the same request classifier and capture before
returning the plan. Those classifier sets have no later public mutation. A required receipt
therefore still has its captured owner. Membership returns no ID without an owner. Begin first
checks the existing request classifier, then obtains the actual owner; expected serialized CSS,
text/effects, escape calls and BTreeMap order remain unchanged.

Completion without an owner returns false. With an owner it retains the original order:
pointer identity, validity, exact observed cardinality and single `OnceLock` completion. A foreign
empty map cannot certify the plan; failed attempts do not occupy its completion seal. A real
captured empty map can complete once under the original empty expected-domain rules. One focused
private contract distinguishes absent, foreign-empty and own-empty states and rejects repeated
completion. Existing source-owned scale assertions are adapted without weakening their semantic
checks. The XY font and series receipts, XML expected values, unsupported settlement, work
charges, writer/checkpoints, cancellation, callbacks and error order are untouched.

## Complete public-operation observations

The preregistered no-theme XY primary independently requires both count and allocated bytes to
strictly decrease. Counts include allocation and reallocation observations.

| Primary | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| XY medium | 1,890 → 1,889 | 178,300 → 178,260 | 103,927 → 103,887 | 15,343 |

All 645 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces, and all 26 errors match exactly. Of 506 jointly stable rows, 25 XY controls improve by
one allocation and 40 bytes each; 481 remain unchanged and none increase any of the four metrics.
Nine improvements reduce peak growth by 40 bytes and sixteen retain it. Retained growth is
unchanged everywhere. Typed or otherwise active routes still capture their real owner; the
removed allocation is the initial unused owner, not their expected terminal payload.

The 139 variable rows remain inconclusive without retry. Baseline has 139 and candidate 138;
only the baseline has `family-flowchart_backedges_subgraphs`. Compared with the preceding union,
this experiment has no additions and loses direction-conflict, long-edge-label and TreeView-02
rows, which are stable on both sides here. No input, observation or row is filtered or resampled.

All 418 catalog cells are stable and retain exact warmup result metadata and all five saved
sample SVG/report identities, together with configuration, diagnostics, theme status and
family metadata.
All eleven XY cells save one allocation and
40 bytes each, across no theme and all ten presets; 407 cells retain all four metrics. Only the
no-theme XY cell lowers peak by 40 bytes; all other peaks and all retained growth stay unchanged.
The catalog continues to cover 35 logical families and 38 fixture aliases.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The actual production code equals the reviewed
draft; formatting differences only affect new test assertions. Capture/reconciliation and the
expected serialized payload remain at their existing phases. Existing quoted-source palette,
zero-size text, foreign owner, complete-terminal-domain, opacity/Clear and exact-byte-budget
contracts remain applicable.

Focused XY all-feature release nextest passes 106 tests. Full public/renderer release nextest
passes 5,613 tests with seven skipped, and strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with existing browser-text-layout
and parser-diagnostic residuals retained; diagnostic residuals are not DOM equivalence. The full
optimized WASM package builds, including wasm-opt; WASM runtime smoke is not repeated. Formatting,
diff and changed-path checks pass. The driver exits zero with all 13 registered steps successful.
Logs, independent reviews, frozen sources/probes and 37 DOM reports remain in the ignored directory.
The measured patch SHA-256 is
`689ac144f3382dde8f3ad960f3ee50ea22feb891f5af978c722812fe19f16cc0`.

The all-family goal remains active. The earlier release observation still has ZenUML, Packet and
Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This allocation result does
not reclassify it. Next evaluate Pie's temporary CSS facts using a preregistered active typed
catalog primary; retain the no-theme path as an unchanged control and keep other receipt/finite
geometry candidates separate.
