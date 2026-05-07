# Override Policy

Overrides are compatibility data, not a substitute for fixing the renderer model. Use them only
when upstream Mermaid behavior depends on browser/font measurement or on intentionally pinned
export quirks that cannot be derived reliably from the semantic model.

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

## Placement Rules

- Generated text data belongs under `crates/merman-render/src/generated/`.
- Shared browser/font measurement belongs in `crates/merman-render/src/text/font_metrics.rs`.
- Flowchart-aware Markdown/HTML/SVG label measurement belongs in
  `crates/merman-render/src/text/metrics.rs`.
- Diagram renderers may call shared text measurement APIs, but should not grow local width
  override branches unless the measurement is genuinely diagram-specific and documented nearby.
- When a new generated override category is added, update `xtask report-overrides` so the footprint
  remains visible in `OVERRIDE_FOOTPRINT.md`.

## Evidence Checklist

Before adding or regenerating text width overrides:

1. Record the fixture path or probe command that produced the upstream value.
2. State whether the upstream source was HTML layout, SVG bbox, computed text length, or root
   viewport export.
3. Add or extend a focused test near the lookup owner.
4. Run the narrow render gate for the touched diagram family.
5. Update `OVERRIDE_FOOTPRINT.md` when generated-module counts change.
6. Prefer deleting stale entries in the same patch when the new model makes them unnecessary.

## Review Questions

Use these questions during review:

- Would a typed model or layout fix remove the mismatch for more fixtures?
- Is the override keyed narrowly enough to avoid hijacking common labels like `plain`, `One`, or
  short class/member names?
- Is the value tied to the Mermaid baseline version currently used by fixtures?
- Can the override be generated from a repeatable upstream probe instead of hand-entered?
- Does the change keep `cargo clippy --all-targets -- -D warnings` green?
