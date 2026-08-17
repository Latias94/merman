# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-08-01

All public binding surfaces accept an optional `options_json` string. Passing null, `None`, `nil`,
or an empty string uses defaults. The same JSON contract is shared by the C ABI, Android JNI, Apple
Swift, Flutter/Dart FFI, and Python UniFFI package.

Reusable engines keep construction options as an immutable baseline. Each operation may supply
request options that deeply merge over that baseline: nested objects merge recursively, while
arrays and scalar leaves replace the baseline value. Request-local overrides do not mutate later
operations. Runtime policy is engine-owned, so reusable requests cannot set it; one-shot operations
may select it while constructing their temporary engine. Resource options are deliberately
stricter: a request may only tighten the constructor's artifact-wide resource ceiling, and an
explicit limit must belong to the selected operation.

Schema `2` rejects unknown top-level fields and unknown fields in compiled option objects so a typo
or removed path cannot be silently ignored. Invalid JSON, invalid UTF-8,
unsupported enum values, or non-finite numeric values return binding errors instead of panicking.
Omitting `version` selects the current schema `2`; explicit legacy versions are rejected rather
than translated implicitly.

## Full Shape

```json
{
  "version": 2,
  "runtime_policy": "deterministic",
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "presentation": {
    "profile": "merman-modern",
    "theme": {
      "preset": "one-dark",
      "appearance": "dark",
      "font_family": "Inter, system-ui, sans-serif",
      "roles": {
        "canvas": "#0f172a",
        "surface": "#111827",
        "text": "#e5e7eb",
        "border": "#475569",
        "line": "#94a3b8",
        "success": "#34d399"
      },
      "series_palette": ["#60a5fa", "#34d399", "#f59e0b"]
    }
  },
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc"
    },
    "themeCSS": ".node rect { stroke-width: 2px; }"
  },
  "parse": {
    "suppress_errors": false
  },
  "ascii": {
    "charset": "unicode",
    "defaultDirection": "leftRight",
    "colorMode": "plain",
    "sequenceMirrorActors": false,
    "xychartVerticalPlotHeight": 5,
    "xychartCategoryBandWidth": 3,
    "xychartHorizontalPlotWidth": 10,
    "relationSummaryDiagnostics": false,
    "theme": {
      "foreground": "#e5e7eb",
      "background": "#111827",
      "line": "#94a3b8",
      "accent": "#60a5fa",
      "muted": "#9ca3af",
      "surface": "#1f2937",
      "border": "#475569"
    }
  },
  "layout": {
    "container_width": 1024,
    "container_height": 768,
    "screen_available_width": 1440
  },
  "environment": {
    "text_measurement": "vendored",
    "math_renderer": "none"
  },
  "resources": {
    "profile": "interactive",
    "limits": {
      "max_source_bytes": 2097152,
      "max_model_items": 32000,
      "max_model_text_bytes": 2097152,
      "max_model_nesting_depth": 256,
      "max_layout_work_units": 800000,
      "max_svg_elements": 250000,
      "max_svg_bytes": 25165824,
      "max_document_diagrams": 256,
      "max_ascii_grid_cells": 250000,
      "max_ascii_layout_work_units": 2000000,
      "max_ascii_document_cells": 250000,
      "max_ascii_output_bytes": 16777216,
      "max_ascii_grapheme_bytes": 4096,
      "max_ascii_nesting_depth": 256,
      "max_raster_width": 4096,
      "max_raster_height": 4096,
      "max_raster_pixels": 16777216,
      "max_embedded_image_bytes": 16777216,
      "max_total_embedded_image_bytes": 33554432,
      "max_embedded_image_pixels": 16777216,
      "max_total_embedded_image_pixels": 33554432,
      "max_pdf_filter_image_pixels": 33554432
    }
  },
  "lint": {
    "profile": "recommended",
    "enable_rules": [
      "merman.authoring.flowchart.explicit_direction"
    ],
    "disable_rules": [
      "merman.authoring.config.prefer_init_directive",
      "merman.git_graph.duplicate_commit_id"
    ],
    "rule_severities": [
      {
        "rule_id": "merman.block.width_exceeds_columns",
        "severity": "hint"
      }
    ]
  },
  "svg": {
    "diagram_id": "my-diagram",
    "pipeline": "parity",
    "scoped_css": ".node rect { stroke-width: 2px; }",
    "css_override_policy": "preserve",
    "root_background_color": "#0f172a",
    "drop_native_duplicate_fallbacks": false
  },
  "raster": {
    "scale": 2,
    "background": "#ffffff",
    "fit_to": { "width": 1200 }
  },
  "jpeg": {
    "quality": 85
  },
  "pdf": {
    "background": "transparent",
    "page_policy": {
      "kind": "fit-css-width",
      "max_width_px": 1200
    }
  }
}
```

Every field is optional.

