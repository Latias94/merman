# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-07-02

All public binding surfaces accept an optional `options_json` string. Passing null, `None`, `nil`,
or an empty string uses defaults. The same JSON contract is shared by the C ABI, Android JNI, Apple
Swift, Flutter/Dart FFI, and Python UniFFI package.

Unknown fields are ignored. Invalid JSON, invalid UTF-8, unsupported enum values, or non-finite
numeric values return binding errors instead of panicking.

## Full Shape

```json
{
  "version": 1,
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "host_theme": {
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
    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
    "output": {
      "pipeline": "resvg-safe",
      "root_background": "canvas",
      "css_override_policy": "strip-existing-important",
      "drop_native_duplicate_fallbacks": false
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
    "maxGridCells": 250000,
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
    "viewport_width": 1024,
    "viewport_height": 768,
    "text_measurer": "vendored",
    "math_renderer": "none",
    "flowchart_elk_backend": "source-ported"
  },
  "resources": {
    "profile": "interactive",
    "max_source_bytes": 2097152,
    "max_svg_bytes": 25165824,
    "max_flowchart_nodes": 8000,
    "max_flowchart_edges": 16000,
    "max_flowchart_subgraphs": 2000,
    "max_label_bytes": 2097152
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
  }
}
```

Every field is optional.

## Top-Level Fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `version` | integer | ignored | Reserved for future options-schema versioning. |
| `fixed_today` | string | system local date | Fixed local "today" date in `YYYY-MM-DD` format for time-dependent diagrams such as Gantt. |
| `fixed_local_offset_minutes` | integer | system local timezone | Fixed local timezone offset in minutes for deterministic local-time parsing and rendering. |
| `host_theme` | object | none | Opt-in host/editor theme profile compiled into Mermaid config and SVG output settings. |
| `site_config` | object | defaults | Mermaid site configuration merged onto the pinned Mermaid defaults before diagram directives are applied. |
| `parse` | object | defaults | Parse behavior. |
| `ascii` | object | defaults | ASCII/Unicode text rendering behavior. |
| `layout` | object | defaults | Layout and text measurement behavior. |
| `resources` | object | `interactive` | Source, layout-model, label, and SVG byte/cardinality budgets. |
| `lint` | object | none | Lint rule enable/disable and severity overrides shared across analysis consumers. |
| `svg` | object | defaults | SVG postprocessing behavior. |

## Fixed Time Options

`fixed_today` and `fixed_local_offset_minutes` are host-level deterministic controls for diagrams
whose semantics depend on local time. Gantt uses them for date parsing, relative fallback dates,
and render-model generation. They apply to parse JSON, layout JSON, SVG rendering, validation, and
ASCII render entry points that parse Mermaid source through the shared engine.

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
references, and origins.

Only Merman rule ids from the lint-rule catalog are accepted here. External linter ids such as
markdownlint, remark, textlint, or `mermaid-lint` rules must stay in the host tool's own
configuration. For example, `mermaid-lint` rules such as `require-direction`, `duplicate-ids`, or
`no-empty-labels` should not be passed through `lint.enable_rules`, `lint.disable_rules`, or
`lint.rule_severities`. Adapters can convert Merman diagnostics outward into an external report
format, but they should not translate external rule ids into `lint.*` options unless Merman exposes
a distinct source-backed `merman.*` rule.

`analyzeDocument(source, options, uri)` uses this same options contract. The URI determines whether
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

`fixed_today` must be a `YYYY-MM-DD` date. `fixed_local_offset_minutes` must be an integer offset
accepted by the fixed-offset timezone model, currently `-1439` through `1439`. Invalid values return
`MERMAN_INVALID_ARGUMENT`.

## Site Config

`site_config` accepts the same Mermaid configuration object that Rust users pass through
`HeadlessRenderer::with_site_config(...)`. It is intended for host-level Mermaid defaults such as
theme selection, `themeVariables`, and Mermaid `themeCSS`:

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

## Host Theme Profile

`host_theme` is an opt-in semantic profile for editor and application previews. It compiles host
roles into Mermaid-compatible `themeVariables`, selected diagram config defaults, and SVG
postprocessing options. Default rendering is unchanged when `host_theme` is omitted.

