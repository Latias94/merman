# FFI API — Milestones

Status: Active
Last updated: 2026-05-30

## M0 — Architecture Decision And Scope

Exit criteria:

- ADR 0066 is the authority for C ABI plus optional UniFFI.
- `DESIGN.md` documents in-scope and out-of-scope surfaces.
- `TODO.md` has vertical slices with explicit validation gates.
- `WORKSTREAM.json` references the ADR and current documents.

## M1 — First C ABI Render Slice

Exit criteria:

- `crates/merman-ffi` exists and builds.
- `merman_render_svg` or the final equivalent exports from a dynamic/static library.
- Inputs validate null pointers, byte lengths, UTF-8, and options payloads.
- Outputs use owned buffers with explicit free.
- Panic containment is tested.

## M2 — Public Protocol

Exit criteria:

- A public header is checked in or generated reproducibly.
- `docs/bindings/FFI_PROTOCOL.md` describes result codes, buffers, options, and compatibility rules.
- Header compile/link smoke tests pass.
- Parse JSON and layout JSON use the same result/buffer/error policy as SVG.

## M3 — Optional Binding Layers

Exit criteria:

- RaTeX math and raster behavior are feature-gated if exposed.
- UniFFI is either prototyped over the same safe facade or explicitly split into a follow-on.
- No generated binding layer becomes the only ABI authority.

## M4 — Release Candidate Closeout

Exit criteria:

- Focused tests, clippy, formatting, and header smoke tests pass.
- Documentation includes a minimal C example.
- Workstream evidence is fresh.
- Remaining platform packages are split into narrower follow-ons.