## Top-Level Fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `version` | integer | `2` | Options-schema version. Version `1` is the incompatible alpha.3 grammar and is rejected. Omitting the field uses the current schema-2 grammar for convenience callers; durable SDK integrations should send `2` explicitly. |
| `runtime_policy` | string | `deterministic` | `deterministic` or `native`. The native policy is an explicit opt-in and fails with a typed missing-capability error unless the artifact contains the required system clock, time-zone, and random adapters. |
| `fixed_today` | string | selected policy date | Overrides the selected policy's local "today" date with a canonical signed-32-bit civil date. Years `0000` through `9999` use `YYYY-MM-DD`; later years use `+YEAR-MM-DD`, and negative years use `-YEAR-MM-DD`. The deterministic policy otherwise uses `1970-01-01`; the native policy reads the system date. |
| `fixed_local_offset_minutes` | integer | selected policy time zone | Replaces the selected policy's time-zone rules with one fixed offset in minutes. The deterministic policy otherwise uses UTC; the native policy uses discovered system time-zone rules. |
| `presentation` | object | none | Optional first-party presentation profile plus independent host semantic theme data. |
| `site_config` | object | defaults | Mermaid site configuration merged onto the pinned Mermaid defaults before diagram directives are applied. |
| `parse` | object | defaults | Parse behavior. |
| `ascii` | object | defaults | ASCII/Unicode text rendering behavior. |
| `layout` | object | defaults | Per-request layout container dimensions. |
| `environment` | object | defaults | Public selection of operation-owned text measurement and optional math rendering. |
| `resources` | object | `interactive` | Source, layout-model, label, and SVG byte/cardinality budgets. |
| `lint` | object | none | Lint rule enable/disable and severity overrides shared across analysis consumers. |
| `svg` | object | defaults | SVG postprocessing behavior. |
| `raster` | object | defaults | Shared PNG/JPEG scale, background, and fit-box behavior. Requires a compiled PNG or JPEG output. |
| `jpeg` | object | defaults | JPEG-specific encoding behavior. Requires the compiled JPEG output. |
| `pdf` | object | defaults | PDF page and background behavior. Requires the compiled PDF output. |

## Runtime Policy

Omitting `runtime_policy` always selects `deterministic`, regardless of which system adapters were
compiled into the artifact. That policy uses Unix epoch time, UTC, a fixed operation seed, and no
timing instrumentation. This makes identical options reproducible across native artifacts, WASM,
and hosts with different local settings.

Set `"runtime_policy": "native"` only when the operation should consult the host clock, complete
system time-zone rules, and random source. Compiling system adapters makes the policy available; it
does not select it. An artifact missing any required adapter rejects the native policy at engine
creation with a typed missing-capability error instead of silently falling back to deterministic
state. Native timing instrumentation remains a separate explicit capability and is not enabled by
the native policy.

Generic binding operation metadata records the selected policy as
`"runtime_policy": "deterministic"` or `"runtime_policy": "native"`. Hosts should retain that
metadata with rendered or analyzed output when reproducibility matters.

PNG and JPEG operations also include an `output_plan` object with requested and effective pixel
dimensions and scale. PDF operations include requested and effective filter-image scale and pixel
counts. A true `limited` flag means the selected resource ceiling reduced the requested plan; hosts
should report the effective values rather than assuming the request was applied unchanged.

## Fixed Time Options

`fixed_today` and `fixed_local_offset_minutes` are host-level deterministic controls for diagrams
whose semantics depend on local time. Gantt uses them for date parsing, relative fallback dates,
and render-model generation. They apply to parse JSON, layout JSON, SVG rendering, validation, and
ASCII render entry points that parse Mermaid source through the shared engine. These values
override the selected runtime policy; they do not implicitly switch a deterministic engine to the
native policy.

## Lint Options

`lint` controls shared analysis rule configuration for diagnostics-first consumers. It uses stable
rule ids from `merman-analysis`.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `lint.profile` | string | `core` | Built-in rule profile: `core`, `recommended`, or `strict`. `core` is conservative and does not enable Merman authoring recommendations. |
| `lint.enable_rules` | array of strings | none | Rule ids to enable even when their profile is not active. Entries must name configurable analysis rules. |
| `lint.disable_rules` | array of strings | none | Rule ids to disable. Entries must name configurable analysis rules. Unknown or internal ids return `MERMAN_INVALID_ARGUMENT`. |
| `lint.rule_severities` | array of objects | none | Per-rule severity overrides as `{ "rule_id": "...", "severity": "error|warning|info|hint" }`. `rule_id` must name a configurable analysis rule. |

`profile`, `enable_rules`, `disable_rules`, and `rule_severities` apply to source lint rules and
semantic warnings alike. They are validated against the public analysis rule registry and can be
used by FFI, UniFFI, WASM, CLI lint, and future editor adapters. `disable_rules` has the highest
precedence. Severity overrides do not enable a rule whose profile is inactive; use
`lint.profile = "recommended"` or `enable_rules` for Merman authoring recommendations.
Bindings expose the same rule registry through their lint-rule catalog metadata surfaces; hosts
should read that catalog when building settings UI instead of duplicating rule ids, evidence
references, origins, or diagnostic tags. Catalog schema `1` treats a missing `tags` field as an
empty list; current deprecation metadata is emitted explicitly as `"tags": ["deprecated"]` rather
than inferred from rule ids or human-readable descriptions.

Only Merman rule ids from the lint-rule catalog are accepted here. External linter ids such as
markdownlint, remark, textlint, or `mermaid-lint` rules must stay in the host tool's own
configuration. For example, `mermaid-lint` rules such as `require-direction`, `duplicate-ids`, or
`no-empty-labels` should not be passed through `lint.enable_rules`, `lint.disable_rules`, or
`lint.rule_severities`. Adapters can convert Merman diagnostics outward into an external report
format, but they should not translate external rule ids into `lint.*` options unless Merman exposes
a distinct source-backed `merman.*` rule.

`analyzeDocument(source, uri, options)` uses this same options contract. The URI determines whether
the payload source is a standalone Mermaid diagram, Markdown, or MDX document; Markdown and MDX
diagnostics, related locations, and fixes are remapped to host-document coordinates. Use
`analyze()` for a single Mermaid diagram body and `analyzeDocument()` for lint integrations that
scan files or Markdown fences.

Rule governance is intentionally conservative because Merman is not the Mermaid project:

