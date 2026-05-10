# Override Policy

Overrides are compatibility data, not a substitute for fixing the renderer model. Use them only
when upstream Mermaid behavior depends on browser/font measurement or on intentionally pinned
export quirks that cannot be derived reliably from the semantic model.

## Mermaid Parity for Generated Overrides

For generated override data, Mermaid parity means matching a reproducible upstream
`mermaid@11.12.3` browser/export fact for a narrow context. It does not mean adding a broad
shortcut because the rendered image looks closer.

Generated overrides are valid only when they encode one of these upstream facts:

- browser/font measurement such as `getBBox()`, `getComputedTextLength()`, or HTML label layout;
- root viewport/export values produced after upstream DOM insertion;
- literal SVG/path quirks that are version-pinned and covered by fixtures or probes.

Generated overrides must not encode:

- missing parser or typed-model fields;
- incorrect layout rank/order/route logic;
- sanitizer, escaping, Markdown, or DOM order bugs;
- values copied from a single failing fixture without a repeatable upstream source.

Each generated override category should have an expected removal trigger. Typical triggers are a
better vendored font table, a browser-probe import, a typed model/layout fix that can derive the
value honestly, or a Mermaid baseline upgrade that removes the pinned behavior.

Generated/manual override counts are guarded by a no-growth budget in `xtask report-overrides`.
Adding entries should be exceptional: first show why the mismatch is not a parser, typed model,
layout, sanitizer, escaping, or DOM-order bug. If the override is still the right fix, update the
budget and `OVERRIDE_FOOTPRINT.md` in the same change so review sees the intentional debt increase.

## Allowed Text Width Overrides

A text width override is allowed only when all of these are true:

- A concrete fixture or browser probe shows the upstream value.
- The mismatch comes from browser/font measurement, such as `getBBox()`,
  `getComputedTextLength()`, `getBoundingClientRect()`, CSS font fallback, glyph overhang,
  kerning, or HTML/SVG label export behavior.
- The parser, typed render model, layout geometry, DOM order, and sanitizer path are already
  correct enough that changing model code would be dishonest or unstable.
- The override is narrow: exact literal, exact diagram family, exact font key, or exact generated
  lookup category.
- A unit, snapshot, or parity fixture guards the lookup.
- The override has a plausible removal trigger, such as a better font table, browser probe import,
  or renderer model cleanup.

## Disallowed Overrides

Do not add a text override when the mismatch is really caused by:

- Missing typed model data.
- Incorrect layout math, padding, margins, rank order, or route geometry.
- Wrong DOM structure, label emission order, escaping, sanitization, or Markdown parsing.
- An outdated fixture baseline.
- A broad fallback that would affect unrelated strings or diagram families.
- A one-off inline literal in a renderer when a generated table or shared lookup can own it.

## Allowed Raw SVG/Path Bridges

A raw SVG/path bridge is allowed only when all of these are true:

- A concrete fixture shows upstream Mermaid emits a literal path, geometry attribute, or DOM shape
  that cannot be reproduced by the current generic layout/emission path.
- The mismatch is known to be a temporary parity bridge, not a substitute for missing typed model
  data or incorrect shared geometry.
- The bridge is narrow: exact diagram family, exact graph shape, and exact emitted attribute.
- The function is named `maybe_override_*` so `xtask report-overrides` can inventory it.
- The implementation has an owner and removal criteria documented nearby.

## Placement Rules

- Generated text data belongs under `crates/merman-render/src/generated/`.
- Shared browser/font measurement belongs in `crates/merman-render/src/text/font_metrics.rs`.
- Flowchart-aware Markdown/HTML/SVG label measurement belongs in
  `crates/merman-render/src/text/metrics.rs`.
- Diagram renderers may call shared text measurement APIs, but should not grow local width
  override branches unless the measurement is genuinely diagram-specific and documented nearby.
- Manual raw SVG/path bridges belong under the diagram-specific
  `crates/merman-render/src/svg/parity/` module, must use the `maybe_override_*` prefix, and must
  include nearby owner/removal notes.
- When a new generated override category is added, update `xtask report-overrides` so the footprint
  remains visible in `OVERRIDE_FOOTPRINT.md`.
- When an existing override category grows, update the explicit no-growth budget in
  `xtask report-overrides` only with the evidence checklist below.

## Evidence Checklist

Before adding or regenerating text width overrides:

1. Record the fixture path or probe command that produced the upstream value.
2. State whether the upstream source was HTML layout, SVG bbox, computed text length, or root
   viewport export.
3. Add or extend a focused test near the lookup owner.
4. Run the narrow render gate for the touched diagram family.
5. Update `OVERRIDE_FOOTPRINT.md` when generated-module counts change.
6. Update the `xtask report-overrides --check-no-growth` budget only when the new entries are
   intentional and reviewable.
7. Prefer deleting stale entries in the same patch when the new model makes them unnecessary.

Before deleting text width overrides:

1. Show the replacement path for every consumer of the lookup, not only the browser/vendored SVG
   render path.
2. If the lookup participates in layout, run the layout snapshot gate for the touched fixtures or
   the full `fixtures_match_layout_golden_snapshots_when_present` test. Block text lookups are the
   current cautionary case: vendored HTML measurement can equal the upstream override while the
   default deterministic layout measurer still differs.
3. Run both the normal DOM parity mode and `parity-root` mode for the touched diagram family when
   root sizing can observe the text metric.
4. Update `OVERRIDE_FOOTPRINT.md`, `TODO.md`, and the no-growth budget when the count changes.

Before adding a manual raw SVG/path bridge:

1. Record the fixture or parity command that exposes the upstream literal behavior.
2. Explain why a typed model, layout, or shared emission fix is not the right immediate change.
3. Add owner/removal notes near the bridge.
4. Run the narrow render gate for the touched diagram family.
5. Update `OVERRIDE_FOOTPRINT.md` and keep the function name visible to `xtask report-overrides`.

## Review Questions

Use these questions during review:

- Would a typed model or layout fix remove the mismatch for more fixtures?
- Is the override keyed narrowly enough to avoid hijacking common labels like `plain`, `One`, or
  short class/member names?
- Is the value tied to the Mermaid baseline version currently used by fixtures?
- Can the override be generated from a repeatable upstream probe instead of hand-entered?
- If this is a raw SVG/path bridge, is it named, owned, and temporary?
- Does the change keep `cargo clippy --all-targets -- -D warnings` green?
