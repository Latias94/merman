# Mermaid 12 round-five repairs — 2026-09-30

## Scope and reference

Continue from `ba0b749ae816db8df3d2340a6ee2addbdff09d2c` on
`refactor/mermaid-12-alignment`, with trusted merge base
`2d70832e25497aae282de9da78d1d6db12f2b475`.
The selected Mermaid 12.0.0 source remains
`98a0945418c76238f15df2afaddbba4272656c3b` and the selection receipt SHA-256 remains
`5a77f613f2d1f380ddaa5d9bab5e8111adb2cd833f319462a916694bebf10e28`.
No dependency, feature, reference graph, comparator tolerance, or residual receipt changes.
No feature split is needed: these changes correct existing family and host behavior.
Evidence is local under `target/branch-review-mermaid12-round5/`.

## Parser ownership and ECMAScript semantics

State and ER now evaluate the rules that precede whitespace at the original lexer
cursor. State INITIAL and struct modes retain different rule orders, and the
State header consumes its upstream whitespace suffix. Whitespace/comment retries
are iterative. Accessibility prefixes move the cursor only after a complete
keyword/separator match; a bare `accTitle` can remain a name, and text between
`accDescr` and a later colon cannot accidentally become a declaration.

The shared ECMAScript whitespace predicate now governs the affected Flowchart
interaction fields and their UTF-8 source spans, Usecase stereotype validation and
trimming, and accessibility normalization. U+FEFF is whitespace; U+0085 is not.
Quoted payloads retain their authored content. ER's post-parse accessibility
normalization uses the same predicate as its lexer, avoiding a second lossy trim.
The existing State unclosed-description EOF behavior is aligned with the pinned
Jison token sequence; ER continues to reject unclosed descriptions.

Tests exercise original review counterexamples, all ECMAScript whitespace code
points in interaction parsing, excluded Unicode characters, declaration/reference
ranges, accessibility fields, and nested State rule priority.

## Agentflow configuration and presentation

The existing Agentflow-to-Flowchart adapter now explicitly projects the settings
owned by Agentflow's renderer: `diagramPadding`, `useMaxWidth`, `titleTopMargin`,
and Dagre spacing. Absent/null runtime fallbacks remain distinct from Engine-merged
schema defaults. Shared shape settings keep their existing ownership.

Spacing uses the resolved registered backend. Dagre observes root overrides before
family values with JavaScript truthiness; ELK retains its adapter defaults and
ignores these spacing settings in all three namespaces. An independent review
caught an initial unconditional projection that would have changed ELK spacing;
that version was corrected before admission. Tests cover both backends, site and
frontmatter input, unrelated Flowchart settings, zero/false/null, and registration
fallbacks. Agentflow title elements and their stylesheet use `agentflowTitleText`.

## Playground host initialization

Both engines now construct their operation from one configuration helper. Host
initialization is a deliberately narrow object: a validated built-in global theme
and a font from the UI's fixed choices. All other user configuration remains in
the source directive, where each engine's existing sanitizer and source policy
apply. Merman retains its strict defaults and hardened keys; only the isolated
Mermaid reference realm applies its own existing host defaults.

This preserves the initialized palette for `theme: "null"` without promoting
`secure`, `securityLevel`, arbitrary CSS, custom resource limits, or nested user
objects into host authority. Raw `themeVariables` remain source configuration.
A user's `secure` array cannot turn a source setting into a protected host setting.
The behavior is documented in the Playground Compare design notes.

Detection, parsing, layout, SVG planning/rendering, ASCII, and benchmark paths
receive the same safe initialization. The canvas projection follows the same
initialization/source distinction. Independent security review caught and rejected
an initial full-config pass-through; the final allowlist uses no duplicated Rust
security-key list or JavaScript CSS parser.

## Validation

Completed native and static checks:

- 1,707 core tests passed: all library tests, the complete semantic snapshot corpus,
  editor semantic facts, nesting limits, and public Usecase contracts.
- 1,477 renderer tests passed: all library tests plus Agentflow and Usecase SVG
  integration tests, including Dagre/ELK namespace isolation.
- After a final interaction-span fix, ten focused Flowchart/ER tests passed. The
  source mapper remains unchanged: the producer now advances by a whole UTF-8
  separator character instead of manufacturing an interior-byte span.
- Clippy passed for core, renderer, and WASM, all features and targets, with warnings
  denied. Workspace formatting passed. No LALRPOP grammar changed.
- 112 Playground unit/integration tests, application and browser TypeScript checks,
  scoped ESLint, and whitespace checks passed after the safe initialization change.

Test counts overlap; they are not an additive unique-coverage total.
Fresh full WASM was rebuilt (transaction `76a16c141fc0`). Nine public WASM
counterexamples passed, covering State rule priority, Flowchart Unicode interaction
separators, ER accessibility, Usecase stereotype trimming, and Agentflow viewport
ownership.

The release-profile affected-family SVG comparison passed for Agentflow, Usecase,
ER, Flowchart, and State in `structure`, `parity`, and `parity-root` modes at three
decimal places. The existing exact browser-text-layout receipts admitted 145
Flowchart and 22 State residual comparisons. No receipt or comparator was changed.
A debug-profile Flowchart comparison differed by one final floating-point digit in
a stroke-dasharray; rebuilding and comparing the release profile matched the
existing receipt.

The complete Playground build rebuilt and verified the opaque Mermaid realm as
well as the main application. All nine Chromium tests passed on the final source:
eight theme cases assert both engines' exact expected computed fill, and one
Merman security case checks that config/frontmatter/directives cannot grant host
security, stylesheet, or resource authority. Automatic, explicit default, null,
invalid, scoped, and authored-secure cases remain distinct.

An earlier `build:prepared` run used a stale opaque realm and showed two apparent
secure-policy differences. A temporary one-sided color assertion was rejected;
only the complete-build results with strict two-sided assertions count as evidence.
The final helper delegates source sanitization to the engines without a redundant
special-case `secure` filter. Final browser evidence:
`browser-strict-final-after-sanitizer.log` (9 passed). Final front-end checks are in
`final-unit-after-sanitizer.log` and `final-eslint-after-sanitizer.log`; the complete
build exited successfully in terminal session `93786`.

## Repair commits

- `f19b32db9`: State/ER lexer priority and accessibility semantics.
- `01e694a6b`: ECMAScript whitespace and valid UTF-8 interaction spans.
- `12e63103f`: Agentflow configuration ownership and backend spacing.
- `969ce9b3f`: Playground theme initialization and source/host trust boundary.

All confirmed round-five findings and the adjacent ER issue are repaired. Existing
unrelated working-tree entries and upstream staging files remain untouched. No push
was performed.

## Remaining boundary

The known deterministic Flowchart title 029 browser-root blocker remains unchanged.
It is a font-estimation boundary, not one of the newly repaired parser/configuration
counterexamples. No full browser-root admission is claimed.
