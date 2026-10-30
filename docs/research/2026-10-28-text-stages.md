# Text preparation follow-up

This follows merged rendering PRs #246 and #247. The follow-up audits source,
layout-probe, and final-terminal boundaries independently. Performance work stays
paused; these correctness results do not establish a performance claim.

## Confirmed counterexamples

| Surface | Trigger | Incorrect behavior | Repair |
| --- | --- | --- | --- |
| State SVG | `state "A&lt;br&gt;B" as A`, HTML labels disabled | Literal characters become a line break or change measured height | Prepare SVG rows before decoding each terminal payload; measure literal rows |
| Usecase HTML Markdown | `markdownAutoWrap:false`, narrow wrapping width | Output NBSP is one unbreakable line but geometry describes several lines | Share configured prepared XHTML between measurement and emission |
| Kanban assigned/ticket | `WWWW<br/>WWWW` | Measure a break, emit its literal spelling | Retain policy-sanitized metadata XHTML and matching geometry |
| Event Modeling identifier | A 90-character `W` identifier | Writer drops measured breaks/hyphens | Retain the generated terminal instead of rebuilding public layout text |
| Event Modeling data | `Data { W W W W W W W W }` | Measuring serialized XHTML changes upstream's source-level probe | Measure generated HTML source first; retain dimensions and final XHTML separately |
| Class HTML notes | `note "A&nbsp;B"` or `note "A<img src=x>B"` | HTML entities/void tags produce invalid SVG XML | Use the XHTML serialization boundary |
| Mindmap HTML | `root["A#60;B"]` | Whole-fragment decode reintroduces a bare `<` after serialization | Restore entity references before HTML consumption |
| SVG Markdown rows | A Markdown label containing `A&lt;br&gt;B` | Writer emits one row, generic height measurement sees two | Union literal styled-word ink at retained row baselines |
| Requirement SVG | A literal first row with a face-specific bbox origin | First-row height and emitted row positions disagree | Preserve the first-row adapter and union ink at retained positions |
| ER generic names | `"A~br~B"` with attributes and HTML labels | The final literal `A<br>B` is measured as two HTML rows | Measure one decoded literal HTML line; preserve the separate no-attributes drawRect branch |
| Browser DOM measurement | 0px/0.5px fonts | Adapter silently raises font size to 1px | Preserve valid CSS sizes and row advances |
| Browser simple-text operations | `A<br/>B` | Literal width differs from pinned drawSimpleText projection | Replace breaks only for the operations that own that projection |
| Canvas measurement | A nonfinite font size | Adapter substitutes 16px and reports handled | Return unhandled |
| Main Class/Kanban/Usecase retention | Small prepared-byte budget, cloned owners, long ticket URL | Some owned strings/containers escape the ledger | Tie reservations to actual shared owners, including generated hrefs |
| Maintenance native math | Global caller `path` CSS with important paint | PNG/JPEG/PDF can disagree with browser formula paint | Explicitly reject actual native formulas plus scoped CSS |

Main and maintenance retain their own prepared-artifact and resource APIs.
Main-only retained accounting is not presented as a new maintenance capability.
The maintenance HTML5 sanitizer backport replaces the streaming HTML rewriter;
its generated DOMPurify policy tables and dependency/legal reports move with it.
Safe HTML5 recovery, namespace serialization, parser work accounting, and final
SVG validation remain explicit contracts.

## Public measurement contract

Main appends `html-line-height` as operation 22 with a non-negative `length` result. Its host assigns the already-decoded string through `textContent` to an unpadded, borderless, nowrap HTML inline-block and returns its client-rect height without interpreting literal `<br>` or decoding entity spellings again.

The 0.9.3 compatibility review defers that extension on maintenance: protocol 1 retains operations 0 through 21, the published exhaustive Rust/UniFFI enum, result kinds, and transport ABI versions. Existing host callbacks continue receiving `Wrapped` and their complete metrics retain their original meaning and provenance. Such hosts can still count literal `<br>` as a line break; an exact host literal-line operation is not part of this patch. Built-in and explicitly installed infallible profiles keep their specialized literal height. The descriptor and generated C, Android, Flutter, Python, UniFFI, and Web projections move together. Maintenance also preserves its existing public Class payload API; main's separate `Arc` retention changes are not backported.

## Independent validation

On main, the theme text/surface oracle now compares the original glyph contribution with
the same browser's unclipped contribution. Partial clip-path, ancestor clipping,
and root viewport clipping are negative controls. Italic whitespace/overhang and
state restoration are positive controls. The observation remains bounded to four
million pixels and uses five screenshots per terminal instead of three.
That main-only oracle does not exist on this maintenance line and is not claimed
as a maintenance change or locally executed maintenance test.

