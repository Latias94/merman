# ADR-0024: DOMPurify Default Allowlists and Generation

## Status

Accepted

## Context

Mermaid uses DOMPurify to sanitize diagram labels/tooltips when `flowchart.htmlLabels` is enabled.
DOMPurify behavior is not just “remove `<script>`”, it also:

- removes unknown tags (while keeping their text content),
- removes unknown attributes,
- applies a URL attribute validation policy,
- preserves `<a target="...">` via Mermaid's DOMPurify hooks.

Our initial headless sanitizer was intentionally minimal, which caused observable drift from Mermaid
in cases like:

- unknown attributes (e.g. `<b foo="bar">`),
- custom elements (e.g. `<custom-tag>`),
- entity-decoded URL schemes in attributes (e.g. `javascript&colon;...`).

For `merman` parity, we need the same default allowlists that DOMPurify ships with (for HTML/SVG/MathML),
and a reproducible way to keep them pinned to Mermaid's baseline dependency version.

## Decision

- Pin the shared browser, reference-CLI, and Rust generation baseline to the release-contract
  version (`dompurify@3.4.17` for the selected Mermaid 12 reference graph).
- For a dependency-only sanitizer patch, regenerate every upstream family once and inspect every
  changed SVG before accepting baseline churn. The historical 3.4.13 refresh changed no generated
  allowlist entries; its candidate SVG deltas were limited to generated node IDs, the wall-clock
  Gantt `today` marker, and external-image loading. That comparison used Mermaid's published
  bundle, which embeds its own sanitizer, and does not prove that the selected npm override ran.
- Build reference execution from Mermaid's module entry point and the selected dependency graph.
  The shared `tools/mermaid-cli/reference-runtime.mjs` builder exposes the same DOMPurify instance
  imported by Mermaid. The renderer must observe a strict HTML-label render calling that instance
  before recording its version and the built artifact's digest in baseline provenance.
- The Mermaid 12 refresh adds the upstream SVG attributes `pointer-events` and `vector-effect`
  from DOMPurify 3.4.15. Browser DOM-node and in-place mutation fixes do not map to the Rust
  string-input sanitizer API.
- DOMPurify 3.4.16 addresses GHSA-p98j-92pf-mc4p in live DOM-node sanitization with
  in-place hooks. Mermaid and the Rust sanitizer use string inputs; no corresponding live-node
  API is introduced. The published browser bundles are tested because this patch also changes
  the upstream bundler from Rollup to Rolldown.
- Generate a Rust module containing DOMPurify's default allowlists via `xtask`:
  - source: `repo-ref/dompurify/dist/purify.cjs.js`
  - output: `crates/merman-core/src/generated/dompurify_defaults.rs`
- Treat `repo-ref/dompurify` as required reference material for the DOMPurify generated-artifact
  gate. If it is absent, `xtask` reports an actionable missing-reference error that points to
  `tools/upstreams/REPOS.lock.json`.
- Verify generated allowlists using:
  - DOMPurify allowlists only: `cargo run -p xtask -- verify-dompurify-defaults`
  - umbrella generated-artifact check: `cargo run -p xtask -- verify-generated`
- Implement a DOMPurify-inspired tag/attribute validation step in `merman-core::sanitize` driven by:
  - `DEFAULT_ALLOWED_TAGS` / `DEFAULT_ALLOWED_ATTR`
  - `DEFAULT_URI_SAFE_ATTRIBUTES` / `DEFAULT_DATA_URI_TAGS`
  - `DEFAULT_FORBID_CONTENTS`, SVG/MathML namespace tag sets, and HTML integration points
  - Mermaid's target-preservation hook semantics (`<a target="...">` survives sanitization, and
    `target=_blank` forces `rel=noopener`)
- Run policy validation on the recovered HTML5 tree (ADR-0021), whose attribute values already
  have browser entity-decoding semantics. Keep the separate entity decoder only for consumers
  validating serialized SVG URI attributes.

## 2026-10-25 Companion Security Selection

Select DOMPurify `3.4.17` at tag `3.4.17`, commit
`a77a3e78b59f72fa4d1c83f22e121ad9cc7d3575`. This replaces the `3.4.16` sanitizer
selection after GHSA-cg2g-xh64-v4q7, GHSA-mrv2-3pqr-j5jr, and GHSA-c6gr-qvv4-5v3f.
Select Puppeteer and Puppeteer Core `25.16.0`, with `@puppeteer/browsers@3.2.6`, at
commit `a207faf527ce4305b33a6c0f9706b090c21c115e`. The updated BiDi dependency uses
the patched Zod 4 line. Preserve each release line's Mermaid selection (`12.2.0` on
main and `12.1.0` on the maintenance line), all other selected companions, and KaTeX.