| Origin | Meaning | Default profile |
| --- | --- | --- |
| `mermaid_syntax` | Syntax or config behavior backed by Mermaid source/docs/fixtures. | `core` |
| `mermaid_compatibility` | Compatibility warnings backed by Mermaid source/docs/fixtures. | `core` |
| `merman_authoring` | Merman recommendations and safe editor assists, not official Mermaid standards. | `recommended` |
| `merman_resource_policy` | Host/runtime budget diagnostics. | `core` |
| `merman_internal` | Contract gaps and internal safety diagnostics. | not configurable |

Current authoring rule ids are `merman.authoring.config.prefer_init_directive`,
`merman.authoring.config.prefer_frontmatter_config`, and
`merman.authoring.flowchart.explicit_direction`.

`fixed_today` must use Merman's canonical civil-date spelling. Years `0000` through `9999` use
exactly four unsigned digits. Later years use a leading `+`; negative years use `-` and at least
four digits. Signed years do not admit unnecessary leading zeroes, and the year must fit an `i32`.
Examples include `2026-02-15`, `+10000-01-01`, and `-10000-01-01`.
`fixed_local_offset_minutes` must be an integer offset accepted by the fixed-offset timezone model,
currently `-1439` through `1439`. Invalid values return `MERMAN_INVALID_ARGUMENT`.

## Site Config

`site_config` accepts the same Mermaid configuration object that Rust users pass through
`Engine::with_site_config(...)` before constructing a `Renderer`. It is intended for host-level
Mermaid defaults such as theme selection, `themeVariables`, and Mermaid `themeCSS`:

```json
{
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc",
      "nodeBorder": "#38bdf8"
    },
    "themeCSS": ".node rect { filter: drop-shadow(1px 1px 1px #000); }"
  }
}
```

`site_config` must be a JSON object. Non-object values return `MERMAN_INVALID_ARGUMENT`. This option
does not apply host palette replacement or product-specific CSS postprocessing; use explicit host
postprocessing for editor-specific colors.

## Presentation

`presentation` has two independent inputs: an optional first-party product profile and an optional host semantic theme. It does not own raw Mermaid configuration or SVG postprocessing. Default rendering is unchanged when `presentation` is omitted or empty.

```json
{
  "presentation": {
    "profile": "merman-modern",
    "theme": {
      "preset": "one-dark",
      "appearance": "dark",
      "font_family": "Inter, system-ui, sans-serif",
      "font_size": "14px",
      "roles": {
        "canvas": "#0f172a",
        "surface": "#111827",
        "surface-alt": "#1f2937",
        "text": "#e5e7eb",
        "subtle-text": "#cbd5e1",
        "border": "#475569",
        "line": "#94a3b8",
        "note-background": "#422006",
        "note-border": "#f59e0b",
        "success": "#34d399"
      },
      "series_palette": ["#60a5fa", "#34d399", "#f59e0b"]
    }
  }
}
```

`presentation.profile` currently accepts `merman-modern`. The profile selects Redux/slate defaults, Neo look, an ELK default for ordinary Flowcharts, and Merman-owned Flowchart SVG presentation. A selected profile is not rejected during Options parsing merely because ELK is absent: `svg-plan-json` reports each profile aspect independently, and only a Flowchart whose final effective renderer still needs ELK is blocked.