The SDK fixture uses a centered red formula. SVG checks resolve its actual
instance and glyph paths; PNG checks require a bounded area and extent of red
formula ink in the fixture's central region. Cross-runtime pixel equality remains
a separate assertion. Empty formula markers plus a black node frame must fail.

CLI math tests explicitly supply Cousine and disable system-font discovery, so
the native-fonts feature recipe does not accidentally depend on system-fonts.
PNG/JPEG/PDF tests cover both rejection and valid unaffected combinations.

## Evidence limits

- Pinned Mermaid semantics take priority over a universal "measure serialized
  XHTML" rule. Event Modeling's source probe intentionally differs from its
  final HTML text assignment.
- Browser font rendering and numerical getBBox differences remain bounded
  platform residuals. No comparator, root receipt, or tolerance was relaxed.
- SDK formula ink proves presence for the controlled fixture, not completeness
  of every arbitrary formula. The text oracle does not claim to detect every
  possible foreground occluder.
- Cross-family scans target actual consumers of the changed contracts. A clean
  bounded audit is not an exhaustive proof that no text defect remains.

## Verification record (2026-10-29)

These are the original text-stage integration results before the 0.9.3 compatibility adjustment; they do not qualify the final release source. The [0.9.3 readiness record](../release/V093_BACKPORT_READINESS.md) tracks the compatibility regressions and immutable preflight separately.

The record distinguishes executed local checks from workflow-owned platform evidence.
The renderer/layout counts below are focused integrated runs, not a new claim that
an entire default workspace run completed locally.

| Check | main | release/0.9.x |
| --- | --- | --- |
| Integrated renderer library, affected consumers and layout snapshots | 3,646 passed; 3 existing skips | 1,978 passed; 1 existing skip |
| Parse-path contracts | 14 passed | 9 passed |
| Deterministic fuzz regressions | 21 passed | 18 passed |
| Host protocol / bindings / WASM focused tests | 15 passed | 15 passed |
| Real Chromium DOM, Canvas, Pie and ER consumers | 18 passed; no skips | 18 passed; no skips |
| Full browser package smoke | 37 diagrams | 37 diagrams |
| WASI formula-presence contracts | 19 passed | 20 passed |
| CI, CLI release metadata and advisory-governance Python contracts | 122 passed | 109 passed |
| Representative feature matrix | 49 builds plus isolated consumers | 42 builds plus isolated consumers |
| Text protocol, native ABI and binding projections | passed | passed |
| Android Rust and Flutter native consumer verifier | passed | passed |
| Complete structure / parity / parity-root comparison | passed | passed |

The native-font integration file passed 10 cases on maintenance; main's broader
native-font owner run passed 33 cases. Maintenance HTML5 sanitizer and related
core consumers passed 68 focused cases. Dependency closure checks passed on both
lines. License source reports, release projections and the third-party contract
passed; advisory checks report no blocking vulnerability, retaining only the two
existing governed font-maintenance findings. The Windows workflow passed
`actionlint 1.7.12` and `zizmor 1.30.1`; its real download/build result remains with
remote `grammar-owner / generation-windows`.

Main's root painted-content oracle examined 3,737 exact generated SVGs and found
zero blocking containment failures. Maintenance examined its own 3,720 SVGs:
`stress_flowchart_title_padding_subgraph_029` remains the sole macOS fingerprint
failure. Both its local and upstream SVG SHA-256 values exactly match the
existing receipt, but the local Chromium/font audit fingerprint differs. The
root receipt was not refreshed; the blocking Linux owner must validate its
original environment. Main-only generated SVGs left in the shared target were
excluded only after matching their archived main bytes and proving the absence
of a maintenance fixture and upstream. The gate/oracle SVG hashes were compared
again after rerender, so evidence is bound to the same generated outputs.

Independent snapshot review refreshed only 25 existing local canonical hashes
on each line. Two obsolete receipts per line were removed because the ordinary
upstream DOM comparison now matches. Main additionally admits only
parity/parity-root for fixture 8368 after a complete controlled upstream
measurement/layout experiment; see the ELK measurement research on main. No
comparator, numerical tolerance or root viewport receipt changed.

The checked-in [comparison](../assets/text-stages/index.html) and
[screenshot](../assets/text-stages/comparison.png) retain each line's own before
results and full-WASM after results. The build manifests preserve test-only source
drift explicitly; only the full Web package was freshly rebuilt. Other package
variants reused older local artifacts and are not current-source rebuild evidence.
Performance optimization remains paused; functional checks and compiled discovery
do not establish timing or memory regression claims.
