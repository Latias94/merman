---
type: Work Progress
status: active
---

# Log

## 2026-06-27
- Extended parser-backed completion beyond payload-only contexts. ER editor facts now emit
  `IdList` expected syntax for parser-recognized identifier lists, `merman-analysis` projects that
  into node-id completion, and LSP completion now prefers those ids on ER `classDef` lines instead
  of directive fallback.
- Extended the same contract to class diagrams. `class`/`classDef`-style name positions now emit
  parser-backed `NodeIdentifier` expected syntax, and `classDef` symbols were promoted from
  outline-only to completion-visible entity symbols so name positions can actually surface as
  completion candidates.
- Verified the slice with `cargo test -p merman-core --lib
  parse_er_editor_facts_record_expected_id_list_spans -- --nocapture`, `cargo test -p merman-core
  --lib -j1 parse_class_editor_facts -- --nocapture`, `cargo test -p merman-analysis --lib -j1
  cursor_context_uses_parser_expected -- --nocapture`, `cargo test -p merman-lsp --test
  completion -j1 completion_uses_er_parser_expected_id_list_context_for_class_def -- --nocapture`,
  `cargo test -p merman-lsp --test completion -j1
  completion_uses_class_parser_expected_node_identifier_context_for_class_def -- --nocapture`,
  and `cargo fmt --all --check`.

## 2026-06-27
- Added the first recovery-state parser expected syntax for flowchart incomplete edge tails.
  `merman-core` now records `NodeIdentifier` after `A-->`-style operator tails, `merman-analysis`
  projects that into completion, and `merman-lsp` prefers node-id candidates over operator suffix
  completions at the cursor.
- Added a direct analysis regression proving parser expected syntax overrides generic completion
  for the new `NodeIdentifier` path.
- Verified the slice with `cargo nextest run -p merman-analysis
  cursor_context_uses_parser_expected_node_identifier_to_override_generic_completion`,
  `cargo test -p merman-core --lib parse_flowchart_editor_facts_recovers_from_incomplete_input --
  --nocapture`, `cargo nextest run -p merman-core
  parse_flowchart_editor_facts_recovers_from_incomplete_input`, `cargo test -p merman-lsp --test
  completion completion_uses_flowchart_parser_identifier_context_after_operator -- --nocapture`,
  `cargo nextest run -p merman-lsp --test completion`, `cargo check -p merman-core -p
  merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-27
- Extended parser-fed expected syntax again, this time for flowchart recovery after an incomplete
  edge operator. `merman-core` now records `NodeIdentifier` as the expected syntax kind at
  `A-->`-style tails, and `merman-analysis` projects that into completion so the LSP prefers
  known node ids instead of operator suffix completions.
- Verified the slice with `cargo test -p merman-core --lib
  parse_flowchart_editor_facts_recovers_from_incomplete_input -- --nocapture`,
  `cargo nextest run -p merman-core parse_flowchart_editor_facts_recovers_from_incomplete_input`,
  `cargo test -p merman-lsp --test completion
  completion_uses_flowchart_parser_identifier_context_after_operator -- --nocapture`,
  `cargo nextest run -p merman-lsp --test completion`, `cargo check -p merman-core -p
  merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-27
- Extended parser-fed payload expected syntax to `flowchart` labels and directive payloads through
  the shared `push_flowchart_payload_symbol` path. This covers node labels, edge labels, style
  payloads, and classDef style payloads without adding LSP-local parsing.
- Verified that flowchart payload positions now suppress generic completion fallback through the
  same expected-syntax projection used by `sequence` and `gantt`.
- Verified the slice with `cargo test -p merman-core --lib parse_flowchart_editor_facts_emit_label_payload_spans -- --nocapture`,
  `cargo test -p merman-lsp --test completion completion_uses_flowchart_parser_payload_context -- --nocapture`,
  `cargo test -p merman-lsp --test completion -- --nocapture`,
  `cargo check -p merman-core -p merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-27
- Extended parser-fed payload expected syntax to the line-oriented `gantt` family. Existing
  `SpannedText` payload helper paths now mark title/config/accessibility/click payload value
  spans as parser-controlled completion contexts without changing Gantt parsing semantics.
- Verified that LSP completion now suppresses generic identifier/header fallback inside Gantt
  payload values through the same core facts -> analysis cursor context -> LSP projection chain
  used by `sequence`.
- Verified the slice with `cargo test -p merman-core --lib gantt_editor_facts_preserve_parser_symbol_spans -- --nocapture`,
  `cargo test -p merman-lsp --test completion completion_uses_gantt_parser_payload_context -- --nocapture`,
  `cargo test -p merman-lsp --test completion -- --nocapture`,
  `cargo check -p merman-core -p merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-27
- Added the first parser-fed expected syntax slice. `merman-core::EditorSemanticFacts` now carries
  `expected_syntax`, `sequence` parser facts emit payload spans for messages, notes, and
  interaction payloads, and `merman-analysis::FenceCursorContext` projects those spans into
  completion context.
- `merman-lsp` now treats parser-controlled payload positions as empty completion contexts instead
  of falling back to generic headers or identifier completion. This proves the intended route:
  parser state -> analysis cursor context -> thin LSP projection.
- Verified the slice with `cargo test -p merman-core --lib parse_sequence_editor_facts_preserve_actor_and_box_spans -- --nocapture`,
  `cargo test -p merman-analysis --lib cursor_context_uses_parser_expected_payload_to_suppress_generic_completion -- --nocapture`,
  `cargo test -p merman-lsp --test completion completion_uses_sequence_parser_payload_context -- --nocapture`,
  `cargo check -p merman-core -p merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-27
- Added the first analysis-owned completion cursor contract. `merman-analysis::FenceTextIndex`
  now exposes `FenceCursorContext` plus `FenceCursorCompletionKind`, covering prefix spans,
  directive/comment classification, and expected completion categories.
- Slimmed `merman-lsp::CompletionContext` into a protocol adapter over that analysis contract.
  It still owns LSP range/text-edit projection, while line-prefix grammar checks have moved out
  of the LSP layer.
- Verified the slice with `cargo test -p merman-analysis --lib cursor_context -- --nocapture`,
  `cargo test -p merman-lsp --lib context::tests::context_classifies_header_operator_and_directive_prefixes -- --nocapture`,
  `cargo test -p merman-lsp --test completion -- --nocapture`,
  `cargo check -p merman-analysis -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-26
- Audited the parser/LSP maturity seam after the shared scanner extraction. The important next
  gap is parser/analysis-owned cursor context for completion, not more transport-local
  line-prefix checks in `merman-lsp`.
- Updated the mature LSP roadmap to make cursor context an explicit analysis contract and to
  clarify parser-generator fit by syntax shape: LALRPOP is a strong candidate for statement/block
  grammars, while indentation-tree and text-heavy families should first expose explicit line event
  streams.
- Corrected a helper-extraction regression in `treemap`: its indentation remains space/Tab-only
  via `split_ascii_indent`, while `mindmap` and `kanban` keep the broader whitespace behavior.
  Added targeted regressions for the scan helper and treemap NBSP hierarchy behavior.

