# Mermaid 12 branch review — 2026-09-29

## Scope and authority

- Branch: `refactor/mermaid-12-alignment`.
- Base: `2d70832e25497aae282de9da78d1d6db12f2b475` (`origin/main`, already merged).
- Reviewed head before follow-up fixes: `bf95f8d922cd556514888bf0467a897c2cfaa050`.
- Diff: `git diff <base>...<head>`; 217 commits, 6,305 paths, including 567 source paths.
- Specification: `docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md` and ADRs 0089–0091.
- Semantic authority: local Mermaid `mermaid@12.0.0`, commit
  `98a0945418c76238f15df2afaddbba4272656c3b`, and pinned elkjs 0.9.3.

Six independent reviewers covered parsers, layout/configuration, SVG/text, public surfaces and
features, repository standards, and the originating specification. Follow-up fixes were reviewed
across ownership boundaries. Findings below are confirmed counterexamples, not an exhaustive proof
of equivalence. Review transcripts and command logs are local evidence under
`target/branch-review-mermaid12/`.

## Standards axis

One confirmed P2 violation: initial-path-stub normalization could delete the tangent controlling an
auto-oriented `marker-start`. Opposite arrow directions could then compare equal. This violates the
repository rule that SVG normalization must be non-semantic. The fix preserves vertices when start
or middle markers may be present, including inherited attributes, inline CSS, stylesheets, and
uncertain escaped declarations. Its regression failed before the implementation change and passed
afterward. No comparator tolerance or accepted-residual mode was broadened. Follow-up cross-review also caught and corrected State token-priority drift and CSS-wide font-variable inheritance in the proposed repairs; neither was left open.

## Specification axis

The independent specification review found the same P2 marker issue against the plan's non-semantic
normalization requirement. It is one defect observed on two axes, not two defects.

The reviewer checked 37 upstream manifests and 3,717 fixture input/SVG hash pairs, version identity,
and the CLI/configuration/lock/selection bindings. New-family catalogs, structured editor grammars,
examples, and feature declarations exist. These checks do not make historical completion reports
current-head verification evidence.

## Confirmed behavior defects and repairs

| Area | Counterexample or failure | Repair |
| --- | --- | --- |
| Agentflow identifiers | `end-user`, `end注文`, `style-guide`, and family keywords with punctuation were accepted as names unlike Mermaid | Respect ordered lexer rules and ASCII word boundaries; use the same predicate for editor rename |
| Agentflow direction | `direction` followed by a tab or ECMAScript whitespace failed | Match the complete upstream direction rule, including ordering and suffix consumption |
| Agentflow accessibility | `accTitle : Heading` failed | Accept the upstream whitespace before the colon |
| Agentflow comments | An unquoted label retained `%%` comments; delimiters inside comments affected scanning | Skip comments in unquoted labels while preserving quoted/Markdown content and metadata |
| Agentflow header | `agentflow-beta TB extra` silently discarded trailing tokens | Require a statement separator after the direction |
| Agentflow Markdown | ``A["`**bold**`"]`` retained delimiters and lost its label type | Carry parsed types through presentation into shared Flowchart rendering for nodes, edges, and containers; preserve metadata precedence |
| State identifiers | `direction --> X` was treated as an invalid direction statement | Consume only a complete direction rule and retain its actual source span; preserve higher-priority links and strings |
| Agentflow rename | Renaming to `flow`, `global`, or `connector` produced an invalid document | Add a family-specific generated rename policy and apply-edit/reparse regressions |
| Usecase rename | Renaming to `package`, `rectangle`, or other forbidden statements produced an invalid document | Share the parser's forbidden-name check with editor validation |
| Usecase SVG text | Plain `<br/>` became a line break | Escape plain labels before the shared createText path |
| Usecase HTML text | Paragraph margin and line-height caused text to leave its foreignObject | Restore the upstream paragraph reset and line-height |
| Usecase line breaks | Literal `\n` was rendered/measured literally | Share upstream-compatible plain-label line-break normalization between measurement and output |
| ELK configuration | JSON `2`, `2.0`, and `2e0` selected different layer bounds | Match elkjs integer parsing and its fallback for invalid values |
| Rustdoc feature coverage | Nested test builds omitted Agentflow and Usecase | Forward both features and render both families through a real proc macro |
| Rustdoc Usecase embedding | The static SVG validator rejected the renderer's actor font-size variable | Admit only six exact variables with bounded literal values; reject CSS-wide inheritance keywords, functions, and URLs |

