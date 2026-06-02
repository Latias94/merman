# HPD-050 - Architecture Group BBox Phase Audit

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Re-audited the two active `+5px` Architecture root rows on current HEAD:

- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`

No production renderer behavior changed in this slice. The result is a classification update and a
negative experiment record.

## Evidence

Focused current reports still show the same root-width deltas:

- `target/compare/architecture_batch5_hpd050_current_debug.md`: upstream `542.926px`, local
  `547.926px`.
- `target/compare/architecture_html_titles_hpd050_current_debug.md`: upstream `479.926px`, local
  `484.926px`.

Structured SVG inspection confirms both rows are controlled by final group rectangles:

- `batch5_long_titles`: upstream group rect `x=-233.462816,width=462.925633`; local
  `x=-236.962816,width=467.925633`.
- `html_titles`: upstream group rect `x=-170.962816,width=399.925633`; local
  `x=-172.462816,width=404.925633`.

Debug output shows local child service bounds are already close to the saved browser facts:

- `Artifacts Storage retention 30d`: `width=222.828125`, `label_half=112.5`,
  `extras_lr=73.5`.
- `Web Front Line 2`: `width=122.570312`, `label_half=64.5`, `extras_lr=25.5`.

The final renderer-side group padding is still `padding + 2.5`, i.e. `42.5px` at the default
`architecture.padding=40`.

## Rejected Experiment

A temporary local experiment changed `ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX` from `2.5` to
`0.0` and ran:

```text
cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_experiment_group_extra_0.md
```

That made the two `+5px` rows width-exact, but their heights became `5px` too short:

- `batch5_long_titles`: upstream `542.926x462.926`, local `542.926x457.926`.
- `html_titles`: upstream `479.926x462.926`, local `479.926x457.926`.

The full report also expanded many group-heavy rows into narrow local max-width mismatches. The
global padding change was reverted before commit.

## Classification

The two `+5px` rows remain Architecture Cytoscape bbox phase residuals, not safe root-override
candidates and not evidence that the renderer should globally remove the `+2.5px` final group bbox
extra.

The next real fix would need a phase-specific model that can distinguish:

- child service contribution into `updateCompoundBounds()`,
- final group `node.boundingBox()` used by Mermaid `svgDraw.ts`,
- SVG root `getBBox()` used by `setupGraphViewbox(...)`,
- manatee relocation / element bbox approximation.

Do not collapse these phases into one global label-width or group-padding formula.