## 2026-06-26
- Closed the first `mindmap` recovery gap in the parser-backed editor facts seam. Later invalid node lines no longer clear earlier symbols in recovery mode, so LSP/editor consumers keep prior outline/completion material even when a later line is malformed.
- Verified that `gantt` already preserves its recovery facts on the current slice; the next refactor pressure point should be a shared linear fact-extraction seam across `mindmap` and `gantt`, not another one-off recovery patch.

## 2026-06-26
- Extracted shared low-level scan helpers into `crates/merman-core/src/diagrams/scan.rs` and pointed both `mindmap` and `gantt` at the same line-ending, case-insensitive prefix, and indent utilities.
- Kept the refactor intentionally narrow: it does not merge family semantics, only the reusable scanner layer that sits under the parser-backed facts seam.
- Re-verified the slice with `cargo test -p merman-core mindmap_editor_facts -- --nocapture`, `cargo test -p merman-core gantt_editor_facts -- --nocapture`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Clarified the maturity roadmap so parser-generator choice is evaluated per family rather than by blanket preference. `mindmap` and `gantt` are the first recommended pressure points for fearless refactors; the rest of the families should follow in capability-matrix order once parser-backed facts and recovery quality are convincing.
- Recorded the design rule that the LSP surface should keep pressuring core correctness and performance. When a family shows grammar ambiguity, recovery pain, or span drift, a local parser rewrite is on the table if it is the best fit for that family.

## 2026-06-26
- Added a preferred quickfix for `merman.compatibility.config.deprecated_flowchart_html_labels`. The fix promotes deprecated `flowchart.htmlLabels` into root-level `htmlLabels` and reuses the shared config/frontmatter rewrite path so the analysis, CLI, and LSP surfaces stay aligned.
- Kept `merman.compatibility.config.deprecated_external_diagram_loading` fixless; it remains an API/config migration rather than a local source rewrite.
- Re-verified the slice with `cargo test -p merman-analysis --test analyzer deprecated_flowchart_html_labels_config_is_core_warning -- --exact --nocapture`, `cargo test -p merman-lsp --lib code_actions::tests::deprecated_flowchart_html_labels_fix_produces_quickfix_action -- --exact --nocapture`, `cargo test -p merman-lsp --test server_smoke lsp_service_smoke_reports_deprecated_flowchart_html_labels_with_quickfix -- --exact --nocapture`, `cargo test -p merman-lsp --lib protocol::tests::rule_catalog_response_contains_governed_authoring_rule -- --exact --nocapture`, `cargo test -p merman-cli --test cli_compat cli_lint_rules_lists_rule_catalog_json`, `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Promoted `merman.authoring.config.prefer_frontmatter_config` from a hint into a preferred migration fix. The analysis layer now rewrites init/initialize directive config into YAML frontmatter using the merged Mermaid config as the source of truth, preserves existing frontmatter fields, and removes the original directive spans.
- Reused the shared init-directive scanner to expose full directive spans plus keyword spans so config migration, lint, code actions, and future parser-backed editor features can share the same directive locator.
- Re-verified the slice with focused analysis, analyzer, LSP code action, and CLI lint-rules/disable/severity tests, plus `cargo fmt --all --check` and `git diff --check`.

## 2026-06-26
- Upgraded `merman.authoring.config.prefer_frontmatter_config` from a fixless authoring hint into
  a fix-backed migration rule. The new analysis rewrite helper uses the parsed Mermaid config as
  the source of truth, preserves existing frontmatter fields, rewrites the merged config under
  YAML `config`, and removes init/initialize directive source spans through `DiagnosticFix`
  metadata that LSP code actions can project.
- Extended `source_directives` with reusable full init-directive spans so future config lint,
  rewrite, completion, and hover work can share the same directive locator instead of rescanning
  strings per rule.

## 2026-06-26
- Narrowed the authoring hint overlap in `merman-analysis`: `merman.authoring.config.prefer_frontmatter_config` now matches both `init` and `initialize` directive aliases, and the alias-reminder hint no longer stacks on top of the frontmatter-config hint for the same source.
- Re-verified the slice with focused `merman-analysis` analysis and analyzer tests plus `cargo fmt --all --check` and `git diff --check`.

## 2026-06-26
- Deepened the mature Mermaid LSP roadmap plan so the finish line is explicit: maturity now
  requires agreement between the capability matrix, public rule catalog/config schema, and the
  fixture/performance gates. U8 was tightened into the actual release gate, and the plan now
  requires any first-class partial capability to remain explicitly documented instead of being
  hidden behind a generic completion label.
- Synced the engineering memory with the same maturity bar so future sessions can resume against a
  shared definition of "done" instead of rediscovering it from chat history.

## 2026-06-26
- Added public coverage for `merman.authoring.config.prefer_frontmatter_config` across analysis,
  analyzer, CLI lint-rules JSON, LSP rule catalog/config schema, and repo docs. The new
  recommended-profile hint steers authors from `%%{init}%%` directives toward diagram frontmatter
  `config` using Mermaid's own directives/configuration docs as evidence.
- Re-verified the full slice with `cargo test -p merman-analysis --lib`,
  `cargo test -p merman-analysis --test analyzer`, `cargo test -p merman-cli --test cli_compat
  cli_lint_rules_lists_rule_catalog_json`, `cargo test -p merman-lsp --lib protocol::tests`,
  `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-26
- Added a new recommended-profile authoring hint,
  `merman.authoring.config.prefer_frontmatter_config`, to steer diagram authors from `%%{init}%%`
  directives toward diagram frontmatter `config` based on Mermaid's `docs/config/directives.md`
  and `docs/config/configuration.md` guidance.
- Kept the hint source-backed and fixless, added analysis/analyzer regressions for the new rule,
  and synchronized the public lint rule catalog plus LSP docs so the exported rule set stays
  discoverable through `merman/ruleCatalog` and `merman/configSchema`.
- Re-verified the slice with `cargo test -p merman-analysis --lib`, `cargo test -p
  merman-analysis --test analyzer`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Tightened the core compatibility lint governance boundary for
  `merman.compatibility.config.deprecated_flowchart_html_labels`: upstream explicitly deprecates
  `flowchart.htmlLabels`, but does not mark `class.htmlLabels` as deprecated in the same schema or
  config warning path, so the rule no longer reports `class.htmlLabels`.
- Removed the over-broad class-diagram regression and restored the rule description/docs to the
  source-backed `flowchart.htmlLabels` contract. This preserves the principle that core lint
  standards are Mermaid-backed, not Merman-invented.
