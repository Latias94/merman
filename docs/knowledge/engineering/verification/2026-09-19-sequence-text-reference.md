# Cyberpunk Sequence: actual reference text and remaining terminals

Date: 2026-09-19. This is reference observation for U7, not preset qualification.
Merman implementation baseline before the text increment: `c72ad55b1`.

## Reproduction

Reference: `repo-ref/modern_mermaid` at
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`, `src/utils/themes.ts`, SHA-256
`9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.
The installed reference Mermaid is **11.12.1**, not Merman's current parity baseline.
Chromium: **151.0.7922.34**. Input:
`crates/merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd`.

The local browser probe loads the reference's installed Mermaid bundle and transpiles the
pinned theme source. Network requests are blocked. It records computed styles of both
`text` parents and their actual `tspan` leaves, matched selector counts, geometry, and frames.
Artifacts and the runnable probe are under
`target/bench/experiments/sequence-text-reference-c72ad55b1/`:
`reference.cjs`, `reference-observations.json`, `reference.svg`, `reference.png`.
The probe was rerun on September 19; these are local experiments, not release receipts.

## Observed visual contract

| Terminal | Actual reference behavior | Typed adaptation / remaining work |
| --- | --- | --- |
| Participant label | `.actor text` matches **zero** elements. The parent `text.actor` receives the `.actor` two-stage drop shadow (sigma 8/16, cyan alpha .5/.3). Its leaf has cyan fill and no stroke. Weight is 400. | Attach two-stage glow to the actual ActorLabel text; do not infer 600 weight or a 3px glyph stroke from the inactive/inherited declarations. Still open at the baseline. |
| Note label | Parent is magenta; actual leaf explicitly fills `rgb(51,51,51)`. Both inherit magenta alpha .4 text-shadow with CSS blur 8px. Inline weight wins: 400. | Preserve Merman's existing readable magenta label color instead of copying the dark leaf override. Add a separate text-only sigma-4 glow. This is a documented readability adaptation, not exact pixel equivalence. |
| Loop keyword and title | Cyan fill, weight 400, cyan alpha .5 text-shadow with CSS blur 10px. | Text-only sigma-5 glow for the applicable LoopLabel surfaces; still open. Do not assume the stylesheet's 600 weight wins. |
| Message label | Cyan fill, weight 400, no text-shadow or filter. | Do not invent message text glow for this recipe. Message line effects are a different terminal. |
| Lifeline | Cyan, width 2px, cyan alpha .6 drop shadow sigma 6. | Full lifeline recipe/consumer remains open. |
| Activation | Translucent cyan fill, cyan 3px border. | Complete the remaining border-width consumption. |
| Loop frame | Four actual line elements; cyan width 2px, cyan alpha .5 drop shadow sigma 4. | Complete frame paint/width/effects without combining them with text effects. |
| Loop keyword box | A polygon; dark fill, cyan width 2px, cyan alpha .4 drop shadow sigma 6. Computed `rx/ry:10px` does not round this polygon. | Preserve polygon geometry; do not invent rounded corners because a non-applicable CSS property is present. |

All observed reference text weights in this scene are 400. CSS declarations alone are not
visual evidence. In particular, a filter on a text parent can affect its child glyphs even when
the child's computed `filter` is `none`; inherited text-shadow and leaf fill must be considered
separately. The screenshot's responsive presentation size is not a controlled pixel-alignment
baseline. Font/profile provenance and final SVG/PNG/PDF scene qualification remain separate work.

This observation refines R1's **applicable source-backed** requirements. It does not remove
requested effects, promote catalog cells, close U7, or establish equivalence for other themes.
