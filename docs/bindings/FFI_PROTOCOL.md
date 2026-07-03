# Merman FFI Protocol

Status: Draft
Last updated: 2026-06-23

This document defines the first C ABI protocol for `merman-ffi`.

Project repository: <https://github.com/Latias94/merman>

Authoritative code:

- `crates/merman-ffi/include/merman.h`
- `crates/merman-ffi/src/lib.rs`
- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/adr/0070-diagnostics-first-analysis-contract.md`
- `docs/workstreams/ffi-api/DESIGN.md`

## Build And Link

Build the C ABI artifacts from the workspace:

```sh
cargo build -p merman-ffi --release
```

`merman-ffi` is configured as `cdylib`, `staticlib`, and `rlib`. C and C-compatible hosts should
include `crates/merman-ffi/include/merman.h` and link the platform-specific artifact from
`target/release`.

Feature examples:

```sh
cargo build -p merman-ffi --release --features ratex-math
cargo build -p merman-ffi --release --features raster,ratex-math
```

The current C ABI exposes SVG, ASCII text, semantic JSON, layout JSON, validation JSON,
diagnostics-first analysis JSON, binding metadata, and optional host text measurement for reusable
engines. Raster byte outputs are not part of this protocol version even though the Rust crate has a
reserved `raster` feature gate. All source-processing functions accept the shared `options_json`
contract documented in `docs/bindings/OPTIONS_JSON.md`.

That shared options contract now also carries a `lint` section for rule profiles, explicit
enable/disable, and severity overrides. Hosts that consume analysis or validation diagnostics
should treat those rule ids as the stable shared contract, not as transport-local behavior. Merman
authoring rules are opt-in through the `recommended` profile or explicit rule enablement; they are
not Mermaid-official standards.

## Stability

The protocol is pre-1.0. Hosts should still treat these rules as the compatibility baseline for the
first FFI release candidate:

- ignore unknown JSON fields
- tolerate missing optional fields
- do not assume JSON field ordering
- release every non-empty result buffer exactly once
- prefer the diagnostics-first analysis payload exposed by a given binding package

## Types

The current ABI protocol version is:

```c
#define MERMAN_ABI_VERSION 2
```

```c
typedef struct MermanBuffer {
    uint8_t* data;
    size_t len;
} MermanBuffer;

typedef struct MermanResult {
    int32_t code;
    MermanBuffer data;
} MermanResult;

typedef struct MermanEngine MermanEngine;

typedef struct MermanEngineResult {
    int32_t code;
    MermanEngine* engine;
    MermanBuffer data;
} MermanEngineResult;

enum {
    MERMAN_WRAP_MODE_SVG_LIKE = 0,
    MERMAN_WRAP_MODE_SVG_LIKE_SINGLE_RUN = 1,
    MERMAN_WRAP_MODE_HTML_LIKE = 2
};

enum {
    MERMAN_TEXT_DIRECTION_AUTO = 0,
    MERMAN_TEXT_DIRECTION_LTR = 1,
    MERMAN_TEXT_DIRECTION_RTL = 2
};

enum {
    MERMAN_TEXT_WHITE_SPACE_NORMAL = 0,
    MERMAN_TEXT_WHITE_SPACE_NOWRAP = 1,
    MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES = 2,
    MERMAN_TEXT_WHITE_SPACE_PRE_WRAP = 3
};

typedef struct MermanHostTextMeasureRequest {
    const uint8_t* text;
    size_t text_len;
    const uint8_t* font_family;
    size_t font_family_len;
    double font_size;
    const uint8_t* font_weight;
    size_t font_weight_len;
    const uint8_t* font_style;
    size_t font_style_len;
    double max_width;
    double line_height;
    double letter_spacing;
    double word_spacing;
    int32_t wrap_mode;
    int32_t direction;
    int32_t white_space;
    uint8_t has_max_width;
} MermanHostTextMeasureRequest;

typedef struct MermanHostTextMeasureResult {
    uint8_t handled;
    double width;
    double height;
    size_t line_count;
} MermanHostTextMeasureResult;

