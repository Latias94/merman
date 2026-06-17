# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-06-09

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
      "css_override_policy": "strip-existing-important"
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
  "layout": {
    "viewport_width": 1024,
    "viewport_height": 768,
    "text_measurer": "vendored",
    "math_renderer": "none",
    "flowchart_elk_backend": "source-ported"
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
| `layout` | object | defaults | Layout and text measurement behavior. |
| `svg` | object | defaults | SVG postprocessing behavior. |

## Fixed Time Options

`fixed_today` and `fixed_local_offset_minutes` are host-level deterministic controls for diagrams
whose semantics depend on local time. Gantt uses them for date parsing, relative fallback dates,
and render-model generation. They apply to parse JSON, layout JSON, SVG rendering, validation, and
ASCII render entry points that parse Mermaid source through the shared engine.

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
      "css_override_policy": "strip-existing-important"
    }
  }
}
```

`host_theme.appearance` accepts `light` or `dark`. `host_theme.output.pipeline` accepts `parity`,
`readable`, `resvg-safe`, or `resvg_safe`. `host_theme.output.root_background` accepts `none`,
`canvas`, or a single CSS declaration value. An empty `{ "host_theme": {} }` is a no-op and does
not force Mermaid `theme=base`.

`host_theme.preset` accepts `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
`gruvbox-dark`, `ayu-light`, or `ayu-dark`. Explicit `roles`, `series_palette`,
`themeVariables`, `site_config`, and `output` fields override the preset. Host theme presets are
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

## SVG Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `svg.diagram_id` | string | renderer default | Overrides the root SVG diagram id. |
| `svg.pipeline` | string | `parity` | `parity`, `readable`, `resvg-safe`, or `resvg_safe`. |
| `svg.scoped_css` | string | none | Host-owned CSS injected after Mermaid CSS and scoped to the root SVG id. |
| `svg.css_override_policy` | string | `preserve` | `preserve`, `strip-existing-important`, or `strip_existing_important`. Controls whether existing Mermaid `!important` flags are stripped before host CSS is applied, and can override `host_theme.output.css_override_policy`. |
| `svg.root_background_color` | string | none | Host-owned root `<svg>` inline `background-color` replacement. |
| `svg.drop_native_duplicate_fallbacks` | boolean | `false` | Adds generic duplicate fallback cleanup for non-`resvg-safe` pipelines. `resvg-safe` already removes generated fallback groups for native SVG `<switch>` text fallbacks. |

`readable` keeps a more inspectable SVG structure. `resvg-safe` rewrites SVG output toward stricter
renderer compatibility, including structural cleanup for labels that already include native SVG
`<switch>` text fallbacks. `drop_native_duplicate_fallbacks` remains available for hosts composing
non-`resvg-safe` pipelines, and its generic text matching should be treated as an explicit
postprocessing choice. HTML label fallback text inherits Mermaid label/root fill colors when
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

## Error Behavior

Invalid options produce binding errors:

| Error | Code name |
| --- | --- |
| Invalid UTF-8 | `MERMAN_UTF8_ERROR` |
| Invalid JSON | `MERMAN_OPTIONS_JSON_ERROR` |
| Unsupported option value | `MERMAN_INVALID_ARGUMENT` |
| Feature-gated format disabled | `MERMAN_UNSUPPORTED_FORMAT` |

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
