# Mermaid 12 branch review, round 3 — 2026-09-29

## Scope and reference identity

This round continued the seven-reviewer audit of the Mermaid 12 branch, including the pending
round-2 renderer changes. The fixed base remains `2d70832e25497aae282de9da78d1d6db12f2b475`;
the starting head was `4f18730039191d97fa59980ba21b1957b57721a2`. The originating specification is
the September 20 alignment plan, ADRs 0089–0091, and the measurement boundary in ADR-0086.
The selected source graph did not change:

- Mermaid `12.0.0`, tag `mermaid@12.0.0`, commit `98a0945418c76238f15df2afaddbba4272656c3b`.
- Selection identity: `9634181804174022a42a1288be799a7612226a4a95b3374642a32515348b40e7`.
- Selection receipt SHA-256: `5a77f613f2d1f380ddaa5d9bab5e8111adb2cd833f319462a916694bebf10e28`.

Ownership stayed explicit across parser, layout, SVG, public consumers, standards, and specification
reviewers. Cross-review checked the fixes before the final gates. Cargo builds were serial with
one build job and incremental compilation disabled. Local evidence is under
`target/branch-review-mermaid12-round3/`. No remote delivery was requested.

## Standards axis

No new documented-standard violation was confirmed by the independent reviewer. A second reviewer
verified the numeric-model extension, duplicate-key handling, input-depth limits, and math-output
sanitization. Existing unrelated local changes were preserved.

The measurement policy and browser oracle remain unchanged. In particular, this round does not
turn an indeterminate filter audit into an accepted residual or replace the canonical deterministic
provider with a browser provider. The revalidated XYChart upstream hash is the only root-viewport receipt change.

## Specification axis

The previous explicit-null ELK finding is withdrawn. The earlier reasoning applied a raw ELK
importer result to the public Mermaid pipeline. Mermaid's `assignWithDepth.ts` ignores null before
`createRootElkGraph` runs; Merman's merge does the same. Runtime probes confirmed that a null
layer bound or null `elk` object preserves the default 4 or a previously configured 2. Regression
coverage now exercises incremental and exact site configuration, frontmatter, and directives.
No new production rejection was introduced.

A different ELK discrepancy was confirmed and repaired: enum names are case-sensitive and are
not trimmed, and the selected ELK provider also accepts Java enum ordinals. Previously `simple`
and ` SIMPLE ` incorrectly selected SIMPLE while numeric `0` selected the fallback. The five
public enum projections now use exact source names and source ordinal order; invalid options use
the provider's own defaults. Interactive node placement is wired to its already-existing kernel.
The selected schema's seven layer-assignment strategies remain the supported scope.

## Confirmed repairs

| Area | Failure | Repair and regression evidence |
| --- | --- | --- |
| Standalone Usecase feature | `diagram-usecase` referenced a helper module gated only by Mindmap/State | Include Usecase in that module's feature condition; reproduce the compile failure and test the independent build |
| Core-only JSON precision | Cargo feature unification hid different floating-point decoding; the largest finite decimal could be rejected | Enable the existing serde_json `float_roundtrip` feature in core and verify the same finite values without renderer feature unification |
| Non-finite JSON numbers | Valid JavaScript JSON numbers such as `1e309` were rejected | Keep strict serde syntax validation and bounded source traversal; retain a small typed infinity map for rendering while compatibility JSON remains null |
| JSON presentation | Infinity became indistinguishable from an authored null | Preserve sign through typed serialization, nested/escaped JSON pointers, duplicate-key replacement, and Dagre/ELK HTML/SVG rendering |
| Agentflow token priority | Greedy direction detection swallowed reserved declarations and click links | Preserve earlier keyword/exclusive-string rules and reject invalid direction positions |
| Agentflow scope closure | `end end direction LR` closed only one container | Yield each END before direction processing; verify both valid closure and an extra invalid END |
| Agentflow indentation | Trimming before keyword dispatch promoted indented END/click/classDef ahead of the greedy direction rule | Preserve the raw lexer cursor and END horizontal-whitespace consumption; verify spaces and NBSP against the complete pinned Mermaid runtime |
| Sequence fonts | Note rendering borrowed message font settings and numeric strings differed from measurement | Share source-backed family font selection between layout and SVG; honor global truthiness and numeric/px sizes |
| ELK configuration | Names were rewritten and ordinal options were ignored | Project exact names, source ordinals, and provider defaults without adding an option framework |

