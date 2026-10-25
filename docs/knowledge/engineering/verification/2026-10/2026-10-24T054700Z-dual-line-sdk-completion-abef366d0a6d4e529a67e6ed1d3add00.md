---
type: "Verification Evidence"
title: "Dual-line SDK completion"
description: "Verification Evidence for Dual-line SDK completion."
timestamp: 2026-10-24T05:47:00Z
record_id: "abef366d0a6d4e529a67e6ed1d3add00"
producer_id: "codex-sdk"
run_id: "headless-sdk-20261024"
related_plan: "docs/plans/2026-10-24-0925-feat-headless-sdk-plan.md"
related_issue: "243"
---

# Verification

Plan: `docs/plans/2026-10-24-0925-feat-headless-sdk-plan.md`.

- Main branch: `feat/headless-sdk-20261024`, based on `83e3ea74b`;
  qualified runtime source `b6b2c352467a850da69f5ae957adc7152b4ba11c`.
  `0ffbf7cf7` subsequently changes only documentation, line-ending attributes and
  verification records, not the qualified runtime.
- Maintenance branch: `feat/headless-sdk-09-20261024`, based on `8eabde972`;
  final runtime source `f26cb764895d558e16ccbbe4de399d771fc01665`.
  Its earlier `f60d7fe49` standard previews are historical, not final math/full evidence.

# Result

Both lines have six locally qualified SDK archives: native C on macOS ARM64, and
WASI in two independent real hosts. All implementation units and local acceptance
examples are complete. This does not claim cross-platform remote qualification or
publication. Both work lines remain local feature branches, not merged release branches.

| Plan scope | Evidence |
| --- | --- |
| U1 / R1, AE1 | Main `0c466e71f`, maintenance `1499bcd88`; complete text wrapping regressions and each line's full SVG structure gate |
| R2, R11 / U6 | Maintenance retains Mermaid 12.1, Options schema 2, original C ABI prefix, implicit native codec font discovery and legacy constrained math; main retains its separate Options schema 3 and policy |
| U2-U4, U7 / R3-R7, AE2 | Bounded copied fonts, shared measurement/painting, independent WASI ABI 1, appended C ABI 3 constructor; ordinary errors, result ownership, reuse and no-preopen real host smokes |
| U5 / R4, R8-R10, AE3-AE4 | Six canonical profiles, actual runtime catalogs and exact dependency reports, archived CMake/pkg-config consumers, manifests and checksums; final artifact evidence below |

# Evidence

Maintenance final-source owner checks:

