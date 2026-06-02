# HPD-080 - ER And Mindmap Invalid Edge Style Tokens

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

After fixing QuadrantChart's invalid default point color, the remaining renderability scan showed
that ER and Mindmap raw SVG edge paths emitted `style="undefined;;;undefined"`. This was not caught
by structural DOM parity because parity mode intentionally ignores most non-root `style` attributes.

The invalid style token does not define useful visual semantics. ER relationship lines and Mindmap
edges already receive their visible stroke/fill behavior from CSS classes.

## Source / Baseline Checks

Checked local and pinned baseline outputs:

- `fixtures/er/basic.mmd`
- `fixtures/mindmap/basic.mmd`
- `fixtures/upstream-svgs/er/basic.svg`
- `fixtures/upstream-svgs/mindmap/basic.svg`

Findings:

- Pinned upstream fixtures also contain `style="undefined;;;undefined"` on these edge paths.
- The token is an upstream artifact, not a meaningful style contract.
- Keeping it in merman raw SVG creates lower-quality headless output and makes public renderability
  smoke expansion harder because `undefined` becomes ambiguous.

## Outcome

- ER relationship paths no longer emit `style="undefined;;;undefined"`.
- Mindmap edge paths no longer emit `style="undefined;;;undefined"`.
- Added focused regressions:
  - ER renderer test rejects `style="undefined"` on the styled relationship fixture.
  - Mindmap public render test rejects `style="undefined"` while preserving the existing geometry
    assertions.

## Verification

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render er_svg_renders_entities_and_relationships --test er_svg_test`
- `cargo test -p merman mindmap_br_variants_031_matches_upstream_node_geometry --test mindmap_br_variants_031 --features render`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render -p merman`

Manual raw-output sample:

- `fixtures/er/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`
- `fixtures/mindmap/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`

## Residual

This intentionally removes a useless invalid style attribute from raw local output while preserving
class-driven visual behavior and structural parity. It should not become a broad policy to remove
all empty `style=""` attributes, because some fixtures still rely on local DOM shape choices.