`presentation.theme.preset` accepts `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, or `ayu-dark`. `presentation.theme.appearance` accepts `light` or `dark`. Role keys use the stable kebab-case semantic IDs published by the Rust theme owner, such as `surface-alt`, `subtle-text`, and `edge-label-background`; unknown role IDs fail closed.

Raw Mermaid overrides belong at top-level `site_config`. Output choices belong under `svg`. The removed `host_theme` group returns a migration-oriented error naming `presentation.theme`, `site_config`, and `svg`; nested `output`, `theme_variables`, and `site_config` fields are not accepted under `presentation.theme`.

Merge precedence is the engine's base config, presentation profile defaults, explicit `presentation.theme`, top-level `site_config`, then diagram frontmatter and directives. In a reusable engine request, omitted or empty presentation values inherit the constructor presentation through normal deep overlay semantics.

## Parse Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `parse.suppress_errors` | boolean | `false` | Enables lenient parse, render, and ASCII operations when true. It is not an analysis option. |

## Analysis Consumers

Diagnostics-first analysis, validation projection, CLI linting, Markdown/MDX scanning, and future
LSP adapters use the same `options_json` envelope. Analysis consumers honor options that affect
deterministic time, Mermaid site config, and resource limits while retaining family parse failures
in the closed analysis snapshot:

- `fixed_today` and `fixed_local_offset_minutes` for time-dependent diagram semantics;
- `site_config` and diagram directives for Mermaid-compatible parse/config behavior;
- `resources.*` for source and model budgets.

`parse.suppress_errors` is deliberately excluded from analysis. It remains a top-level shared
binding option for parse, render, and ASCII operations, and is rejected inside `analysis` or
`merman` analysis wrappers.

Render-only options such as `layout.*`, `svg.*`, and host text-measurement settings should not be
required for the default analyzer. Layout-backed or render-backed diagnostics may opt into those
fields later, but they must be profile-controlled and reported through the same diagnostic payload
defined by ADR 0070.

## ASCII Options

`ascii` applies to `render_ascii` and reusable engines that call ASCII rendering. These options do
not affect SVG, parse JSON, layout JSON, or validation output.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `ascii.charset` | string | `unicode` | `unicode` or `ascii`. |
| `ascii.width_profile` / `ascii.widthProfile` | string | `unicode` | `unicode` follows the pinned non-CJK width table; `cjk` treats East Asian ambiguous authored characters as wide and uses single-cell ASCII structural glyphs because Unicode box drawing is East Asian Ambiguous. Select the profile that matches the target terminal. |
| `ascii.default_direction` / `ascii.defaultDirection` | string | `leftRight` | `leftRight`/`left_right` or `topDown`/`top_down` for families that need a default terminal direction. |
| `ascii.color_mode` / `ascii.colorMode` | string | `plain` | `plain`, `truecolor`, or `html`. |
| `ascii.theme` | object | none | Terminal color palette with required `foreground` and `background` plus optional `line`, `accent`, `muted`, `surface`, and `border`. |
| `ascii.box_border_padding` / `ascii.boxBorderPadding` | non-negative integer | `1` | Horizontal padding inside terminal node boxes. |
| `ascii.graph_padding_x` / `ascii.graphPaddingX` | non-negative integer | `5` | Horizontal padding around terminal graph layouts. |
| `ascii.graph_padding_y` / `ascii.graphPaddingY` | non-negative integer | `5` | Vertical padding around terminal graph layouts. |
| `ascii.flowchart_node_label_wrap_width` / `ascii.flowchartNodeLabelWrapWidth` | positive integer | `40` | Maximum display-cell width of an ordinary Flowchart node label before grapheme-safe wrapping and node sizing. This is a terminal-column policy, not Mermaid's SVG `wrappingWidth` in CSS pixels. |
| `ascii.sequence_participant_spacing` / `ascii.sequenceParticipantSpacing` | non-negative integer | `5` | Minimum spacing between sequence participants. |
| `ascii.sequence_message_spacing` / `ascii.sequenceMessageSpacing` | non-negative integer | `1` | Vertical spacing between sequence messages. |
| `ascii.sequence_self_message_width` / `ascii.sequenceSelfMessageWidth` | integer at least `2` | `4` | Width reserved for sequence self-message loops. |
| `ascii.sequence_mirror_actors` / `ascii.sequenceMirrorActors` | boolean | `false` | Renders mirrored bottom participant boxes for sequence diagrams. |
| `ascii.xychart_vertical_plot_height` / `ascii.xychartVerticalPlotHeight` | positive integer | `5` | Compact vertical XYChart plot height. |
| `ascii.xychart_category_band_width` / `ascii.xychartCategoryBandWidth` | positive integer | `3` | Compact vertical XYChart category width. |
| `ascii.xychart_horizontal_plot_width` / `ascii.xychartHorizontalPlotWidth` | positive integer | `10` | Compact horizontal XYChart value axis width. |
| `ascii.relation_summary_diagnostics` / `ascii.relationSummaryDiagnostics` | boolean | `false` | When true, Class/ER `relations:` readability fallbacks include a `reason:` row such as `crossing`, `route_collision`, or `overlay_collision`. Resource limits return structured errors instead. |

`relationSummaryDiagnostics` is intentionally opt-in. Default text output stays stable and omits
internal fallback reasons; hosts can enable the field for support logs, diagnostics panels, or tests
that need to classify why a dense Class/ER relation layout used a summary.

The terminal-grid budget is resource policy, not an ASCII presentation option. Set `resources.limits.max_ascii_grid_cells`; the removed `ascii.max_grid_cells` and `ascii.maxGridCells` fields are rejected with a migration error.

## Layout Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `layout.container_width` | positive finite number | renderer default | Available width of the host layout container in CSS pixels. |
| `layout.container_height` | positive finite number | renderer default | Available height of the host layout container in CSS pixels. |
| `layout.screen_available_width` | positive finite number | `layout.container_width` | Browser `screen.availWidth` in CSS pixels. This is distinct because Mermaid's C4 renderer uses the available screen width rather than the owning container width. |

Container dimensions describe the element that owns diagram layout, not the browser page viewport
or the final SVG viewBox. Browser hosts that need C4 parity should also pass
`layout.screen_available_width`; headless hosts can omit it for deterministic container-width
behavior. The removed `layout.viewport_width` and `layout.viewport_height` names are rejected;
update requests rather than relying on an alias.

## Render Environment Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `environment.text_measurement` | string | `vendored` | `vendored`, `parity`, or `deterministic`. |
| `environment.math_renderer` | string | `none` | `none` or `ratex`. `ratex` requires the `math` feature. |

This is a breaking schema change: `layout.text_measurer` and `layout.math_renderer` are rejected.
Move them to `environment.text_measurement` and `environment.math_renderer`, respectively.

Root Viewport is an operation-owned rendering protocol, not a binding option. Diagram families
provide source-backed content bounds and family-specific root semantics; the shared protocol
normalizes dimensions, computes sizing and `viewBox`, emits root attributes and accessibility
chrome, and finalizes deferred roots. The public `environment` object configures only text
measurement and optional math rendering. Container dimensions belong under `layout.*`; SVG identity
and host-owned output postprocessing belong under `svg.*`. Unknown `environment` fields are rejected.

## Resource Options

`resources` controls artifact-wide deterministic budgets. Cargo features decide which capabilities
are compiled; the resource profile bounds work inside the selected semantic, SVG, or native export
operation. Native PNG, JPEG, and PDF export also scans the host system font database on first use
and caches it for the process lifetime. That host-dependent scan is not caller-configurable and is
not bounded by `resources.*`; multi-tenant or hard-limit hosts must isolate the process accordingly.

Native PNG, JPEG, and PDF embedded images accept only `data:` URLs. The exporters do not resolve
filesystem paths or network URLs. Their default decode budgets allow 16,777,216 bytes and
16,777,216 intrinsic pixels per image, and 33,554,432 bytes and 33,554,432 intrinsic pixels in
aggregate. These four budgets are part of `resources.limits` so constructor ceilings and
per-request tightening use the same contract as semantic and SVG limits.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `resources.profile` | string | `interactive` | `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input`. |
| `resources.limits.max_source_bytes` | positive integer | profile value | Source bytes checked before parse/render work. |
| `resources.limits.max_document_diagrams` | non-negative integer | profile value | Host-document analysis Mermaid fence count for Markdown and MDX; `0` rejects the first Mermaid fence. |
| `resources.limits.max_model_items` | positive integer | profile value | Aggregate semantic entities and relationships across every diagram family. |
| `resources.limits.max_model_text_bytes` | positive integer | profile value | Aggregate UTF-8 text retained by the typed semantic model. |
| `resources.limits.max_model_nesting_depth` | positive integer | profile value | Maximum semantic nesting depth before layout. |
| `resources.limits.max_layout_work_units` | positive integer | profile value | Deterministic family-accounted derived geometry and layout candidate work. |
| `resources.limits.max_svg_bytes` | positive integer | profile value | SVG bytes checked after emission and after postprocessing. |
| `resources.limits.max_svg_elements` | positive integer | profile value | SVG element cardinality checked before recursive postprocessing. |
| `resources.limits.max_ascii_grid_cells` | positive integer | profile value | Checked logical extent for grid-backed terminal renderers. |
| `resources.limits.max_ascii_layout_work_units` | positive integer | profile value | Deterministic ASCII planning, traversal, routing, and paint work. |
| `resources.limits.max_ascii_document_cells` | positive integer | profile value | Aggregate display cells across logical terminal document rows. |
| `resources.limits.max_ascii_output_bytes` | positive integer | profile value | Actual bytes emitted by the selected Plain, ANSI, TrueColor, or HTML encoder. |
| `resources.limits.max_ascii_grapheme_bytes` | positive integer | profile value | UTF-8 bytes allowed in one terminal grapheme cluster. |
| `resources.limits.max_ascii_nesting_depth` | positive integer | profile value | Semantic nesting depth traversed by terminal renderers. |
| `resources.limits.max_raster_width` | positive integer | profile value | Maximum final PNG or JPEG width in pixels. |
| `resources.limits.max_raster_height` | positive integer | profile value | Maximum final PNG or JPEG height in pixels. |
| `resources.limits.max_raster_pixels` | positive integer | profile value | Maximum final PNG or JPEG pixel count. |
| `resources.limits.max_embedded_image_bytes` | positive integer | profile value | Maximum decoded data-URL bytes for one embedded image in PNG, JPEG, or PDF export. |
| `resources.limits.max_total_embedded_image_bytes` | positive integer | profile value | Maximum aggregate decoded data-URL bytes across embedded images. |
| `resources.limits.max_embedded_image_pixels` | positive integer | profile value | Maximum intrinsic pixels for one embedded raster image. |
| `resources.limits.max_total_embedded_image_pixels` | positive integer | profile value | Maximum aggregate intrinsic pixels across embedded raster images. |
| `resources.limits.max_pdf_filter_image_pixels` | positive integer | profile value | Maximum retained pixel area for PDF filter-image rasterization after deterministic downsampling. |

The seven render/model limits are intentionally family-neutral. Each family performs source-backed,
deterministic accounting for its own nodes, relationships, nesting, synthesized geometry, and
candidate scans, then charges those values to the shared model and layout budgets. Hosts therefore
choose a workload profile instead of maintaining diagram-specific threshold tables.

`max_document_diagrams` belongs to host-document analysis, not to a single Mermaid diagram or render policy. Document-analysis operations accept and preserve it, while standalone analysis, model, layout, ASCII, SVG, and export operations reject it as out of scope. Profiles define this dimension consistently with every other binding resource: `interactive` allows 256 diagrams, `constrained` 128, `trusted-native` 1,024, and `unbounded-for-trusted-input` leaves it unbounded.

The six `max_ascii_*` limits belong to ASCII rendering. `max_ascii_grid_cells` replaces the removed `ascii.max_grid_cells` option; the other limits independently bound work before layout amplification, logical document materialization, mode-specific encoded bytes, individual grapheme storage, and nested traversal. Resource excess always returns structured resource details and never silently changes a Diagrammatic projection into StructuredText.

`max_pdf_filter_image_pixels` belongs only to PDF export. It bounds the retained raster area created when SVG filters require an intermediate image; a request may tighten the constructor ceiling, while the exporter deterministically downsamples within that ceiling or returns a structured resource error when no valid plan exists.

The runtime catalog also reports fixed backend hard caps. SVG rendering owns `svg_backend_tree_nodes` and `svg_backend_tree_depth` in the `svg_postprocess` phase, so they apply to SVG output and to every native export derived from that SVG. Native export additionally owns `max_svg_conversion_isolation_depth`, `max_svg_conversion_filter_primitives_per_filter`, `max_total_svg_conversion_filter_primitives`, `max_svg_conversion_subroots`, and `max_nested_svg_images` in the `svg_conversion` phase. All seven remain finite under `unbounded-for-trusted-input` and cannot be supplied in `resources.limits`; generated builders reject attempts to override them.

A reusable engine constructor accepts the union of resource limits enforced by its compiled operations. Per-request overlays are operation-specific: standalone analysis accepts the source budget and host-document analysis additionally accepts the document fence budget; semantic JSON and SVG planning accept source/model budgets; ASCII additionally accepts all six terminal resource budgets; layout JSON additionally accepts layout work; SVG accepts the render budget; PNG and JPEG additionally accept raster and embedded-image budgets; PDF accepts embedded-image and filter-image budgets. An overlay may
select a stricter effective profile or lower a limit, but it cannot raise a constructor limit,
replace a finite ceiling with an unbounded value, clear `resources`, or set an unrelated limit.
One-shot operations may choose any valid profile, but their explicit limits follow the same
operation ownership.

`interactive` is the default for binding surfaces. `constrained` is tighter and is enforced by
the Typst plugin for every call; caller-provided `resources.limits` may tighten that transport
ceiling, while a looser profile or override returns an options error. `trusted-native` is intended for CLI or
controlled batch rendering. `unbounded-for-trusted-input` is an explicit opt-out for trusted inputs,
not a browser or server default.

The profile is a resource-policy choice, not a promise of isolation. `interactive` assumes a
cooperative author and is suitable for an editor or local preview; it does not stop a hostile or
multi-tenant caller from consuming CPU after admission. A public service should select
`constrained` and additionally enforce a wall-clock timeout, memory limit, concurrency quota, and
preemption or process isolation at the host boundary. `trusted-native` is for a controlled CLI or
batch job. `unbounded-for-trusted-input` must only be used inside an outer trusted sandbox.

### Profile Decision Table

| Workload | Profile | Required host controls | Output guidance |
| --- | --- | --- | --- |
| Browser editor/Playground preview | `interactive` | Abort stale requests; cap concurrent renders | `parity` SVG for browser display |
| Public upload or multi-tenant API | `constrained` | Timeout, memory, concurrency, and preemption/isolation | `resvg-safe` before raster/PDF conversion |
| Local `merman-cli` export | `trusted-native` | Process-level cancellation for batch automation | Choose `parity` or `resvg-safe` per consumer |
| Typst package transport | `constrained` | Typst host remains responsible for process limits | Package-owned SVG contract |
| Large, fully trusted offline export | `unbounded-for-trusted-input` | Outer process/container isolation and explicit output quotas | Caller owns final output limits |

These defaults are engineering admission baselines, not latency or memory SLOs. Hosts should
measure their own diagrams and set explicit overrides only after observing peak memory and timeout
behavior. The SVG backend also enforces an internal tree-depth capability; it is not a public
override because increasing it would not make the backend stack-safe.

### Calibration Evidence

Resource-policy changes require a reproducible stress run and a record of the observed boundary:

```sh
cargo bench -p merman --bench flowchart_stress
cargo bench -p merman --bench mindmap_layout_stress
cargo nextest run -p merman-render -p merman-bindings-core
```

Layout-work ceiling changes must additionally run the fail-closed release calibration probe against
the closed public fixture manifest and an adjacent typed rejection boundary:

```sh
CARGO_BUILD_JOBS=1 cargo build --locked --release -p merman \
  --example layout_work_calibration --features complete-svg

