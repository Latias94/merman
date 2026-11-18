# Pie Terminal Support

Pie uses a family-owned percentage-bar projection over its canonical typed sections. This view
is explicitly named **Pie proportions**. It provides a shared 0–100% scale; it does not claim
circle, donut, or angular geometry.

## Proportions and complete values

Every section keeps its canonical order and a stable P key. Fifty terminal cells span 100%, and
each bar rounds independently to the nearest cell. Percentages use the sum of **all** listed
values. They are shown to one decimal place; rounded percentages need not total 100%. Both bar
lengths and displayed percentages round exact half-unit ties upward from the canonical f64 values.

A positive section that rounds below one cell uses `*`, explicitly distinguished from a 2% bar.
Zero sections have empty bars and remain listed. Values below the percentage display precision
use `<0.1%`. Neither a tiny category nor its contribution to the denominator is dropped. This is
an intentional terminal difference from Mermaid SVG's filtering of arcs smaller than 1%.

Short labels appear beside their bars, quoted with terminal-safe escaping. Labels that do not
fit there use `[P#]` and remain complete in the keyed data below. No label is truncated or grouped
into an invented Other category. `showData` adds the raw value beside each bar; exact values are
always present in the data records, including when `showData` is false.

The visible title uses the model title, then the declared source title if needed. Complete records
retain model title, accessibility fields, source title/configuration, their null or empty defaults,
and future serialized fields.
Donut, legend placement, colors, hover, and browser sizing remain disclosed configuration rather
than terminal geometry or interaction.

## Numeric and identity boundary

Nonempty finite nonnegative values with a positive total have a Diagrammatic primary. Values are
accumulated exactly as integers in units of 2^-1074 in a fixed-size buffer before threshold comparisons.
Overflow of the ordinary f64 total, subnormal ratios, and division error near rounding boundaries
therefore do not change bar admission or rounding. Tiny values remain exact records even when
their share is below the display scale.

Empty, all-zero, negative, or non-finite borrowed models have a measured StructuredText primary
with the reason proportions are unavailable. No arbitrary normalization, zero substitution, or
invented percentage is applied. Exact records preserve negative zero, signed infinity, and NaN
payload bits. Source parser errors remain errors; typed records do not bypass source validation.

The source parser follows Mermaid's first-label-wins rule for duplicates and rejects negatives
before duplicate suppression. Independently authored typed models may contain repeated labels;
their P keys retain every occurrence without coalescing values or changing order.

## Host policies and evidence

Canonical and Strict are the admitted policies. ASCII/Unicode structures and Unicode/CJK cell
measurement share semantics; CJK uses ASCII structural glyphs. All five primary encodings are
available. Plain width fallback emits complete keyed records; Allow keeps the measured wide
primary, while Error reports its measured width. There is no Pie Adaptive graph mode.

Chart construction, quoted labels, records, encoding, and viewport replacement share the caller's
operation and cumulative work ledger. Scoped document storage avoids admitting discarded
candidates twice. Authored spaces survive trim requests and narrower record reflow. Work, grid,
document, output-byte, depth, cancellation, and deadline failures remain errors under existing
limits; no larger default is introduced for Pie.

`crates/merman-ascii/tests/pie_model.rs` owns the small/typical/dense 80/100/120-column floor, independent filled-cell
error bounds and common origins, zero/tiny/extreme values, duplicate order, safe complete labels,
metadata, encodings, viewports, resource boundaries, and cancellation. The output module tests
the raw-envelope record boundary. Facade, CLI, bindings, and generated Playground capability
tests keep admission and the selected projection consistent.
The [admission report](../ASCII_PHASE_GATE_REPORT.md#pie-proportion-admission) records complete
80/100/120-column dimensions and the vertical cost against the record-only baseline.

The pinned Mermaid Pie DB, grammar, and renderer provide semantic evidence. Neither the copied
`mermaid-ascii` oracle nor its inspected moving renderer implements Pie. Trident label/completion
replays are local diagnostics; its restrictive proportional observer does not independently
validate this richer bar-and-record presentation. The explicit bar geometry tests own that claim.
