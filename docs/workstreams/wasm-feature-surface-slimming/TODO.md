# WASM Feature Surface Slimming -- TODO

Status: Open
Last updated: 2026-06-10

## M0 -- Evidence And Contract Freeze

- [x] WFS-010 [owner=planner] [deps=none] [scope=docs/workstreams/wasm-feature-surface-slimming,docs/adr]
  Goal: Freeze host targets, dependency evidence, import evidence, package-surface terminology, and
  ADR changes required before feature edits.
  Validation: `git diff --check -- docs/workstreams/wasm-feature-surface-slimming`
  Review: Confirm that "browser WASM" and "Typst/pure WASM" are documented as separate surfaces.
  Evidence: `docs/workstreams/wasm-feature-surface-slimming/DESIGN.md`;
  `docs/workstreams/wasm-feature-surface-slimming/EVIDENCE_AND_GATES.md`.
  Handoff: Start here before touching Cargo features.

- [x] WFS-020 [owner=codex] [deps=WFS-010] [scope=crates/xtask,docs/workstreams/wasm-feature-surface-slimming]
  Goal: Add or script repeatable dependency/import measurement gates for feature profiles.
  Validation: profile cargo-tree checks; `wasm-tools print` import allowlist checks; size snapshot
  output stored outside the repo or in a generated report.
  Review: The gate should fail on `__wbindgen_placeholder__`, `js-sys`, WebCrypto, or JS Date in
  pure-wasm outputs.
  Evidence: `xtask profile-budget` landed with dependency, import, export, and wasm size checks for
  pure-wasm and typst-wasm profiles.
  Handoff: This task should not change production behavior.

## M1 -- Host Capability Profile

- [x] WFS-030 [owner=codex] [deps=WFS-020] [scope=crates/merman-core/src/runtime.rs,crates/merman-core/src/lib.rs,crates/merman-bindings-core/src/common.rs]
  Goal: Replace implicit local-time behavior in core parsing with explicit host time capabilities
  while preserving existing native defaults.
  Validation: `cargo nextest run -p merman-core gantt`; fixed-time binding tests;
  pure-wasm import probe.
  Review: Full/native behavior may keep local time; pure profiles must not link JS Date or system
  current-time paths.
  Evidence: `host-clock` now owns `chrono/clock`; `merman-core --no-default-features` keeps
  deterministic UTC fallback behavior and no longer pulls `chrono` through `js-sys` or
  `wasm-bindgen` on `wasm32-unknown-unknown`.

- [x] WFS-040 [owner=codex] [deps=WFS-020] [scope=crates/merman-core/src/diagrams/block.rs,crates/merman-core/src/diagrams/git_graph.rs,crates/merman-core/src/runtime.rs]
  Goal: Replace `Uuid::new_v4` render-model/parser use with deterministic or host-provided ID
  generation where required.
  Validation: block/gitGraph semantic tests; pure-wasm import probe has no WebCrypto imports.
  Review: Mermaid parity for visible output must remain stable; random-looking IDs should be
  deterministic under pure profiles.
  Evidence: `host-random` now owns UUID v4 IDs. `merman-core --no-default-features` no longer
  depends on `uuid` or `getrandom`; generated block and gitGraph IDs are deterministic in
  no-host-random profiles.

- [x] WFS-050 [owner=codex] [deps=WFS-030] [scope=crates/dugong,crates/manatee,crates/merman-render,crates/merman-core]
  Goal: Fence timing instrumentation so `web-time`/JS performance imports are not linked into pure
  wasm profiles.
  Validation: render/core timing tests where present; import allowlist gate.
  Review: Do not remove native debug/timing capability; make it profile-owned.
  Evidence: `host-timing` now owns core parse timing instrumentation and optional `web-time`.
  `merman-core --no-default-features` passes pure-wasm and typst-wasm dependency gates.

## M2 -- Core Dependency Slimming

- [x] WFS-060 [owner=codex] [deps=WFS-020] [scope=crates/merman-core/src/preprocess,crates/merman-core/Cargo.toml]
  Goal: Split frontmatter YAML and directive JSON5 support from the minimal parser profile.
  Validation: default/full preprocessing tests still pass; minimal profile tests define documented
  unsupported behavior or a lighter parser path.
  Review: Preserve Mermaid-compatible defaults; do not silently drop config overrides in full mode.
  Evidence: `full-config` now owns `serde_yaml` and `json5`; no-default core uses a small inline
  metadata/directive parser and strips closed YAML frontmatter without applying title/config.
  no-default wasm dependency tree has no matches for `serde_yaml`, `unsafe-libyaml`, `json5`, or
  `pest`.