The State identifier problem predates this branch; it was found by the requested adjacent-problem
search. Other entries concern the branch's new or changed surfaces. No confirmed P0/P1 finding was
reported by these reviewers.

## Validation

Completed checks:

- Core and editor: 1,698 tests passed after the final parser changes.
- Agentflow/Usecase SVG integration: 19 tests passed, including both Markdown output modes.
- SVG comparator: 42 tests passed; the new marker-start regression was first observed failing.
- Root-contract canaries: 9 tests passed.
- Renderer library: all 1,436 tests passed, including ELK projection and the final CSS-wide
  keyword hardening in static SVG validation.
- Rustdoc with only `svg,diagram-agentflow,diagram-usecase`: the real proc-macro feature-selection
  test passed after reproducing and fixing its original Usecase rejection.
- 28 additional renderer family singleton builds passed with ELK/Cytoscape available. The six
  remaining families were covered in the immediately preceding main-merge validation.
- Generated State LALRPOP source and editor-language contracts are fresh.
- Web contract check passed: 35 WASM exports, 45 runtime bindings, and 5 package entries checked
  through TypeScript.

Workspace formatting passed. Clippy passed with warnings denied for all targets and features of
`merman-core`, `merman-editor-core`, `merman-render`, `merman-rustdoc`, and `xtask`.
The full CI SVG comparison passed in `structure`, `parity`, and `parity-root` after the single
receipt revalidation below: 37 diagram types, 3,736 selected fixtures, 3,717 rendered, and
19 existing harness skips. No diagram selector or additional skip option was used.

Representative reproduction commands (run Cargo serially):

```text
cargo nextest run --locked -p merman-core -p merman-editor-core --all-features --lib --test structure --test-threads 4
cargo nextest run --locked -p merman-render --all-features --lib --test-threads 4
cargo nextest run --locked -p merman-render --all-features --test agentflow_svg_test --test usecase_svg_test --test-threads 2
cargo nextest run --locked -p merman-rustdoc --no-default-features --features svg,diagram-agentflow,diagram-usecase --test feature_selection --test-threads 1
cargo nextest run --locked -p xtask -E 'test(svgdom::tests) | test(root_contract)' --test-threads 2
cargo clippy --locked -p merman-core -p merman-editor-core -p merman-render -p merman-rustdoc -p xtask --all-features --all-targets -- -D warnings
cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root
```

## One revalidated exact residual receipt

The first release corpus run rejected the existing local signature for Flowchart fixture
`upstream_cypress_newshapes_spec_newshapessets_newshapesset5_tb_allpairs_035` in parity/parity-root.
The pre-review debug executable still passed the old receipt. XML comparison of its SVG with the
new release SVG found exactly one changed attribute across the same 126 elements: path
`L_n11_n22_0` used `stroke-dasharray: 0 0 162.18093162295358 4; stroke-dashoffset: 0;;` before and
`stroke-dasharray: 0 0 162.1809316229536 4; stroke-dashoffset: 0;;` afterward. The difference is
approximately 2.84e-14. Every other attribute, node, and text value was identical. An independent
reviewer repeated the XML comparison.

This signature does not pass through the marker-stub normalizer; that fix cannot explain the hash
change. The receipt preserves styles exactly and quantizes only path-data operands, so this final
floating-point digit remains visible to the gate. Revalidation changed only its local signature
from `73f9a97de53cb20c92272eebdef24f9edc72e8c98cb3379add60c648e15ba8c6` to
`491dc56b40a3db29d3e1d787414b3234c32e5f71dd69d350908395b75f837c7a`.
Input/upstream hashes, admitted modes, precision, normalization, and all other receipts remain
unchanged. This receipt binds the release output used by the CI gate; the older debug artifact
has the preceding last-bit representation and does not satisfy the refreshed exact signature.
The gate continues to reject any further unreviewed signature drift.

## Limits and remaining differences

- Explicit `elk.layeringLayerBound: null` retains the adapter's pre-existing default of 4;
  the raw ELK importer rejects null. Numeric representation and invalid non-null fallback are fixed;
  this adapter difference remains explicit.
- Browser text measurement, fonts, and foreignObject geometry remain bounded residual areas.
- Existing parity modes mask path coordinates. Preserving marker-bearing vertices fixes the newly
  identified normalization error; it does not prove every coordinate-only arrow change equivalent.
- This review did not rebuild every WebAssembly/mobile/native package or rerun every platform CI
  lane. The central CI workflow remains their owner. Existing WASM was used only to reproduce old
  behavior; post-fix claims use newly compiled Rust tests.
