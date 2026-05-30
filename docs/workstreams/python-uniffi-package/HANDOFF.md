# Python UniFFI Package - Handoff

Status: Closed
Last updated: 2026-05-31

## Current State

This lane is closed. Prior UniFFI work already provides `MermanEngine` over `merman-bindings-core`;
this lane added the Python package scaffold and proved that the generated module plus cdylib can be
imported and called from Python.

Confirmed:

- `bindings/python/merman` contains a package scaffold and generated-artifact ignores.
- `generate_python_package` can populate a package directory from a built `merman-uniffi` cdylib.
- `bindgen_smoke` stages a temporary package, imports `merman`, and calls
  `MermanEngine.render_svg` plus `MermanEngine.parse_json`.
- `docs/bindings/PYTHON_UNIFFI.md` documents the local generation flow and current limits.

## Remaining Work

No remaining task belongs to this lane. Follow-on lanes:

- PyPI/wheel publishing matrix.
- Android JNI/Kotlin wrapper.
- iOS Swift Package/XCFramework wrapper.
- Flutter/Dart FFI wrapper.

## Guardrails

- Do not touch `crates/merman-ascii` files.
- Do not commit generated Python binding files or copied cdylibs.
- Do not publish crates or Python packages from this lane.
- Keep wheel matrix and PyPI publishing as follow-on work unless explicitly requested.
