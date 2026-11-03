---
title: Second evidence-led maintenance review
date: 2026-11-03
status: review-complete
source_commit: 3a1acd36b705df8960c479f5fa74ae93ea01d74b
---

# Second evidence-led maintenance review

This is a new review of the source after the eleven maintenance changes, not a
reopening of their original proposals. The inspected source is
`3a1acd36b705df8960c479f5fa74ae93ea01d74b`, on
`refactor/fearless-maintenance-20261103`. The working tree was clean during
investigation. This review is the only repository change in this round. No
production implementation, dependency update, build, publication, or full test
suite was performed.

## Decision

Keep the current model/source/render/host architecture. The strongest new work is
inside existing owners: bound RoughR generation before it creates large temporary
outputs; make configuration lifecycle ownership survive sharing; reconcile queued
lifecycle requirements instead of dropping them; retain ownership of cancelled
Worker requests until their responses drain; and remove adapter-local projections
that demonstrably lose facts or duplicate initialization.

The maintainer permits breaking development changes. That permission makes a
clean final interface available; it does not prove that an apparent duplicate is
safe to remove. Every proposed retirement below still needs its behavioral oracle
and consumer migration. Published artifacts and historical evidence remain
immutable. This round was requested as review, so it records implementation work
rather than carrying it out.

## Method and evidence

