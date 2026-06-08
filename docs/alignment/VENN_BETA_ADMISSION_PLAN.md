# Venn Beta Admission Plan (Mermaid@11.15.0)

Status: Proposed
Last updated: 2026-06-08
Pinned Mermaid commit: `41646dfd43ac83f001b03c70605feb036afae46d`

This document records the source-backed plan required before `venn-beta` can become an implementation workstream in `merman`.

## Problem

Mermaid 11.15 includes `venn-beta`, but local support currently has no detector, parser, semantic model, layout, renderer, fixtures, upstream SVG baselines, or compare command.

The parser and DB are small enough to port directly. The risky part is layout/rendering: Mermaid delegates circle placement and intersection geometry to `@upsetjs/venn.js@2.0.0`, then mutates the generated SVG with D3 and optionally replaces shapes with RoughJS for `look: "handDrawn"`. A local renderer must not approximate that geometry with ad hoc circle formulas and then claim Mermaid parity.

## Source Evidence

- Detector: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennDetector.ts` accepts `/^\s*venn-beta/` and exposes diagram id `venn`.
- Grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/parser/venn.jison` supports `set`, `union`, `text`, indented text, `style`, quoted identifiers, bracket labels, and numeric sizes.
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennDB.ts` sorts set identifiers for stable keys, defaults single-set size to `10`, defaults union size to `10 / len^2`, and rejects unknown union identifiers.
- Renderer: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/vennRenderer.ts` calls `venn.VennDiagram()` for SVG generation and `venn.layout()` for text-node placement.
- Styles: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/styles.ts` defines title, circle text, intersection text, and text-node font/color CSS.
- Config: `repo-ref/mermaid/packages/mermaid/src/config.type.ts` defines `VennDiagramConfig` with `width`, `height`, `padding`, and `useDebugLayout`; `defaultConfig.ts` wires `defaultConfigJson.venn`.
- Dependency: `repo-ref/mermaid/pnpm-lock.yaml` pins `@upsetjs/venn.js@2.0.0`.
- Tests/docs: `repo-ref/mermaid/packages/mermaid/src/diagrams/venn/parser/venn.spec.ts`, `vennRenderer.spec.ts`, and `repo-ref/mermaid/docs/syntax/venn.md`.

## Proposed Solution

Implement `venn-beta` only after the layout dependency is made explicit as a first-class adapter.

```mermaid
flowchart LR
    Source["venn-beta source"] --> Parser["Rust parser + Venn semantic model"]
    Parser --> Layout["Venn layout adapter"]
    Layout --> Svg["Stage B SVG renderer"]
    Theme["PresentationTheme Venn roles"] --> Svg
    Upstream["@upsetjs/venn.js 2.0.0 source audit or port"] --> Layout
    Svg --> Compare["compare-venn-svgs + upstream baselines"]
```

The implementation lane should have these slices:

1. Detector and typed parser: add `venn` detector for `venn-beta`, port parser behavior from `venn.jison`, and create a typed model with subsets, text nodes, style entries, title, accessibility metadata, and effective `venn` config.
2. Parser fixtures: port upstream parser cases for labels, sizes, text nodes, style declarations, quoted identifiers, unknown unions, and invalid `set` / `union` arity.
3. Layout adapter decision: either port the relevant `@upsetjs/venn.js@2.0.0` layout algorithm into Rust or introduce a dedicated deterministic adapter whose output can be compared against the pinned package. This decision must be made before SVG rendering.
4. Stage B SVG renderer: emit Mermaid-shaped `.venn-circle`, `.venn-intersection`, `.venn-title`, `.venn-text-nodes`, `.venn-text-area`, and `foreignObject` text-node DOM after layout is source-backed.
5. Theme roles: add `PresentationTheme::venn()` for `venn1..venn8`, `vennTitleTextColor`, `vennSetTextColor`, `primaryColor`, `primaryTextColor`, `textColor`, `titleColor`, `background`, font family, and style override precedence.
6. Fixture and compare gate: import syntax-doc and parser-source fixtures, generate `fixtures/upstream-svgs/venn`, add `xtask compare-venn-svgs`, and keep the family out of the main matrix until family-local structural DOM parity is green.

## Alternatives Considered

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Port `@upsetjs/venn.js@2.0.0` layout logic into Rust | Pure Rust, deterministic, no runtime JS dependency, matches headless architecture | Requires source audit of optimization and geometry code before renderer work | Preferred if the source surface is manageable |
| Use a JS/WASM adapter for `@upsetjs/venn.js` during layout | Highest layout fidelity initially | Adds non-Rust runtime dependency, complicates CLI/FFI packaging, weakens headless portability | Only acceptable as a temporary comparison oracle, not the default renderer path |
| Implement a local approximate circle solver | Fastest to code | Not source-backed, likely DOM/geometry drift, violates admission rubric | Rejected |
| Parse-only `venn-beta` support | Low risk, gives early diagnostics/model access | Users expect visible diagrams; no parity value for preview users | Defer unless a caller explicitly needs parse-only metadata |

## Success Metrics

| Metric | Target | Measurement |
|---|---|---|
| Parser coverage | Upstream parser spec behavior covered by semantic snapshots | `cargo nextest run -p merman-core venn` |
| Layout source parity | Layout adapter outputs match the pinned `@upsetjs/venn.js@2.0.0` oracle for initial fixtures within documented tolerance | Dedicated layout tests or fixture snapshots |
| SVG structural parity | Family-local Venn DOM parity passes for committed upstream baselines | `cargo run -p xtask -- compare-venn-svgs --check-dom --dom-mode parity --dom-decimals 3` |
| Matrix admission | `venn` is not admitted to `compare-all-svgs` until detector, parser, layout, renderer, baselines, and compare command all exist | `cargo run -p xtask -- check-alignment` |

## Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|---|---|---:|---|
| Venn layout drift from `@upsetjs/venn.js` | High | High | Treat layout adapter as the first implementation decision; use pinned package output as an oracle before writing renderer DOM |
| Browser/D3 serialization noise | Medium | Medium | Normalize only non-semantic D3 wrapper differences in the family compare adapter; do not hide geometry or label differences |
| `foreignObject` text-node parity differs across renderers | Medium | High | Document strict HTML/browser text-metric residuals separately from structural DOM parity |
| RoughJS hand-drawn output expands scope | Medium | Medium | Defer `look: "handDrawn"` until classic SVG parity is green; do not emulate rough output with classic paths |
| Packaging impact from a JS layout dependency | High | Medium | Prefer Rust port; if a JS oracle is used, keep it in test/tooling, not runtime packages |

## Admission Decision

`venn-beta` should remain not admitted for now. The next actionable work is a source audit of `@upsetjs/venn.js@2.0.0` to decide whether the layout algorithm should be ported to Rust or used only as a comparison oracle. Implementation should not start from renderer code.

## Initial Gates For A Future Workstream

- `cargo nextest run -p merman-core venn`
- `cargo nextest run -p merman-render venn`
- `cargo run -p xtask -- compare-venn-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- check-alignment`
