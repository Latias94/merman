# Python UniFFI Package - Milestones

Status: Active
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

Exit criteria:

- Workstream documents exist and agree on the Python package target.
- ASCII work is explicitly out of scope.
- Python execution smoke is named as the main gate.

Status: complete.

## M1 - Package Scaffold And Generator

Exit criteria:

- `bindings/python/merman-uniffi` contains a package scaffold.
- A Rust generator example can generate Python bindings from a built cdylib into the package module
  directory.
- The native cdylib is copied beside the generated module.

Status: complete. The scaffold lives under `bindings/python/merman-uniffi`, and
`generate_python_package` can populate a package directory from the built cdylib.

## M2 - Importable Python Smoke

Exit criteria:

- `cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke` stages a
  temporary Python package.
- The staged package imports as `merman_uniffi`.
- Python calls `MermanEngine.render_svg` and `MermanEngine.parse_json` successfully.

Status: complete. The bindgen smoke imports the staged package and calls both SVG render and
semantic JSON parse from Python.

## M3 - Documentation And Closeout

Exit criteria:

- Binding docs explain the package staging command and generated artifact policy.
- Evidence logs include fresh pass/fail command results.
- Workstream is closed or has explicit follow-on lanes for wheel publishing.

Status: complete. Binding docs and evidence were updated; wheel publishing is split out as follow-on
work.