python3 tools/bench/run_layout_work_calibration.py \
  --authoritative-date YYYY-MM-DD \
  --out-dir target/bench/layout-work-calibration-YYYY-MM-DD \
  --timeout-seconds 300 \
  --full-repeats 5
```

The wrapper runs the closed corpus five times in fresh processes, verifies byte-identical reports,
and establishes the first rejected node/edge cardinality by scanning the complete accepted prefix
without assuming monotonicity. It then launches isolated semantic, layout, SVG, end-to-end, exact
`W-1`, and boundary probes. Each timing command owns a managed process group so a timeout terminates
descendants independently of leader exit or pipe closure. The unconditional performance-contract
CI lane runs the same timeout regression suite. The wrapper requires an empty output directory and
records the executable, source, manifest, runner, host, timeout, exit status, timing-file byte
lengths, peak RSS, output, and raw report hashes in one summary. Darwin uses `/usr/bin/time -l`;
Linux uses GNU `time -v`.
Unsupported timing formats fail closed rather than silently omitting RSS.

The current `interactive` calibration is recorded in
[`interactive_layout_work_calibration_2026-08-07.md`](../performance/interactive_layout_work_calibration_2026-08-07.md).

For each changed budget, record the fixture/source hash, profile, explicit overrides, host target,
peak RSS (or WASM linear memory), timeout, successful output size, and the first rejected cardinality.
Do not infer a safe limit from a single warm render: compare cold parse, layout, SVG postprocess, and
failure paths separately. The benchmark methodology documents the phase boundaries and evidence
format used by the Playground and comparison tools.

Limit ids are closed under Options JSON schema `2`: an unknown id, a value below the
descriptor's minimum, a non-overridable hard cap, or a removed flat or family-specific field is
rejected.
The runtime contract publishes every accepted id, its phase, whether it is overridable, and the
exact value or `null` for every profile. This avoids copying profile values into host libraries.

## Runtime Contract Discovery

Query the loaded artifact rather than inferring capabilities or resource values from a package
name. Runtime-contract schema `1` includes the transport API version, package identity,
`options_schema_versions`, binding-owned `payload_schemas`, `metadata_ids`, transport-callable
capability/output/operation IDs, and the resource descriptor. Each resource limit records its
`operation_ids`, so a generic SDK can show only limits accepted by the selected operation. The
system adapter IDs contain clock, time-zone, and randomness only when the all-or-nothing `native`
policy is selectable; incomplete native sets and timing instrumentation unified by another Cargo
user are omitted. Every artifact exposes the input limits enforced by the invariant
`semantic-json` operation. SVG artifacts expose the complete render limit set; narrower analysis
and ASCII artifacts publish only the additional limits their callable operations enforce.

The same catalog's `output_contracts` array has exactly one entry for every
`capabilities.output_ids` value. Each entry publishes its media type and nullable system-font and
embedded-image environment contracts. Hosts should validate the complete nested shape before using
those facts while tolerating additive object fields within schema `1`. Native PNG, JPEG, and PDF
entries disclose first-use, process-global, host-dependent system-font discovery and an
embedded-image policy limited to data URLs with default decode budgets; SVG and ASCII report both
environment contracts as `null`.

| Surface | API |
| --- | --- |
| C | `MermanNativeApi.runtime_catalog()` after `merman_get_native_api()` |
| Android/Kotlin | `Merman.runtimeCatalogJson()` |
| Apple/Swift | `Merman().runtimeCatalogJson()` |
| Flutter/Dart | `Merman.open().runtimeCatalog` |
| UniFFI/Python | `Merman.runtime_catalog_json()` / `merman.get_runtime_catalog(api)` |
| Web/TypeScript | `runtimeCatalog()` |

The runtime-contract schema is independent of native ABI `3`, UniFFI binding API `4`, and payload
schema numbers. Reject a contract schema newer than the host understands before interpreting its
nested fields. Detailed language catalogs are not embedded in this flat object: use the
transport's named metadata API (`metadata_collect` for the C ABI) for
`supported-diagrams`, `diagram-family-capabilities`, lint rules, themes, and ASCII capabilities.
Schema-1 consumers must tolerate additive fields. Bundled SDK wrappers require the complete current
shape for every field they decode; they do not silently emulate older producers. A generic C host
may accept an older compatible producer only when it feature-detects omitted discovery fields and
does not depend on them.

## SVG Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `svg.diagram_id` | string | renderer default | Overrides the root SVG diagram id. |
| `svg.viewbox_padding` / `svg.viewBoxPadding` | non-negative finite number | `8` | Extra CSS-pixel padding around the computed SVG viewBox. |
| `svg.pipeline` | string | `parity` | `parity`, `readable`, or `resvg-safe`. |
| `svg.scoped_css` | string | none | Host-owned CSS injected after Mermaid CSS and scoped to the root SVG id. |
| `svg.css_override_policy` | string | `preserve` | `preserve` or `strip-existing-important`. Controls whether existing Mermaid `!important` flags are stripped before host CSS is applied. |
| `svg.root_background_color` | string | none | Host-owned root `<svg>` inline `background-color` replacement. |
| `svg.drop_native_duplicate_fallbacks` | boolean | `false` | Adds generic duplicate fallback cleanup after readable or `resvg-safe` fallback generation. `resvg-safe` already removes generated fallback groups for native SVG `<switch>` text fallbacks, and this option covers additional native/fallback duplicate surfaces. |

`readable` keeps a more inspectable SVG structure. `resvg-safe` rewrites SVG output toward stricter
renderer compatibility, including structural cleanup for labels that already include native SVG
`<switch>` text fallbacks. `drop_native_duplicate_fallbacks` remains an explicit host choice for
additional native/fallback duplicate surfaces, including hosts that already request `resvg-safe`.
Its generic text matching should be treated as an opt-in postprocessing policy. HTML label fallback
text inherits Mermaid label/root fill colors when
available, so dark host profiles do not fall back to unreadable legacy text colors.

`svg.pipeline` also selects the output contract. The default `parity` value intentionally preserves
Mermaid-compatible SVG and can include `<foreignObject>` HTML labels. Hosts that need to feed SVG
bytes into strict SVG renderers, rasterizers, or PDF converters should request `resvg-safe`
explicitly instead of treating the default SVG as export-safe input.

`svg.scoped_css` is for host-owned styling, not Mermaid parity CSS. Selectors are scoped to the
root SVG id and injected after Mermaid's styles so host rules have normal cascade priority. When
`svg.pipeline` is `resvg-safe`, merman sanitizes the injected CSS after insertion to preserve the
raster-safe contract as far as the built-in sanitizer can. Hosts still own CSS trust, palette
semantics, and renderer-specific compatibility.

`svg.root_background_color` is narrower than host CSS. It rewrites the root `<svg>` inline
`background-color` value, or adds one when missing. This is useful for editor previews that need the
diagram canvas to match the host surface. The value must be a single CSS declaration value; use
`"transparent"` when the host wants no opaque root background.

## Native Export Options

`raster` applies to PNG and JPEG, `jpeg` applies only to JPEG, and `pdf` applies only to PDF. A reusable engine constructor accepts the option groups compiled into its artifact, while a one-shot call or request overlay rejects groups unrelated to that operation. An artifact without the corresponding output rejects the known group instead of silently ignoring it.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `raster.scale` | positive finite number | `1` | Device-pixel scaling applied before the output resource ceiling. |
| `raster.background` | supported color string | transparent | `transparent`, `white`, `black`, or 3/4/6/8-digit hex. JPEG requires an opaque result. |
| `raster.fit_to.width` | positive integer | none | Optional target width in pixels. At least one fit dimension is required. |
| `raster.fit_to.height` | positive integer | none | Optional target height in pixels. At least one fit dimension is required. |
| `jpeg.quality` | integer from `1` to `100` | exporter default | JPEG encoder quality. |
| `pdf.background` | supported color string | transparent | Optional PDF page background using the same supported color vocabulary. |
| `pdf.filter_scale` / `pdf.filterScale` | positive finite number | `4` | Requested sampling scale for localized SVG filter images. The exporter may reduce it to satisfy `max_pdf_filter_image_pixels`. |
| `pdf.page_policy.kind` | string | `fit-svg` | `fit-svg`, `fixed`, or `fit-css-width`. |
| `pdf.page_policy.width_pt` / `height_pt` | positive finite number | required by `fixed` | Fixed PDF page dimensions in points. |
| `pdf.page_policy.max_width_px` | positive finite number | required by `fit-css-width` | Maximum responsive SVG width in CSS pixels before conversion to PDF points. |

PDF filter sampling is explicit, while its localized-image ceiling remains part of `resources.limits`. Generic operation metadata reports the requested and effective scale when the exporter reduces sampling to satisfy `max_pdf_filter_image_pixels`.

## Examples

Readable SVG with a stable id:

```json
{
  "svg": {
    "diagram_id": "docs-flow",
    "pipeline": "readable"
  }
}
```

External Mermaid theme defaults for plain source:

```json
{
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc"
    }
  },
  "svg": {
    "diagram_id": "host-preview"
  }
}
```

Readable SVG with generic duplicate native/fallback labels removed:

```json
{
  "svg": {
    "pipeline": "readable",
    "drop_native_duplicate_fallbacks": true
  }
}
```

Resvg-safe SVG with host-scoped CSS:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "diagram_id": "host-preview",
    "scoped_css": ".node rect { fill: #111827; } .merman-foreignobject-fallback-text { fill: #f8fafc; }",
    "css_override_policy": "strip-existing-important"
  }
}
```

