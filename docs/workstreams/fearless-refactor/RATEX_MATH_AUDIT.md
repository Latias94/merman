# RaTeX Math Renderer Audit

Date: 2026-05-30

This note records the first RaTeX-vs-Mermaid/KaTeX audit for merman's optional
`ratex-math` feature.

## Scope

- Backend under review: `merman_render::math::RatexMathRenderer`.
- Mermaid reference path: local `NodeKatexMathRenderer` probe using the pinned
  `tools/mermaid-cli` dependencies.
- Diagram paths checked: Flowchart HTML labels and Sequence math labels.
- RaTeX source reference: `repo-ref/RaTeX`; its README documents broad typical math support and
  command-level KaTeX gaps for DOM/trust extensions such as `\includegraphics`, `\htmlClass`,
  `\htmlData`, `\htmlId`, and partial `\htmlStyle`.

## Dimension Samples

Flowchart samples use Mermaid's flowchart HTML-label shell at 16px:

| Formula | KaTeX probe width | KaTeX probe height | RaTeX width | RaTeX height |
| --- | ---: | ---: | ---: | ---: |
| `x^2` | 19.875 | 18.65625 | 15.546875 | 13.828125 |
| `\frac{1}{2}` | 11.6875 | 30.59375 | 11.84375 | 32.125 |
| `\sqrt{x+3}` | 49.671875 | 18.9375 | 50.03125 | 16.640625 |
| `\pi r^2` | 29.875 | 18.65625 | 23.75 | 13.828125 |
| `\alpha` | 11 | 9 | 10.296875 | 6.890625 |
| `\sqrt{2+2}=\sqrt{4}=2` | 122.265625 | 18.9375 | 120.890625 | 17.3125 |

Sequence samples use Mermaid's `width: fit-content` math shell:

| Formula | KaTeX probe width | KaTeX probe height | RaTeX layout label width | RaTeX layout label height |
| --- | ---: | ---: | ---: | ---: |
| `x^2` | 17.6875 | 15.15625 | 16 | 19 |
| `\frac{1}{2}` | 10 | 27.46875 | 12 | 34 |
| `\sqrt{x+3}` | 41.90625 | 16.78125 | 50 | 19 |
| `\pi r^2` | 25.6875 | 15.15625 | 24 | 19 |
| `\alpha` | 10 | 7 | 10 | 19 |
| `\sqrt{2+2}=\sqrt{4}=2` | 101.5 | 16.78125 | 121 | 19 |

## Findings

- A single global RaTeX scale factor is not defensible. Width and height deltas change by formula
  and by diagram shell. For example, `\sqrt{x+3}` is almost width-identical in Flowchart but much
  wider in Sequence, while `\frac{1}{2}` is slightly wider/taller in RaTeX for Flowchart.
- RaTeX can render the current Flowchart docs math fixture formulas, including fractions,
  radicals, `\text`, cases, matrices, and `\overbrace`. The fixture now has feature-gated SVG
  coverage.
- Sequence can render pure math participant and note labels through RaTeX. It now also renders
  single-formula prose/math messages such as `Solve: $$\sqrt{2+2}$$` by using a Sequence-specific
  render hook plus layout-side text/math metric composition.

## Decision

Do not add ad hoc RaTeX calibration constants yet. Sequence mixed prose/math support should stay
model-based: prose fragments are measured with the existing Sequence text measurer, while pure math
fragments are measured by the selected math backend. Flowchart mixed prose/math remains out of scope
until its HTML-label DOM metrics are modeled or calibrated with clear provenance.

## Verification

Fresh validation on 2026-05-30:

- `cargo fmt --check`
- `cargo nextest run -p merman-render --features ratex-math --test flowchart_svg_test --test sequence_svg_test ratex`
- `cargo nextest run -p merman-render --features ratex-math --lib ratex_math_renderer`
- `cargo check -p merman-cli --features ratex-math`

The no-feature CLI gate was not rerun because this change only affects the feature-gated RaTeX
renderer path plus documentation.

## Follow-ups

- Add a small audit helper that emits the RaTeX/KaTeX dimension table from local probes instead of
  relying on manually captured command output.
- Extend Flowchart math measurement only after deciding whether mixed prose/math HTML labels should
  target visual ink parity, DOM bbox parity, or stable pure-Rust deterministic output.
- Revisit multiple formulas on one line separately; Mermaid's current greedy `$$...$$` replacement
  treats that case differently from a non-greedy fragment parser.
