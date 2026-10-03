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

### Refreshing baseline bindings

Regenerating upstream SVGs does not approve new residuals. Review changed artifacts before
rebinding the upstream digests in the browser-text and label-geometry catalogs, and update
any corresponding `SEMANTIC_LABEL_FIXTURE_CONTRACTS` digest in `compare/labels.rs`. Keep local
signatures and geometry records unchanged when the reviewed local behavior has not changed;
new behavior needs separate source-backed evidence. For a suspected browser measurement
change, replay the old and selected runtimes in the same browser to isolate the cause.

Run the affected catalog and semantic-label tests and focused fixture comparisons after the
refresh. The Linux CI owner performs the complete DOM, semantic-label, and root-paint gates.
Do not batch-replace hashes merely to bypass stale-receipt failures.

### Mermaid 12.1 reference transition

The browser catalog migrated from 159 to 139 entries: 21 receipts were removed because their
comparisons now match, and one Sequence receipt was added after review. Admitted modes, input
bindings, and the three-decimal policy remain unchanged for retained entries. Of the current 139
upstream artifacts, 126 retain the digest recorded at the trusted 12.0 base
`d23ae6c469293aeb1029ddc6862367140bc760df`; the other 13 comprise changed or newly admitted bindings.
The review of the original 159-entry catalog identified 33 changed upstream artifacts:

- All 28 changed Sequence artifacts, one Timeline artifact, and the Class numeric-font-size
  artifact are reproduced byte-for-byte by the 12.0 runtime in the same Chromium headless-shell
  151.0.7922.77 environment as the selected 12.1 runtime. These differences belong to the earlier
  collection environment, not a newly introduced diagram semantic. Replays use the maintained
  seeded renderer, seed 1, fixed clock 1704067200000, and 800px viewport/container.
- The Flowchart distant-edge-label fixture and the State multiple-transition fixture change only
  label translations, following 12.1 `resolveEdgeLabelPosition`: retain the layout anchor and add
  the updated-minus-original path midpoint delta.
- The Sequence `stress_br_in_messages_notes_011` fixture is re-admitted with an exact local SVG signature because Mermaid 12.1 changes the upstream wrapped message to one line while the native renderer retains two lines; this is a browser text-measurement residual, and Strict remains blocking.
- The State three-way-concurrency fixture also changes compound routing under the new
  `elk.orientFeedbackEdges` behavior. The native source port restores semantic edge direction
  after provider layout; the controlled-measurement State regression checks every routed edge's
  path commands and control-point count against the new baseline.

Source rebinding does not approve local signature drift. Current native comparisons must still
validate every local signature, and any changed local output requires separate behavior review.
The 45-fixture Flowchart browser measurement matrix has only one changed upstream artifact: the
three label translations above. Its measured node and route geometry remains unchanged.

## Root reports

`compare-all-svgs` forwards `--report-root` to diagram families that support the root-delta report.
Text measurement is always deterministic; there is no single-value selector.

Example:

- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root`

The Linux release lane follows the successful DOM/semantic comparison with
`npm run oracle:root-viewport --prefix playground/tests`. The browser oracle mounts the generated
SVGs in Chromium, captures transparent alpha outside each root, and compares structural overflow
with the pinned upstream artifact. Browser-owned SVG text, HTML label paint, and RoughJS output are
diagnostic. A new structural overflow edge or a deeper edge is blocking. The only exact root-paint
receipt catalog is `fixtures/_verification/root-viewport-residuals.json`; each entry binds both SVG
hashes, and stale or unused entries fail rather than widening a tolerance.

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
