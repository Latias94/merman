---
title: Evidence-led maintenance and Mermaid 12.3 alignment
date: 2026-11-03
source_commit: 84ef69b30f128d8a4f955e3aafb50f6378a4de5c
---

# Evidence-led maintenance and Mermaid 12.3 alignment

## Goal and authority

Implement the accepted findings in the [second maintenance review](../reviews/2026-11-03-maintenance-round-2.md)
and align development with the exact Mermaid `12.3.0` behavior graph. The maintainer
authorizes breaking development changes, retiring replaced implementations, local
environment preparation, Codex subagents, and focused Conventional Commits.
Continue on `refactor/fearless-maintenance-20261103`; its initial tree is clean.
Published artifacts and independently versioned syntax baselines retain their
historical identity. External delivery requires a separate request.

The [OpenAI maintenance article](https://developers.openai.com/blog/maintaining-software-with-the-gpt-6-family)
provides the investigation method: trace indirect consumers, inspect history,
challenge apparent redundancy, and match evidence to claims. The review supplies
the concrete failure mechanisms and deletion boundaries. Neither authorizes
removing a safeguard merely because its code appears duplicated.

## Implementation units

| Owner and outcome | Scope and retirement | Verification |
| --- | --- | --- |
| Controlled hand-drawn generation | RoughR generation, Venn/Ishikawa adapters, existing work meter. Remove uncontrolled production paths. Coordinate later shared path cleanup and upstream render changes. | Bounded pre-allocation failure, internal cancellation, ordinary ops/SVG and subsequent PRNG state. |
| Shared configuration lifecycle | Store managed JSON within the shared allocation; remove outer last-reference guessing and recursive trait paths. | Deep independent equality/formatting/COW and deterministic concurrent release in isolated processes. |
| Reconstructible host configuration | Accumulate one authored host layer and derive effective defaults; remove independent incremental effective merging. | Actual edge acceptance/rejection after ill-typed/typed updates, cold/hot inheritance and explicit presentation ownership. |
| Extension lifecycle reconciliation | Reconcile latest desired configuration and unfulfilled requirements; remove latest-precomputed-event scheduling. | Blocked restart→ignore and stop→disabled-status, stale startup cleanup and configuration/status agreement. |
| Worker transport ownership | Bound sent pending/draining requests, retain deadlines after cancellation, separate completed replay history. | Capacity, late valid/malformed results, unknown/replayed IDs, timeout, disposal and continued work. |
| Render process-tree drain | Escalate against owned transport completion, remove root-exit-as-tree-completion assumption. | Existing error distinctions, process-seam regression, real POSIX descendants, bounded settlement and post-close behavior. |
| Editor host location | Specify comment/fence/unfinished-buffer behavior, expose analysis-owned lightweight Rust locator, migrate LSP and align independent extension extraction. | Shared comment/colon/MDX/CRLF/Unicode cases, diagram admission, diagnostics, tokens and preview coordinates. |
| Shared error projection | Project Typst details from shared binding ownership; delete manual four-field selection. | Parse diagnostics and other detail variants, wide integers, safe operation echo, real WASM→Typst where available. |
| Native task allocation | Share the retained N-API engine allocation with accepted tasks; delete boundary value clones. | Cold/reused/concurrent async tasks, sync warmup, overrides, failure, cancellation, deadlines and disposal. |
| Controlled final glyph evidence | Thread operation control into fallible evidence collection; delete infallible uncontrolled traversal. | Mid-traversal cancellation, shared subroot deduplication, strict glyph coverage and sticky terminal errors. |
| Exact fuzz dependency recipes | Optional dependency groups and required features, gate unrelated helpers, retain seven sanitizer/replay targets. | Each real closure and harness build, locked complete coverage owner, release lock projection; report costs only if measured. |
| Known CI tool consumers | Map WASM size/module tools to Web/Typst owners; retain conservative generic fallback. | Single-path planner regressions, mixed-path fail-closed behavior and applicable real artifact checks. |
| Lean-core negative consumers | Give existing compile-fail doctests an explicit Linux feature-contract owner. | Defaults-off Flowchart/Sequence/State doctests; preserve positive coverage without feature-union contamination. |
| Current documentation | Correct context/schema and benchmark/feature recipes, current navigation and integrated-check guidance. | Owner manifests/commands, links, skill validation; retain historical ADRs and measurements. |
| Mermaid 12.3 reference graph | Verify exact tag/commit, registry integrity/provenance, companion ranges and source identity; update selected descriptors/locks and regenerate projections with existing tooling. | Official npm signatures, source/materialized/base-bound verification, installed graph consistency and runtime builder tests. |
| Mermaid 12.3 behavior | Inventory registrations, parsers, semantic/config/theme/render/editor/host deltas, port observable behavior and regenerate actual references/provenance. | Source-backed focused regressions, parser generation if needed, actual browser oracle and applicable three-mode structural/parity comparisons. |
| Markdown accessibility follow-up | After release alignment, fix [issue 276](https://github.com/Latias94/merman/issues/276): derive batch image alternative text from parsed `accDescr`, with the reference CLI's escaping and fallback rules. | The issue's ER reproduction, multiline and escaped descriptions, mixed batches, and a real CLI invocation against the selected reference. |

## Conditional decisions

Resolve these against the implemented tree before deleting their existing owner:

- ASCII metrics: preserve emitted extent, trimming, Unicode/deferred text,
  count-before-allocation and explicit work accounting. Reuse existing metrics
  only after an independent decoding oracle establishes equivalence. Retain the
  current scan: encoder admission returns a byte count, while final metrics also
  decode styling, observe emitted trimming, and join Unicode graphemes across
  fragments. No equivalent existing result or measured saving justifies deletion.
- Rough path preparation: reuse normalized segments and remove Block's local
  outline-first protocol only after ops and following random state agree.
  State's different fill-first recipe remains family-owned.
- Flutter one-shot: specify runtime policy, duplicate raw fields, authoring
  profiles, byte limits and cancellation ordering before a controlled C one-shot
  ABI migration. Retain the existing temporary engine in this round: a direct
  shared one-shot migration still initializes a Rust engine and changes concrete
  raw-option, authoring-budget and pre-cancellation error contracts. Existing
  smoke tests and their historical repairs establish consumers of that policy.
- Fresh resolution: exercise representative real family APIs and resolved
  package identities using the existing compatibility owner; retain historical
  recipes and describe its actual resolution-compilation guarantee.

No standalone deletion of the six public async wrappers or benchmark-profile
reset is planned: the review found real consumers and retention evidence.

## Scheduling and integration

### Reusable investigation and implementation prompt

```text
Investigate {scope} against the selected source and the maintenance review.
Trace direct, indirect, generated and public consumers; inspect relevant Git
history before calling a responsibility redundant. Breaking development changes
are authorized. Retire a replaced owner and migrate its consumers together.

For each finding, give its concrete trigger, file/line, observed or inferred
failure, replacement owner, deletion boundary, and independent behavioral oracle.
Preserve resource admission, cancellation, error precedence and source identity.
Do not infer a performance gain from fewer lines or a behavior pass from source.
Challenge proposed deletions against counterexamples and intentional safeguards.

Implement accepted findings only within {write_set}, using apply_patch. Other
workers share this tree: preserve their edits. Do not run Cargo, edit locks,
commit, publish, generate repository baselines, or expand tooling into a compiler.
Report a short checkpoint as soon as cross-file wiring can compile. Distinguish
actual red/green observations, source deductions, and validation still pending.

The coordinator owns serial bounded Cargo, exact dependency admission, generated
artifacts and final integration. A separate reviewer challenges the completed
change. Resolve verified findings and retire completed workers before handoff.
```

The coordinator immediately prepares the upstream source and package environment.
Independent writers own disjoint paths: RoughR/render generation; configuration;
extension lifecycle/processes; Worker transport. Subsequent waves cover adapters,
export, editor host extraction, CI/fuzz, documentation and conditional migrations.
Source-delta investigation may run read-only alongside implementation. Shared
render/config/ABI surfaces are integrated in dependency order.

Workers inspect existing tests before behavior changes, write directly with
`apply_patch`, preserve all peer/user changes, and report changed paths plus
evidence. They do not run Cargo, change locks, commit, or publish. The coordinator
owns serial bounded Cargo builds, generators, verification and commits, reusing
`target/`. Node tests may run independently when their files/resources do not
conflict. Focused failures are diagnosed before broader verification.

## Verification contract and completion

Each accepted unit must remove its replaced responsibility, migrate consumers,
pass its targeted behavioral oracle, and receive independent review. Upgrade
admission follows the [playbook](../release/MERMAID_UPGRADE_PLAYBOOK.md) and complete
[checklist](../../.agents/skills/align-mermaid-release/references/admission-checklist.md).
The trusted reference-transition base is the initial source commit above.

After focused diagnosis, run one applicable final integrated verification at the
settled source, plus independent reference trust, generated source, artifact,
browser and platform owners that the integrated command does not cover. Use
`nextest` for Rust runtime tests and `cargo fmt` for formatting. Keep the
Tree-sitter package baseline independent unless separately admitted.

Record the resulting commits, exact tests, conditional dispositions, intentional
breaking changes and remaining environment-only gaps in a completion receipt.
Do not claim unobserved red failures, hosted-CI success, measured speedups, browser
qualification, or supply-chain admission. The goal completes only after the
requested implementation and release alignment are substantiated; progress lives
in commits and the goal, rather than edits to this plan's decisions.