The requested [OpenAI maintenance article](https://developers.openai.com/blog/maintaining-software-with-the-gpt-6-family)
was retrieved again on November 3. Its useful method is to investigate full
inventories and indirect consumers, verify apparent retirement against history
and real usage, preserve intentional safeguards, independently challenge
proposals, and prioritize measured costs. The article does not establish Merman
performance expectations or instruct a wholesale architecture rewrite.

Six independent Codex discovery agents covered the domains below. Six additional
agents challenged core, rendering, host, N-API, editor, and verification proposals.
All inherited the session model; there was no cross-model comparison. The
coordinator checked cross-domain ownership, selected counterexamples,
documentation recipes, real GitHub run metadata, and two executable state-machine
reproductions. All subagent work was read-only.

| Domain | Production paths inspected | Limits |
| --- | --- | --- |
| Core/model/markup/facade | Configuration, managed JSON, preprocessing, family catalog, semantic/presentation construction, public orchestration | Not every family grammar or HTML5 sanitizer branch |
| Render/layout/export/ASCII | Family preparation/emission, RoughR, text measurement, graph backends, output reports, final glyph evidence | Not every shape, fixture, or numerical kernel |
| SDKs/CLI | Shared binding operations, C/UniFFI/WASM/WASI/JNI/Node/Typst, Flutter, Python, Apple and CLI adapters | Not every generated binding or platform device |
| Editor/browser | Analysis, editor snapshots, LSP syntax, Markdown/rustdoc, Worker clients, extension lifecycle and preview operations | Not every rule, completion table, UI or benchmark realm |
| Tools/release/CI | xtask, scripts, artifact/feature recipes, selected workflows and release consumers | Not all remote settings, historical runs or release targets |
| Documentation/context | Root navigation, ADR status, workstream inventories, repository skills and current recipe consumers | Inventory is not line-by-line review of historical journals |

The prior [completion record](../knowledge/engineering/verification/2026-11/2026-11-03T130731Z-fearless-completion-e06db0b4c0734e9ea4f96b46564c2e57.md),
[October simplification review](2026-10-29-simplification-audit.md), current ADRs,
callers, feature declarations and relevant Git history were used to exclude
completed work. A public export with no repository caller was not classified as
dead. No bulk deletion list is justified by this review.

### Evidence actually executed

- A temporary Node 26.11.0 probe imported the current `WorkerClient` and
  `runRenderProcess` implementations. It used injected transport/process seams;
  it was not a browser, WASM, extension-host or real process-tree qualification.
- With `tombstoneLimit: 2`, three sent queries were cancelled before any response.
  The first legitimate response changed Worker termination count from zero to one;
  a subsequent query failed with unknown request ID. This reproduced E2 below.
- A fake child received SIGTERM, reported root exit, and retained open pipes.
  After the grace interval the implementation emitted no SIGKILL and the Promise
  remained pending. Manually emitting `close` finally rejected it as timed out.
  This reproduced the scheduling defect in E3, not OS-level descendant cleanup.
- The existing CI planner was queried with each of the two tool paths in T2.
  Its source dispatch and consumer chains establish the missing owner mappings;
  no hosted workflow was dispatched.
- The existing `worker-client.test.ts` suite passed all 26 tests under Node
  26.11.0, with no skips. Its green result alongside the reproduction confirms
  that cancelled-but-undrained capacity pressure is not covered by those tests.

Probe source and output are session-local at
`/tmp/merman-round2-repro-20261103.mjs` and
`/tmp/merman-round2-repro-results.json`. They are reproducibility aids, not durable
release receipts. All other new runtime counterexamples below remain static
source deductions unless stated otherwise. No huge-geometry or stack-overflow
input was executed.

### Hosted observations

Read-only GitHub API access worked through the configured local HTTP proxy after
direct access failed. The [successful mainline CI run 41600392535](https://github.com/Latias94/merman/actions/runs/41600392535)
ran source `0649021aa9bca54add6911b3551d4228943c0987` on November 3. Its 68
non-skipped jobs account for 37,180 seconds, or 619.7 unweighted runner-minutes,
using each job's start/completion timestamps. Seven `Build libFuzzer harness`
steps account for 5,694 seconds, or 94.9 runner-minutes; individual steps took
583–863 seconds. The three Tree-sitter harness builds account for 2,274 seconds,
or 37.9 runner-minutes. Their ordinary lockfile checks add 472 seconds across
seven jobs.

These are one-run observations, not averages, billed platform-weighted minutes,
post-refactor measurements, or promised savings. The fuzz manifests, workflow and
sources are unchanged between that source and the reviewed HEAD, but their
production dependencies have changed. Do not reattribute the measurements to a
new build or assume all compilation can disappear.

The current open-PR listing contained dependency PR #260 and DrawingList PR #121.
No fetch, merge or update of either was performed. Metadata also showed failed
CI/fuzz jobs. Complete failure logs were not obtained: redirected downloads
failed or timed out with partial content. Their causes are unclassified; they are
not evidence that a test is flaky or a regression was introduced by this branch.

## Prioritized implementation inventory

Severity describes the demonstrated failure mechanism, not implementation
order or measured performance. P1 denotes the resource-control gap; P2 denotes
bounded behavior or verification defects; P3 denotes maintenance opportunities.
Static findings still require a regression before implementation.

| ID | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| R1 | P1 | Venn/Ishikawa directly perform unbounded RoughR generation | Accept; first characterization and repair |
| C1 | P2 | Shared configuration can bypass its iterative lifecycle | Independently accepted; constructed deep-value exposure |
| C2 | P2, limited | Semantic effective config and cumulative overrides can diverge | Downgraded to Rust host-inheritance consistency |
| E1 | P2 | A later irrelevant config event can discard a required LSP restart/stop | Independently accepted; blocked-queue precondition |
| E2 | P2 | Cancelled in-flight Worker responses are evictable history | Reproduced in actual client with injected port |
| E3 | P2 | Root exit suppresses process-tree escalation while pipes remain open | Reproduced at process seam; POSIX integration required |
| E4 | P2 | Independent fence scanners activate fenced text inside HTML comments | Accept direction; pin editor-host semantics first |
| H1 | P2 | Typst manually projects errors and drops parse diagnostic details | Independently accepted |
| H2 | P3 | N-API asynchronous tasks clone cold per-engine initialization state | Independently accepted; benefit unmeasured |
| X1 | P2 | Final glyph-evidence traversal cannot observe operation control | Accepted for bounded cancellation observation |
| T1 | P2 cost | Every fuzz target compiles the union of all harness dependencies | Accept dependency split; measure build topology separately |
| T2 | P2 | Shared WASM tools do not select their real artifact CI owners | Independently accepted |
| T3 | P2 | Four existing lean-core compile-fail doctests have no standing CI owner | Independently accepted |
| D1 | P2/P3 | Current context/skill recipes drift and repeat integrated checks | Accept current-entry repair; retain historical records |

Independent challenges accepted R1, H1, H2, T2, T3, E1 and E2 with the scope
limits recorded below. C1's challenge rated the direct deep-value reliability
failure P1; the coordinator retains P2 for this inventory because default source
admission is bounded and no production incident was demonstrated. C2 was narrowed
to Rust host inheritance rather than a remote budget bypass. Flutter and fresh
resolution were downgraded from alleged contract failures; Rough path preparation
was downgraded from a broad performance proposal to local responsibility cleanup.
X1, E3 and E4 were checked by the coordinator rather than a separate challenger;
their implementation oracle remains explicit. This is not a twelve-vote consensus
or a claim that every candidate has independent runtime verification.

### R1 — Put resource control inside hand-drawn generation

**Evidence.** `crates/merman-render/src/svg/parity/venn/render.rs:50` and
`:99` call RoughR directly; intersections also generate fill paths at `:306`.
Ishikawa calls path and rectangle generation at
`svg/parity/ishikawa/render.rs:295` and `:316`. `venn/config.rs:24` clamps canvas
dimensions only from below. `venn.rs:381` estimates layout from sets,
intersections and iterations, not canvas magnitude. The hachure loop at
`crates/roughr/src/filler/scan_line_hachure.rs:146` sorts/advances scanlines and
accumulates lines before output reaches the bounded SVG writer.

A small single-set hand-drawn Venn with very large finite width/height can thus
request enormous generation while source/model counts remain small. Ishikawa can
also amplify geometry through large font size. These inputs were not executed.
The existing guard at `roughjs_common/paths.rs:981` only covers its callers and a
particular circle recipe; it is not an arbitrary path/rectangle resource proof.

**Final ownership and retirement.** Keep family drawing recipes and connect the
shared adapter to the existing operation work meter. RoughR sampling,
subdivision, edge processing, scanline loops and line/op growth need narrow,
fallible work checks and bounded cancellation checkpoints. Reject nonfinite or
nonadvancing calculations. Delete these uncontrolled production calls after
migration. Do not copy one circle's gap/step estimate onto every primitive.

**Counterevidence and oracle.** Preserve upstream drawing order and random-state
consumption, including invisible outlines and Venn's two-pass cross-hatching.
Characterize budget failure before excessive generation, cancellation inside
generation, ordinary ops/SVG and subsequent PRNG state. Use family SVG/RoughR
tests and existing parity policies. Medium effort; RSS and latency are unknown.

### C1 — Give the shared config allocation its deep-value destructor

**Evidence.** `crates/merman-model/src/config.rs:16` stores `Arc<Value>`.
Its outer `Drop` at `:184` iteratively clears only when `Arc::get_mut` succeeds.
Two concurrently dropped handles can both observe sharing before their Arc
fields decrement; the last decrement then recursively drops the raw `Value`.
`Debug` at `:20` and equality at `:29` also delegate to recursive raw values.
Public `from_value` accepts deep constructed values; existing small-stack COW
tests show depth is an intentional lifecycle concern, not categorically banned.
Independent challenge confirmed the legal concurrent-drop interleaving and
recursive trait paths. This report retains P2 because the demonstrated exposure
is directly constructed deep values; default source admission has depth bounds.

**Final ownership and retirement.** Store the existing managed JSON owner inside
the Arc. `json/managed.rs:208`, `:214`, `:245`, `:265` already own iterative clone,
drop, equality and formatting. Keep Mermaid merge policy in config. Remove the
outer last-reference guess and raw recursive equality/formatting paths.

**Counterevidence and oracle.** Sequential drop often succeeds, defaults can be
permanently cached, and source admission limits reduce exposure. This is not a
demonstrated default-text exploit. Preserve cheap shared cloning, COW counts and
value equality independently of source history. Check independent equal deep
values, formatting and final concurrent release in isolated subprocesses;
random stress alone is not proof. Medium effort; stack threshold unknown.

### C2 — Make host configuration reconstructible from one authority

**Evidence.** `merman-core/src/semantic.rs:366` merges incoming host config into
both cumulative overrides and already-defaulted effective config. Mermaid merge
ignores certain object/scalar type mismatches (`merman-model/src/config.rs:277`,
`:307`); it is not associative. Applying `maxEdges: {ignored: true}` and then
`maxEdges: 1` leaves an object in overrides but `1` in effective config. Facade
`merman/src/render.rs:965` reconstructs presentation from overrides and gets the
default `500`. Flowchart's parser budget at `diagrams/flowchart.rs:717` makes the
difference observable on a two-edge diagram.

**Final ownership and retirement.** Choose and document cumulative host-layer
semantics in core, then rebuild effective config from that owner, as the full
engine does. Delete the independent incremental-effective update. If later
valid host values should repair earlier invalid types, change the host-layer
policy consistently; do not patch maxEdges at the facade or change the pinned
Mermaid merge algorithm.

**Counterevidence and oracle.** The trigger supplies an ill-typed host value, not
an ordinary well-typed config. Public construction currently admits it and merge
tests specify ignore behavior. Independent challenge found that the upstream
site-config API itself maintains effective and delta state, so two representations
are not inherently redundant. The defect is the inconsistent inherited host
policy. Generic bindings reject `maxEdges`; this is not a demonstrated remote
budget bypass. Preserve explicit independently installed presentation engines,
secure-array replacement, source authority and cold/hot
inheritance. Test acceptance/rejection and exact budget, not just two matching
config objects. Small-to-medium effort; reconstruction cost unmeasured.

### E1 — Reconcile lifecycle requirements instead of latest events

**Evidence.** `tools/vscode-extension/src/extension.ts:100` computes an action at
event time. `runLanguageClientAction:180` increments the global generation for
every action; `applyLanguageClientAction:283` discards older queued actions.
Queued restart followed by an unrelated `ignore` can therefore execute neither
restart nor reconciliation. Disabled status can similarly supersede a queued
stop. Startup-staleness compensation does not cover actions skipped before
startup.

The concrete interleaving requires a blocked queue, for example an unfinished
configuration notification, an existing client, and successive server-path and
VS Code `merman.preview.background` setting changes. The preview toolbar's
session-only background toggle is not a trigger. Ordinary consecutive edits do
not guarantee that timing window.

**Final ownership and retirement.** Keep the extension lifecycle owner, record
unfulfilled restart/stop/config requirements and reconcile latest desired
configuration against actual client state at dequeue. Remove the assumption that
the newest precomputed event subsumes previous requirements. Retain stale-start
generation checks and cleanup; no generic state-machine framework is needed.

**Oracle.** Block the queue, inject restart→ignore and stop→disabled-status, then
assert final process/config/status agreement. Keep action classification and
startup-cleanup tests. Medium effort; event frequency unmeasured.

### E2 — Retain ownership of sent cancelled requests until drained

**Evidence.** `playground/src/editor/worker-client.ts:616` removes pending work
when the caller cancels; `RequestTombstoneLedger.add:755` stores it in the same
evictable ledger as completed requests. `handleResponseWithoutPending:635`
poisons unknown IDs. Worker execution is synchronous and still returns a result.
The temporary three-query probe reproduced a legitimate response being rejected
after eviction. Default capacity is 256; actual UI incidence is unknown.

**Final ownership and retirement.** Keep sent-but-cancelled requests in a bounded
draining state until their result is validated/consumed or the transport closes.
Bound admissions and keep transport deadlines; cancellation currently clears its
request timer. Remove insertion-order eviction of outstanding response ownership.
Completed replay history can stay bounded. Do not tolerate arbitrary
unknown IDs or silently skip malformed late results.

**Oracle.** Extend existing injected-port tests for capacity pressure, late valid
and malformed results, duplicate/unknown IDs, deadlines, disposal and continued
queries. Browser/WASM qualification remains separate. Medium effort.

### E3 — Let process-tree completion depend on transport drain

**Evidence.** `tools/vscode-extension/src/render-process.ts:98` escalates only
while the root process still has null exit/signal codes, but `:141` settles only
on `close`. A wrapper may exit after SIGTERM while a descendant keeps the pipes
open. The temporary process-seam probe reproduced skipped escalation and a
Promise that settled only after manually closing the transport.

**Final ownership and retirement.** Keep tree termination, timers, pipes and
settlement in `runRenderProcess`. Remove root-exit-as-tree-completion logic;
escalate while the owned transport/tree is not drained, and clean up after actual
completion. Preserve abort/timeout/output-limit error distinctions.

**Counterevidence and oracle.** Existing tests cover a live root ignoring TERM,
not a root exiting ahead of descendants. A real POSIX child-tree test must prove
descendant reclamation and bounded rejection. Review PID/group reuse and
post-close signaling before implementation; do not infer Windows taskkill
semantics from the POSIX example. Small-to-medium effort.

### E4 — Share editor-host fence location without semantic coupling

**Evidence.** `merman-analysis/src/document.rs:853`,
`merman-lsp/src/syntax_highlighting.rs:558` and extension
`preview-source.ts:283` independently scan fences. Each recognizes a Mermaid
fence inside a multiline HTML comment. Analysis uses that scanner for diagram
limits as well as facts, so hidden text can consume diagram admission and produce
diagnostics. `merman-doc/src/markdown.rs:564` already has an independent test
excluding commented fences, but its rustdoc contract is broader and different.

**Final ownership and retirement.** Expose a lightweight editor-host locator
from the existing analysis document owner; share it with Rust admission and LSP
syntax segmentation without constructing semantic snapshots. Remove LSP's local
delimiter/open/skip scanner. Keep VS Code extraction independent of an optional
LSP; align its host behavior using shared cases rather than a cross-language
parser framework. Retain rustdoc transformation separately.

**Counterevidence and oracle.** Specify comments, unfinished buffers, colon
fences and MDX deliberately before changing behavior. Test diagram counts,
diagnostics, tokens, CodeLens/preview, CRLF and Unicode coordinates together.
Do not promise full CommonMark conformance from a new comment heuristic.
Medium-to-large effort; host product contract needs characterization.

### H1 — Project Typst error details from their shared owner

**Evidence.** `merman-typst-plugin/src/lib.rs:290` manually copies resource,
icon, cancellation and authoring details, but omits the later-added diagnostic
field in `merman-bindings-core/src/common.rs:755`. Render parse errors at
`render/request.rs:767` produce it; `distribution/typst/merman/src/render.typ:27`
already forwards details. This affects parse-error envelopes, not all analysis
diagnostics, which have their own successful analysis payload.

**Final ownership and retirement.** Let existing `ErrorDetails` provide one
ordinary-integer projection. Delete Typst's four-field selection and keep its
independent outer envelope and safe operation-name handling.

**Oracle.** Check real malformed source through plugin WASM and final Typst
`mermaid-result`: code, original UTF-8 span and diagram type survive. Preserve
existing detail variants, omitted-empty behavior and operation echo limits.
Do not reuse JS-safe integer encoding; verify Typst JSON's wide-number boundary.
Small effort. No intentional diagnostic exclusion was found by challenge.

### H2 — Share the N-API task's engine allocation

**Evidence.** `merman-node/src/napi_transport.rs:41` clones `BindingEngine` into
each asynchronous task. Its per-instance `OnceLock`s clone as values. Purely
asynchronous no-override SVG calls repeatedly initialize their cold task copies;
the retained NativeEngine never becomes warm. Sync prewarming changes this
behavior; analysis/ASCII and explicit overrides are separate paths. This is
structural repeated initialization, not a measured latency regression.

**Final ownership and retirement.** NativeEngine and tasks should share an
`Arc<BindingEngine>`; remove the whole-engine clone at this task boundary. Keep
per-request operation controls, limits, override materialization, operation-level
theme clones, and public `BindingEngine::Clone`. Disposal rejects new work while
accepted tasks retain their reference. No global cache or render mutex.

**Oracle.** Combine existing core materialization evidence, task Arc identity,
and real installed asynchronous N-API probes. Cover cold repeated/concurrent
requests, sync warmup, override recovery, failures, cancellation and dispose.
Test deadline observation after contention on cold initialization. Small change,
medium verification; end-to-end benefit unknown.

### X1 — Observe control during final glyph evidence collection

**Evidence.** `merman-export/src/font_environment.rs:125` returns an infallible
font plan and takes no control. Its final-tree collection at `:208`, `:227`,
`:264` walks admitted faces, groups/subroots and positioned glyphs. Raster and
PDF call it at `lib.rs:1935` and `:2285`. Outer backend settlement rechecks
terminal state, so the proven gap is observation delay, not cancellation
incorrectly returning success.

**Final ownership and retirement.** Pass the existing control into the font
evidence owner, check bounded loops, and return a fallible plan. Remove the
uncontrolled infallible traversal. Keep final visible-glyph proof; resolver
history cannot replace it.

**Oracle.** Reuse font/admission tests for cancellation after traversal begins,
shared-subroot deduplication, strict glyph coverage, sticky terminal priority
and no partial plan publication. Small-to-medium effort; delay unmeasured.

### T1 — Narrow fuzz dependency recipes before changing runner topology

**Evidence.** `fuzz/Cargo.toml:14` unconditionally selects Mermaid, FFI,
export/math and Tree-sitter dependencies. Three Tree-sitter harnesses do not
need Mermaid execution, while `fuzz/src/lib.rs:3` unconditionally compiles its
helpers. Every matrix row checks the full package before its ASan build. Hosted
observations above establish substantial duplicated build cost.

**Final ownership and retirement.** Use a small number of optional dependency
groups and native `required-features` in the existing fuzz manifest. Gate
unrelated shared helpers. Remove the all-dependency requirement from
Tree-sitter-only harnesses and replace repeated whole-package checks with exact
recipes plus one complete locked coverage owner. Keep all seven targets,
sanitizers, seeds, regression inputs and discovery budgets.

**Counterevidence and oracle.** Independent runners provide isolation and
parallel discovery. Do not replace them with one long serial job merely to
minimize builds. Validate each actual dependency closure, all ASan builds and
committed inputs; inspect fuzz lock release projection. Only then measure
whether grouped build handoff beats independent jobs. Medium effort; no
cross-run target cache or bespoke build framework is proposed.

### T2 — Close the CI planner's known tool-consumer mappings

**Evidence.** `scripts/ci_plan.py:150` falls back to core/hygiene for xtask.
`wasm_size_matrix.rs:240` owns both Web and Typst budgets;
`wasm_module_surface.rs:84` is consumed by final Typst validation. There is no
automatic core→Web/Typst fan-out. Changing either tool alone can pass a PR with
the actual artifact owner intentionally skipped.

**Final ownership and retirement.** Add exact mappings before the generic xtask
rule: `wasm_size_matrix.rs` → core/hygiene/Web/Typst;
`wasm_module_surface.rs` → core/hygiene/Typst. Retire their treatment as
core-only inputs. Do not infer a Rust call graph or broaden every xtask change.

**Oracle.** Add single-path cases to existing planner tests and retain same-run
fail-closed aggregation plus real owner checks. Mainline full selection does not
replace PR selection. Small effort; necessary coverage may increase CI cost.

### T3 — Execute existing lean-core negative API contracts in CI

**Evidence.** Four `compile_fail` doctests in `merman-core/src/lib.rs:2` assert
presentation/editor APIs are absent from a lean build. The standing workflow
only runs Render/facade doctests (`.github/workflows/ci.yml:599`). Nextest and
feature `cargo check` do not execute dependency doctests. Independent consumers
currently enable these capabilities and do not supply the same negative proof.

**Final ownership.** Add the existing `xtask verify` recipe to the Linux
feature-contract owner:

```console
cargo test --locked -p merman-core --no-default-features --features diagram-flowchart,diagram-sequence,diagram-state --doc
```

No new tests/framework are needed to invent the requirement. Verify that an
accidental protected export makes the lane fail without workspace feature union
masking the lean recipe. Keep positive family/feature tests. Small effort;
current source gating is correct and new runtime cost is unknown.

### D1 — Repair current instructions, not historical facts

Four specific current-entry issues form one documentation workstream:

1. `CONTEXT.md:81` states facts schema 2 and rejects other versions. Current
   `merman-analysis/src/payload.rs:8` and Web consumers require 3. Correct/link
   the current-development contract; published alpha.1 legitimately remains 2.
2. Performance skill `references/benchmark-surfaces.md:22` uses pipeline with
   only `svg`, and PR skill `SKILL.md:211` omits `compat-json,editor-facts`.
   `merman/Cargo.toml:461` requires them. The performance surface also lists
   `parse_known_type` as current rather than a history alias. Update or remove
   copied recipes; manifests/corpus/current benchmark guide remain owners.
3. Root navigation prioritizes the historical June issue ledger and superseded
   ADRs. Point current work first to current architecture/alignment owners;
   retain the ledger as a historical investigation that requires revalidation.
4. Upgrade skill `SKILL.md:223` and playbook closing blocks can be read as
   running generated/alignment checks, then integrated strict, then fmt/clippy
   again at the same revision. Explain focused diagnosis versus one final
   integrated pass; preserve distinct reference trust and platform/artifact
   checks outside strict.

The documentation inventory found 41 workstream manifests, 40 closed/completed
and one active. No current automation was found reopening them. Do not delete
historical decisions, measurements or task files just because they are old.
Repair navigation and current copied claims instead. Review instructions through
the existing writing-for-agents owner before editing repository skills. Use
actual command/manifest evidence and link review, not new prose snapshot gates.
Small effort; agent time/token savings have not been measured.

## Conditional follow-ons and rejected interpretations

| Proposal | Current evidence and condition | Decision |
| --- | --- | --- |
| ASCII output metrics | Canvas/styled encoders discard metrics; `layout_selection.rs:119` and `output.rs:444` rescan their own output, including ANSI/HTML decoding. Return text plus existing `OutputMetrics` from encoders and remove production roundtrips only after independent decoding agrees for trim, empty rows, wide/deferred text and every encoding. Preserve count-before-allocation and explicit work accounting. | Conditional, medium-to-large; benefit unmeasured. Do not replace actual emitted extent with canvas rectangle. |
| Rough path preparation | `roughjs_common/paths.rs:303` parses/normalizes a single sampled set three times and multiple sets twice; Block duplicates outline-first preparation at `block/render.rs:326`. Reuse existing normalized segments and delete only Block's protocol copy after ops and subsequent random state agree, including Block-only build. | Downgraded to local P3 cleanup. State's fill-first and family DOM logic remain distinct. |
| Flutter one-shot | `merman_ffi.dart:2501`, `:2653` reinterprets options and emulates one-shot through a temporary engine. A generic controlled C one-shot slot could delegate to the existing native artifact contract and retire local helpers. | Downgraded: current documentation explicitly promises a temporary engine; cancellation ordering is not a proven violation. Nested runtime-policy, duplicate raw fields, authoring profiles and byte-budget semantics differ from direct core execution. Specify/unify those before deleting the Dart path; coordinate ABI discovery/generated libraries. |
| Fresh-resolution consumer | `verify_prerelease_compatibility.py:26` selects `ascii,svg` with defaults off, so current family branches are unselected; its consumer at `:193` is empty. Improve representative family/API coverage using existing owner and real resolved package identities. | Downgraded: empty main still compiles dependencies and the gate promises resolution compilation, not complete source compatibility. Preserve historical-version recipes and precise exceptions; no published failure proved. |
| Six async parse wrappers | They are synchronous work behind async-shaped public methods and have actual xtask/test callers. | Not a standalone strong candidate: ADR-0008 permits composition wrappers; external migration cost is unknown. Retire only as part of an independently justified public migration. |
| Remove benchmark profile reset | Native-memory discovery cleans the bench profile; repeated work can look wasteful. | Rejected without replacement provenance evidence: recorded cross-checkout fingerprint incidents explain the safeguard. Existing batch lanes already share actual builds. Do not replace it with another cache/provenance framework during this review. |

No runtime or artifact improvement is claimed for the conditional candidates.

## Execution sequence and deletion conditions

Start with independently reviewable workstreams. The order below prioritizes
bounded correctness and clear ownership before speculative performance work.

1. **Characterize and fix R1/C1/C2.** Use deterministic work counters, isolated
   lifecycle probes and explicit configuration outcomes before touching kernels
   or config storage. These are separate write scopes. R1 and later Rough path
   cleanup share files and should be sequential or share one owner.
2. **Repair editor lifecycle E1/E2/E3.** Each has a distinct owner and can run in
   parallel. Promote the temporary E2/E3 reproductions into owning regression
   tests, then cover actual browser/extension/POSIX consumers. Do not broaden the
   product validation claim from the injected seams alone.
3. **Close adapter gaps H1/X1/H2.** H1 owns error projection, X1 export traversal,
   H2 N-API task ownership. Retain exact integer, operation/control and
   dispose/override contracts. H1 and Flutter work both touch shared bindings;
   coordinate rather than overlapping patches.
4. **Repair proof routing T2/T3 and current instructions D1.** Use existing
   classifier tests, negative consumers and documentation owners. No new global
   source parser, mandatory prose gate or duplicated current-issue ledger.
5. **Implement and measure T1.** First prove exact fuzz dependency closure and
   unchanged ASan/replay coverage. Compare hosted build and queue cost with the
   observed run; choose any build-once topology only from new evidence.
6. **Resolve E4 and conditional proposals separately.** Pin editor Markdown
   behavior, Flutter one-shot semantics, ASCII metrics and RoughR PRNG oracles
   before deletion. Defer candidates whose extra abstraction or migration cost
   exceeds their demonstrated benefit.

For every accepted workstream, migrate every in-scope consumer, delete the old
responsibility and temporary adapters, record intentional behavior changes, and
run the narrow existing owner before widening. Cargo remains serial and bounded,
reusing `target/`; code-reading agents do not run parallel full builds. Update
actual descriptors/generated bindings and migration guides only when their
contract changes. Independent review closes findings, not just test failures.

### Reusable second-pass prompt

```text
At immutable commit [SHA], inventory [SCOPE] and investigate substantial unused
or repeated work, stale behavior, and ownership defects. Work read-only. Exclude
the completed maintenance retirements and check current ADRs and actual callers.

For every candidate trace entry to consumer, including feature-selected,
generated, dynamic and external public surfaces. Give a concrete counterexample,
history, the strongest retention argument, evidence gaps, final owner, exact
consumer migration and responsibility to delete. Zero findings is acceptable.

Separate behavior defects, equivalent refactors and optional public migrations.
Breaking development changes are available, but safeguards, source semantics,
budgets, cancellation, error priority, lifecycle and published identity are real
contracts. Do not claim a public export is dead from an empty repository search.

Use measured cost when available; otherwise label cost unknown. Identify the
existing behavioral oracle and the missing negative case. Do not build a source
analyzer, cache, framework or compatibility layer just to justify a deletion.
Return severity, confidence, effort, coverage limits and deliberate non-removals.
```

Challenge each serious candidate independently with implementation/callers,
contract tests and the proposed existing owner. The challenge must return
accept/downgrade/reject, not consensus by default. Preserve downgrades in the
report. A workstream is ready for implementation only after its stated behavior
and deletion conditions are concrete.
