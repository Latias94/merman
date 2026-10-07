# Historical Class edge-label CSS witness

These files retain the exact CSS declarations used when the Class edge-label projection
baseline was pinned in `0457ba0444306354c6e7ac6b758c1deadec29d4b` (`test(theme): pin Class edge label projection baseline`).
They are historical retirement evidence, not the current Mermaid stylesheet. Do not refresh
them when updating `fixtures/upstream-svgs` or change their expected digest to match new defaults.

| Witness | Original path at the pinned commit |
| --- | --- |
| `classic-dagre.css` | `fixtures/upstream-svgs/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_a_custom_theme_056.svg` |
| `handdrawn-dagre.css` | `fixtures/upstream-svgs/class/upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_class_with_text_label_033.svg` |
| `classic-elk.css` | `fixtures/upstream-svgs/class/upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_a_custom_theme_055.svg` |

Each file contains the first complete declaration beginning with each of these selectors,
extracted byte-for-byte from the original SVG's first `<style>` block, in this order:

1. `.edgeLabel[data-look="neo"]`
2. `.edgeLabel .label rect`
3. `.labelBkg`
4. `.edgeLabel .label span`

The test hashes declarations without the added line separators, in classic/Dagre,
handDrawn/Dagre, then classic/ELK order. Its original SHA-256 remains
`04444eecf91d09da2f892e045833c097c2496957799e232a6d81756db325984b`.

Mermaid 12 baseline refresh `0fc6e388c` changed the hand-drawn fixture's default colors.
Reading that mutable fixture from a historical-digest test incorrectly changed the witness.
The separate current-upstream selector test continues to check current SVG group structure;
this CSS-only historical witness makes no current-output or DOM-structure claim.
