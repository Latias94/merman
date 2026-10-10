# Sequence actor composed shadow consumer (U7)

Scope: ordinary Sequence participant rectangles, including enabled bottom mirrors.
This increment does not qualify the complete Sequence reference scene or close C7a.

## Behavior

- Static unqualified/Default Actor effect rules and Actor bindings use the existing ordered,
  zero-spread shadow implementation. The public Cyberpunk graph retains its 8/16 sigma stages.
- Rule ownership and explicit Clear suppress the binding. Matching ordinal effects, custom
  classes, composite glyphs, unsupported graphs and unknown relative stroke widths keep residuals.
- Definitions attach to the actual rectangle only. Labels and lifelines are separate terminals.
- The writer records a filter application only after output succeeds. Missing candidates or
  absent resolution cannot be promoted to Applied or NotApplicable.
- Root bounds include the outward-rounded filter region with the effective source/typed stroke.
  No layout positions are changed. Materialization uses existing resource admission and work metering.
- Native filters use the existing `-theme-effect-` ID convention and existing exporter validation.
- Reuse the renderer node index and rectangle coordinate function; no second whole-layout index,
  dependency, embedded font, public schema version or resource-budget increase.

## Evidence

The pre-implementation public-preset regression failed with one remaining Actor effect residual:
`/tmp/sequence-actor-shadow-red.log`. An early focused run passed nine tests and failed one invalid
fixture color (`cyan`); the test now uses the protocol's accepted hex representation.

Chromium 151.0.7922.34 consumed a freshly linked Release renderer output from the public preset:
four rectangles have 3px borders, 10px radii and real filters; text/lines have none. Each individual
filter restoration changes painted screenshot bytes. The screenshot was inspected. This uses host
fonts and is not a same-font reference comparison. Recipe fingerprint remains
`1304dfbf2ec85f464d2dd8c685e2023b84cd9e2d6268ab11b529d531de6e9767`.

Artifacts: `target/bench/experiments/sequence-actor-shadow-18959affb/`, including source/rlib hashes,
SVG, screenshots, capture source and browser observations.

## Verification

- Renderer Release nextest (`merman-render`, `layout-cytoscape,embedded-fonts`, library,
  `sequence_svg_test`, `theme_support_discovery_test`): 2,842 passed, 3 skipped.
- Added ordinal/resource boundary test and re-ran all four Actor shadow integration tests:
  4 passed, 84 filtered out. The initial test compile used `unwrap_err` on a non-Debug success
  type; replacing it with `err().expect(...)` fixed the test harness without production changes.
- Native Release nextest (`merman`, `svg,png,pdf,layout-cytoscape,embedded-fonts`,
  `theme_composed_effects`): 6 passed, 0 skipped. Existing State, Flowchart and XY behavior remains
  covered. Sequence uses both color spaces and SourceGraphic/Previous composition; PNG colored
  pixels distinguish the shadow stages. Public preset PNG/PDF admission must be Portable or
  HostDependent, with exactly four filters, four references and eight shadow stages.
- Standalone native facade build, without test dependencies: captured PNG/PDF via the public
  `Renderer` and inspected the PNG. Both report HostDependent solely for system-font use and
  retain the same four-filter/eight-stage receipt. The artifact manifest and rlib hash are
  recorded in the experiment's `capture-contract.json`.
- Minimal-profile SVG (`merman-render`, no defaults, only `layout-cytoscape`): all 11 Actor
  geometry/shadow tests passed, with 73 other tests filtered out. No embedded-font feature.
- Release Clippy (`merman-render`, `layout-cytoscape,embedded-fonts`, library): succeeded,
  retaining 160 warnings in the existing codebase. Minimal-profile test compilation also
  retains an unused FontStack/FontStyle import warning; no warning-free claim is made.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- PDFium rasterized the freshly captured PDF at 96dpi. The image was inspected: all four
  participant glows are visible. The local library path/hash and raster size are recorded in
  `pdfium-observations.json`. This is visual inspection, not a calibrated reference comparison.

Logs are `/tmp/sequence-actor-shadow-{renderer,boundaries,native,browser,minimal,clippy}.log`.
The standalone build manifest is `/tmp/sequence-actor-shadow-native-artifacts.jsonl`.
They are local experiment evidence, not a release candidate receipt.

## Review

The actual simplification pass ran reuse, quality and efficiency lenses serially in one
independent Codex context because agent capacity was occupied. Applied two suggestions: reuse
`node_left_top` and the existing node index. It also found the initial filter ID did not follow
the native scanner convention; this was corrected before native validation. No shared proof
framework or additional fallback machinery was added.

The separate code review checked eight applicable lenses serially in one independent Codex
context and found no remaining actionable defects. It includes the final native/minimal-profile
checks and keeps this increment separate from full U7. Review artifacts are under
`/tmp/compound-engineering-501/ce-code-review/20260919-072905-21667484/`; timestamps in local
artifact names are retained as generated.

## Limits

The public recipe currently requests only a subset of the complete reference Sequence scene.
Accepting its current declared facets does not prove missing message, note, loop, lifeline and
text styling/effects. Complete scene PNG/PDF visual comparison, other profiles/hosts, catalog
qualification and performance/footprint closure remain open under the active plan.

The native screenshot also makes the remaining message/marker work visible: its message
arrowheads appear gray while the browser capture shows cyan. This is outside the newly attached
Actor filters; the next U7 message/marker tranche must establish ownership and native pixel
expectations rather than treating the current actor-only checks as full-scene acceptance.
