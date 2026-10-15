# Flowchart HTML node-label effects

Date: 2026-09-19. Base: `ccf07bfaa`. Status: scoped validation complete.
Scope: the ordinary HTML NodeLabel increment of U6 in
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.

## Consumer boundary

Ordinary classic Process, RoundedRectangle, Diamond, Circle and DoubleCircle labels can now
apply the existing shared shadow graph to HTML glyphs. The writer leaves its empty SVG rectangle
outside the filtered group and puts only the transparent foreignObject inside. Filter IDs,
application counts, source ownership, Clear reconciliation and viewport outsets use existing owners.

Preparation uses layout-owned label width/height, the same effective font style as the writer and
its top-left label translation. A one-em host allocation covers ordinary glyph overhang; the shared
effect supplies the additional shadow outsets. This is an allocation policy, not exact browser ink
measurement. Native export still checks actual projected glyph/filter/root containment and actual
filter receipts. Custom host measurements cannot authorize this allocation: the existing built-in
Wrapped, WrappedWithRawWidth and ComputedLength carriers must be available.

The exact sanitizer used by emission supplies the examined HTML. This increment accepts only
attribute-free p/span/br markup and visible text. Math, rich/attributed markup, specialized shape
placement and prepared embedded-font HTML remain outside this consumer. Complex input retains a
residual rather than filtering images/backgrounds or reporting a successful text-only application.

HTML class rules differ from native shape rules: generated `.class span` rules can paint a label
background or change its geometry. Preparation checks the actual assigned/default/node classes,
including the generated edgeLabel/icon-shape/image-shape paragraph backgrounds. Existing source
CSS parsing is reused for a bounded property set; it does not interpret general CSS. Subgraph
classes are not treated as inherited: their title/cluster groups are siblings of node groups in the
actual writer. Positive tests preserve this distinction. Added source scans are work-metered and
run only for requested winning HTML effects; no persistent HTML cache was added.

There is no new dependency, embedded font, public schema or public preset recipe change. EdgeLabel
HTML backgrounds still require separate terminal treatment. This record does not close U6, U9,
C7a, or qualify the complete public Cyberpunk scene.

## Verification

- Proof-first positive failed with `UnverifiedFamilyTheme` before the consumer was implemented;
  the rich/source-style negative passed. A math fixture was removed from this particular test
  because missing math capability is rejected during preparation, before the consumer under test.
- First scoped Release run passed 43/43 before the final Clear/class-boundary additions.
- Extending Clear coverage exposed the old native-only Clear admission condition. Eligible HTML
  Clear now retains a separate prepared state until the real writer emits the label; it adds no
  filter application or viewport extent. Rich/source-class/special-shape negatives cover both a
  shadow request and Clear. The historical intermediate 2,662-test run is not final-source evidence.
- Final renderer/facade Release regression passed **2,662/2,662**, with two existing skips.
  It includes the final Effect/Clear negative matrix, source-residual baseline comparison,
  nested viewport/Clear checks, and the native HTML projection regression suite.
- Real PNG pixels contain the requested pink text shadow. PNG and PDF retain matching actual
  filter receipts for ordinary, nested multiline and blank-line HTML labels. Custom host metrics
  retain a theme residual in both native-SVG and HTML modes. Embedded-fonts were not enabled in
  these final commands; no embedded-font HTML qualification is claimed.
- Chromium 151.0.7922.34 consumed output from the public `Renderer` SVG entry point with a custom
  effect theme: six positive-area foreignObjects, no shape terminals inside their filter groups,
  and transparent computed backgrounds throughout the filtered HTML. Toggling the filters reduced
  detected pink pixels from 2,032 to 8. The probe uses the real family writer; it does not call
  compile_preset or qualify the public Cyberpunk recipe. Its native PNG admission retains only
  the expected SystemOrHostFontDependency (HostDependent), not a missing-theme/filter reason.
- Scoped Clippy, `cargo fmt --all -- --check`, and `git diff --check` passed. Existing dead-code
  and large-error warnings remain; no unrelated warning cleanup was attempted.
- Independent review finished without retained findings. Seven lenses ran serially in one separate
  Codex context, not seven independent reviewers. The four reviewed file hashes match the final
  tested source, source-set SHA-256
  `90644b5d473277b7c6d7fe8d70315c0b7e17923d22577e8cfebfa916ccbb6b61`.
  Parent final verification closes its pending runtime gate for this bounded increment.
  Review artifacts: `/tmp/compound-engineering-501/ce-code-review/20260919-181714-html-node/`.
- The simplification pass ran inline after the harness rejected additional reviewers at its thread
  limit. It reused the existing source parser and class resolver, removed duplicated effect lookup,
  and avoided introducing semantic-ancestor CSS analysis or a retained HTML cache.

Commands use serial Cargo, `CARGO_BUILD_JOBS=1`, and the shared target:

```text
cargo nextest run --release --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test flowchart_node_effects --test theme_flowchart_edge_effects --test resvg_safe_typography --test-threads 2 --no-fail-fast
cargo clippy --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test flowchart_node_effects --test theme_flowchart_edge_effects --test resvg_safe_typography
```

Final logs: `/tmp/flowchart-html-node-verified-final.log` and
`/tmp/flowchart-html-node-clippy.log`. Browser evidence is
`/tmp/flowchart-html-node-browser.{mjs,json,png}`; the temporary Rust probe is archived at
`/tmp/flowchart-html-node-probe.rs`, not kept as a project example.
These are development evidence, not release or
installed-package receipts. No full workspace, browser matrix, platform build, public qualification,
package-size comparison or performance benchmark was run for this increment.
