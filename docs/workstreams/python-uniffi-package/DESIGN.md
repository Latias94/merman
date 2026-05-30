# Python UniFFI Package

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

`merman-uniffi` can already generate a Python binding source file from the Rust cdylib metadata, but
the previous lane stopped before proving the package shape that a Python user actually imports. A
generated Python file alone is not enough: the cdylib must be placed where the generated loader
expects it, and a smoke test must execute a real `MermanEngine` call from Python.

## Relevant Authority

- ADRs:
  - `docs/adr/0066-ffi-binding-strategy.md`
- Existing docs:
  - `docs/bindings/UNIFFI.md`
  - `docs/release/PUBLISH_ORDER.md`
- Prior workstreams:
  - `docs/workstreams/uniffi-bindings`
  - `docs/workstreams/workspace-release-versioning`
- Local reference:
  - `repo-ref/RaTeX/docs/binding-architecture.md`

## Problem

The current Python UniFFI proof only checks generated source text. That misses the two failure modes
that matter for a real package:

- Python cannot load the native cdylib because it is not adjacent to the generated module.
- The generated wrapper imports but does not successfully call into the Rust engine.

## Target State

- A documented Python package layout exists for generated UniFFI bindings.
- The package layout keeps the generated Python module and platform cdylib in the same import
  package, matching UniFFI's Python loader behavior.
- A repeatable generator command can populate the package staging directory from a built
  `merman-uniffi` cdylib.
- A Python smoke test imports the staged package and calls `render_svg` and `parse_json`.
- Generated Python and native binaries remain generated artifacts, not committed source.

## In Scope

- Python package staging under `bindings/python/merman-uniffi`.
- A small Rust generator example using UniFFI bindgen APIs already present behind
  `bindgen-smoke`.
- Integration smoke coverage in `crates/merman-uniffi/tests/bindgen_smoke.rs`.
- Binding docs and workstream evidence.

## Out Of Scope

- Publishing wheels to PyPI.
- Building manylinux, macOS universal, or Windows wheel matrices.
- Replacing the C ABI or changing the `merman-uniffi` public API.
- Android, iOS, Flutter, React Native, Node, or Ruby packaging.
- ASCII renderer changes.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| UniFFI Python loads the native library from the generated module directory. | High | UniFFI 0.31.1 Python template uses `os.path.dirname(__file__)` while loading the cdylib. | Package staging must be adjusted to put the native binary beside the generated module. |
| Python is available for local smoke verification. | High | `python --version` reports Python 3.13.11 on 2026-05-30. | The Rust source-generation gate can still pass, but the Python execution gate is blocked. |
| The first package deliverable should be a local staging/package scaffold, not a publish-ready wheel matrix. | High | Workspace crate publishing itself still has upstream crates.io ordering constraints. | Split wheel CI and PyPI metadata into a later release lane. |
| Generated Python and copied cdylibs should not be committed. | High | Prior `uniffi-bindings` lane kept generated platform source out of git. | Add ignore rules or documentation if staging output risks accidental commits. |

## Architecture Direction

Use UniFFI's generated Python module as the low-level package implementation:

```text
cargo build -p merman-uniffi
        |
        v
target/<profile>/merman_uniffi.{dll,so,dylib}
        |
        v
UniFFI bindgen example
        |
        v
bindings/python/merman-uniffi/src/merman/
  __init__.py                 stable package shim
  merman_uniffi.py            generated UniFFI module
  merman_uniffi.{dll,so,dylib} native library beside generated module
```

The stable `__init__.py` should re-export the generated API and fail with a clear message if the
generated files have not been staged. This gives Python users a normal `import merman` entry
point while keeping generated files out of source control.

## Closeout Condition

This lane can close when:

- the package scaffold and generator command exist,
- the smoke test imports a staged Python package and calls Rust through the generated wrapper,
- docs explain the current package layout and wheel follow-ons,
- evidence records fresh passing commands,
- and no ASCII files are modified by this lane.