- Re-verified the corrected slice with `cargo test -p merman-analysis --lib html_labels`,
  `cargo test -p merman-analysis --test analyzer`, `cargo test -p merman-lsp --lib
  protocol::tests`, `cargo test -p merman-cli --test cli_compat cli_lint_rules_lists_rule_catalog_json`,
  `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-26
- Broadened the Mermaid-backed core compatibility lint
  `merman.compatibility.config.deprecated_flowchart_html_labels` so it now covers both
  `flowchart.htmlLabels` and `class.htmlLabels` directive/config shapes through the shared
  `source_directives` matcher, keeping the rule id stable while expanding the upstream-backed
  coverage.
- Added analyzer and source-lint regressions for `classDiagram` init directives so the shared
  deprecated-`htmlLabels` contract stays span-precise and continues to surface as a core warning
  without quickfixes.
- Re-verified the slice with `cargo test -p merman-analysis --lib`, `cargo test -p
  merman-analysis --lib --test analyzer`, `cargo test -p merman-lsp --lib protocol::tests`,
  `cargo test -p merman-cli --test cli_compat cli_lint_rules_lists_rule_catalog_json`, `cargo
  check -p merman-analysis -p merman-cli -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-26
- Added a second Mermaid-backed core compatibility config lint:
  `merman.compatibility.config.deprecated_external_diagram_loading`. It reports deprecated
  `lazyLoadedDiagrams` and `loadExternalDiagramsAtStartup` directive config and recommends the
  `registerExternalDiagrams` runtime API instead of encoding a Merman-specific standard.
- Kept the rule source-backed and span-precise by reusing `merman-analysis::source_directives`
  for directive key-path matching, while leaving the diagnostic intentionally fixless because the
  replacement is an API migration rather than a safe local rewrite.
- Re-verified the new rule and its surface area with `cargo test -p merman-analysis --lib`,
  `cargo test -p merman-analysis --lib --test analyzer`, `cargo test -p merman-lsp --lib
  protocol::tests`, `cargo test -p merman-cli --test cli_compat cli_lint_rules_lists_rule_catalog_json`,
  `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, `cargo fmt --all --check`, and
  `git diff --check`.

## 2026-06-26
- Extracted the directive/config source scanner from `merman-analysis::rules` into the internal
  `source_directives` module. Rules now ask for init-directive config key paths such as
  `flowchart.htmlLabels` / `config.flowchart.htmlLabels` instead of embedding path-specific
  scanners.
- Kept the deprecated `flowchart.htmlLabels` diagnostic behavior unchanged while moving the
  scanner's unterminated-directive, quoted `}%%`, config-wrapper, root-key, and non-init-directive
  coverage beside the reusable module.
- Re-verified the refactor with `cargo test -p merman-analysis --lib`, `cargo check -p
  merman-analysis`, `cargo check -p merman-lsp`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Added the Mermaid-backed core compatibility lint rule
  `merman.compatibility.config.deprecated_flowchart_html_labels`. It reports directive usage of
  deprecated `flowchart.htmlLabels` with evidence from Mermaid `config.ts`, `config.type.ts`, and
  `docs/config/directives.md`, while intentionally carrying no quickfix until config rewrite spans
  and formatting are structurally safe.
- Added a directive object scanner for this lint path so the diagnostic range points at the
  offending `htmlLabels` key, including `init.config.flowchart.htmlLabels`, without changing the
  Mermaid parser or render behavior. The existing directive wrapper scan now ignores `}%%` inside
  quoted strings.
- Added analysis and LSP regressions proving the core warning is emitted, can be disabled, is
  discoverable through the shared rule catalog/config schema, and does not produce a code action
  when no `DiagnosticFix` metadata is present.
- Corrected the public evidence contract after review: rule catalog entries and LSP documentation
  now cite pinned Mermaid GitHub commit URLs instead of local upstream checkout paths. Local
  upstream checkouts remain development-only evidence mirrors.

## 2026-06-26
- Added standard pull diagnostics to `merman-lsp`: `textDocument/diagnostic` now returns full or
  unchanged reports from the shared analysis payloads, `workspace/diagnostic` returns document
  reports for tracked snapshots, and `workspace/diagnostic/refresh` is requested when supported
  after configuration changes.
- The server now advertises `diagnostic_provider` with pull support, while still keeping the
  existing push diagnostics path for open/save/configuration updates.
- Re-verified the diagnostics slice with focused pull-request smoke tests, `cargo fmt --all
  --check`, and `cargo check -p merman-lsp`.

## 2026-06-26
- Added completion resolve support to `merman-lsp`: completion items now carry stable resolve
  metadata, `completionItem/resolve` adds Markdown documentation, and resolve keeps `insertText`
  and `textEdit` stable.
- The completion provider now advertises `resolve_provider: true`, and protocol tests plus a real
  `tower_lsp` service smoke test prove the resolved item preserves edit behavior while exposing
  the extra documentation payload.
- Re-verified the completion slice with `cargo nextest run -p merman-lsp --no-fail-fast
  --status-level=all --final-status-level=all`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Added the second editor-agnostic Merman LSP extension request, `merman/configSchema`, and
  advertised it beside `merman/ruleCatalog` under
  `ServerCapabilities.experimental.merman.requests`.
- The config schema response exposes accepted settings roots, lint profiles, diagnostic
  severities, configurable rule-id enums from the shared analysis catalog, and a JSON Schema-style
  description of the active `lint`, `parse.suppress_errors`, `resources.max_source_bytes`,
  `site_config`, `fixed_today`, and `fixed_local_offset_minutes` options.
- Documented the request in `docs/lsp/EXTENSION_PROTOCOL.md` and synchronized
  `docs/lsp/README.md` plus `docs/lsp/CAPABILITIES.md` so plugin authors can build settings UI,
  completion, and validation hints without Merman owning editor-specific UI.
- Verified the config schema slice with `cargo nextest run -p merman-lsp --no-fail-fast
  --status-level=all --final-status-level=all`, `cargo fmt --all --check`, and `git diff --check`.

## 2026-06-26
- Added the first editor-agnostic Merman LSP extension request, `merman/ruleCatalog`, and
  advertised it under `ServerCapabilities.experimental.merman.requests` so plugins can discover
  rule metadata without depending on a Merman-owned UI package.
- Added `crates/merman-lsp/src/protocol.rs` as the home for custom LSP protocol constants and
  response schemas, documented the extension in `docs/lsp/EXTENSION_PROTOCOL.md`, and updated all
  server smoke tests to use the real `MermanLanguageServer::service()` builder that registers
  custom methods.
- Verified the LSP protocol slice with `cargo fmt --all --check`, `cargo test -p merman-lsp --lib
  --tests`, and `git diff --check`.

## 2026-06-26
- Productized the governed lint rule catalog: `RuleDescriptor` now carries machine-readable
  `evidence` references, `RuleCatalogEntry` serializes id/description/evidence/severity/category/
  profile/origin/configurability/fixability, and the catalog is exposed through Rust, CLI
  `lint-rules`, binding-core, C FFI, Android JNI, UniFFI, WASM, Web TS, Flutter/Dart, and Apple
  Swift surfaces.
- Rebuilt the Web WASM package with the local proxy at `http://127.0.0.1:10809` so wasm-opt could
  download Binaryen, and rebuilt the local macOS Apple XCFramework to verify the Swift wrapper
  against the current FFI header.