typedef MermanHostTextMeasureResult (*MermanHostTextMeasureCallback)(
    MermanHostTextMeasureRequest request,
    void* user_data
);
```

`MermanBuffer.data == NULL` means there is no payload. `len == 0` means the payload is empty.
`MermanEngine` is an opaque handle owned by Rust.

## ABI Introspection

Platform wrappers should check ABI compatibility before making render calls:

```c
uint32_t merman_abi_version(void);
const char* merman_package_version(void);
size_t merman_buffer_struct_size(void);
size_t merman_result_struct_size(void);
size_t merman_engine_result_struct_size(void);
size_t merman_host_text_measure_request_struct_size(void);
size_t merman_host_text_measure_result_struct_size(void);
```

- `merman_abi_version()` returns `MERMAN_ABI_VERSION`.
- `merman_package_version()` returns a static null-terminated string owned by Rust. Do not free it.
- The `*_struct_size()` functions return Rust-side struct sizes so hosts can catch packing or
  header/library mismatches at startup.

## Result Codes

| Code | Name | Meaning |
| ---: | --- | --- |
| 0 | `MERMAN_OK` | Success. |
| 1 | `MERMAN_INVALID_ARGUMENT` | Pointer/length combination or option value is invalid. |
| 2 | `MERMAN_UTF8_ERROR` | Source or options bytes are not valid UTF-8. |
| 3 | `MERMAN_OPTIONS_JSON_ERROR` | Options JSON could not be parsed. |
| 4 | `MERMAN_NO_DIAGRAM` | No Mermaid diagram was detected. |
| 5 | `MERMAN_PARSE_ERROR` | Mermaid parsing failed. |
| 6 | `MERMAN_RENDER_ERROR` | Layout, SVG, or postprocessing failed. |
| 7 | `MERMAN_UNSUPPORTED_FORMAT` | Requested output is not enabled or not implemented. |
| 8 | `MERMAN_PANIC` | Rust panic was caught at the ABI boundary. |
| 9 | `MERMAN_INTERNAL_ERROR` | Serialization, allocation, or other unexpected internal failure. |
| 10 | `MERMAN_RESOURCE_LIMIT_EXCEEDED` | Source, layout-model, label, or SVG resource budget was exceeded. |

## Memory Ownership

Every non-empty `MermanResult.data` returned by Rust must be freed with:

```c
void merman_buffer_free(MermanBuffer buffer);
```

Passing `{ NULL, 0 }` is a no-op. Double-free is caller misuse.

## Input Rules

- `source == NULL && source_len == 0` is accepted as empty source.
- `source == NULL && source_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- `options_json == NULL && options_len == 0` means defaults.
- `options_json == NULL && options_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- Non-empty source/options buffers must be UTF-8.

## Reusable Engine

Hosts that render many diagrams with the same `options_json` can create a reusable engine:

```c
MermanEngineResult merman_engine_new(
    const uint8_t* options_json,
    size_t options_len
);
void merman_engine_free(MermanEngine* engine);
```

When `code == MERMAN_OK`, `engine` is non-null and `data` is empty. The caller must release the
engine with `merman_engine_free`. When `code != MERMAN_OK`, `engine == NULL` and `data` contains the
same JSON error payload used by `MermanResult`.

The reusable-engine entry points capture the options at creation time:

```c
MermanResult merman_engine_render_svg(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_render_ascii(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_parse_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_layout_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_analyze_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_analyze_document_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len,
    const uint8_t* uri,
    size_t uri_len
);
MermanResult merman_engine_analyze_document_facts_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len,
    const uint8_t* uri,
    size_t uri_len
);
MermanResult merman_engine_validate_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
```

Passing `engine == NULL` returns `MERMAN_INVALID_ARGUMENT`. Engines may be shared across render
calls, but callers must not free an engine while another thread is using it.

### Host Text Measurement

Hosts that already own a font stack can install a text measurement callback on a reusable engine:

```c
MermanResult merman_engine_set_text_measure_callback(
    MermanEngine* engine,
    MermanHostTextMeasureCallback callback,
    void* user_data
);
```

The callback applies to future render/layout calls made through that engine. Passing
`callback == NULL` resets the engine to the text measurer selected by `merman_engine_new`
`options_json`.

`MermanHostTextMeasureRequest` string pointers are UTF-8 byte slices valid only for the duration of
the callback. The callback must not store them. `max_width` is meaningful only when
`has_max_width != 0`; `wrap_mode`, `direction`, and `white_space` are the corresponding
`MERMAN_*` constants. `line_height`, `letter_spacing`, and `word_spacing` are CSS-pixel values.

Return `handled=0` for measurement requests the host does not support. `merman` then falls back
to its vendored Mermaid-compatible measurer for that request. If an engine is used concurrently,
the callback and `user_data` must be thread-safe.

The callback is synchronous and runs on the render/layout call path. Do not block it on UI-thread
work, font loading, WebView JavaScript, platform channels, or another isolate. If the host cannot
answer from already-loaded font state or a prepared cache, return `handled=0` for that request.

For `MERMAN_WRAP_MODE_HTML_LIKE` requests with `has_max_width != 0`, hosts should measure the
natural no-wrap width first and only apply `max_width` when that natural width is larger. Returning
`max_width` for short labels can make diagrams wider than the browser or native preview surface
would make them.

This callback is the recommended accuracy path for native and embedded hosts. Headless rendering
cannot know the exact browser or UI toolkit font fallback chain, glyph shaping, hinting, and
subpixel rounding that will be used when the SVG is displayed. Hosts that need exact label layout
should measure with the same browser canvas/DOM, WebView, or native text system used for display,
then return those metrics through this callback. The C ABI only transports measurement requests and
fallbacks; it does not embed a system font engine. Platform guidance and test recommendations live
in [`HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md).