The Usecase typed JSON field `nonFiniteNumbers` defaults to an empty map and is omitted when empty.
Old serialized typed models continue to deserialize; finite-value and compatibility projections
retain their existing shape. External Rust struct literals for this branch's new UsecaseJsonNode
type need the additional field. No dependency or Cargo feature family was added; `float_roundtrip`
was already used by the renderer and is now owned by the parser that requires it.

Cross-review caught and closed the consecutive-END case, the independent-feature compile failure,
the raw-cursor indentation rule, accessibility precedence, and the floating-point
feature-unification blind spot. An initial click-link test also needed to
expect the existing URL-space encoding (`%20`). A title-host test incorrectly required a fallback
on a phase that never called the host; it now checks that unrelated operations remain on their
configured profile.

## Class semantic receipt precision

The full SVG run exposed three stale exact signatures in
`class/stress_class_many_relations_labels_020` (`id_A_D_3`, `id_B_D_4`, `id_E_A_7`).
Both SVGs were unchanged: local SHA-256
`ce83137b7b06a6e69b5d8024a6052b024ba8b15b98ef34a2262a79ea14dfd68d`, upstream
`8e8387d5cb031a1ab2730ae909f2f373d974d2892937985502f0d0b6ee2900b7`.
Enabling `float_roundtrip` corrected the comparator's JSON decoding of the embedded
`data-points` literal `432.54249999999996`. Its correctly rounded binary64 yields
`432.542` at the existing three-decimal precision; the old one-ULP-high decoding yielded
`432.543`. Only those three local point strings were corrected. All label, path, marker,
world geometry, upstream signatures, reasons, and comparison rules stayed unchanged.
Evidence: `class-semantic-revalidation.md` and `class-semantic-exact-diff.json`.

## Title viewport: verified host path, unchanged deterministic residual

Fixture `flowchart/stress_flowchart_title_padding_subgraph_029` is a font-measurement residual,
not a missing title union or an incorrect style projection. The existing public `TitleBBoxX`
channel was exercised through a complete native render using actual measurements from Chromium
151.0.7922.34 on Windows. CDP confirmed Arial/ArialMT fallback for the configured
`"Recursive Variable", arial, sans-serif`, 18px title.

The measured left/right extents were each 195.609375px. With all other measurement requests left
on their deterministic profiles, the host render changed only these root attributes:

- `viewBox`: `12 -27 376 241` to `4.390625 -27 391.21875 241`.
- `max-width`: 376px to 391.219px.

The no-host control was byte-identical to the canonical comparison SVG. Every descendant of the
host-rendered SVG was identical to the control. The unchanged browser oracle classified the
host result as upstream-inherited: the additional side overflow disappeared, leaving the same
bottom shadow depth as upstream. The paired host audit had zero blockers and no unused receipts.
The host regression also covers asymmetric extents, the original title anchor, both graph
backends, and measurement provenance.

Evidence: `title-browser-measurement.json`, `title-host-structural-proof.json`,
`title-host-provenance.json`, and `title-host-audit.json`. The temporary native probe's source is
archived with those local artifacts; it is not a production font table or committed fixture hack.