- Fixed a binding-core ASCII option-path defect found while testing: shared `parse` options now
  live under `options.analysis.parse`, and the regression checks the stored option shape instead
  of invoking the currently hanging ASCII render path.
- Verified the catalog/export slice with `cargo fmt --all`, focused analysis/binding/CLI/FFI/UniFFI
  Rust tests, UniFFI bindgen nextest smoke, `cargo check -p merman-wasm`, Web WASM/TS build and
  smoke, Flutter analyze, Dart FFI smoke, Apple Swift smoke, wiki-memory validation, and
  `git diff --check`.

## 2026-06-26
- Added ADR 0072 for lint rule governance: Merman rule IDs remain under `merman.*`, authoring
  recommendations use `merman.authoring.*`, and Mermaid syntax/compatibility origins require
  pinned source, docs, fixture, or reproducible compatibility evidence.
- Extended `merman-analysis` rule descriptors with `origin` and `default_profile`, added
  `core`/`recommended`/`strict` lint profiles, and added explicit `enable_rules` support across
  `options_json` and CLI lint.
- Renamed the authoring rule IDs to `merman.authoring.config.prefer_init_directive` and
  `merman.authoring.flowchart.explicit_direction`. These rules now default to the `recommended`
  profile as hints; the default `core` profile no longer emits Merman authoring recommendations.
- Verified the governance slice with `cargo fmt --all`, `cargo fmt --all --check`, `cargo test -p
  merman-analysis --lib --tests`, `cargo test -p merman-core flowchart --lib`, `cargo test -p
  merman-cli cli_lint --test cli_compat`, `cargo test -p merman-lsp code_actions --lib`,
  `cargo test -p merman-lsp --lib --tests`, wiki-memory validation, and `git diff --check`.

## 2026-06-26
- Extended `DiagramWarningFact` with optional parser-backed `span` and `fixSpan`, and taught the
  flowchart grammar to retain the header span so missing-direction warnings no longer depend on
  whole-document fallback ranges.
- Promoted `merman.flowchart.missing_direction` to a fix-backed shared rule: `merman-analysis`
  now maps the fact span into the diagnostic range and maps the fix span into a preferred
  `DiagnosticFix` that inserts ` TB` at the flowchart header.
- Added core, analysis, and LSP regressions proving the warning fact JSON shape, analysis fix
  metadata, ordinary Mermaid quickfix action, and Markdown-fence host-document edit range.
- Split flowchart source direction from effective render direction: semantic JSON now preserves an
  omitted header direction as `null`, while the typed render model still receives the effective
  `TB` default for layout/render consumers.
- Updated core snapshot normalization so Mermaid parity goldens continue comparing upstream
  `warnings` output while production models can carry merman-specific `warningFacts` metadata for
  analysis and LSP.
- Verified the slice with `cargo fmt --all`, `cargo nextest run -p merman-core --no-fail-fast`,
  `cargo nextest run -p merman-analysis --no-fail-fast`, and
  `cargo test -p merman-lsp --lib --tests`. A combined
  `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast` run was interrupted after
  hanging in nextest test-binary listing; the package-level replacement checks above passed.

## 2026-06-25
- Added a parser-backed flowchart lint rule for missing header direction, with the core flowchart
  model now defaulting omitted headers to `TB` while still emitting `warningFacts` for the shared
  analysis contract.
- Registered `merman.flowchart.missing_direction` in the shared rule descriptor table and proved
  it can be disabled through `AnalysisRuleConfig` while remaining visible through analysis, CLI,
  and LSP consumers.
- Added core, analysis, CLI, and LSP regressions covering the new flowchart warning fact, the rule
  mapping, and the default-direction model behavior.
- Re-verified the slice with `cargo fmt --all`, `cargo nextest run -p merman-core
  parse_diagram_flowchart_without_direction_defaults_to_tb_and_warns
  parse_flowchart_render_model_carries_missing_direction_warning_fact
  parse_diagram_flowchart_keyword_flowchart --no-fail-fast`, `cargo nextest run -p
  merman-analysis flowchart_missing_direction
  semantic_warning_facts_map_flowchart_missing_direction_rule_id
  rule_descriptors_expose_stable_rule_metadata --no-fail-fast`, `cargo nextest run -p
  merman-analysis -p merman-cli --no-fail-fast`, and `cargo nextest run -p merman-lsp --no-fail-fast`.

## 2026-06-25
- Tightened `merman-lsp` completion handling so directive-oriented lines like `classDef` and
  `click` no longer fall back to the generic `flowchart TD` header prompt when no other items are
  available.
- Added regressions proving directive lines stay on directive completions, do not offer node ids,
  and do not regress to the new-diagram fallback prompt.
- Re-verified the slice with `cargo fmt --all --check` and
  `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`.

## 2026-06-25
- Tightened the text-scan fallback in `merman-analysis::editor` so `init`, `initialize`, and
  `wrap` directive lines no longer leak into node IDs or outline items when parser-backed facts
  are unavailable.
- Added a regression proving those directive prefixes stay out of fallback node-id projection while
  ordinary diagram content still projects normally.
- Re-verified the slice with `cargo fmt --all --check` and
  `cargo nextest run -p merman-analysis --no-fail-fast`.

## 2026-06-25
- Tightened the shared lint/config rule-id contract so `options_json` and CLI lint now reject
  unknown or internal rule ids instead of silently accepting them, and the analysis crate now
  exposes a public configurable-rule registry view for future completion surfaces.
- Added regressions in `merman-analysis`, `merman-bindings-core`, and `merman-cli` proving the
  shared configuration surface still accepts valid public rule ids while rejecting unknown and
  internal ones.
- Re-verified the slice with `cargo fmt --all --check` and
  `cargo nextest run -p merman-analysis -p merman-bindings-core -p merman-cli`.

## 2026-06-25
- Removed the silent fallback from `merman-analysis` rule lookup. Rule descriptors are now
  resolved through an explicit registry, and unknown analysis rule ids surface as internal
  contract gaps instead of collapsing into the generic semantic warning bucket.
- Added a regression proving unknown semantic warning fact ids now emit an explicit internal
  diagnostic, while known block and gitGraph warning facts still map to their stable rule ids.
- Added a regression proving analyzer-level rule lookup gaps surface as internal errors, so future
  missing rule registrations fail loudly during lint/LSP development instead of degrading silently.
- Re-verified the slice with `cargo fmt --all --check` and `cargo test -p merman-analysis --lib -- --nocapture`.

## 2026-06-25
- Added semantic-token delta support to `merman-lsp`, including cached previous token state,
  full/delta capability advertisement, and a smoke test that exercises `textDocument/semanticTokens/full`
  followed by `textDocument/semanticTokens/full/delta`.
- Updated the LSP docs and engineering memory so semantic tokens are now described as full/range/delta
  rather than deferred delta work.
- Wired `workspace/semanticTokens/refresh` into `merman-lsp` configuration changes when the client
  advertises refresh support, so semantic-token consumers now get an explicit requery signal after
  analyzer settings change.