Node `24.21.0` and npm `12.0.2` installed the locked graph with lifecycle scripts
disabled. The official npm signatures command passed independently on both lines,
with no invalid or missing records and 56 verified attestation records each. The
selection receipts bind the preserved raw stdout digests and exact transitions from
the respective trusted base selections; registry signatures also cover DOMPurify,
which does not publish an attestation in this output. DOMPurify's selected Apache-2.0
license is byte-identical to the canonical license already stored in the repository.

Bounded behavior evidence comprises eight reference-runtime tests on each line, six
parser-oracle tests on main, and actual Flowchart rendering and unsafe-HTML cleaning
through the shared sanitizer on both lines. These browser probes used the explicitly
selected local Chrome Headless Shell `151.0.7922.77`; they do not claim validation of
Puppeteer's new default browser. The 2026-10-13 Mermaid admission and older release
records retain their historical package identities. Source selection does not relabel
existing generated allowlists or SVGs: the existing generation, provenance refresh,
and strict comparison owners must still validate those artifacts before acceptance.

The main-line recovered-tree sanitizer also applies the `SAFE_FOR_XML` markup probe
to explicitly allowed `noscript` text, matching the new string-input behavior.
Its regression covers entity-authored markup and the explicit `SAFE_FOR_XML: false`
opt-out. The maintenance line retains its separate legacy HTML rewriter; updating
its generated allowlists does not claim a backport of main's recovered-tree API.

### Reference artifact review

The existing owner rerendered all 37 families: 3,737 SVGs on main and 3,720 on
the maintenance line. The manifests record the actual Darwin ARM64, `en-US`,
Chrome `151.0.7922.77`, runtime artifact and font-probe identities rather than
retaining the former Windows environment. Respectively 1,173 and 1,164 SVGs
changed. XML review found numerical measurement/layout changes, one Class title
split moving without losing characters, and seven Sequence note reflows without
changing their complete text, participants, messages or explicit line breaks.
These artifact changes do not introduce renderer constants or comparator rules.

Review rebound 50 existing browser-text snapshots per line. Five Sequence cases
now match all three DOM modes and their receipts are retired, reducing the main
catalog from 132 to 127 entries and maintenance from 139 to 134. Retained entries
keep the same inputs, local signatures, modes and precision. The three changed
semantic-label fixture bindings retain their identities and local geometries;
the owner re-extracted five C4 upstream geometries after numerical-only XML
review. Class and Requirement change only generated node-label `max-width`
styles, not their registered edge-label geometry.

Four ELK fixtures were instrumented again on each line:

- `upstream_cypress_flowchart_handdrawn_spec_fdh27_set_text_color_of_nodes_and_links_according_to_styles_when_027`
- `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020`
- `upstream_cypress_flowchart_spec_24_keep_node_label_text_if_already_defined_when_a_style_is_appli_024`
- `upstream_cypress_newshapes_spec_newshapessets_newshapesset6_lr_md_html_false_094`

All eight instrumented SVGs are byte-identical to their regenerated baselines.
The captured invocation inputs and raw provider outputs justify four SVG hash
updates and 18 numerical scalar updates per line. Two additional serial replays
verify the SVG-text label dimensions through `labelHelper`'s effective bbox and
`withMinWidth`, rather than substituting the final label group's raw bbox. Those
label dimensions remain unchanged. Other entries retain their original captured
measurements and browser identity; this is not a fresh collection of all 45
fixtures. No routes are reconstructed from final SVG paths.

The XYChart out-of-domain paint receipt was independently replayed through each
line's unchanged oracle (v11 and v10), using Playwright `1.64.0` and Chromium
`156.0.8078.4`. Old-reference controls and the reviewed new-reference entry pass
strict matching; the old entry correctly rejects the new upstream bytes. Root
dimensions, capture bounds, eligibility and structural overflow depth do not
change, and main's live decision fingerprint remains identical. Only the bound
upstream SVG hash changes. This retains the existing out-of-domain diagnostic;
it does not repair clipping, admit a new residual or alter any paint threshold.

## Consequences

- `sanitizeText` / `removeScript` outputs are closer to Mermaid's actual DOMPurify behavior and more
  robust against bypass inputs.
- We can update allowlists deterministically by updating `tools/upstreams/REPOS.lock.json`,
  materializing `repo-ref/dompurify`, and regenerating.
- Default namespace transitions are covered by the generated sets and HTML5 tree builder.
  Custom element handling and the complete DOMPurify configuration surface remain incremental.

## References

- Mermaid common sanitizer: `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- DOMPurify dist baseline: `repo-ref/dompurify/dist/purify.cjs.js`
- DOMPurify source lock: `tools/upstreams/REPOS.lock.json`
- Generator: `crates/xtask/src/main.rs`
- Generated allowlists: `crates/merman-core/src/generated/dompurify_defaults.rs`