```text
cargo nextest run --locked -j2 -p merman-render --features all-diagrams,layout-cytoscape,layout-elk,math --lib --test math_native_svg_test --test math_edge_background_test --test swimlane_svg_test --test svg_fallback_transform_test -E 'binary(math_native_svg_test) | binary(math_edge_background_test) | binary(swimlane_svg_test) | binary(svg_fallback_transform_test) | test(svg::fallback) | test(math::tests)' --no-fail-fast
cargo nextest run --locked -j2 -p merman-export -p merman-bindings-core -p merman-ffi -p merman-wasi --all-features --no-fail-fast
cargo run --locked -j2 --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

- Results: 128 focused renderer tests and 549 shared-owner tests pass; six existing
  host-managed font-catalog tests remain explicitly ignored. The exact complete
  structure gate passes after the final math/cascade fixes, without comparator edits.
- Main's final all-feature bindings-core and FFI runs pass 402 and 74 tests;
  earlier renderer, font, WASI, generated-contract, feature-matrix, dependency and legal
  owner evidence remains in the linked main verification record.
- Maintenance's representative feature matrix, exact dependency closures, generated
  ABI/binding projections, 13 license reports and 406 legal projection files passed
  before the renderer-only math repair. That repair adds no dependency or feature input;
  exact artifact reports are nevertheless regenerated for the final archives.
- Maintenance final narrow Python contracts pass 59 tests; real host adapters pass
  15 Python tests and the Go tests. The preceding broader maintenance/main Python
  contract sets passed 188/212 tests. Workflow analyzers passed before the final
  renderer-only changes; no workflow was executed remotely.
- Git's actual `core.autocrlf=true` filters preserve LF and exact content for all three
  public SDK headers on both lines and the production OFL license. This is not a Windows
  compiler qualification. Formatting, path lengths and diff whitespace checks pass.

Every Cargo build ran serially with two jobs and the shared existing target directory.
Python adapters use wasmtime 50.0.1 and Pillow 13.0.0; Go uses wazero 1.12.0.
The verified local cargo-about 0.9.2 executable is isolated under `target/tools`;
the user's global installation was not replaced.

Final artifact qualification:

- Main archives: `target/sdk-qualified/b6b2c352467a/archives/` in the main checkout;
  maintenance archives: `target/sdk-qualified/f26cb764895d/archives/` in the
  maintenance checkout. Each neighboring `input/` contains the exact staged bytes,
  raw/runtime catalogs, legal reports, manifests and host evidence.
- Both six-profile sequences end with `ALL_LOCAL_SDKS_PASS`. Every archive passes
  the existing `sdk_archive.py verify` checks with its exact source commit and
  local version 0.9.2. An independent final pass checks all twelve archive checksums.
- All ten line/flavor/fixture combinations have byte-identical SVG across native,
  wazero and wasmtime. The existing qualification harnesses compare decoded RGBA
  pixels for basic fixtures and, in math/full, actual formula fixtures.
- Each native archive is extracted and its shared/static CMake consumers build and
  pass both CTests. Staging normalizes and ad-hoc signs the macOS dylib before hashes
  are captured. `otool` and actual dyld tracing verify loading from the extracted
  archive, not the producer's build tree.
- Both WASI flavors with math pass actual formula SVG/PNG execution, runtime
  import/export audit, no-preopen policy and each line's own constrained-resource
  behavior. Standard remains free of math/ELK; math remains free of ELK. These are
  runtime-catalog and dependency-closure facts, not inferred product names.

# Follow-up

- Native Linux, Intel macOS and Windows remain unexecuted locally. The read-only SDK
  producer and immutable preflight own the 18 profile/target combinations. GNU Linux
  currently qualifies Ubuntu 24.04 glibc, not older glibc or musl. Platform-only ignored
  system-font tests keep their controlled-host owner.
- Review closure is recorded in `bf027ae9b62c4f6a88b33d3c3198095f`: five concrete
  maintenance math findings were fixed and rechecked. No agent remains open.
- The workspace version remains 0.9.2. Local archives are previews, not replacements
  for immutable v0.9.2 assets. Choose the next release version and run the exact-source
  remote qualification before any publication; publishing, pushes, tags, registry
  operations and GitHub issue/PR mutations are not authorized by this execution.
- DrawingList is excluded. Existing legacy memory rollups are not adopted or rewritten;
  their prior validation problems do not justify changing unrelated history.
  The final bundle validation still exits 1 for historical records without required
  frontmatter; no error identifies these new SDK shards. The legacy root views also
  remain unadopted. This is a memory-store maintenance limit, not a passing SDK gate.

# Citations

- `verification/2026-10/2026-10-24T045011Z-sdk-final-main-verification-5c879877f58241728f7b34816d64945a.md`.
- `subagents/2026-10/2026-10-24T054320Z-maintenance-native-math-review-bf027ae9b62c4f6a88b33d3c3198095f.md`.
- `distribution/sdk/README.md`, `platforms/wasi/README.md`,
  `docs/release/PACKAGE_SURFACES.md`, `docs/release/RELEASING.md`.
- Local owner logs: `/tmp/merman-sdk-09-native-math-final-green.log`,
  `/tmp/merman-sdk-09-native-math-final-owners.log`,
  `/tmp/merman-sdk-09-native-math-final-svg-gate.log`,
  `/tmp/merman-sdk-09-artifacts-final-qualified.log`.
