# Config and Frontmatter Support Matrix

This document tracks Mermaid config/frontmatter behavior that is intentionally supported by
`merman`. It separates input plumbing from rendered behavior so support claims do not imply more
than the local evidence proves.

Capability levels:

- **Accepted**: an API, CLI flag, frontmatter block, or directive can receive the value without
  rejecting it.
- **Merged**: the value reaches `ParseMetadata.config` / `effective_config` or the render engine's
  site config with Mermaid-compatible precedence.
- **Consumed**: parser, detector, layout, sanitizer, theme, or SVG code reads the value and changes
  local behavior.
- **Rendered**: semantic/layout/SVG tests or upstream goldens prove an observable result for the
  field.

## Entry-Point Merge Semantics

| Input source | Accepted | Merged | Evidence | Notes |
| --- | --- | --- | --- | --- |
| Engine site config | Yes | Yes | `Engine::with_site_config`, `crates/merman-core/src/parse_pipeline.rs`, `site_config_deep_merge_handles_deep_public_config_with_small_stack` | Generated Mermaid defaults load first; caller site config is deep-merged on top. |
| Bindings `options_json.site_config` | Yes | Yes | `crates/merman-bindings-core/src/common.rs`, `render_svg_accepts_external_site_config`, `explicit_site_config_overrides_host_theme_profile_variables` | Non-object values are rejected. Explicit site config overrides host-theme-generated values. |
| CLI `--configFile` / `--config-file` | Yes | Yes | `crates/merman-cli/src/cli.rs`, `crates/merman-cli/src/config.rs`, `config_file_theme_overrides_cli_theme`, `config_file_theme_variables_and_theme_css_affect_svg`, `non_object_config_file_fails_before_rendering` | CLI `--theme` is applied first; JSON object config file values are deep-merged after it. Non-object JSON is rejected like bindings `site_config`. |
| CLI `--handDrawnSeed` | Yes | Yes | `crates/merman-cli/src/config.rs` | Stored as root `handDrawnSeed` in site config. SVG-family proof is field-specific below. |
| Frontmatter `config` | Yes | Yes | `parse_merges_frontmatter_and_directive_config`, `site_config_deep_merge_handles_deep_frontmatter_config_with_small_stack` | Parsed as Mermaid config overrides. Directives merge after frontmatter. |
| Frontmatter `title` | Yes | Metadata only | `parse_metadata` title tests in `crates/merman-core/src/tests/misc.rs` | Sanitized with the effective config before metadata is returned. It is not a general config field. |
| Frontmatter `displayMode` | Yes | Yes | `crates/merman-core/src/preprocess/mod.rs`, `parse_maps_top_level_frontmatter_diagram_config` | Mermaid special case mapped to `gantt.displayMode`. |
| Frontmatter top-level diagram namespaces | Compatibility layer | Yes | `parse_maps_top_level_frontmatter_diagram_config`, `parse_frontmatter_config_takes_priority_over_diagram_compat` | Known diagram namespaces such as `gantt`, `flowchart`, `class`, `er`, `state`, and `xyChart` are mapped into `config.<diagram>`. Explicit `config` values take priority. |
| Frontmatter arbitrary top-level YAML fields | No | No | `parse_maps_top_level_frontmatter_diagram_config` | Unknown keys are ignored, matching the narrow upstream frontmatter surface. |
| `%%{init: ...}%%` directives | Yes | Yes | `parse_merges_frontmatter_and_directive_config`, deep directive stack tests | Directive config is merged after frontmatter, so directive values win. |
| Directive top-level `config` field | Yes | Yes | `parse_metadata_with_type_sync_moves_init_config_without_detection` | Mirrors Mermaid's behavior by moving directive `config` into the detected diagram-specific namespace. |

## Field Capability Matrix

