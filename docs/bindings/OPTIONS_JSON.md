# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-05-31

All public binding surfaces accept an optional `options_json` string. Passing null, `None`, `nil`,
or an empty string uses defaults. The same JSON contract is shared by the C ABI, Android JNI, Apple
Swift, Flutter/Dart FFI, and Python UniFFI package.

Unknown fields are ignored. Invalid JSON, invalid UTF-8, unsupported enum values, or non-finite
numeric values return binding errors instead of panicking.

## Full Shape

```json
{
  "version": 1,
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
| `parse` | object | defaults | Parse behavior. |
| `layout` | object | defaults | Layout and text measurement behavior. |
| `svg` | object | defaults | SVG postprocessing behavior. |

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

`readable` keeps a more inspectable SVG structure. `resvg-safe` rewrites SVG output toward stricter
renderer compatibility.

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