- [x] WFS-070 [owner=codex] [deps=WFS-020] [scope=crates/merman-core/src/sanitize.rs,crates/merman-core/src/utils.rs,crates/merman-core/Cargo.toml]
  Goal: Feature-gate DOM-style sanitization and URL canonicalization dependencies in the minimal
  profile without weakening default/full security behavior.
  Validation: sanitization tests for full/default; minimal profile tests for documented safe fallback
  behavior; cargo tree delta for `lol_html`, `url`, and ICU/idna transitive dependencies.
  Review: Security behavior changes need ADR notes.
  Evidence: `full-sanitization` now owns `lol_html` and `url`; no-default core uses conservative
  HTML escaping while preserving Mermaid `<br/>` tags and keeps dangerous URL protocol filtering.
  pure-wasm and typst-wasm dependency budget gates reject `serde_yaml`, `json5`, `lol_html`, `url`,
  and JS/host crates.

- [x] WFS-080 [owner=codex] [deps=WFS-030,WFS-040,WFS-060] [scope=crates/merman-core/src/family.rs,crates/merman-core/src/diagram]
  Goal: Make diagram family facts project profile-specific detector/parser/render registrations.
  Validation: default/full registry tests; tiny/minimal registry tests; supported diagram metadata
  tests through bindings.
  Review: Keep feature decisions centralized; avoid scattering `cfg` checks in adapters.
  Evidence: `family.rs` now projects semantic parser facts, typed render parser facts, and
  supported diagram metadata from `BaselineRegistryProfile`. Tiny/no-default excludes the
  full-only large-feature registrations (`mindmap`, `architecture`, `flowchart-elk`) while keeping
  common flowchart/parser aliases. Validation:
  `cargo fmt --check -p merman-core`;
  `cargo check -p merman-core`;
  `cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown`;
  `cargo nextest run -p merman-core registry`;
  `cargo nextest run -p merman-core --no-default-features registry`;
  `cargo nextest run -p merman-bindings-core metadata`.

## M3 -- WASM Package Surfaces

- [ ] WFS-090 [owner=codex] [deps=WFS-030,WFS-040,WFS-050,WFS-080] [scope=crates/merman-wasm,platforms/web,docs/release]
  Goal: Split browser WASM package variants or feature presets so consumers can choose core-only,
  render, ascii, and full browser bundles.
  Validation: `cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown`
  for each preset; npm TypeScript build/smoke if package wrappers change.
  Review: Browser package may keep wasm-bindgen; the API must label it as browser/JS WASM.
  Evidence: initial `xtask wasm-size-matrix` landed and recorded raw/stripped size tables for
  browser and Typst presets separately. `docs/release/PACKAGE_SURFACES.md` and
  `crates/merman-wasm/README.md` now label `merman-wasm` as the browser/wasm-bindgen surface.
  Remaining work: use the matrix to decide public browser package variants and wire the
  TypeScript/npm wrapper changes.

- [ ] WFS-100 [owner=codex] [deps=WFS-030,WFS-040,WFS-050,WFS-080] [scope=crates/merman-typst,docs/release,docs/bindings]
  Goal: Add an experimental Typst/wasm-minimal-protocol transport or probe crate with no
  wasm-bindgen dependency.
  Validation: wasm import allowlist contains only `typst_env` protocol imports; exported `memory`
  exists; a wasmi or Typst smoke call returns SVG/JSON bytes for the admitted subset.
  Review: Start with a small admitted subset instead of pretending full Mermaid parity is already
  Typst-ready.
  Evidence: `wasm-tools print` import/export snapshot and smoke output.

## M4 -- Release And Compatibility

- [ ] WFS-110 [owner=planner] [deps=WFS-090,WFS-100] [scope=docs/adr,docs/release,README.md]
  Goal: Record public package semantics, feature defaults, migration notes, and release gating.
  Validation: docs diff check; package surface docs include browser and Typst/pure wasm as distinct
  entries.
  Review: Any changed default feature behavior must have an ADR.
  Evidence: release docs and ADR updates.

- [ ] WFS-120 [owner=codex] [deps=WFS-110] [scope=workspace]
  Goal: Final verification and closeout.
  Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-core -p merman-render -p merman-bindings-core`; browser wasm preset builds; Typst/pure wasm import allowlist gate.
  Review: Summarize residual unsupported families and split follow-on work if needed.
  Evidence: closeout journal and updated `WORKSTREAM.json`.