The canonical deterministic SVG is still unchanged and remains blocking under the Windows root
paint gate. ADR-0086 deliberately does not claim that font-agnostic estimates match arbitrary
installed fonts. Existing exact-residual admission also rejects this fixture's active-filter
indeterminate evidence. No receipt, padding constant, font coefficient, classification change,
or policy exception was added to make this result green. Hosts needing display-faithful title
bounds can use the existing measurement callback; this evidence does not make the deterministic
provider exact. Ubuntu rasterization was not verified in this local run.

## Validation

- Core/editor/WASM final library and structure suite: 1,754 tests passed.
- Independent `diagram-usecase` core build: 23 targeted numeric/model tests passed, after
  reproducing both the missing feature condition and the finite-number overflow failure.
- Agentflow final parser suite: all 54 tests passed after the indentation and accessibility
  precedence fixes. The final combined parser/integration run passed all 73 tests.
- Semantic golden fixtures, editor fixture corpus, Usecase public contracts, source presentation,
  nesting limits, and Agentflow position contracts: all 19 integration tests passed.
- Renderer and ELK library tests plus Agentflow/Usecase/Flowchart/Sequence SVG tests: 1,692
  passed in the combined run; the sole failed host-test expectation was corrected and passed
  separately. One redundant enum-mapping test from that run was removed; behavior-level enum
  projections and the selected provider remain covered.
- Full-feature all-target Clippy passed for core, renderer, ELK, CLI, WASM, and xtask with
  warnings denied, including a final rerun after the raw-cursor repair. The independent core
  Usecase library also passed Clippy with warnings denied. `cargo fmt --all -- --check` passed.
- Actual browser-title host render: original browser oracle passed with zero blockers; no-host
  control is byte-identical to canonical, with unchanged descendants in the host result.
- Release three-mode SVG comparison executed all 37 families. Its only failures were the three
  Class point signatures described above. After rebuilding xtask with the corrected receipt,
  the Class fixture passed `structure`, `parity`, and `parity-root`; the complete Agentflow
  family also passed all three modes after the final parser change. All other families retain
  the successful full-run results. No comparison rule was relaxed.
- Browser delta: all 3,717 local/upstream SVG pairs match the round-two final hashes.
  The original browser owner reran the title 029 and XYChart cases; both complete entries match
  the previous audit. The other 3,715 entries reuse hash-bound evidence with unchanged browser,
  owner, lockfile, and residual contracts. Combined results: 1,321 contained, 2,348
  upstream-inherited, 46 browser-owned diagnostics, one exact residual, and one blocker (029).
  No new blocker or unused receipt appeared. The official owner exit status remains 1 for 029;
  image-label fixture 136 remains contained. Evidence: `root-viewport-delta-inputs.json`,
  `root-viewport-delta.json`, and `root-viewport-combined-summary.json`.

## Remaining boundary

JSON strings containing isolated UTF-16 surrogate escapes remain rejected by the existing Rust
valid-Unicode string model. JavaScript can retain such code units. This round does not add an
alternate UTF-16 value model or replace those code units with different characters while claiming
semantic equivalence. This is separate from the repaired finite and non-finite number behavior.

## Commit disposition

Validated changes have been split into local Conventional Commits:

- `9f3b20f2b`: Agentflow token priority and consecutive scope closure.
- `a489e91b7`: Sequence measurement/render font selection.
- `3006b348f`: ELK exact enum names, ordinal projection, and null-merge regression evidence.
- `565c8179c`: Flowchart fork direction and image-label wrapping.
- `af41cef80`: Public host title-measurement contract test.
- `cecf38f61`: Agentflow family sizing configuration precedence.
- `d85da5f75`: Registered graph-layout capabilities and missing-root failure semantics.
- `efd722d3f`: Agentflow raw-cursor direction precedence, including accessibility priority.
- `7329095d1`: Atomic Usecase numeric-model/math rendering repair and exact Class point receipts.
- `16612323e`: Revalidated XYChart upstream root-residual identity.

The numeric model and its renderer changes were committed together. These scoped fixes do not
claim full browser-root admission.
No changes have been pushed.
