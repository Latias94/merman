# HPD-050 - RaTeX Math Label Line-Break Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`RatexMathRenderer::math_only_lines(...)` is compiled only with the optional `ratex-math` feature,
but it still initialized a static regex on the render path:

```rust
regex::Regex::new(r"(?i)<br\s*/?>").unwrap()
```

Pinned Mermaid 11.15.0 defines the shared source shape in
`repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`:

```ts
export const lineBreakRegex = /<br\s*\/?>/gi;
```

`crates/merman-render/src/text/wrap.rs` already carries a direct scanner for this source shape via
`split_html_br_lines(...)`, and the mixed KaTeX-like path in `math.rs` already used that helper.

## Changes

- Removed the local RaTeX `<br>` regex initialization from `math_only_lines(...)`.
- Reused `split_html_br_lines(...)` for pure-math labels, matching the mixed math path and ordinary
  HTML-label wrapping.
- Added feature-gated coverage for uppercase `<BR />`, whitespace before the optional slash, and
  a `<brx>` lookalike that must not split a same-line multi-formula label.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render --features ratex-math ratex_math_renderer` -
  passed, `4` tests run.
- `cargo +1.95 fmt --check -p merman-render` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'LINE_BREAK_RE|Regex::new\(r"\(\?i\)<br|regex::Regex|<br\\s' crates/merman-render/src/math.rs` -
  no RaTeX math line-break regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `832`
  lines parsed.

## Boundary

This is a render-path panic-surface cleanup for optional RaTeX math labels. It does not change the
shared text wrapping scanner, Node/KaTeX renderer probing, non-math labels, Mermaid preprocessing,
core sanitization, semantic parsing, SVG baselines, root viewport formulas, or Architecture
residual classification.