## SVG Rendering

```c
MermanResult merman_render_svg(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 SVG bytes. If the native library is built without the `render`
feature, this function returns `MERMAN_UNSUPPORTED_FORMAT`.

Passing `NULL/0` options returns Mermaid-parity SVG. Hosts that need export-safe SVG without adding
a raster byte ABI should pass:

```json
{ "svg": { "pipeline": "resvg-safe" } }
```

On error, `data` contains UTF-8 JSON:

```json
{
  "version": 1,
  "ok": false,
  "code": 6,
  "code_name": "MERMAN_RENDER_ERROR",
  "message": "layout failed: ..."
}
```

## ASCII Rendering

```c
MermanResult merman_render_ascii(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 terminal text. If the native library is built without the `ascii`
feature, this function returns `MERMAN_UNSUPPORTED_FORMAT`.

ASCII-specific behavior is configured through the shared `options_json.ascii` object. For example,
Class/ER relation-summary fallback diagnostics are disabled by default, but hosts can opt in when
they need support logs to explain why a dense relation graph used `relations:` output:

```json
{
  "ascii": {
    "charset": "ascii",
    "maxGridCells": 1,
    "relationSummaryDiagnostics": true
  }
}
```

When enabled, Class/ER summaries include a row such as
`reason: grid_budget actual=12 limit=1` under `relations:`. See
[`OPTIONS_JSON.md`](OPTIONS_JSON.md) for the full ASCII options shape.

## Options JSON

Pass `NULL/0` for defaults. Non-empty options use the shared tolerant JSON object documented in
`docs/bindings/OPTIONS_JSON.md`.

## Parse JSON

```c
MermanResult merman_parse_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 semantic model JSON. The current payload mirrors
`merman-cli parse` without `--meta`. If the native library is built without the `render` feature,
this function returns `MERMAN_UNSUPPORTED_FORMAT`.

## Layout JSON

```c
MermanResult merman_layout_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 layout JSON using the same `LayoutedDiagram` shape as
`merman-cli layout`. If the native library is built without the `render` feature, this function
returns `MERMAN_UNSUPPORTED_FORMAT`.

## Diagnostics-First Analysis JSON

ADR 0070 defines analysis JSON as the canonical diagnostics payload for validation, lint, Markdown
scanning, and LSP adapters. These symbols are part of the active ABI:

```c
MermanResult merman_analyze_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

MermanResult merman_engine_analyze_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);

MermanResult merman_analyze_document_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len,
    const uint8_t* uri,
    size_t uri_len
);

MermanResult merman_analyze_document_facts_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len,
    const uint8_t* uri,
    size_t uri_len
);

MermanResult merman_engine_analyze_document_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len,
    const uint8_t* uri,
    size_t uri_len
);

MermanResult merman_engine_analyze_document_facts_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len,
    const uint8_t* uri,
    size_t uri_len
);
```

These functions return `MERMAN_OK` when the analysis payload was produced. Diagram errors are
represented inside `data` as diagnostics. Transport errors such as invalid pointers, invalid UTF-8,
invalid options JSON, panics, and internal serialization failures remain non-zero
`MermanResult.code` values.

The default analyzer is expected to be render-free. Optional layout or render checks may be added
later behind feature/profile controls, but must use the same payload shape and report disabled
checks as diagnostics or profile metadata rather than changing the top-level transport contract.

Initial payload shape:

```json
{
  "version": 1,
  "valid": false,
  "summary": {
    "errors": 1,
    "warnings": 0,
    "infos": 0,
    "hints": 0
  },
  "source": {
    "kind": "diagram",
    "path": null,
    "diagram_index": null,
    "language": "mermaid"
  },
  "diagnostics": [
    {
      "id": "merman.parse.no_diagram",
      "severity": "error",
      "category": "parse",
      "message": "no Mermaid diagram detected",
      "code": 4,
      "code_name": "MERMAN_NO_DIAGRAM",
      "diagram_type": null,
      "span": {
        "byte_start": 0,
        "byte_end": 0,
        "line": 1,
        "column": 1,
        "end_line": 1,
        "end_column": 1,
        "lsp_range": {
          "start": { "line": 0, "character": 0 },
          "end": { "line": 0, "character": 0 }
        }
      },
      "related": [],
      "help": null
    }
  ]
}
```

`span` is optional. When present, `byte_start` and `byte_end` are canonical UTF-8 byte offsets in
the source slice passed to the analyzer. Line/column fields are 1-based for CLI output.
`lsp_range` is 0-based and UTF-16-oriented for language-server adapters. Markdown and MDX scanning
should map fence-local spans back to host-document ranges before returning diagnostics to callers
that asked for document analysis.

## Validation JSON

```c
MermanResult merman_validate_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

This ABI v2 function returns `MERMAN_OK` when the validation payload was produced. Invalid source is
reported in `data`:

```json
{
  "valid": false,
  "error": "no Mermaid diagram detected",
  "code": 4,
  "code_name": "MERMAN_NO_DIAGRAM"
}
```

Validation is a legacy compatibility projection over render-free diagnostics analysis. It does not
require the native library to be built with the `render` feature. The payload shape is kept for older
consumers: `valid` is derived from the diagnostics summary, and the top-level `error`, `message`,
`code`, and `code_name` fields mirror the first error diagnostic when present.
Bindings may add optional fields such as `version`, `summary`, or `diagnostics` while preserving the
legacy top-level fields. New integrations should prefer analysis JSON after the active package
exports it.

## Metadata JSON

```c
MermanResult merman_supported_diagrams_json(void);
MermanResult merman_ascii_capabilities_json(void);
MermanResult merman_diagram_family_capabilities_json(void);
MermanResult merman_lint_rule_catalog_json(void);
MermanResult merman_supported_themes_json(void);
MermanResult merman_supported_host_theme_presets_json(void);
```

`merman_supported_diagrams_json`, `merman_supported_themes_json`, and
`merman_supported_host_theme_presets_json` return UTF-8 JSON string arrays. Theme metadata reports
Mermaid core theme names and host/editor presets accepted by `options_json.host_theme.preset`. The
same buffer ownership rules apply.

`merman_ascii_capabilities_json` returns a UTF-8 JSON array of objects:

```json
[
  {
    "diagram_type": "gantt",
    "display_name": "Gantt",
    "support_level": "summary",
    "summary_fallback": false,
    "supported_semantics": ["titles", "sections", "tasks", "dates"],
    "limits": ["no terminal timeline geometry", "output is a readable task summary"],
    "evidence": [
      {
        "kind": "local_coverage",
        "source": "crates/merman-ascii",
        "note": "summary renderer is covered by ASCII tests"
      }
    ]
  }
]
```

Hosts should use `support_level` and `summary_fallback` to label ASCII rendering before invoking
`merman_render_ascii`. The support levels are `full`, `partial`, `summary`, and `unsupported`.

`merman_diagram_family_capabilities_json` returns a UTF-8 JSON array of objects:

```json
[
  {
    "diagram_type": "flowchart",
    "metadata_id": "flowchart",
    "has_semantic_parser": true,
    "has_render_parser": true
  }
]
```

`merman_lint_rule_catalog_json` returns a UTF-8 JSON array of objects from the shared
`merman-analysis` rule catalog:

```json
[
  {
    "id": "merman.authoring.flowchart.explicit_direction",
    "description": "Recommend explicit flowchart header directions and offer an insertion quickfix.",
    "evidence": [
      "repo-ref/mermaid/packages/mermaid/src/docs/syntax/flowchart.md",
      "crates/merman-core/src/diagrams/flowchart.rs",
      "docs/adr/0072-lint-rule-governance.md"
    ],
    "default_severity": "hint",
    "category": "semantic",
    "default_enabled": false,
    "default_profile": "recommended",
    "origin": "merman_authoring",
    "configurable": true,
    "fixable": true
  }
]
```

Hosts should use this catalog for rule-selection UI, config completion, and documenting whether a
rule is Mermaid-backed compatibility or a Merman authoring recommendation. `evidence` contains
local source, fixture, or ADR references that justify the rule classification.

This is diagnostic metadata for profile-aware hosts. `diagram_type` is the Mermaid parser/detector
id and may include aliases such as `flowchart-v2`; `metadata_id` is the public supported-diagram
id when the family contributes one.

## Threading

The first ABI is stateless. Calls may be made concurrently as long as callers obey buffer ownership
rules.