```json
{
  "host_theme": {
    "preset": "one-dark",
    "appearance": "dark",
    "font_family": "Inter, system-ui, sans-serif",
    "font_size": "14px",
    "roles": {
      "canvas": "#0f172a",
      "surface": "#111827",
      "surface_alt": "#1f2937",
      "text": "#e5e7eb",
      "subtle_text": "#cbd5e1",
      "border": "#475569",
      "line": "#94a3b8",
      "note_background": "#422006",
      "note_border": "#f59e0b",
      "success": "#34d399"
    },
    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
    "themeVariables": {
      "nodeBorder": "#38bdf8"
    },
    "output": {
      "pipeline": "resvg-safe",
      "root_background": "canvas",
      "css_override_policy": "strip-existing-important",
      "drop_native_duplicate_fallbacks": false
    }
  }
}
```

`host_theme.appearance` accepts `light` or `dark`. `host_theme.output.pipeline` accepts `parity`,
`readable`, `resvg-safe`, or `resvg_safe`. `host_theme.output.root_background` accepts `none`,
`canvas`, or a single CSS declaration value. `host_theme.output.drop_native_duplicate_fallbacks`
opts into removing fallback groups whose text duplicates native `<text>` after readable or
`resvg-safe` fallback generation. It is off by default because repeated labels can be intentional in
unrelated nodes. An empty `{ "host_theme": {} }` is a no-op and does not force Mermaid `theme=base`.

