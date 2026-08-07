# Error Diagram Minimum Slice

This document defines the admitted Mermaid `error` diagram contract in `merman`.

Baseline: Mermaid `@11.16.1`.

## Source authority

- Registration and the no-op parser:
  `repo-ref/mermaid/packages/mermaid/src/diagrams/error/errorDiagram.ts`.
- Fixed SVG geometry and labels:
  `repo-ref/mermaid/packages/mermaid/src/diagrams/error/errorRenderer.ts`.
- Registry ordering and fallback selection:
  `repo-ref/mermaid/packages/mermaid/src/diagram-api/diagram-orchestration.ts`.

## Supported contract

- `error` is detected as the upstream diagram id `error`.
- The semantic model is deliberately minimal and serializes as `{ "type": "error" }`, matching
  upstream's no-op parser and empty database.
- With parse-error suppression enabled, a failed diagram parse resolves to the same typed Error
  semantic/layout/SVG path instead of a separate fallback renderer.
- Typed layout projects the fixed upstream `viewBox` of `0 0 2412 512` and responsive maximum
  width of `512px`.
- The SVG renderer emits the six upstream error-icon paths, the `Syntax error in text` label, and
  the pinned Mermaid version label through the normal family artifact pipeline.

## Fixture evidence

The normalized corpus under `fixtures/error` contains four source-backed inputs:

| Fixture | Upstream evidence | Contract exercised |
| --- | --- | --- |
| `basic.mmd` | `errorDiagram.ts` registration and no-op parser | Direct `error` detection and rendering |
| `upstream_pkgtests_statediagram_spec_024.mmd` | Commented arrow-direction sample in `stateDiagram.spec.js` | Suppressed legacy State parse failure |
| `upstream_pkgtests_statediagram_v2_spec_024.mmd` | Commented arrow-direction sample in `stateDiagram-v2.spec.js` | Suppressed State v2 parse failure |
| `upstream_pkgtests_statediagram_spec_030.mmd` | Skipped `if` statement sample in `stateDiagram.spec.js` | Suppressed unsupported State syntax |

Each fixture owns a semantic golden, typed-layout golden, pinned upstream SVG, and schema-v2
generated-complete provenance entry.

## Admission and gates

`error` is part of the Primary SVG matrix. Its family-local gate is:

```bash
cargo run -p xtask -- compare-error-svgs --check-dom --dom-mode parity
cargo run -p xtask -- compare-error-svgs --check-dom --dom-mode parity-root
```

The compare fact uses suppressed parsing for the three invalid State sources, then verifies the
same typed Error artifact and DOM contract used by the direct `error` input. Comparator behavior
remains the shared narrow DOM normalization; there is no Error-specific fixture rewrite.
