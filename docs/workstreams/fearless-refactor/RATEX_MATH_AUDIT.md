# RaTeX Math Renderer Audit

Date: 2026-05-30

This note records the first RaTeX-vs-Mermaid/KaTeX audit for merman's optional
`ratex-math` feature.

## Scope

- Backend under review: `merman_render::math::RatexMathRenderer`.
- Mermaid reference path: local `NodeKatexMathRenderer` probe using the pinned
  `tools/mermaid-cli` dependencies.
- Repro helper: `cargo run -p merman-render --features ratex-math --example ratex_math_audit`.
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
  radicals, `\text`, cases, matrices, and `\overbrace`. Flowchart also supports single-formula
  prose/math labels by composing text fragments with measured math fragments.
- Sequence can render pure math participant and note labels through RaTeX. It now also renders
  single-formula prose/math messages such as `Solve: $$\sqrt{2+2}$$` by using a Sequence-specific
  render hook plus layout-side text/math metric composition.

## Decision

Do not add ad hoc RaTeX calibration constants yet. Mixed prose/math support should stay model-based:
prose fragments are measured with the owning diagram's text measurer, while pure math fragments are
measured by the selected math backend. Flowchart and Sequence now support one formula per line;
multiple formulas on one line are still intentionally left to a separate parser decision because
Mermaid's current `$$...$$` replacement is greedy.

## Verification

Fresh validation on 2026-05-30:

- `cargo fmt --check`
- `cargo nextest run -p merman-render --features ratex-math --test flowchart_svg_test --test sequence_svg_test ratex`
- `cargo nextest run -p merman-render --features ratex-math --lib ratex_math_renderer`
- `cargo run -p merman-render --features ratex-math --example ratex_math_audit`
- `cargo check -p merman-cli --features ratex-math`

The no-feature CLI gate was not rerun because this change only affects the feature-gated RaTeX
renderer path plus documentation.

## Follow-ups

- Extend `examples/ratex_math_audit.rs` with mixed prose/math probes if Flowchart or Sequence
  calibration needs to compare composed text/math labels directly.
- Revisit multiple formulas on one line separately; Mermaid's current greedy `$$...$$` replacement
  treats that case differently from a non-greedy fragment parser.
