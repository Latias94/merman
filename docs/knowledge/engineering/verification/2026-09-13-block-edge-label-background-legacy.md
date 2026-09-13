# Block EdgeLabelBackground legacy consumer verification

Date: 2026-09-13

The Block `EdgeLabelBackground.fill` route is now owned by the Block typed plan while preserving the existing CSS
consumer. The Block SVG writer emits the resolved value into the family-scoped
`.edgeLabel`, `.edgeLabel rect`, and `.labelBkg` CSS selectors, and the label
writer emits matching edge-label terminals for both HTML and SVG label modes.

The migration characterization test is:

```text
cargo nextest run --locked -p merman-render --test block_svg_test \
  -E 'test(block_edge_label_background_typed_route_has_a_real_css_consumer)'
```

The test passed on the current revision after the typed-plan cutover. Its evidence summary is:

- `required_count = 1`
- `applied_count = 1`
- `compatibility_residual_count = 0`
- `theme_residual_count = 0`

The typed plan owns the stylesheet color and records each emitted edge-label
terminal. The compatibility bridge can now be retired only after the independent
route inventory and clean-checkout retirement witness are updated. The full Block SVG test binary also passed (`44/44`).
