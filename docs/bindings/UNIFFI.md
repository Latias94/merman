# UniFFI Bindings

Status: experimental generated-binding surface.

`merman-uniffi` exposes a small UniFFI object API over `merman-bindings-core`:

- `MermanEngine::new()`
- `MermanEngine::abi_version()`
- `MermanEngine::package_version()`
- `MermanEngine::render_svg(source, options_json)`
- `MermanEngine::render_ascii(source, options_json)`
- `MermanEngine::parse_json(source, options_json)`
- `MermanEngine::layout_json(source, options_json)`
- `MermanEngine::validate(source, options_json)`
- `MermanEngine::analyze_json(source, options_json)`
- `MermanEngine::analyze_document_json(source, options_json, uri)`
- `MermanEngine::analyze_document_facts_json(source, options_json, uri)`
- `MermanEngine::reusable_engine(options_json)`
- `MermanEngine::reusable_engine_with_text_measurer(options_json, measurer)`
- `MermanEngine::supported_diagrams()`
- `MermanEngine::ascii_capabilities()`
- `MermanEngine::supported_themes()`
- `MermanEngine::supported_host_theme_presets()`
- `MermanEngine::diagram_family_capabilities()`
- `MermanEngine::lint_rule_catalog()`
- `MermanEngine::configurable_lint_rule_catalog()`
- `MermanReusableEngine` render/parse/layout/validation methods
- `MermanReusableEngine` analysis and document-analysis methods
- `MermanReusableEngine::set_text_measurer(measurer)`
- `MermanReusableEngine::clear_text_measurer()`
- `MermanTextMeasurer` callback interface
- `MermanError::Binding { code, code_name, message }`

`MermanEngine::ascii_capabilities()` returns typed records with `diagram_type`, `display_name`,
`support_level`, `summary_fallback`, `supported_semantics`, `limits`, and `evidence`, so generated
bindings can label `full`, `partial`, and summary-only ASCII support before rendering.

The C ABI in `merman-ffi` remains the canonical low-level protocol. UniFFI is a convenience layer for
Swift, Kotlin, Python, and Ruby package lanes.
The optional `options_json` argument uses the shared contract documented in
`docs/bindings/OPTIONS_JSON.md`.
That contract includes the shared `lint` section for profiles, explicit rule enable/disable, and
severity overrides, so UniFFI, CLI lint, FFI, and WASM can all drive the same analysis behavior.
`lint_rule_catalog()` returns the same rule ids, evidence references, profiles, origins,
configurability, and fixability exposed by the other metadata surfaces. Merman authoring
recommendations remain opt-in through `recommended` or explicit rule enablement.

## Analysis And Validation

`validate` is the current compatibility method. It returns the legacy validation payload with
top-level `valid`, `error`, `message`, `code`, and `code_name` fields.

ADR 0070 makes diagnostics-first analysis the canonical method for linting, CI, editor integrations,
and future LSP adapters. UniFFI exposes it as JSON rather than inventing a UniFFI-only diagnostic
record model. Generated bindings may add typed helpers later, but the JSON payload should remain
byte-for-byte compatible with the C ABI and WASM surfaces.

`validate` is implemented as a projection over analysis diagnostics so existing package users keep
working while new integrations can consume `analyze_json`, `analyze_document_json`, or
`analyze_document_facts_json`.

Generated bindings use Merman's built-in headless measurer by default. Hosts that need DOM,
WebView, Core Text, Android, Flutter, or another platform font stack can use
`MermanReusableEngine` with `MermanTextMeasurer`, either at construction time through
`MermanEngine::reusable_engine_with_text_measurer` or later through
`MermanReusableEngine::set_text_measurer`. `MermanReusableEngine::clear_text_measurer` restores the
engine's original built-in measurer. The callback should return `None` for unsupported or uncached
requests so Merman can fall back to vendored metrics.

## Bindgen Smoke

Run:

```bash
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

This test builds the `merman-uniffi` cdylib, generates Python bindings from the embedded UniFFI
metadata into a temporary package, copies the native library beside the generated module, imports
the package with Python, and calls `MermanEngine.abi_version`, `MermanEngine.package_version`,
`MermanEngine.render_svg`, `MermanEngine.render_ascii`, `MermanEngine.parse_json`,
`MermanEngine.layout_json`, `MermanEngine.validate`, metadata methods,
`MermanEngine.analyze_document_json`, `MermanEngine.analyze_document_facts_json`,
`MermanEngine.ascii_capabilities`, `MermanEngine.diagram_family_capabilities`,
`MermanEngine.reusable_engine_with_text_measurer`,
`MermanReusableEngine.analyze_document_json`,
`MermanReusableEngine.analyze_document_facts_json`, `MermanReusableEngine.set_text_measurer`,
`MermanReusableEngine.clear_text_measurer`, plus a `MermanError.Binding` error-path check.

Generated Swift, Kotlin, Python, or Ruby files are not committed by this lane. Platform-specific
package layouts should be split into follow-on lanes.

## Other UniFFI Targets

UniFFI 0.31.1 can generate Kotlin, Python, Ruby, and Swift bindings. Merman currently ships only the
Python package scaffold through UniFFI. Android/Kotlin and Apple/Swift already have C ABI wrappers
under `platforms/android` and `platforms/apple`, so generated UniFFI Kotlin or Swift should be a
deliberate follow-on if the generated API is meant to replace or supplement those wrappers. Ruby is
not currently a release surface.

## Python Package Scaffold

See `docs/bindings/PYTHON_UNIFFI.md` for the current Python package layout and local generation
command. The package scaffold lives under `platforms/python/merman`; generated Python source
and native libraries are ignored.