| Config field | Accepted | Merged | Consumed | Rendered | Evidence / residual |
| --- | --- | --- | --- | --- | --- |
| `secure` | Yes | Yes | Yes | Config contract only | Directive/frontmatter sanitization removes attempted `secure` mutations, and `secure_filtered_overrides_removes_default_secure_keys_recursively` proves Mermaid's default secure list prevents diagram config from changing protected keys anywhere in the override object. `site_config_secure_keys_protect_effective_config_from_diagram_config` proves custom site `secure` lists still protect matching keys while preserving `ParseMetadata.config` as user-authored evidence. Imported upstream fixtures that relied on external Cypress render config now use `fixtures/_config/site_config_overrides.json` as harness-level site config instead of depending on diagram-local protected keys. |
| `theme` | Yes | Yes | Yes | Yes | `render_svg_accepts_external_site_config`, `config_file_theme_overrides_cli_theme`, `config_file_theme_variables_and_theme_css_affect_svg`, `theme_renderability_smoke.rs`; family-specific coverage is tracked in `docs/rendering/diagram-theme-coverage.md`. |
| `themeVariables` | Yes | Yes | Yes | Yes | Shared render config helpers, `crates/merman-render/src/svg/parity/theme/*`, family CSS/style modules, `theme_renderability_smoke.rs`, family SVG tests, and `config_file_theme_variables_and_theme_css_affect_svg`. Tests assert visible SVG style values that consume variables. |
| `themeCSS` | Yes | Yes | Yes | Yes | Scoped CSS postprocessor in `crates/merman-render/src/svg/parity.rs`; `render_svg_accepts_external_site_config` and `config_file_theme_variables_and_theme_css_affect_svg` assert scoped CSS plus `data-merman-postprocess="scoped-css"`. Coverage proves scoping/injection, not arbitrary CSS cascade parity for every selector. |
| `securityLevel` | Yes | Yes | Yes | Partial by family | Core sanitizer and URL formatting consume the root field (`format_url_matches_mermaid_utils_spec`, sanitize tests, and family parser tests for flowchart/class/state/gantt links). `flowchart_svg_security_level_controls_unsafe_click_href_rendering`, `class_svg_security_level_controls_unsafe_click_href_rendering`, and `state_svg_security_level_controls_unsafe_click_href_rendering` prove visible SVG link behavior for the current headless renderers: unsafe strict-mode hrefs do not become `xlink:href`, while loose site config preserves Mermaid-compatible custom/raw links. Gantt has parser/model evidence; upstream Gantt wraps links through browser DOM postprocessing for sandbox mode, so standalone SVG coverage should wait for an intentional headless link-emission design. Mermaid's browser iframe behavior for `sandbox` is outside the headless SVG contract. |
| `look` | Yes | Yes | Partial by diagram | Partial by diagram | `crates/merman-render/src/config.rs`, `look_svg_test.rs`, `theme_renderability_smoke.rs`. Do not claim universal `look` behavior until the family has a focused SVG assertion. |
| `layout` | Yes | Yes | Partial | Flowchart ELK subset | Detector/family selection paths preserve flowchart layout side effects; `parse_metadata_with_type_sync_preserves_flowchart_elk_layout_side_effect`, `full_build_detects_flowchart_elk_and_sets_layout`, `render_svg_returns_svg_for_flowchart_elk`, and `headless_renderer_renders_flowchart_elk_svg`. `layout: elk` now reaches a lightweight `elk.layered` subset, not full upstream ELK SVG parity. |
| `flowchart.defaultRenderer` | Yes | Yes | Detector/family selection + Flowchart ELK subset | Flowchart ELK subset | `crates/merman-core/src/detect/mod.rs`, `crates/merman-core/src/family.rs`, flowchart detector tests, flowchart-elk metadata tests, and Flowchart ELK render smoke tests. `elk` selection is preserved and renderable through the lightweight layout backend; parity-gated upstream ELK fixtures remain a separate lane. |
| `class.defaultRenderer` | Yes | Yes | Detector branching | Detector only | `crates/merman-core/src/detect/mod.rs`, `engine_with_site_config_preserves_default_renderer_for_detection`, class detector coverage. Renderer-specific DOM deltas need targeted tests when behavior diverges. |
| `state.defaultRenderer` | Yes | Yes | Detector branching | Detector only | `crates/merman-core/src/detect/mod.rs` and state detector coverage. Renderer-specific DOM deltas need targeted tests when behavior diverges. |
| Root `htmlLabels` | Yes | Yes | Partial by family | Partial by family | Core sanitizer and State layout now follow Mermaid's root-over-deprecated precedence (`sanitize_more_uses_root_html_labels_before_deprecated_flowchart_fallback`, `state_layout_settings_use_root_html_labels_before_deprecated_flowchart_fallback`, `state_layout_root_html_labels_override_deprecated_flowchart_html_labels`). Flowchart, class, and ER label config read root `htmlLabels`; `class_svg_test.rs`, `er_svg_test.rs`, and flowchart layout/SVG tests cover visible behavior. `state_svg_root_html_labels_override_deprecated_flowchart_label_dom` proves State ordinary node and transition labels switch to SVG text when root `htmlLabels=false`, even if deprecated `flowchart.htmlLabels=true`; `state_svg_root_html_labels_false_uses_svg_text_for_cluster_titles`, `state_svg_root_html_labels_false_uses_svg_text_for_notes`, `state_svg_root_html_labels_false_uses_svg_text_for_rect_with_title`, `state_svg_root_html_labels_false_uses_svg_text_for_empty_edge_labels`, and `state_svg_root_html_labels_false_uses_svg_text_for_self_loop_edge_labels` cover the known State label DOM paths. |
| Deprecated `flowchart.htmlLabels` | Yes | Yes | Yes | Partial by family | Flowchart-compatible fallback remains supported when root `htmlLabels` is unset. Covered by flowchart/class/ER tests plus State config/layout/SVG tests (`state_layout_settings_use_root_html_labels_before_deprecated_flowchart_fallback`, `state_layout_root_html_labels_override_deprecated_flowchart_html_labels`, `state_svg_root_html_labels_override_deprecated_flowchart_label_dom`, `state_svg_root_html_labels_false_uses_svg_text_for_cluster_titles`, `state_svg_root_html_labels_false_uses_svg_text_for_notes`, `state_svg_root_html_labels_false_uses_svg_text_for_rect_with_title`, `state_svg_root_html_labels_false_uses_svg_text_for_empty_edge_labels`, `state_svg_root_html_labels_false_uses_svg_text_for_self_loop_edge_labels`). Kept for upstream compatibility. |
| `fontFamily` | Yes | Yes | Yes | Partial by family | Mirrored into `themeVariables.fontFamily`; render helpers and family settings read root/theme values. Covered by `parse_init_font_family_mirrors_legacy_theme_variable_like_upstream`, `parse_init_theme_variable_font_family_overrides_legacy_root`, and theme/font SVG tests. Browser font metrics remain a bounded residual. |
| `fontSize` | Yes | Yes | Yes | Partial by family | Shared render helpers, class layout, sequence settings, Gantt layout, `class_svg_test.rs`, `crates/merman-render/src/svg/parity/theme/tests.rs`, and sequence settings tests. Root `fontSize` and `themeVariables.fontSize` have Mermaid-specific precedence per family. |
| `handDrawnSeed` | Yes | Yes | Partial by family | Focused SVG by family | CLI stores root seed; flowchart, ER, requirement, and state rough-path config read it (`crates/merman-render/src/er/config.rs`, `requirement/config.rs`, `state/config.rs`, `svg/parity/flowchart/render/node.rs`). `flowchart_svg_hand_drawn_seed_controls_visible_rough_paths`, `er_svg_hand_drawn_seed_controls_visible_rough_paths`, `requirement_svg_hand_drawn_seed_controls_visible_rough_paths`, and `state_svg_hand_drawn_seed_controls_visible_rough_paths` lock same-seed determinism and different-seed visible rough path changes for current rough-path consumers. This proves local seed plumbing and deterministic SVG emission, not exhaustive RoughJS parity for every shape. |
| `gantt.displayMode` | Yes | Yes | Yes | Layout golden + SVG focused | Frontmatter special case and layout consumption are covered by `parse_maps_top_level_frontmatter_diagram_config`, `fixtures/gantt/config_frontmatter_layout_fields.golden.json`, and `fixtures/gantt/config_frontmatter_layout_fields.layout.golden.json`; `gantt_svg_frontmatter_config_fields_affect_visible_output` proves the configured compact layout reaches visible SVG rows/tasks. |
| `gantt.topAxis` | Yes | Yes | Yes | Layout golden + SVG focused | `fixtures/gantt/config_frontmatter_layout_fields.layout.golden.json` locks `top_axis: true` and non-empty top ticks from frontmatter `config.gantt.topAxis`; `gantt_svg_frontmatter_config_fields_affect_visible_output` asserts both top and bottom SVG grid groups. |
| `gantt.rightPadding` | Yes | Yes | Yes | Layout golden + SVG focused | `fixtures/gantt/config_frontmatter_layout_fields.layout.golden.json` locks `right_padding: 10.0`; `gantt_svg_frontmatter_config_fields_affect_visible_output` asserts the resulting visible row width. |
| `gantt.useWidth` | Yes | Yes | Yes | Layout golden + SVG focused | `fixtures/gantt/config_frontmatter_layout_fields.layout.golden.json` locks the configured `width: 420.0`; `gantt_svg_frontmatter_config_fields_affect_visible_output` asserts the root SVG `viewBox`/`max-width`. |
| `gantt.numberSectionStyles` | Yes | Yes | Yes | Layout golden + SVG focused | `fixtures/gantt/config_frontmatter_layout_fields.layout.golden.json` locks `number_section_styles: 2.0` and alternating section classes; `gantt_svg_frontmatter_config_fields_affect_visible_output` asserts the corresponding row/title classes in SVG. |

