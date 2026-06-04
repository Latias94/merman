# HPD-050 - Architecture Post-Strict Root Queue Classification

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After the strict `RectangleD.intersects(...)` fix, full Architecture `parity-root` is still an
expected diagnostic failure with `20` root/style mismatch rows. The previous focused evidence
proved that the three direct width tails are split into child-content drift plus a stable final
group expansion drift, and that changing global group padding regresses the family.

This pass re-read the full post-strict queue instead of continuing to reason from only those three
direct-width fixtures.

## Evidence

- `target/compare/architecture-report-parity-root-strict-intersect-final`
- `target/compare/architecture-delta-post-strict-20-hpd050`
- `target/compare/architecture-delta-post-strict-probe-join-top5-hpd050`
- Existing browser probe inputs from
  `F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Commands:

- `cargo run -p xtask -- debug-architecture-delta ... --out target\compare\architecture-delta-post-strict-20-hpd050`
- `cargo run -p xtask -- debug-architecture-delta ... --probe-dir F:\SourceCodes\Rust\merman\target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-post-strict-probe-join-top5-hpd050`

## Classification

The `20` post-strict mismatch rows are not one residual family.

1. Direct group-width tails:
   - `stress_architecture_batch5_long_titles_and_punct_076`: `+5px`
   - `stress_architecture_html_titles_and_escapes_041`: `+5px`
   - `stress_architecture_unicode_and_xml_escapes_019`: `+3px`
   - Probe join still decomposes these as child content `+3/+3/+1` plus final expansion `+2`.

2. Source-shaped service/body/final phase rows with negative child-content width:
   - `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`: `-2.5px`
   - Browser child content is wider than local by `+5/+3`, while the same final expansion split is
     still `browser=63px` vs. `local=65px`.
   - The report also shows a large service/group X displacement, so this is not a sibling of the
     direct-width rows even though it shares the final expansion component.

3. Nested/group aggregate phase rows:
   - `stress_architecture_nested_groups_002`: `+2.5px`
   - Direct child groups and direct services both show small content-width and X-position drift.
     The root width is controlled by nested aggregate placement, not by a single service label.

4. Small group-rect / root lattice tails:
   - Examples: `batch3_long_group_titles_wrapping_055`, `long_ids_030`,
     `batch5_group_edges_across_nested_groups_075`, `long_group_titles_018`, and the `0.5px`
     group/service rows.
   - These are mostly group rect width or symmetric position tails around `1px` / `0.5px`, not
     broad layout failures.

5. Top-level service icon/text root-bounds tails:
   - The three fallback-icon fixtures and `external_icons_demo_012` have no group rects.
   - Their reports show single-service or top-level-service position/root bbox tails, so they should
     not be folded into the group-content model.

## Outcome

No production code changed.

The next useful HPD-050 experiment should be narrower than "global labelWidth" or "global group
padding": test a source-shaped service final-bbox contribution model on a small fixture set that
includes both positive-content rows (`076/041/019`) and negative-content rows (`093/002`). Any
candidate must preserve full Architecture `parity-root`; improving only the three direct-width rows
is no longer a valid success condition.

## Boundary

Do not treat the post-strict `20` rows as a single constant drift. In particular:

- exact Cytoscape `labelWidth` lookup is still insufficient by itself;
- global final group expansion remains rejected;
- top-level service/root-bounds icon rows need a separate root SVG bbox audit;
- nested aggregate rows need child-group/final `node.boundingBox()` phase evidence before a code
  change.
