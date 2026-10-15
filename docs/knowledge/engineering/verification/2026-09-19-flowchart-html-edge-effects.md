# Flowchart HTML edge-label effects

Date: 2026-09-19. Base: `7ae2d5994`. Status: scoped validation complete.
Scope: HTML EdgeLabel in U6 of the theme product boundaries plan. This does not close U6,
U9, catalog qualification or C7a.

## Drawing boundary

A text effect must not filter the label background. Ordinary generated labels have one
`div.labelBkg > span.edgeLabel > p` content box. In the pinned Mermaid 11.17.2 browser
output, the div paints the configured RGB at alpha .5 and the paragraph paints the same
RGB at its configured alpha. The inline span around a block paragraph does not paint an
additional layer. Therefore these two equal-area colors compose to alpha `.5 + .5 * a`.

The effect path keeps one foreignObject and its natural HTML layout. Its div paints the
composited background, the span and paragraph are transparent, and the paragraph alone
references the existing shared SVG shadow graph through CSS. The padded outer SVG rectangle
remains separate. Existing background selection, explicit source/config precedence and
background evidence stay with FlowchartLabelBackgroundPlan. Background Clear remains a
separate unsupported request; clearing a text effect does not clear the background.

An initial implementation moved both background layers to SVG rectangles using layout
measurements. It passed structural and native checks, but a real browser rejected that
approach: an ordinary label measured 50.88px rendered at 59.98px, and an automatically
wrapped label measured 72px high rendered at 96px. The rectangles left part of the label
uncovered. That implementation was replaced rather than qualifying the mismatch or
restricting the feature to opaque colors.

## Native projection

The family writer supplies a local filter projection request on the foreignObject, paired
with the actual paragraph CSS reference. The existing fallback accepts only the canonical
single-paragraph structure and a matching local reference, with no visible text outside it
or attributed/rich descendants. This request is not portable evidence. Projection keeps a
background rectangle outside a filtered SVG text group; actual SVG/usvg filter definitions,
applications and glyph containment still undergo existing native receipt verification.

The existing bounded cascade checks whether the generated elements can remain at their
original position. Only explicitly overridden, normal-priority background paint properties
are isolated. Unbounded or important matching declarations retain the readable fallback,
remove its local filter and leave the native effect receipt incomplete. Added elements,
depth and bytes use the existing postprocess budgets.

Transparent descendants no longer erase their parent's fallback background. The correction
includes transparent keywords, initial/unset and zero-alpha colors, while retaining the
existing no-inheritance, explicit-initial and partial-run boundaries. It does not add general
ancestor alpha compositing or a CSS layout engine.

This corrects the former native single-background approximation on the new effect path.
It does not promise identical native pixels to that approximation. The legacy/configured
`transparent` fade still produces a half-alpha black div; this source behavior is preserved
here, and must not be described as fully transparent theme paint. Resolving that product
semantic requires a separate background-owner change, not an effect-specific exception.

## Verification and limits

Local investigation/probe material:

- `/tmp/flowchart-html-edge-background-20260919/`
- `/tmp/flowchart-html-edge-red.log`
- `/tmp/flowchart-html-edge-browser.json` (rejected rectangle approach)
- `/tmp/flowchart-html-edge-css-probe.{mjs,json,png}` (browser design probe)

The first broad run on the reviewed implementation passed 2,662 of 2,664 tests. It caught
a projection guard rejecting the writer's legal empty CSS declaration before
`background:transparent`; the focused positive fixture now reproduces that exact spelling.
The other failure was an overly specific assertion requiring a separate `Hello` text node
when the readable fallback legitimately combined adjacent text. Neither failure is counted
as a passing final-source run.

Chromium 151.0.7922.34 measured the three production HTML paragraphs and their backgrounds
at exactly the same positions and sizes: 59.984375 × 24, 50.59375 × 48 and 200 × 96px.
The glyph background is transparent, and the configured red alpha .4 becomes the expected
composited .7. Toggling the paragraph filters changes 46,216 channels by more than one;
43,480 of those changes are away from background edges. Reconstructing the original two
background layers on the same DOM changes 96 channels by more than one, all within one
pixel of a background boundary (maximum delta 13); no interior difference over one was
observed. This is a bounded edge rasterization residual, not pixel-identical evidence.
The long-wrap stress probe also retains an existing host allocation/browser wrapping
mismatch; preserving its natural background does not qualify general HTML layout or
complete-preset readability.

The final production build also passed opaque, legacy/configured transparent and asymmetric
padding probes. Every paragraph/background box matched in the browser; native PNG admission
was HostDependent solely because of the system font. Transparent configuration preserved the
legacy half-alpha black background with no raster difference. The opaque probe had 96 changed
edge channels (maximum delta 32), again no interior difference over one. These four results are
archived under `/tmp/flowchart-html-edge-final-matrix/summary.json`. The glyph/background
geometry and pixel toggle checks were rerun on the final build, not inferred from the earlier
browser-only design probe.

Final renderer/facade Release regression passed **2,664/2,664**, with two existing skips.
The suite covers Effect/Clear, source markup/style refusal, semitransparent/transparent/opaque
backgrounds, asymmetric padding, readable fallback and actual PNG/PDF receipts and pixels.
The embedded-font feature was not enabled in this run; no embedded-font HTML qualification is
claimed. Scoped Clippy passed with existing warnings.

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test flowchart_node_effects --test theme_flowchart_edge_effects --test resvg_safe_typography --test-threads 2 --no-fail-fast
CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman -p merman-render --no-default-features --features merman/svg,merman/png,merman/pdf,merman/layout-cytoscape --lib --test flowchart_node_effects --test theme_flowchart_edge_effects --test resvg_safe_typography
```

The full structure gate passed across 35 comparison commands: 3,732 selected fixtures,
3,702 rendered, 30 existing skips and 66 exact reviewed browser-text-layout residual
comparisons. No comparator policy or residual receipt was changed. This proves the selected
raw/source SVG structure policy, not root-viewport, full browser or native parity.

```text
CARGO_BUILD_JOBS=1 cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
cargo fmt --all -- --check
git diff --check
```

The structure log is `/tmp/flowchart-html-edge-dom-structure.log`; its report totals are
in `/tmp/flowchart-html-edge-dom-summary.json`. Formatting and whitespace checks passed.

Independent review completed without retained findings. Seven review lenses and three
simplification lenses ran serially in one separate Codex context, not ten independent
reviewers. The final empty-declaration correction has a separate static addendum. The
six reviewed file hashes match the final tested source; source-set SHA-256:
`968b3fd83594879cf7f112561584f06bf70a1d80c633c942c06ba5f772166a9b`.
Parent verification closes the pending runtime gate for this bounded increment. Review
artifacts: `/tmp/compound-engineering-501/ce-code-review/20260919-192723-html-edge/`.
The original pending verdict remains historical; it is not presented as an independent
runtime rerun.

Final logs: `/tmp/flowchart-html-edge-verified-final.log` and
`/tmp/flowchart-html-edge-clippy.log`. Browser script:
`/tmp/flowchart-html-edge-final-browser.mjs`. Temporary Renderer probe:
`/tmp/flowchart-html-edge-probe.rs`; it is not a project example.

No dependency, embedded font, public schema or public recipe is added. Host layout remains
an allocation rather than exact browser ink measurement. Rich HTML, Markdown, math,
non-classic looks, unsupported source styles and unowned measurement paths retain residuals.
Full-preset scene acceptance, installed packages, the browser matrix and performance/size
comparisons are not established by this increment.
