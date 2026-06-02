# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-06-02

All public binding surfaces accept an optional `options_json` string. Passing null, `None`, `nil`,
or an empty string uses defaults. The same JSON contract is shared by the C ABI, Android JNI, Apple
Swift, Flutter/Dart FFI, and Python UniFFI package.

Unknown fields are ignored. Invalid JSON, invalid UTF-8, unsupported enum values, or non-finite
numeric values return binding errors instead of panicking.

## Full Shape

```json
{
  "version": 1,
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
    "math_renderer": "none"
  },
  "svg": {
    "diagram_id": "my-diagram",
    "pipeline": "parity"
  }
}
```

Every field is optional.

## Top-Level Fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `version` | integer | ignored | Reserved for future options-schema versioning. |
| `site_config` | object | defaults | Mermaid site configuration merged onto the pinned Mermaid defaults before diagram directives are applied. |
| `parse` | object | defaults | Parse behavior. |
| `layout` | object | defaults | Layout and text measurement behavior. |
| `svg` | object | defaults | SVG postprocessing behavior. |

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

`text_measurer=deterministic` is useful for repeatable tests. `text_measurer=vendored` uses bundled
font metrics when available.

## SVG Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `svg.diagram_id` | string | renderer default | Overrides the root SVG diagram id. |
| `svg.pipeline` | string | `parity` | `parity`, `readable`, `resvg-safe`, or `resvg_safe`. |
| `svg.drop_native_duplicate_fallbacks` | boolean | `false` | Drops generated fallback label groups only when their text duplicates native SVG `<text>`. Useful with `readable` or `resvg-safe` for hosts that rasterize or restyle SVG output. |

`readable` keeps a more inspectable SVG structure. `resvg-safe` rewrites SVG output toward stricter
renderer compatibility. `drop_native_duplicate_fallbacks` is opt-in so fallback-only labels are not
lost by default.

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

Resvg-safe SVG with duplicate native/fallback labels removed:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "drop_native_duplicate_fallbacks": true
  }
}
```

Deterministic layout for tests:

```json
{
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
