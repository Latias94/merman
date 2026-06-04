# HPD-090 Kanban Baseline Refresh

Kanban was part of the broad stale stored-SVG set for Mermaid 11.15 baseline preparation.

Outcome:

- Regenerated all `87` `fixtures/upstream-svgs/kanban/*.svg` files to the pinned Mermaid 11.15
  baseline.
- DOM parity was not a pure fixture refresh. Mermaid 11.15 now scopes Kanban section/item group ids
  by diagram id, and item title labels carry the markdown label class.
- Updated `crates/merman-render/src/svg/parity/kanban.rs` so:
  - section and item group DOM ids use `<diagram-id>-<raw-id>`;
  - prototype-like ids such as `__proto__` and `constructor` remain renderable after prefixing;
  - item title XHTML labels use `nodeLabel markdown-node-label`, while section labels,
    ticket/assigned labels, and empty placeholders keep `nodeLabel`.

Verification:

- `cargo nextest run -p merman-render kanban_dom_ids_are_scoped_by_diagram_id` - passed.
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.

Residual note:

- Kanban is removed from the HPD-090 broad stale queue. The remaining broad stale families are
  `mindmap` and `radar`, followed by narrow refreshes for `class`, `timeline`, and Flowchart HTML
  demo KaTeX fixtures.