- Added regression coverage for the semantic-token refresh handshake, including the client
  capability probe and the request/response roundtrip on configuration change.
- Added LSP smoke regressions proving the shared rule config contract also handles source-byte-limit severity
  overrides on both initialize and didChangeConfiguration, so the server surface now matches the
  CLI/library behavior for `merman.resource.source_bytes_exceeded`.
- Added LSP smoke regressions proving the shared rule config contract also handles severity
  overrides on both initialize and didChangeConfiguration, so the server surface now matches the
  CLI/library behavior for `merman.parse.no_diagram`.
- Added an LSP smoke regression proving the shared rule config contract also survives server
  initialization, so `merman.parse.no_diagram` and source-byte-limit diagnostics can be disabled
  through `initialize` payloads as well as CLI/library entry points.
- Added CLI regressions proving the shared rule config contract also reaches `merman-cli`, so
  `no_diagram` and source-byte-limit diagnostics can be disabled from the command surface as well
  as from the library API.
- Expanded the analysis rule contract so core parse/resource/compatibility/internal diagnostics
  now have stable descriptors, and `no_diagram` plus source-byte-limit diagnostics can be disabled
  through the shared rule config without breaking the rest of the analysis pipeline.
- Added `workspace/symbol` to the published LSP contract docs and engineering state so the
  snapshot-backed workspace lookup path is no longer described as deferred.
- Added a workspace/symbol LSP handler that reuses the existing outline projection from tracked
  document snapshots, so product-wide symbol lookup now works without introducing a new parser
  path.
- Advertised `workspace/symbol` in server capabilities and added focused regressions for the
  handler, capability flag, and workspace-symbol filtering over outline items.
- Re-verified the slice with `cargo fmt --all --check`, `cargo check -p merman-lsp`, and focused
  `cargo test -p merman-lsp` runs for workspace symbol filtering, capabilities, and the LSP smoke
  path.
- Removed the last message-based heuristic fallback from `merman-analysis` semantic warning
  projection. The analysis layer now derives warning diagnostics from `warningFacts.ruleId`
  directly, so warning routing no longer depends on diagram-specific message text.
- Tightened the semantic warning rule surface so only fact-backed rule IDs remain in the stable
  descriptor table; the generic "block warning" and "gitGraph warning" descriptors were removed.
- Re-verified the slice with `cargo fmt --all --check`, `cargo check -p merman-analysis`, and a
  focused `cargo test -p merman-analysis semantic_warning_facts_use_rule_ids_even_when_messages_differ -- --nocapture`.
- Removed the legacy string-based semantic warning projection for block and gitGraph. Those
  families now emit structured `warningFacts`, and `merman-analysis` projects warnings from the
  shared rule IDs instead of matching message text.
- Updated core and analysis regressions to assert the new `warningFacts` surface, while keeping the
  existing compatibility path out of the production contract.
- Re-verified with `cargo fmt --all --check`, `cargo check -p merman-core -p merman-analysis
  -p merman-render`, and focused `cargo test -p merman-core` / `cargo test -p merman-analysis`
  warning regressions.
- Centralized the analysis options JSON contract in `merman-analysis`, so bindings-core and
  `merman-lsp` now share the same lint/config parsing path for `initialize` and
  `workspace/didChangeConfiguration`.
- Added a namespaced-wrapper regression for the shared analysis options contract and kept the
  existing lint configuration coverage in both bindings-core and LSP smoke tests.
- Re-verified the slice with `cargo fmt --all --check`, `cargo check -p merman-analysis
  -p merman-lsp -p merman-bindings-core`, and the focused bindings/LSP tests for initialization
  and configuration replay.
- Added shared lint rule configuration to `BindingOptions`/`options_json`, including rule
  disablement and severity overrides, so FFI/UniFFI/WASM can drive the same analysis config as CLI.
- Added binding-core regressions proving `analyze_json` honors lint rule configuration and severity
  overrides through the shared JSON surface.
- Updated the bindings docs to describe the new `lint` section in `OPTIONS_JSON.md` and to note
  that the shared contract spans FFI, UniFFI, WASM, and CLI.
- Re-verified with `cargo fmt --all --check` and focused `cargo test -p merman-bindings-core`
  lint-config regressions; the broader package test sweep still hits the pre-existing
  `flowchart-elk` render regression.

## 2026-06-24
- Added protocol-independent `DiagnosticFix` and `DiagnosticFixEdit` metadata to
  `AnalysisDiagnostic`, keeping empty fixes out of the JSON payload so the existing ADR-0070 schema
  shape remains stable for diagnostics without safe edits.
- Preserved fix metadata in LSP `Diagnostic.data` and added a `merman-lsp::code_actions` provider
  that returns quickfix actions only for diagnostics carrying explicit safe fixes.
- Advertised `textDocument/codeAction` quickfix capability, wired the server handler, and added
  regressions proving actions appear for fix-backed diagnostics and are absent without fix metadata
  or when non-quickfix actions are requested.
- Added the first fix-backed lint rule, `merman.config.prefer_init_directive`, which reports
  `%%{ initialize: ... }%%` directive aliases and offers a preferred source edit to replace
  `initialize` with canonical `init`.
- Remapped Markdown-fence fix edits back into host-document coordinates so quickfixes edit the
  source document rather than fence-local byte ranges.
- Added stable lint rule descriptors and a shared rule-config surface to `merman-analysis`, then
  wired CLI lint to disable rules or override severities through the same analysis config.
- Re-verified the code-action foundation with `cargo test -p merman-analysis -p merman-lsp --lib
  --tests`.
- Replaced the analysis index's name-only reference table with typed `FenceReferenceGroup`
  entries keyed by symbol name plus `EditorSymbolKind`.
- Routed LSP definition, references, prepare-rename, and rename through item-based typed reference
  group queries so same-name entities with different semantic kinds no longer collide.
- Added analysis and LSP regressions proving same-name different-kind entities stay in separate
  reference/rename groups.
- Re-verified the typed-reference slice with `cargo test -p merman-analysis -p merman-lsp --lib
  --tests`; `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast` currently stalls
  while listing the `merman-lsp` binary target, so cargo test is the authoritative package-level
  verification for this slice.
- Added the first U6 semantic-token provider: `merman-lsp` now advertises
  `textDocument/semanticTokens/full` and serves full-document tokens from parser-backed
  `FenceSemanticItem` records.
- Defined a stable semantic-token legend from `EditorSymbolKind` plus `mermanEntity`,
  `mermanOutline`, and `mermanPayload` role modifiers so syntax highlighting does not need
  LSP-local parsing or text heuristics.
- Added semantic-token regressions for entity/outline/payload roles, Markdown absolute UTF-16
  ranges, multiline payload splitting, initialize capability wiring, and the full-document handler.
- Re-verified the semantic-token slice with `cargo nextest run -p merman-analysis -p merman-lsp
  --no-fail-fast`; 82 tests passed.