`host_theme.preset` accepts `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
`gruvbox-dark`, `ayu-light`, `ayu-dark`, `merman-modern`, or `mermaid`. `merman-modern`
selects Redux, Neo, the ELK flowchart renderer, a restrained slate palette, padded edge labels, and compact
rounded corners, so rendering it requires an `elk-layout` build.
`mermaid` explicitly selects upstream Mermaid defaults and parity SVG output. Explicit `roles`,
`series_palette`, `themeVariables`, `site_config`, and `output` fields override the preset. Host theme presets are
separate from Mermaid core theme names returned by `supported_themes`. Binding surfaces expose the
stable preset list through `supported_host_theme_presets` / `supportedHostThemePresets`-style
metadata helpers.

Merge precedence is Mermaid defaults, then `host_theme` derived config, then explicit
`host_theme.themeVariables` / `host_theme.site_config`, then top-level `site_config`, then diagram
directives. Explicit `svg.*` options override profile output options.

## Parse Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `parse.suppress_errors` | boolean | `false` | Enables lenient parsing when true. |

## Analysis Consumers

Diagnostics-first analysis, validation projection, CLI linting, Markdown/MDX scanning, and future
LSP adapters use the same `options_json` envelope. Analysis consumers should honor options that
affect parsing, deterministic time, Mermaid site config, and resource limits:

- `fixed_today` and `fixed_local_offset_minutes` for time-dependent diagram semantics;
- `site_config` and diagram directives for Mermaid-compatible parse/config behavior;
- `parse.*` for parser strictness;
- `resources.*` for source and model budgets.

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
| `ascii.default_direction` / `ascii.defaultDirection` | string | `leftRight` | `leftRight`/`left_right` or `topDown`/`top_down` for families that need a default terminal direction. |
| `ascii.color_mode` / `ascii.colorMode` | string | `plain` | `plain`, `truecolor`, or `html`. |
| `ascii.theme` | object | none | Terminal color palette with required `foreground` and `background` plus optional `line`, `accent`, `muted`, `surface`, and `border`. |
| `ascii.sequence_mirror_actors` / `ascii.sequenceMirrorActors` | boolean | `false` | Renders mirrored bottom participant boxes for sequence diagrams. |
| `ascii.xychart_vertical_plot_height` / `ascii.xychartVerticalPlotHeight` | positive integer | `5` | Compact vertical XYChart plot height. |
| `ascii.xychart_category_band_width` / `ascii.xychartCategoryBandWidth` | positive integer | `3` | Compact vertical XYChart category width. |
| `ascii.xychart_horizontal_plot_width` / `ascii.xychartHorizontalPlotWidth` | positive integer | `10` | Compact horizontal XYChart value axis width. |
| `ascii.max_grid_cells` / `ascii.maxGridCells` | positive integer | `250000` | Maximum terminal grid cells for graph-like ASCII layouts before fallback or error behavior. |
| `ascii.relation_summary_diagnostics` / `ascii.relationSummaryDiagnostics` | boolean | `false` | When true, Class/ER `relations:` fallback summaries include a `reason:` row such as `grid_budget actual=12 limit=1`, `crossing`, `route_collision`, or `overlay_collision`. |

`relationSummaryDiagnostics` is intentionally opt-in. Default text output stays stable and omits
internal fallback reasons; hosts can enable the field for support logs, diagnostics panels, or tests
that need to classify why a dense Class/ER relation layout used a summary.

## Layout Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `layout.viewport_width` | positive finite number | renderer default | Overrides layout viewport width. |
| `layout.viewport_height` | positive finite number | renderer default | Overrides layout viewport height. |
| `layout.text_measurer` | string | renderer default | `vendored` or `deterministic`. |
| `layout.math_renderer` | string | renderer default | `none` or `ratex`. `ratex` requires the `ratex-math` feature. |
| `layout.flowchart_elk_backend` | string | `source-ported` | `source-ported`, `source_ported`, `source`, or `compat`. Selects the Flowchart ELK backend. |

`text_measurer=deterministic` is useful for repeatable tests. `text_measurer=vendored` uses bundled
font metrics when available.
`flowchart_elk_backend=compat` is an alpha fallback for the older lightweight Flowchart ELK backend;
the default source-ported backend follows the pinned Mermaid adapter and Eclipse ELK layered port.

## Resource Options

`resources` controls render-wide deterministic budgets. These limits are separate from raster
pixel/PDF limits; disabling raster limits does not disable source, layout, label, or SVG limits.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `resources.profile` | string | `interactive` | `interactive`, `typst-package`, `trusted-native`, or `unbounded-for-trusted-input`. |
| `resources.max_source_bytes` | positive integer | profile value | Source bytes checked before parse/render work. |
| `resources.max_svg_bytes` | positive integer | profile value | SVG bytes checked after emission and after postprocessing. |
| `resources.max_flowchart_nodes` | positive integer | profile value | Flowchart nodes plus subgraph layout nodes. |
| `resources.max_flowchart_edges` | positive integer | profile value | Flowchart edge cardinality. |
| `resources.max_flowchart_subgraphs` | positive integer | profile value | Flowchart hierarchy cardinality. |
| `resources.max_label_bytes` | positive integer | profile value | Aggregate Flowchart ids, labels, subgraph titles, and tooltips. |

`interactive` is the default for binding surfaces. `typst-package` is tighter and is injected by the
Typst plugin when the caller does not provide `resources`. `trusted-native` is intended for CLI or
controlled batch rendering. `unbounded-for-trusted-input` is an explicit opt-out for trusted inputs,
not a browser or server default.

## SVG Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `svg.diagram_id` | string | renderer default | Overrides the root SVG diagram id. |
| `svg.pipeline` | string | `parity` | `parity`, `readable`, `resvg-safe`, or `resvg_safe`. |
| `svg.scoped_css` | string | none | Host-owned CSS injected after Mermaid CSS and scoped to the root SVG id. |
| `svg.css_override_policy` | string | `preserve` | `preserve`, `strip-existing-important`, or `strip_existing_important`. Controls whether existing Mermaid `!important` flags are stripped before host CSS is applied, and can override `host_theme.output.css_override_policy`. |
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
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "layout": {
    "text_measurer": "deterministic",
    "viewport_width": 1024,
    "viewport_height": 768
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
    "profile": "typst-package",
    "max_flowchart_nodes": 500
  }
}
```

## Error Behavior

Invalid options produce binding errors:

| Error | Code name |
| --- | --- |
| Invalid UTF-8 | `MERMAN_UTF8_ERROR` |
| Invalid JSON | `MERMAN_OPTIONS_JSON_ERROR` |
| Unsupported option value | `MERMAN_INVALID_ARGUMENT` |
| Feature-gated format disabled | `MERMAN_UNSUPPORTED_FORMAT` |
| Resource budget exceeded | `MERMAN_RESOURCE_LIMIT_EXCEEDED` |

Platform wrappers surface those errors through their native exception type:

- C ABI: non-zero `MermanResult.code` with a JSON error payload.
- Android: `MermanException`.
- Apple: `MermanError.binding`.
- Flutter/Dart: `MermanException`.
- Python UniFFI: `MermanError.Binding`.

## Typed Wrapper Follow-On

The stable low-level contract should remain JSON so the C ABI does not grow for every option. Higher
level platform packages can add typed option builders later, then serialize to this JSON shape
internally.
