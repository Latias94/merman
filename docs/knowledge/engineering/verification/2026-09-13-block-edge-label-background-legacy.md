# Block EdgeLabelBackground legacy consumer verification

Date: 2026-09-13

The Block `EdgeLabelBackground.fill` route remains a real legacy compatibility
consumer. The Block SVG writer emits the resolved value into the family-scoped
`.edgeLabel`, `.edgeLabel rect`, and `.labelBkg` CSS selectors, and the label
writer emits matching edge-label terminals for both HTML and SVG label modes.

The characterization test is:

```text
cargo nextest run --locked -p merman-render --test block_svg_test \
  -E 'test(block_edge_label_background_legacy_route_has_a_real_css_consumer)'
```

The test passed on the current revision. Its evidence summary is:

- `required_count = 0`
- `compatibility_residual_count = 1`
- `theme_residual_count = 0`

This is intentional: the route is still owned by the compatibility bridge and
must not be retired until a Block typed plan owns the stylesheet and records the
actual label terminals. The full Block SVG test binary also passed (`44/44`).