## Known Gaps

- `layout: elk` is renderable for Flowchart through the lightweight `merman-layout-elk` backend,
  but this is not full upstream ELK SVG parity. Treat it as an `elk.layered` subset until the
  dedicated Flowchart ELK lane admits fixture coverage.
- Mermaid default and custom site `secure` lists are enforced against diagram config when
  producing `effective_config`. Imported upstream fixtures that need non-default host config use
  explicit test-harness site config in `fixtures/_config/site_config_overrides.json`; keep new
  imported render options there rather than reintroducing diagram-local protected-key overrides.
- `look` is not a universal all-diagram contract. Renderers should only claim support after tests
  verify both effective config propagation and rendered SVG/CSS consumption.
- `handDrawnSeed` has focused Flowchart, ER, Requirement, and State SVG proof for same-seed
  determinism and different-seed visible rough path changes. Broad RoughJS parity remains
  shape/family-specific and should still be admitted with source-backed tests.
- `securityLevel` has shared sanitizer/model coverage and focused Flowchart/Class/State SVG link
  coverage. Browser-specific `sandbox` iframe isolation is not represented by standalone headless
  SVG output, so remaining family SVG tests should focus on observable link/target/sanitization DOM.
- Gantt frontmatter/config merge semantics are covered, and layout code consumes the key fields,
  but several fields still deserve small layout/SVG fixtures that prove observable geometry.
- Top-level frontmatter compatibility is intentionally narrow. Global Mermaid config keys such as
  `theme`, `look`, and `layout` should still be written under `config`.
