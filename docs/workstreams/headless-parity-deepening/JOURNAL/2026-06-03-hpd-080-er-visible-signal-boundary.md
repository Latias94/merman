# HPD-080 - ER Visible Signal Boundary

Task: HPD-080 visible rendering defect triage.

## Question

Does the public ER dark-theme smoke count colors that current ER DOM actually consumes?

## Source Audit

- Pinned Mermaid 11.15 `er/styles.ts` emits `.entityBox`, `.relationshipLabelBox`, `.labelBkg`,
  `.edgeLabel`, `.label`, node shape, relationship line, marker, and data-color rules.
- Current local compact ER output uses XHTML node and edge labels inside `foreignObject`, not native
  `<text>` labels.
- Current edge-label DOM has `.labelBkg` and `<span class="edgeLabel">`, but no
  `.relationshipLabelBox`.
- Current node/edge output consumes line, marker, simple-node, rough-entity inline, XHTML label, and
  edge-label CSS surfaces.

## Outcome

No production renderer defect was found. Updated the representative ER public smoke to count only
current visible surfaces: line/node colors, `nodeTextColor` through XHTML labels,
`tertiaryColor` through the current `.labelBkg` rgba fade, and `edgeLabelBackground` through the
current XHTML edge label.

Added `er_theme_smoke_counts_current_xhtml_label_and_edge_dom_as_visible` in
`crates/merman/tests/theme_renderability_smoke.rs`. The test documents that direct
`.relationshipLabelBox` fills and native edge-label text CSS are provider/native-text coverage for
the current sample.

## Verification

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke er_theme_smoke_counts_current_xhtml_label_and_edge_dom_as_visible` -
  passed, 1 test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, 10
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, 364 JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

## Residual

ER remains covered for current node, relationship, marker, XHTML label, and edge-label DOM. Do not
count `.relationshipLabelBox`, native `.edgeLabel .label text`, or `data-color-id` provider rules
as visible unless a future fixture emits matching current DOM.
