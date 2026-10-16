---
title: Demand-Driven Theme Rendering - Refactoring Approach
type: refactor
date: 2026-10-16
stage: complete
execution: code
---

# Demand-Driven Theme Rendering - Refactoring Approach

## Intended result

Ordinary rendering pays for the selected Mermaid styles, layout and serialization. Explicit typed themes add their actual selections. Requested reports summarize actual consumer outcomes. No path creates a second representation merely to prove that the first representation was used.

This continues the completed responsibility cleanup in `9a297b72f`; it does not reopen its retired protocols or contract migrations. The measured release-range reference is `docs/performance/native_v0_9_cross_family_macos_2026-10-16.md`. The starting implementation for fresh adjacent comparisons is `1ccdfc6b8`, including the Binaryen packaging workaround; freeze the exact final baseline before editing production code.

The sequence below records the accepted approach. Implementation and verification completed on
2026-10-17; see the [decision and measurement checkpoint](../performance/native_theme_demand_driven_macos_2026-10-17.md).
Block source-index sharing remains deferred after auditing its real consumers. No release-range
recovery percentage or confirmed latency benefit is claimed.

## Design rules

1. Keep one style-resolution and emission path. Source/config precedence, explicit clears, built-in themes, typography and native-consumer behavior continue to use the actual executing owners.
2. Delete unconsumed state entirely. A branch around an unnecessary receipt is an interim step, not the final architecture when the receipt compares a writer with its own plan.
3. Keep only decisions needed to render: selected paints, measurement inputs, source ownership and winner/ordinal information that changes precedence or requested results.
4. Actual writers contribute minimal outcomes to the existing family report owner when needed. Applied requires actual visible consumption; source-shadowed, unsupported, unknown and inapplicable remain distinct. An explicit theme can still require admission outcomes on a data-only API.
5. No theme/report-only allocation or model traversal is justified without an actual consumer. Ordinary source-style and built-in-theme processing can still require traversal.
6. Put exact retained-byte admission and sticky failures in the bounded output owner. Cancellation must remain bounded through emitted and discarded input, callbacks and final return. XML escaping remains a producer obligation.
7. Prefer immutable fixed defaults and shared baseline values over recomputation or per-occurrence copies. Do not introduce a cache framework, parallel unthemed renderer or generic evidence interpreter.

## Evidence and measurement correction

The existing 76-input, 38-family comparison remains useful. Its 19 identical-output rows support direct diagnostic comparisons; the 57 changed-output rows are release context. For this refactor, compare against the current implementation, so all 76 supported SVG rows should retain their current byte identities.

The Kanban tool receipt reports +76.23% after 8 A/A pairs per executable and 8 AB/BA pairs. However, its head A/A order-effect interval excludes zero: 53.10 to 577.50 ns, or log ratio 0.000662 to 0.007428. The stricter low-latency contract in `docs/performance/BENCHMARKING.md` requires zero inside every calibration interval. Preserve the tool receipt and measurements, but do not admit that run as satisfying the documented low-latency confirmation gate.

Before a fresh timing claim, minimally reconcile the existing comparator with that contract, then register calibration-derived thresholds and the fixed confirmation schedule. This is a bounded correction to the existing harness, not a new benchmarking framework. Do not relax thresholds or reuse the old confirmation samples.

## Proposed implementation sequence

### 1. Remove residual certification at family owners

**Owners:** `crates/merman-render/src/packet/theme/`, `architecture/theme/`, `kanban/theme.rs`, `state/compatibility.rs`, `family/preparation.rs` and their SVG writers.

- Packet: remove expected-count prewalks, copied font/fill maps, class-string self-comparisons and baseline terminal receipts. For requested mechanisms, aggregate actual writer decisions using the existing semantic roles and selector order.
- Architecture: remove typography-only inventory walks and copied CSS strings used to compare with the same plan. Derive requested visibility/application from the actual service/group/edge/icon-text writes. Keep Cytoscape layout typography separate from inherited SVG typography.
- Kanban: remove CSS/layout/label string copies whose only consumer is a receipt. Borrow the default group style and retain actual strings only when emission needs them. Preserve prepared label measurement and inherited-run/source ownership.
- State: delete `StateValueProvenance.selected_paths` and its merge/deduplication. Preserve the `owned` decision and fallback order.
- Replace tests and current guidance that describe deleted protocols. Keep tests for appearance, source precedence, partial application and strict errors.

Use independent family commits. The deletion target includes explicit-theme self-comparisons; merely gating the current no-theme constructors is insufficient where the same writer can produce the outcome directly.

**Acceptance:** current SVG bytes preserved; meaningful explicit-theme reports preserved; no baseline certification maps, copied evidence strings or proof-only traversals remain in these owners. Measure actual allocations/work before assigning a latency benefit.

### 2. Make fixed environment defaults cheap

**Owners:** `environment.rs`, `diagram_theme/assets.rs`, `diagram_theme/admission.rs` and the facade request/environment constructors.

- Represent the asset-free system-font catalog identity as canonical fixed data rather than hashing the same constants on each construction. Preserve its existing fingerprint bytes and meaning.
- Represent closed-set default capability policies and finite font-source orders compactly. Preserve custom restrictions, ordering, equality and public iteration behavior.
- Share immutable resource defaults where appropriate; operation counters, deadlines and cancellation remain operation-owned.
- Audit overwritten default construction at public request call sites. Keep the existing E2E benchmark unchanged for the original comparison; a separately named reused-environment lane can answer a different lifecycle question.

