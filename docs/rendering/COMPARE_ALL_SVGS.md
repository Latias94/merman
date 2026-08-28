# Compare All SVGs (Mermaid Parity)

This note documents the `xtask compare-all-svgs` helper, which runs the per-diagram SVG parity
checks in one shot and aggregates failures.

## Run

- Strict investigation mode, where reviewed browser text-layout residuals remain blocking:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`

- Release policy with browser text layout reported as diagnostic:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout`

- Use a specific strict DOM comparison mode for all diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

- Only run a subset of diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --diagram flowchart --diagram sequence`

- Skip some diagrams:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --skip gantt --skip flowchart`

## Outputs

- Local SVGs are written under `target/compare/<diagram>/`.
- Per-diagram reports are written under `target/compare/`.
  - When `--dom-mode` is provided, reports are mode-suffixed to avoid overwriting across runs:
    - `target/compare/<diagram>_report_<mode>.md` (e.g. `target/compare/state_report_parity_root.md`)
  - When `--dom-mode` is omitted, per-diagram compare tasks use their default report paths
    (typically `target/compare/<diagram>_report.md`).

Every report labels its evidence as raw/source SVG bytes or raw/source SVG-DOM. It also records that
browser-visible and resvg-safe evidence were not collected. In particular, a passing Theme, Block,
or Gantt structure/parity comparison is not evidence about computed colors, edge contact, or tick
overlap in a browser. Those claims belong to browser tests with their build-freshness and viewport
preconditions; resvg-safe claims belong to output-pipeline and `usvg` / `resvg` gates.

When `--check-dom` is enabled, registered families also run the semantic edge-label gate. Reports
record expected fixtures, compared fixtures, samples, and accepted exact residuals. Zero samples,
missing registered fixtures, stale residuals, or a label/path identity mismatch fail the aggregate
even when the selected DOM profile passes. The contract is documented in
`docs/alignment/SEMANTIC_LABEL_PARITY.md`.

## Browser text layout diagnostics

`--diagnostic-browser-text-layout` is intentionally narrower than `continue-on-error`. It consults
`fixtures/_verification/browser-text-layout-residuals.json`, whose entries bind a reviewed fixture,
input digest, pinned upstream SVG digest, admitted comparison modes, the exact three-decimal
comparison policy, and a canonical deterministic local SVG signature. The local signature requires
an `<svg>` document root, rejects processing instructions, and preserves namespace URIs, element
order, text (including non-breaking spaces), stylesheet content, IDs, classes, and every non-path
attribute value. Attribute order is canonicalized because it is not XML semantics. Only numeric
operands inside `path d` are rounded to three decimals, which removes the verified last-bit
ARM/x86/Linux float drift while retaining path commands and geometry changes visible at the gate's
precision. The catalog covers measurement-led cases in Architecture, Class, Flowchart, Gantt,
Journey, Sequence, Timeline, and Treemap, whose pinned Mermaid implementations derive wrapping,
path topology, task-label placement, or adaptive font size from browser
`getBBox()`/`getComputedTextLength()` results that the font-agnostic deterministic fallback does
not claim to reproduce.

Render failures, malformed upstream or local DOM, semantic-label failures, operation-provenance
failures, invalid roots, changed root sizing policy, unregistered fixtures, unlisted modes, changed
input/upstream digests, changed local signatures, stale receipts, and every other DOM mismatch
remain blocking. A changed node, class, id, text, stylesheet, namespace, element order, path command,
or path coordinate at three-decimal precision therefore cannot reuse an old receipt. Omitting the
flag restores blocking upstream DOM comparison for parity work.

## Root reports

`compare-all-svgs` forwards `--report-root` to diagram families that support the root-delta report.
Text measurement is always deterministic; there is no single-value selector.

Example:

- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root`

## Notes

- `parity-root` depends on the headless `getBBox()`-like bounds approximation in `merman-render`.
  It treats `<a>` as a transform container (so link-wrapped nodes contribute correctly), and it
  ignores non-rendered containers like `<defs>`/`<marker>` when deriving the root viewport.
- State diagrams derive `viewBox`/`max-width` from emitted SVG bounds and fall back to layout
  geometry only when emitted bounds are unavailable. This policy is operation-owned and has no
  process-global switch.

## Precision

- `--dom-decimals 3` is the current stability gate for `parity-root`.
- `--dom-decimals 6` is a useful stress test for root viewport parity (`viewBox` + `max-width`),
  but it is expected to surface small residual numeric drift as we continue to tighten the
  headless bbox + viewport pipeline.
  - Some drift is inherent to browser font and float behavior. Production output is never adjusted
    by fixture id; bounded browser-only residuals stay visible in parity reports and accepted
    residual policy.
  - New or changed residuals fail the gate unless an exact browser-text-layout receipt is reviewed
    and committed. Fix source-backed semantics, layout, or emitted geometry rather than adding a
    root pin or font-specific fallback.
- Semantic-label geometry always uses its independent three-decimal contract. Raising DOM
  precision to six decimals cannot disable or re-quantize signed label evidence.