- Started U3 from the mature LSP/lint roadmap by adding role-aware parser-backed semantic items to
  `merman-analysis::FenceTextIndex`; entity, outline, and payload facts are now retained after the
  existing completion/outline/reference projections are derived.
- Exported `FenceSemanticItem` and `FenceSemanticRole` so future lint, semantic-token, and
  code-action providers can consume parser-backed payload spans without LSP-local parsing.
- Added `FenceTextIndex::semantic_item_at_offset` and wired hover to prefer parser-backed semantic
  items before falling back to outline/fence hover, so payload spans can now produce hover content.
- Added `FenceTextIndex::entity_item_at_offset` and routed definition/references/prepareRename/
  rename through entity-only semantic queries so payload facts stay out of navigation targets.
- Strengthened sequence LSP regression coverage so payload facts must be retained as semantic
  payload items while staying out of completion IDs and outline items.
- Added a structure regression proving sequence title payload hover reports the parser-backed
  payload detail.
- Added a structure regression proving payload semantic items are not navigation targets.
- Focus-verified with `cargo nextest run -p merman-analysis editor::tests -p merman-lsp
  sequence_payload_facts_do_not_pollute_completion_ids --no-fail-fast`.
- Re-verified the current U2/U3 worktree with `cargo nextest run -p merman-core --no-fail-fast`,
  `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`, `cargo fmt --all --check`,
  `git diff --check`, and engineering wiki validation.
- Re-ran `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast` after wiring payload
  hover; 78 tests passed.
- Re-ran `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast` after wiring
  entity-only navigation queries; 79 tests passed.
- Deepened sequence editor facts so `title`, `accTitle`, `accDescr`, message text, note text, and
  `links`/`link`/`properties`/`details` interaction bodies are parser-backed payload-only spans,
  with `links`/`link`/`properties`/`details` directive prefix tracking.
- Fixed sequence payload selection so payloads named like their directive prefix, such as
  `title: title` or `accTitle: Title`, select the payload text rather than the directive keyword.
- Added core and LSP regressions proving sequence payload facts preserve exact spans while staying
  out of completion IDs and outline items.
- Re-verified with `cargo nextest run -p merman-core --no-fail-fast` and `cargo nextest run -p
  merman-analysis -p merman-lsp --no-fail-fast`.
- Continued U2 from the mature LSP/lint roadmap by shrinking `FenceTextIndex::from_text` so
  payload-only directive lines such as `click`, `linkStyle`, `accTitle`, `accDescr`, and `title`
  no longer project into node IDs or outline entries; only their directive prefixes are retained.
- Added a regression proving the text-scan fallback records those payload directive prefixes
  without leaking payload symbols into the node-id or outline surfaces.
- Re-verified the analysis and LSP suites with `cargo nextest run -p merman-analysis -p merman-lsp
  --no-fail-fast`, plus `cargo fmt --all`.
- Created `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md` as the umbrella
  plan for product-grade Mermaid LSP and lint maturity across semantic facts, lint rules, code
  actions, semantic tokens, configuration, packaging, and readiness gates.
- Updated current engineering memory so the active long-term goal now points at the mature LSP/lint
  roadmap, with U1 capability tracking as the next implementation slice.
- Added `progress/2026-06-24-mature-lsp-roadmap.md` to preserve the new roadmap state and external
  reference context for future sessions.
- Implemented the first U1 slice by adding `docs/lsp/README.md`, `docs/lsp/CAPABILITIES.md`, and a
  parser-backed family capability matrix test in `crates/merman-lsp/tests/capabilities.rs`.
- Verified U1 with `cargo nextest run -p merman-lsp --no-fail-fast` and `cargo fmt --all --check`.
- Deepened flowchart directive facts so `style`, `classDef`, and `class` statements now preserve
  parser-backed spans for style targets, class targets, class definitions, style strings, and class
  names with entity/outline/payload roles.
- Added regressions proving complete and recovered flowchart parses preserve those directive
  payload spans while keeping style/class values out of completion.
- Deepened flowchart editor facts so node labels and edge labels now carry span-rich payload
  facts through the lexer, LALRPOP AST, and recovered token stream; edge-label payloads are
  deduplicated across expanded chain edges to avoid repeated semantic occurrences.
- Added regressions proving complete and recovered flowchart parses preserve label payload spans
  and selections without polluting completion.
- Re-verified with `cargo nextest run -p merman-core parse_flowchart_editor_facts --no-fail-fast`,
  `cargo nextest run -p merman-core flowchart --no-fail-fast`,
  `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`, `cargo fmt --all --check`,
  and `git diff --check`.
- Added source-backed recovery diagnostics for hand-written/line-scanned editor facts: Gantt now
  reports invalid weekday/weekend values, unrecognized statements, missing-header recovery, and
  unterminated multiline `accDescr` blocks; Mindmap now reports unterminated node delimiters.
- Added analyzer regressions proving Gantt/Mindmap recovered editor diagnostics are projected as
  `merman.parse.recovered_editor_facts` warnings through the shared diagnostics payload.
- Re-verified with `cargo nextest run -p merman-core gantt mindmap --no-fail-fast`,
  `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`,
  `cargo nextest run -p merman-ffi -p merman-wasm -p merman-cli --no-fail-fast`,
  `cargo fmt --all --check`, and `git diff --check`.
- Added parser-backed recovery diagnostics to `EditorSemanticFacts`, so grammar-backed flowchart/sequence/state/class/ER editor parsers now attach LALRPOP error messages plus parser-token/EOF byte spans when they recover from partial input.
- Projected those recovery diagnostics through `merman-analysis` as `merman.parse.recovered_editor_facts` warnings, keeping the existing parse error and letting LSP/CLI/FFI/WASM consume the same payload seam.
- Fixed the Rust FFI smoke-test mirror to include the existing `analyze_json` field, and split the wasm `analyze_json` smoke test by target so native nextest uses the binding-core byte payload while wasm32 keeps the `JsValue` path.
- Verified with `cargo nextest run -p merman-core --no-fail-fast`, `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`, `cargo nextest run -p merman-ffi -p merman-wasm -p merman-cli --no-fail-fast`, `cargo test --workspace --no-run`, `cargo fmt --all --check`, and `git diff --check`; `cargo nextest run --workspace --no-fail-fast` was interrupted after test binary listing hung for several minutes, but package-level coverage passed and workspace compilation succeeded.
- Replaced the `stateDiagram` AST-plus-supplemental-token editor fact split with a single
  token-backed `StateEditorEvent` stream; complete parses now use the same entity/payload
  projection as recovered buffers, with the grammar parse result only selecting provenance.
- Deepened `mindmap` directive payload facts so `:::class` and `::icon(...)` decoration values
  now ride the shared parser event stream as payload-only spans without polluting node-id
  completion or outline.
- Deepened `gantt` multiline accessibility facts so `accDescr { ... }` blocks now produce
  payload-only parser facts with cross-line spans; unterminated blocks preserve recovered payload
  facts for partial editor buffers.
- Deepened `classDiagram` display-label and relation-multiplicity facts so quoted class labels and
  relation cardinality strings are payload-only parser facts without polluting completion or
  outline.