**Acceptance:** default creation/cloning removes repeated fixed SHA/collection work without changing policy decisions or host-font discovery. Record cold/fresh and reused-operation controls independently.

### 3. Consolidate output accounting and cancellation ownership

**Owners:** `crates/merman-render/src/svg/parity/output.rs` and the existing resource meter.

`XmlTextOutput` currently observes each formatting fragment, then forwards retained runs to `BoundedSvgOutput`, whose append path observes again. Small formatting fragments can defeat the intended chunk cadence.

- Keep XML character-data escaping and cross-fragment `]]>` state in the serializer.
- Let one operation-owned cadence account for scanned work and retained appends. Avoid duplicate cancellation observations for the same short fragment.
- Preserve exact overflow/byte checks before every allocation-growing append or replacement; spare capacity must never authorize excess retained bytes.
- Account for long discarded/invalid-scalar input too, so cancellation does not depend on emitted byte count.
- Retain callback-boundary observations, sticky failures and a final cancellation observation before success.

Profile the already-isolated render stage before choosing the implementation; use macOS sampling/Instruments and the existing allocation probes. No tracing/profiling dependency is required for this decision.

**Acceptance:** identical SVGs, bounded cancellation work and exact byte ceilings; allocation/observer evidence plus fresh Kanban/Timeline/Packet measurements, with Class and large Flowchart controls. Do not preserve incidental poll counts as a compatibility contract.

### 4. Remove duplicate style representations

**Owners:** ER visual preparation and writer, Mindmap theme preparation and writer; follow with State group typography and Block source indexing where measurements justify it.

- ER: share one source/baseline text visual per entity. Store actual typed cell differences sparsely rather than four independently serialized dual-mode visuals per attribute. Remove the overwritten rectangle formatting on the entity path while retaining subgraph styling.
- Resolve HTML/SVG lowering at its actual consumer. If a prepared artifact can be rendered with different options, preserve that reuse; do not freeze an incidental label mode during preparation.
- Mindmap: remove default edge vectors and fallback-value forwarding used only by absent theme outcomes. Preserve explicit Node.stroke-to-Neo-edge inheritance and source-owned Edge.stroke behavior.
- State: prepare composite typography only for the existing effective-group predicate, including note groups; ordinary leaves retain normal label typography.
- Block: share the actual merged source index where it is rebuilt. Preserve repeated-ID field merge order and useful terminal bindings.

**Acceptance:** allocation/retention curves independently vary entities, attributes, edges and style-token length. Default and requested-theme outputs remain identical. No interning/cache subsystem or second rendering path is introduced.

### 5. Integrate and close the release-range investigation

- Run affected nextest suites per owner; after shared-writer integration run the existing all-family DOM and renderer/bindings/native-consumer checks.
- Exercise built-in default/dark/neutral themes, empty typed themes, actual presets and partial requests; include source/class overrides, Clear, unsupported winners, ordinal selectors, hidden/empty text and custom measurement.
- Keep strict portability, opaque/custom SVG admission, native export, browser publication, resource-limit and cancellation cases at their real consumer seams.
- Run SVG-only and the full native feature closures, then first-party Web/Node consumers when their owners change. Cargo remains serial and timing runs exclude builds/profiling/tests.
- Use all 76 SVG inputs for current-base/candidate correctness and broad diagnostics. Confirm representative public operations only after calibration. Keep v0.9.0 as historical context for the output-changing rows.
- Remove superseded implementation-only tests and stale current guidance together with their code. Keep revision-scoped performance history and raw failed/rejected experiments.

## Measurement priorities

| Question | Primary evidence | Controls |
| --- | --- | --- |
| Did family proof deletion remove unused work? | Exact allocations/retained bytes or work counts at the affected owner | Default, partial typed request, source override, empty/hidden terminals |
| Did environment construction improve? | Public fresh-operation and reused-environment lanes measured separately | Cold initialization and custom policy behavior |
| Did writer consolidation improve latency? | Fresh same-output public E2E AB/BA after A/A calibration | Kanban, Timeline, Packet, Class, large Flowchart; byte and cancellation boundaries |
| Did ER/Mindmap representation improve scaling? | Independent model-cardinality/token-length allocation curves | Exact SVG and requested outcomes at each scale |
| Did the final integration regress another family? | 76-input current-base/candidate diagnostics and semantic checks | All 38 families, unchanged capability recipe |

Deletion/representation changes may be accepted for proven work or allocation removal without inventing a minimum speedup. Latency claims still require the documented calibrated lane. No fixed percentage of full v0.9.0 recovery is promised before measurement.

## Recommended first batch

Freeze the current baseline and correct the evidence classification. Then implement Packet, Architecture, Kanban and State cleanup as separate owner commits while profiling the shared writer. Follow with immutable environment defaults and the measured writer change. ER/Mindmap representation work follows those shared corrections, so its remaining cost is measured against the simpler pipeline.

Theme authoring compilation redesign, release dependency upgrades and the existing compile/schema deferrals remain separate work. There is no unresolved product choice required to start this sequence within the maintainer's previously authorized cleanup scope.
