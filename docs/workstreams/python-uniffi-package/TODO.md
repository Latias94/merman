# Python UniFFI Package - TODO

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

- [x] PUP-010 [owner=planner] [deps=none] [scope=docs/workstreams/python-uniffi-package]
  Goal: Freeze the Python UniFFI package lane around staged package generation plus real Python
  import/call smoke coverage.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md, and
  CONTEXT.jsonl exist and agree.
  Evidence: docs/workstreams/python-uniffi-package/DESIGN.md
  Context: docs/workstreams/python-uniffi-package/CONTEXT.jsonl
  Handoff: DONE. This lane intentionally avoids ASCII files and does not attempt PyPI wheel
  publishing.

## M1 - Package Scaffold And Generator

- [x] PUP-020 [owner=codex] [deps=PUP-010] [scope=platforms/python/merman,crates/merman-uniffi]
  Goal: Add a Python package staging scaffold plus a generator command that emits the generated
  UniFFI Python module and copies the built cdylib into the package module directory.
  Validation: cargo check -p merman-uniffi --features bindgen-smoke --examples
  Review: Generated Python files and native libraries must not be committed.
  Evidence: generator example plus package scaffold files.
  Context: docs/workstreams/python-uniffi-package/CONTEXT.jsonl
  Handoff: DONE. Added `platforms/python/merman` package scaffold, generated artifact ignores,
  and a `generate_python_package` example that emits Python bindings from the cdylib and copies the
  native library beside the generated module.

## M2 - Importable Python Smoke

- [x] PUP-030 [owner=codex] [deps=PUP-020] [scope=crates/merman-uniffi/tests/bindgen_smoke.rs]
  Goal: Extend the UniFFI smoke to stage an importable Python package and execute
  `MermanEngine.render_svg` and `MermanEngine.parse_json` from Python.
  Validation: cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
  Review: The smoke should fail on loader/API drift rather than only checking generated text.
  Evidence: nextest output recorded in EVIDENCE_AND_GATES.md.
  Context: docs/workstreams/python-uniffi-package/CONTEXT.jsonl
  Handoff: DONE. The bindgen smoke now stages a temporary Python package, imports it with Python,
  and executes `MermanEngine.render_svg` plus `MermanEngine.parse_json`.

## M3 - Documentation And Closeout

- [x] PUP-040 [owner=codex] [deps=PUP-030] [scope=docs/bindings,docs/workstreams/python-uniffi-package]
  Goal: Document the Python package flow, record verification evidence, and close the workstream.
  Validation: cargo fmt -p merman-uniffi -- --check && git diff --check
  Review: Docs must state current experimental status and wheel publishing follow-ons.
  Evidence: docs/bindings/PYTHON_UNIFFI.md and EVIDENCE_AND_GATES.md.
  Context: docs/workstreams/python-uniffi-package/CONTEXT.jsonl
  Handoff: DONE. Added Python binding docs, recorded evidence, and closed the lane with wheel
  publishing left as explicit follow-on work.