- Recorded the remaining class break as recovered diagnostics and broader parser event-stream/lint
  work, not more class-string heuristics.
- Deepened `stateDiagram` payload facts so state display labels, colon descriptions, relation
  labels, positioned/floating note text, class/style/click payloads, and accessibility text are all
  parser-backed payload-only spans.
- Recorded the remaining state architecture boundary after editor fact unification: render/model
  locality can adopt a similar event stream later if it proves higher value than recovered
  diagnostics.
- Deepened `classDiagram` residual payload facts so relation labels, `note` text,
  `accTitle:`/`accDescr:` values, and multiline class accessibility descriptions are retained as
  payload-only parser facts without leaking into LSP completion or outline.
- Recorded the remaining higher-return parser breaks after class payload deepening: state
  event-stream unification and recovered parser diagnostics.
- Deepened `classDiagram` editor facts so `classDef` ids now land as outline facts, `cssClass`
  quoted target lists stay entity references, inline `:::` and `cssClass` style class names are
  payload-only, and `style`/`classDef` raw style strings plus `click ... call ...` callback
  functions/args are preserved as payload spans.
- Re-verified the class slice with `cargo nextest run -p merman-core parse_class_editor_facts_preserve_parser_symbol_spans --no-fail-fast`,
  `cargo nextest run -p merman-lsp class_member_outline_facts_do_not_pollute_completion_ids --no-fail-fast`,
  `cargo nextest run -p merman-core class --no-fail-fast`,
  `cargo nextest run -p merman-core editor_facts --no-fail-fast`,
  `cargo nextest run -p merman-analysis --no-fail-fast`,
  `cargo nextest run -p merman-lsp --no-fail-fast`, and `cargo fmt --all --check`.
- Deepened `stateDiagram` editor facts so `classDef` ids become outline facts, `class`/`style`
  state targets remain entity references, and `class`/`style`/`click`/`accTitle`/`accDescr`
  payloads stay span-rich for future lint and semantic consumers without polluting LSP
  completion.
- Re-verified the state slice with `cargo nextest run -p merman-core parse_state_editor_facts_preserve_parser_state_spans --no-fail-fast`,
  `cargo nextest run -p merman-lsp state_documents_use_parser_facts --no-fail-fast`,
  `cargo nextest run -p merman-core state --no-fail-fast`,
  `cargo nextest run -p merman-core editor_facts --no-fail-fast`,
  `cargo nextest run -p merman-analysis --no-fail-fast`,
  `cargo nextest run -p merman-lsp --no-fail-fast`, and `cargo fmt --all --check`.
