# Astro-Mermaid issue 77: ownership and integration assessment

Date: 2026-10-26. Analysis only: no production edit, Cargo build, new timing campaign,
upstream comment or pull request. The previous native performance objective remains suspended.

## Decision

The feedback justifies a targeted typography correctness repair and a JavaScript icon-service
extension, not another general XML, theme or rendering rewrite. Merman's speed and closer output
are useful adoption signals; clipping and the browser payload are genuine disadvantages.
Do not dismiss the clipping as merely approximate fonts or the comparison as worthless.
Conversely, no icon registration, different Mermaid baselines/defaults and an untested server
transport are not proof that the core lacks those features.

## Evidence and version boundaries

- [Issue 77](https://github.com/joesaby/astro-mermaid/issues/77), updated 2026-10-24, had no
  comments when inspected. Its results are one shared-sandbox observation, not a controlled
  Merman revision comparison.
- Astro results/source: commit `1460832791b3c517907407da0a6d28049ad0a5b7`, PR 80.
  That branch lacks the Merman engine file; the actual four-line adapter comes from
  [PR 79 commit ff3c730](https://github.com/joesaby/astro-mermaid/blob/ff3c730ede90eb2701e568199b9920d075725944/spike/engines/merman.js).
  It calls `initMerman()` and `renderSvg(source)` without options or a host measurer.
- Actual published packages: `@mermanjs/{web,web-render,node,node-wasm}@0.9.2`, downloaded
  without npm installation/lifecycle execution and checked against registry SHA-512 integrity.
  Release source: `8e303a43b548c9e40e73f0bc657925d66e7d4fb5`, Mermaid 12.1, Options JSON 2.
- Current remote main: `93034e2076322e6e52515999e6b3c39cf4d69aff`, inspected as a separate
  source snapshot. Main's package version also says 0.9.2, but its Mermaid 12.2, compiled themes,
  prepared sources and transport 8 remain unreleased. See the
  [release boundary](https://github.com/Latias94/merman/blob/93034e2076322e6e52515999e6b3c39cf4d69aff/docs/release/README.md).
  No current-main runtime qualification is claimed here.

Raw evidence, browser screenshots/requests and independent slice reports are retained in
`target/research/astro-mermaid-77-20261026/` in this worktree. `provenance.json` records package
identities and the original scratch layout. The browser experiment is a correctness probe,
not a benchmark or an admitted production fix.

## What the timing and size results actually establish

The recorded first-call sums are 5,814.5 ms for Mermaid.js, 1,129.8 ms for Merman and
653.4 ms for the prebuilt Typst mmdr artifact. The sums of per-diagram warm medians are
4,882.1 / 662.3 / 549.1 ms. Merman's recorded warm result is below Mermaid.js for all 79
inputs, but below mmdr for only ten. These are descriptions of the submitted JSON, not new
measurements, confidence intervals or an equal-quality ranking.

The [harness](https://github.com/joesaby/astro-mermaid/blob/1460832791b3c517907407da0a6d28049ad0a5b7/spike/bench.html)
excludes top-level imports from initialization timing, uses one instance across sequential
sources, and does not publish final SVGs to the page during timing. Merman uses deterministic
measurement; Mermaid.js uses its browser rendering path. Changing measurement policy must
not retain the old speed number while claiming the new policy's quality.

The [comparator](https://github.com/joesaby/astro-mermaid/blob/1460832791b3c517907407da0a6d28049ad0a5b7/spike/compare.mjs)
uses ASCII word-set similarity and dimensions, not visible text containment or semantic
equivalence. Zero exceptions and text similarity 1 do not catch clipped labels. The mmdr
download script also does not bind the observed binary to a digest. These limitations qualify
the findings without negating the reported useful differences.

Published artifact bytes independently match the size concern:

| Package 0.9.2 | Raw WASM bytes | npm tarball bytes |
| --- | ---: | ---: |
| `@mermanjs/web` | 13,382,222 | 5,236,735 |
| `@mermanjs/web-render` | 11,394,688 | 4,543,126 |

Render-only saves 1,987,534 raw bytes, 14.85%, but does not make Merman comparable to the
reported 2.1 MB mmdr artifact. Tarball size is not HTTP-encoded WASM size. Full additionally
ships analysis/editor/ASCII, while render retains all diagram families, math and both layout
backends. This existing product split should be used before considering a new narrow SKU.

## Confirmed clipping and a causal browser defect

The archived class-2 SVG still contains full `Vehicle` and `Driver` text. A clean Chromium
156 macOS arm64 session displays the reported clipping, and the actual published WASM
reproduces it. Merely enabling `createBrowserTextMeasurementSession()` does not solve it.

The measurement request contains a font family ending in a declaration semicolon:
`"trebuchet ms", verdana, arial, sans-serif;`. Assigning
`normal bolder 14px ...;` or `normal bolder 16px ...;` to Canvas `context.font` is rejected;
the getter stays `10px sans-serif`. Both calls therefore measure `Vehicle` as 32.24609375 px.
Without the trailing separator, the actual fonts become bold 14/16 px and the corresponding
widths become 47.7900390625 / 54.6171875 px. This is a direct browser control, not a deduction
from a screenshot or a missing-font theory.

| Class-2 scenario | Vehicle container width | Actual text width, local SVG units |
| --- | ---: | ---: |
| Published default | 40.3125 | 48.265646 |
| Published browser measurer | 32.25 | 48.265636 |
| Classic + browser measurer | 32.25 | 54.624981 |
| Classic + callback-only family normalization | 54.625 | 54.624975 |

The final row is a diagnostic callback that removes only the trailing separator, not a
repository patch. Driver similarly reaches 46.359375 versus 46.359373 px. Default styling
still has a measurement/output font-family mismatch and a small residual even with that
normalization; this experiment does not certify all labels, fonts or diagrams.

Current-main [canvasTextWidth](https://github.com/Latias94/merman/blob/93034e2076322e6e52515999e6b3c39cf4d69aff/platforms/web/src/runtime-render.ts#L418-L439)
still concatenates the request into `context.font` and trusts the resulting measurement without
detecting rejected assignment. However, current Class typography already normalizes its layout
family and removes the trailing semicolon before these requests: see
[Class typography](https://github.com/Latias94/merman/blob/93034e2076322e6e52515999e6b3c39cf4d69aff/crates/merman-render/src/class/theme/text.rs#L377-L386).
Do not claim the current default Class path reproduces that exact published invalid-font input.
The generic adapter remains vulnerable to invalid requests, and current Class still explicitly
uses different root-first measurement and theme-first stylesheet families. That is source-level
evidence of a remaining ownership concern, not a current-main screenshot reproduction. A
current-main renderer has not been built in this investigation.

### Correct repair boundary

Separate the upstream wrapping-budget probe from final displayed-label measurement. The
former may intentionally use root configuration; the latter must consume the effective CSS
family, size, style and weight. Do not flatten distinct upstream probe/render sizes into one.
Mermaid 12.2 itself uses a
[calculateTextWidth budget followed by real HTML bounds](https://github.com/mermaid-js/mermaid/blob/1fb7869866dd619168d840291c9c54b4f129758b/packages/mermaid/src/diagrams/class/shapeUtil.ts#L117-L168).
Reuse the existing Class prepared-label plan and CSS parsing/font ownership, rather than copying raw CSS
declaration syntax into a font-value field. Preserve quoted semicolons inside family names;
the diagnostic suffix removal is not a proposed general parser. The browser adapter must not
silently report a stale/default Canvas font after invalid assignment. Reuse the existing host
measurement protocol and make unavailable measurement follow its documented fallback policy.

Do not add fixture padding, globally expose overflow, force every render through a browser,
reintroduce font heuristics or change deterministic rendering into ambient font discovery.
Tests must cover actual visible bounds, font-ready behavior, malformed/quoted family values,
relative weights and sequential requests that could otherwise reuse stale Canvas state.

## Other feedback: which layer owns the change?

| Feedback | Assessment | Appropriate action |
| --- | --- | --- |
| Square boxes / per-class colors | The comparison uses Mermaid.js 11.17.2 versus Merman's Mermaid 12.1 defaults, with no equivalent options forwarded. Not established as a new theme bug. | Compare equivalent `site_config` theme/look/font settings first; keep explicit version-specific behavior. |
| Icon packs show `?` | Neither spike registers packs, so the run does not test pack support. Rust already has an icon registry, but public Web/Node constructors do not expose registry injection. | Add the missing JS constructor/session service by reusing the existing immutable registry owner; host loads pack data. |
| Web cannot SSR | An intentional browser package boundary, not a DOM dependency of the Rust core. | Use the already published Node native or explicit Node-WASM transport; do not remove the Web guard. |
| SSR requires native | Incomplete: `@mermanjs/node-wasm@0.9.2` exists. | Direct published Node-WASM smoke renders light/dark Flowchart and the class-2 source; this is not a full Astro integration test. |
| Dark / autoTheme | Not exercised by the spike. Mermaid dark config already has a binding path; page theme observation belongs to Astro. | Forward theme/config correctly; for SSG consider two uniquely namespaced theme variants selected by host CSS. |
| New headless SDK | C/WASI SDK services do not automatically expand Node/Web APIs or published 0.9.2. | Reuse existing Node packages for SSR. Add Node memory-font construction only if font-aware SSR is a defined requirement. |

The [transport contract](https://github.com/Latias94/merman/blob/93034e2076322e6e52515999e6b3c39cf4d69aff/crates/merman-bindings-core/src/transport_contract.rs#L140-L174)
permits no Node constructor services and only host text measurement for Web. The icon gap
therefore cannot be fixed by stuffing `iconPacks` into site configuration. Network loading,
pack URL policy and Astro lifecycle should remain outside the Rust renderer. Existing
`BindingEngineServices` and registry admission should remain the implementation owner.

The published [Node-WASM package](https://github.com/Latias94/merman/blob/8e303a43b548c9e40e73f0bc657925d66e7d4fb5/platforms/node/packages/node-wasm/package.json)
is Node-specific, not a universal edge/Deno/browser transport. Its smoke here establishes
three successful direct package calls, not native-platform installation or Astro SSR fidelity.
Node does not currently expose the font constructor/callback services of the larger SDK.

## Recommended order and acceptance

1. Repair the typography value/measurement/output seam, respecting main's existing input
   normalization rather than reimplementing it. Freeze these class examples as
   regressions, test the common CSS value owner and actual browser measurement, then verify
   representative labels across HTML and native SVG paths. Consider a compatible maintenance
   backport only after release-line and main behavior are separately tested.
2. Add scoped Web/Node icon registry construction using existing Rust services. Keep pack
   loading asynchronous and host-owned, data admission bounded, and native/Node-WASM exposure
   explicit. Test register-name overrides, missing icons, isolation and disposal.
3. Document/test a thin Astro adapter using Web-render in browsers and one reusable Node
   engine at build time. Forward version-correct config, unique page/block/theme IDs and
   diagnostics. Preserve host HTML admission and do not confuse `resvg-safe` with a general
   browser-safety or fidelity guarantee.
4. Treat size as a real product tradeoff. Prefer render-only or build-time output first;
   require an actual deployment byte/capability budget before maintaining another artifact.

No broad performance rerun, XML refactor, generic plugin framework or shared mutable global
registry is justified by this issue. These are concrete correctness and integration tasks,
not a resumption of the suspended native-regression program.

## Appendix: release Class residual review after the HTML title repair

This subsequent implementation review covers only the eight existing `stable_edge` receipts
for `class/stress_class_many_relations_labels_020`, against the release line's pinned Mermaid
12.1.0 baseline. It does not refresh upstream SVGs or establish browser pixel parity.

The final title now uses `class_prepare_html_label` in
`crates/merman-render/src/class.rs`: the emitted XHTML is measured with the effective CSS font,
including the title group's `bolder` weight. This replaces the previous Markdown measurement
path and its 1/64-pixel width grid (`crates/merman-render/src/text/wrap.rs`). The fixture's
single-letter deterministic title width changes from 8.40625 to 8.4; the rendered title height
remains 21. The existing wrapping-budget probe remains separate from final HTML measurement.
This follows Mermaid's `packages/mermaid/src/diagrams/class/shapeUtil.ts` distinction between
the width-budget calculation and final `getBBox`/`getBoundingClientRect` bounds.

The resulting geometry is not merely a horizontal translation. Existing strict `outside`
boundary comparisons in `crates/merman-render/src/elk_edge_geometry.rs`, followed by terminal
straightening in `crates/merman-render/src/elk_terminal_jogs.rs`, can select different terminal
segments after node widths change. In particular, `id_A_C_2` (`reads`) retains a jog and its
label anchor changes from y=144.000 to y=148.833; several target port rows and terminal bends
also change. The eight directed source/target identities are unchanged, and all 16 extracted
endpoints still attach to the intended side of their owning node. The old blanket claim that
numeric port rows and directed route geometry agree with the browser baseline is therefore
removed rather than carried into the new receipts.

Review evidence:

- The existing full Class structure report records 251 selected, 250 rendered and one
  intentional skip. Its only mismatches are these eight stale exact semantic geometry
  receipts; it is not a passing gate report.
- From the release worktree, the existing release `target/release/xtask` was run with
  `MERMAN_EMIT_LABEL_RESIDUAL_CANDIDATES=1 compare-class-svgs --check-dom --dom-mode structure
  --dom-decimals 3 --filter stress_class_many_relations_labels_020`. It emitted exactly the
  eight registered keys. The existing extractor emits candidates only after label text,
  explicit label presentation, associated edge presentation (including markers), and relevant
  stylesheet signatures pass semantic checks. Candidate mode intentionally exits unsuccessfully
  and does not admit a comparison.
- The input SHA-256 remains
  `6134c7861579118e7e5849aff872d7f0585ac1bad5ac067bf29a256d2a3cc92c`; the upstream SVG SHA-256
  remains `cb5543297d473c9b3b3a55c09b321398c6e7dbd7fd2863c5158a46a290ed7341`.
- Only each receipt's `local` and `reason` fields change. `evidence_kind` remains
  `browser-measurement`: it identifies the pinned browser-versus-deterministic measurement
  residual, not a new browser measurement campaign or pixel-parity certification. The existing
  three-decimal exact contract remains in force. The catalog is compiled through `include_str!`,
  so admission must be checked by the maintainer's rebuilt release gate.

The captured candidates, actual release SVG and pre-update full report are retained under
the repository root's `target/research/class-label-residual-refresh-20261026/`. Main requires
its own renderer evidence and candidate capture; release coordinates are not evidence for main.