Resvg-safe SVG with a host-owned canvas color:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "diagram_id": "host-preview",
    "root_background_color": "#0f172a"
  }
}
```

Deterministic layout for tests:

```json
{
  "runtime_policy": "deterministic",
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "environment": {
    "text_measurement": "deterministic"
  },
  "layout": {
    "container_width": 1024,
    "container_height": 768
  }
}
```

Lenient parsing:

```json
{
  "parse": {
    "suppress_errors": true
  }
}
```

Strict resource profile override:

```json
{
  "resources": {
    "profile": "constrained",
    "limits": {
      "max_model_items": 500
    }
  }
}
```

## Error Behavior

Invalid options produce binding errors:

| Error | Native ABI 3 status |
| --- | --- |
| Invalid UTF-8 | `MERMAN_NATIVE_STATUS_UTF8_ERROR` |
| Invalid JSON | `MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR` |
| Unsupported option value | `MERMAN_NATIVE_STATUS_INVALID_ARGUMENT` |
| Feature-gated operation disabled | `MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION` |
| Resource budget exceeded | `MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED` |

Resource failures add an optional `details.resource` object to the existing error JSON. It contains `cause`, `limit_id`, `phase`, `actual`, `max`, and `profile`. The stable `cause` is `ceiling` when an effective maximum was exceeded and `arithmetic_overflow` when safe work accounting could not represent the required amount. Node, Web/WASM, Android JNI, and native ABI error JSON project `actual` and `max` into a lossless representation: values through `9007199254740991` (`Number.MAX_SAFE_INTEGER`) are numbers, while larger `u64` values are canonical unsigned decimal strings without leading zeroes, up to `18446744073709551615`. Consumers must accept both forms and must not coerce the string form through a floating-point number; typed non-JSON binding payloads retain their unsigned-integer fields. Parser and ASCII renderer failures may additionally expose a bounded `details.diagnostic` object with stable `code`, optional byte `span` (`start`, `end`, `kind`), and safe `field`/`diagram_type` context. These fields are machine-readable and must not be recovered by parsing the human-facing `message`; complete source text is never embedded by default. Consumers that understand payload schema `1` should tolerate these additive objects; errors without structured details omit `details`, preserving the previous shape.

Platform wrappers surface those errors through their native exception type:

- C ABI: non-zero `MermanNativeStatus`, mirrored in `MermanNativeResult.status`, with the structured
  JSON error payload in `MermanNativeResult.metadata_or_error_json`.
- Android: `MermanException.exactResourceDetails`, plus `resourceDetails` when both counts fit `Long`.
- Apple: the optional `resource` field on `MermanError.binding`.
- Flutter/Dart: `MermanException.exactResourceDetails`, plus `resourceDetails` when both counts fit a signed 64-bit `int`.
- Python UniFFI: the optional `resource` field on `MermanError.Binding`.

## Typed Wrapper Follow-On

The stable low-level contract remains JSON so the C ABI does not grow for every option. Generated
typed builders now sit above that contract and are produced from the Rust resource descriptor:

| Platform | Generated API |
| --- | --- |
| C | include `merman_resource_contract.h`; `MERMAN_BINDING_OPTIONS_SCHEMA_VERSION`, stable profile/limit string macros, and `*_MINIMUM` / `*_OVERRIDABLE` describe the JSON document the host serializes |
| Android/Kotlin | `MermanResourceOptionsBuilder` / `MermanResourceOptions` with an optional profile and `MermanResourceOverrideId` |
| Apple/Swift | generated `resourceOptionsJson(profile:overrides:)` |
| Flutter/Dart | `MermanResourceOptionsBuilder` / `MermanResourceOptions` with optional profile and `MermanResourceOverrideId` |
| Python/UniFFI | `ResourceOptionsBuilder` / `ResourceOptions` with optional profile and `ResourceOverrideId` / `MermanResourceOverrideId` |
| Web/TypeScript | closed `ResourceProfile`/`ResourceOverrideId` unions and `resourceOptions()`; use `rawResourceOptionsJson()` only for an explicitly external contract |

Builders validate profile IDs, overridable limit IDs, and each descriptor-owned minimum before serialization. Leaving profile unset emits no `resources.profile`, so a reusable request inherits its constructor ceiling instead of silently selecting `interactive`. They do not duplicate the profile budget table; hosts should query the runtime contract when presenting values or settings UI.