- Continued the LSP completion foundations follow-up on `feat/diagnostics-analysis-contract`.
- Verified the new `merman-lsp` crate compiles and its focused tests pass, including completion, diagnostics, and document-store coverage.
- Recorded durable engineering memory for the current plan so later sessions can resume without rereading the full chat.
- Extracted `merman-analysis::document::analyze_document` as the shared plain/markdown document-analysis seam, and switched CLI lint plus LSP publishing to it so fence scanning and diagnostic mapping stay centralized.
- Re-verified `cargo fmt --all --check`, `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, and `cargo test -p merman-analysis -p merman-cli -p merman-lsp --tests` after the shared seam landed.
- Added a `DocumentStore` regression proving newer versions replace older snapshots while keeping the latest fence metadata and diagram type.
- Re-verified `cargo fmt --all --check` and `cargo test -p merman-lsp --tests` after the versioned snapshot regression landed.
- Reframed the next LSP slice from pure completion polish toward a shared structure layer for hover/documentSymbol so the same snapshot seam can feed future symbol-oriented features.
- Added a first-pass `merman-lsp::structure` module to explore hover/documentSymbol on top of the existing snapshot seam, then tightened it after an initial compile check surfaced interface mismatches.
- Updated the durable engineering memory to say the current follow-up is the shared structure layer rather than only completion metadata.
- Extended the same fence-local structure layer beyond hover/documentSymbol to cover `textDocument/definition`, `textDocument/references`, `textDocument/prepareRename`, and `textDocument/rename` with shared snapshot-driven navigation facts.
- Re-verified `cargo fmt --all`, `cargo check -p merman-lsp`, and `cargo test -p merman-lsp --tests` after the navigation surface landed.
- Confirmed that the next slice should not force a repository-wide parser rewrite. The follow-up is now a new parser/semantic seam plan plus ADR, so later LSP and lint work can consume span-rich parser facts instead of raw-text heuristic scans.
- Centralized the current editor-facing fence structure into `merman-analysis::FenceTextIndex`, removed the separate LSP completion/navigation scan implementations, and re-verified `cargo fmt --all` plus `cargo test -p merman-analysis -p merman-lsp`.
- Added the initial `merman-core::EditorSemanticFacts` contract and migrated flowchart node/subgraph editor symbols to parser-backed original-text spans; `merman-lsp` now prefers those facts through `FenceTextIndex::from_core_facts` while preserving directive-prefix completion state.
- Re-verified `cargo fmt --all`, `cargo nextest run -p merman-core parse_flowchart_editor_facts`, `cargo nextest run -p merman-analysis editor::tests`, and `cargo nextest run -p merman-lsp` after the flowchart parser-backed editor facts landed.
- Added `EditorSemanticCompleteness` and `FenceTextIndexSource` provenance so recovered parser facts can be distinguished from complete parser facts and old text scans.
- Flowchart editor fact extraction now recovers symbols from the masked lexer token stream when LALRPOP parsing fails on incomplete buffers, and LSP has a regression proving incomplete flowcharts use `ParserRecovered` rather than `TextScan`.
- Migrated Sequence as the second family onto `EditorSemanticFacts`: actor/participant/message/note/box symbols now come from the sequence lexer token stream with complete/recovered provenance, and LSP has regressions proving sequence fences use parser facts instead of text scans.
- Migrated State as the third family onto `EditorSemanticFacts`: `StateStmt` now carries source spans, state grammar preserves spans for parser-backed state symbols, incomplete buffers recover from the state lexer token stream, and LSP regressions prove state fences use `ParserComplete`/`ParserRecovered` instead of text scans.
- Recorded the current fearless-refactor rule: for families with deterministic lexer/parser seams, old raw-text editor scans are only a migration fallback; future class/ER/state-reference work should extend core facts rather than adding LSP heuristics.
- Migrated Class as the fourth family onto `EditorSemanticFacts`: class/namespace/relation/member-owner/directive-target/interaction-target symbols now come from the class lexer token stream with LALRPOP complete/recovered provenance, and LSP regressions prove class fences use parser facts instead of text scans.
- Recorded the next class-specific deepening opportunities: member-level spans, annotation payload spans, and directive payload reference spans should be modeled in core facts before improving product-grade rename/lint for those constructs.
- Migrated ER as the fifth family onto `EditorSemanticFacts`: `IdList` now preserves per-id spans, entity/relation/attribute/class/style/classDef facts come from the ER lexer token stream, and LSP regressions prove ER fences use `ParserComplete`/`ParserRecovered` instead of text scans.
- Fixed ER incomplete attribute block recovery so the lexer emits the EOF error once and exits block mode, preventing editor fact recovery from hanging on partial buffers.
- Migrated Mindmap as the first hand-written-family tracer bullet onto `EditorSemanticFacts`: its line parser now produces a shared event stream for DB/render semantics and editor facts, preserving node spans, class/icon directives, inline-header spans, multiline labels, and recovered incomplete-delimiter facts.
- Fixed `merman-lsp` feature profile drift by giving the LSP crate default `core-full`/`core-host` feature passthroughs, so product LSP detection includes mindmap and no longer silently uses the tiny core registry.
- Recorded the next high-return fearless-refactor candidates: `gantt` should follow the mindmap event-stream pattern, while class/ER/state/mindmap can be deepened with payload spans and recovered diagnostics before product-grade lint/rename work.
- Migrated Gantt onto `EditorSemanticFacts`: task ids, `after`/`until` dependency references, `click` targets, and directive prefixes now come from the Gantt parser statement rules with complete/recovered provenance.
- Exposed Gantt relative-reference ranges from the date parser helper so editor facts reuse the same Mermaid-backed dependency matcher as render semantics.
- Kept Gantt `section` as a directive prefix rather than a node id to avoid polluting task-id completion; future section document-symbol support should use role-aware or outline-only facts.
- Made Gantt editor completeness tolerant of original-source YAML front matter and Mermaid init directives, preserving complete provenance while still using original byte spans.
- Added Gantt LSP regressions proving complete and incomplete documents use `ParserComplete`/`ParserRecovered`, and added `gantt` diagram-header completion.
- Re-verified `cargo fmt --all`, `cargo nextest run -p merman-core editor_facts --no-fail-fast`, `cargo nextest run -p merman-core gantt --no-fail-fast`, `cargo nextest run -p merman-analysis --no-fail-fast`, and `cargo nextest run -p merman-lsp --no-fail-fast`.
- Added `EditorSemanticRole` to the core editor semantic contract so parser facts can be projected as entity, outline-only, or payload-only symbols.
- Updated `merman-analysis::FenceTextIndex::from_core_facts` to respect semantic roles, keeping payload facts out of completion/navigation while still projecting entity and outline facts.
- Deepened ER editor facts so attribute names are outline-only symbols and attribute type/key/comment spans are preserved as payload facts with accurate source spans.
- Added regressions proving ER payload facts do not pollute completion ids while core facts still preserve the spans for future lint consumers.
- Deepened Class editor facts so class-body members and inline `Class: member` entries are outline-only symbols, while annotation names are payload-only spans for future lint/semantic consumers.
- Added LSP regressions proving class member outline facts and annotation payload facts do not pollute completion ids, and re-verified class/core editor facts plus analysis/LSP suites.
- Deepened Gantt editor facts so `section` titles are parser-backed outline-only symbols while remaining excluded from task-id completion.
- Re-verified Gantt core tests plus `merman-analysis` and `merman-lsp` after the Gantt section outline projection landed.
- Deepened Gantt directive facts so `title`, `dateFormat`, `axisFormat`, `tickInterval`, `includes`, `excludes`, `todayMarker`, `weekday`, and `weekend` values are payload-only parser spans for future lint/semantic consumers.
- Added regressions proving Gantt directive payloads stay out of task-id completion and outline projection, then re-verified Gantt/core editor facts plus analysis/LSP suites.
- Deepened Gantt click facts by making the existing click parser span-aware and preserving `href` URLs, callback names, and callback args as payload-only facts while keeping click task ids as entity references.
- Re-verified Gantt, core editor facts, `merman-analysis`, and `merman-lsp` after the click payload projection landed.
- Deepened Gantt single-line accessibility facts so `accTitle:` and `accDescr:` values are payload-only parser spans while remaining out of completion and outline projection.
- Replaced the old Gantt multiline `accDescr { ... }` future note with a cross-line payload span
  collector.
- Deepened Class interaction facts so quoted URLs/tooltips and link targets are payload-only parser spans while class interaction targets remain entity facts.
- Re-verified Class, core editor facts, `merman-analysis`, and `merman-lsp` after class interaction payload projection landed.

## 2026-06-23
- Confirmed alpha-stage fearless refactor scope for diagnostics-first analysis: canonical `analyze_json`, legacy `validate_json` projection, CLI lint, Markdown fence diagnostics, LSP-ready position mapping, ADR, and engineering memory are in scope.
- Wrote `docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md` as the execution plan for the diagnostics-first analysis core.
- Recorded that Mermaid JS should remain parity evidence and fixture/harness input, not a runtime fallback for Rust, CLI, WASM, FFI, or platform hosts.
- Fixed local macOS shell compatibility so `python` resolves to `python3` from non-interactive zsh via `~/.zshenv`.
- Added `docs/adr/0070-diagnostics-first-analysis-contract.md` and updated FFI, UniFFI, options JSON, and README docs so `analyze_json` is the canonical diagnostics payload and `validate_json` is the compatibility projection.
- Added the `merman-analysis` workspace crate with diagnostics payload types, source descriptors, severity/category enums, diagnostic spans, UTF-16 LSP position mapping, and schema/source-map tests.
- Added the render-free `merman-analysis::Analyzer` pipeline, status-code mirror, semantic warning registry, and analyzer tests for no-diagram, parse errors, unsupported diagrams, valid flowcharts, GitGraph duplicate commit IDs, Block width overflow, source byte limits, and panic status mapping.
- Migrated `merman-bindings-core` so canonical `analyze_json` and legacy `validate_json` are both derived from the same analyzer, and threaded `analyze_json` through C FFI, UniFFI, WASM, and platform wrappers while keeping existing `validate` compatibility paths intact.
- Rebuilt the browser package surface so `@mermanjs/web` now exports `analyze()` / `analyzeJson()` in its checked-in `pkg` and `dist` artifacts, and updated Flutter/web/protocol docs to describe diagnostics analysis as present rather than future work.
- Added first-class `merman-cli lint` support on top of `merman-analysis`, with canonical JSON/text output, Markdown/MDX fence scanning, `--stdin-file-name` for stdin linting, fence-related diagnostic remapping, CLI coverage, and README help text.

## 2026-06-18
- Verified source-backed Flowchart ELK probes are green.
- Ported compound parent-end external dummy net-flow handling in `merman-elk-layered` closer to ELK `calculateNetFlow` behavior.
- Added regression coverage for parent-end external dummy net-flow behavior and existing compound metadata tests still pass.
- Ported inside-self-loop handling so ELK `insideSelfLoops.activate` nodes create nested graphs and `inside_self_loops_yo` edges are imported into the source node nested graph.
- Added regression coverage for inside-self-loop nested graph creation and kept source-backed probe coverage green.
- Verified `cargo test -p merman-elk-layered --tests`, `cargo test -p merman-layout-elk --tests`, `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`, and `cargo fmt --all`.
