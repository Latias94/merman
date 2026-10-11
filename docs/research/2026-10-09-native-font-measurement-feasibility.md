# Native Font Measurement Feasibility

Date: 2026-10-09. Scope: opt-in CLI measurement consistent with native
PNG/JPEG/PDF painting. This is research, not an implementation or browser parity
promise. **Verified** means source inspection or an experiment below;
**Inference** means a design conclusion; **Open** requires further work.
Sources use repository-relative path:line.

## Assessment

**Inference:** feasible as a bounded native capability. Host routing exists and
the export crate already owns a font-aware SVG dependency. Connect these owners
rather than adding fonts or browser-font tables to the core.
Sources: `crates/merman-render/src/environment.rs:875`,
`crates/merman-export/Cargo.toml:74`,
`docs/adr/0086-deterministic-text-measurement-without-vendored-font-tables.md:28`.

**Verified limitation:** the 19-operation protocol does not express all geometry
used by renderers. Title vertical extents bypass the host. Improved widths and
wrapping do not establish complete text containment.
Sources: `crates/merman-render/src/generated/text_measurement_abi.rs:57`,
`crates/merman-render/src/text/svg_metrics.rs:30`,
`crates/merman-render/src/svg/parity/flowchart/viewbox.rs:461`.

## Measurement Boundary

**Verified:** four phases exist: `Layout`, `Wrap`, `SvgBBox`, and
`ComputedLength`. One host can serve all four. Requests contain operation,
phase, text, style, width limit, and wrap mode. Unsupported, invalid, and failed
results use the configured fallback and record provenance.
Sources: `crates/merman-render/src/environment.rs:22`,
`crates/merman-render/src/environment.rs:787`,
`crates/merman-render/src/environment.rs:812`,
`crates/merman-render/src/environment.rs:883`,
`crates/merman-render/src/environment.rs:1049`.

**Verified:** specialized operations select their own phase; only `Measure`
uses the facade's default phase.
Source: `crates/merman-render/src/environment.rs:981`.

The operation inventory below is **verified**; adapter requirements are
**inferences** from documented semantics.
Sources: `crates/merman-render/src/generated/text_measurement_abi.rs:57`,
`crates/merman-render/src/generated/text_measurement_abi.rs:146`,
`crates/merman-render/src/text/measure.rs:69`.

| Group | Operations | Adapter Requirement |
| --- | --- | --- |
| Metrics | `Measure`, `MermaidCalculateTextDimensions` | Width, height, lines with operation-specific whitespace |
| Advances | `ComputedLength`, `CanvasMeasureTextWidth` | Shaped advance distinguished from ink width |
| Horizontal extents | `BBoxX`, `BBoxXWithAsciiOverhang`, `TitleBBoxX` | Anchor-relative asymmetric bounds |
| Widths | `SimpleBBoxWidth`, `RawBBoxWidth`, `TspanBBoxWidth`, `WrapProbeBBoxWidth`, `BoundingClientRectWidth` | Preserve SVG/HTML/tokenization distinctions |
| Vertical scalars | `TspanBBoxHeight`, `SimpleBBoxHeight`, `RawBBoxHeight`, `CreateTextBBoxYOffset`, `CreateTextMiddleBBoxYOffset` | Height and baseline-offset semantics |
| Wrapping | `Wrapped`, `WrappedWithRawWidth` | Consistent wrapping plus requested raw width |

**Verified limitation:** `TextStyle` contains family, size, weight, and style;
`TextMetrics` contains width, height, and line count. Neither carries explicit
line height, letter spacing, language, direction, variable axes, or a full bbox.
Sources: `crates/merman-render/src/text/types.rs:18`,
`crates/merman-render/src/text/types.rs:38`.

**Verified:** the width callback retains approximate line heights, vertical
metrics, and baselines. It cannot supply full native geometry.
Source: `crates/merman-render/src/text/deterministic.rs:93`.

## Injection And Ownership

**Verified:** Mermaid CLI rendering creates a deterministic environment before
its SVG request and has an environment replacement method. Raw SVG conversion
creates a separate environment for postprocessing.
Sources: `crates/merman-cli/src/config.rs:39`,
`crates/merman-cli/src/config.rs:304`,
`crates/merman-cli/src/render/prepare.rs:538`,
`crates/merman-cli/src/render/execute.rs:219`.

**Inference:** graphical-output preparation is the integration point. Create a
font context before layout or SVG conversion, install its host policy, and pass
it to export. Post-SVG-only changes leave node geometry already computed.
Sources: the boundaries above and
`crates/merman-render/src/environment.rs:1587`.

**Verified:** native exporters configure usvg internally and parse finished
SVGs. Both use a private shared system-font database. Current entry points take
export options, not a caller-owned font context.
Sources: `crates/merman-export/src/lib.rs:1409`,
`crates/merman-export/src/lib.rs:1449`,
`crates/merman-export/src/lib.rs:1457`,
`crates/merman-export/src/lib.rs:1718`,
`crates/merman-export/src/lib.rs:2764`.

**Verified:** selection respects ordered families, case-insensitive names,
weight/stretch/style, and default missing-character fallback. Raster and PDF
have separate helpers, but inspected fallback ordering and monospace detection
currently implement the same logic; separation is not a current discrepancy.
Sources: `crates/merman-export/src/lib.rs:2783`,
`crates/merman-export/src/lib.rs:2795`,
`crates/merman-export/src/lib.rs:2807`,
`crates/merman-export/src/lib.rs:2941`,
`crates/merman-export/src/lib.rs:2966`,
`crates/merman-export/src/lib.rs:3029`.

**Inference:** keep ownership in the existing export boundary. One context owns
discovery and consistent resolver options for measurement and painting. Neither
a provider registry nor system discovery inside the renderer is necessary.
Sources: `crates/merman-export/Cargo.toml:74`,
`crates/merman-export/src/lib.rs:2736`,
`crates/merman-render/src/environment.rs:875`.

## foreignObject And CSS

**Verified:** foreignObject conversion obtains the session's controlled Wrap
measurer. Its width probes call `measure_wrapped` with `HtmlLike`; one shared
host policy therefore reaches fallback conversion as well as diagram layout.
Sources: `crates/merman-render/src/svg/pipeline/builtin/foreign_object.rs:31`,
`crates/merman-render/src/svg/pipeline/context.rs:329`,
`crates/merman-render/src/svg/fallback/html.rs:144`.

**Verified limitation:** conversion is explicitly best effort. It turns break
tags into newlines, strips other tags, and emits text with one resolved style.
Mixed inline runs are not preserved as independently styled runs by this
extraction path; accurate shaping cannot restore removed styling.
Sources: `crates/merman-render/src/svg/fallback.rs:28`,
`crates/merman-render/src/svg/fallback/html.rs:113`,
`crates/merman-render/src/svg/fallback.rs:196`,
`crates/merman-render/src/svg/fallback.rs:254`,
`crates/merman-render/src/svg/fallback.rs:1458`.

**Verified limitation:** fallback CSS admits bounded font, line-height, and paint
properties, and rejects `var()`, `calc()`, and `env()`. Its line height is outside
`TextStyle` and directly positions output lines. Improving horizontal probes
does not make arbitrary HTML/CSS layout supported.
Sources: `crates/merman-render/src/svg/fallback/cascade.rs:91`,
`crates/merman-render/src/svg/fallback/cascade.rs:112`,
`crates/merman-render/src/svg/fallback/cascade.rs:1268`,
`crates/merman-render/src/svg/fallback/cascade.rs:1282`,
`crates/merman-render/src/svg/fallback.rs:263`.

## Vertical Geometry And PDF

**Verified limitation:** title ascent/descent are fixed font-size multiples.
Flowchart uses the host for horizontal title extents but the fixed helper for
vertical extents. ER, gitgraph, requirement, state, and usecase also call it.
Existing height/baseline operations do not replace these call sites.
Sources: `crates/merman-render/src/text/svg_metrics.rs:5`,
`crates/merman-render/src/text/svg_metrics.rs:30`,
`crates/merman-render/src/svg/parity/flowchart/viewbox.rs:459`,
`crates/merman-render/src/svg/parity/er/render.rs:649`,
`crates/merman-render/src/svg/parity/gitgraph/render.rs:1297`,
`crates/merman-render/src/svg/parity/requirement/render.rs:511`,
`crates/merman-render/src/svg/parity/state/render.rs:390`,
`crates/merman-render/src/svg/parity/usecase.rs:287`.

**Inference:** complete containment requires a narrowly defined vertical-extents
operation and caller migration, or another explicit geometry contract. Do not
choose a protocol change without a measured affected case; do not promise full
bounds from an unchanged protocol-v1 adapter.

**Verified:** PDF parses through usvg and paints through krilla-svg, with page
transforms and localized filter rasterization. It can share font policy but is
still a separately validated backend.
Sources: `crates/merman-export/src/lib.rs:1718`,
`crates/merman-export/src/lib.rs:1730`,
`crates/merman-export/src/lib.rs:1745`,
`crates/merman-export/src/lib.rs:2050`.

**Open:** check PDF embedded glyphs and extraction for CJK, combining marks,
ligatures, and emoji. Shared discovery alone does not establish these results.

## usvg API Experiment

An isolated executable is under `target/native-font-research/`, outside the
workspace and ignored by Git. Its local manifest pins usvg/resvg 0.47.0 and uses
the current Merman library through a path dependency. It does not modify the
production renderer, CLI, exporter, workspace manifest, or workspace lockfile.
The source, machine-readable results, and SVG/PNG artifacts are retained there.

**Verified API facts:** `Tree::from_str` accepts `Options::fontdb` and
`font_resolver`. `Text::bounding_box()` exposes the SVG font-metric box, not a
tight ink box. `Text::flattened().bounding_box()` exposes path/image bounds in
the text's local coordinates; transform these with `Text::abs_transform()`
before comparing with the final viewport. `Text::layouted()` exposes positioned
glyphs, their actual font IDs, and glyph IDs. `Tree::fontdb()` resolves those
IDs to selected font faces. These public APIs are sufficient for the prototype;
no upstream patch or private API is needed.
Sources: usvg 0.47.0 `src/tree/text.rs:646`, `src/tree/text.rs:704`,
`src/tree/text.rs:714`, `src/tree/mod.rs:1670`, `src/text/layout.rs:28`,
`src/text/mod.rs:23`, `src/parser/mod.rs:146`.

**Verified limitation:** positioned glyphs do not expose an advance field, and
the font-metric box is not a general `getComputedTextLength` replacement. usvg's
internal layout computes cluster advances, bidirectional runs, variable axes,
and missing-glyph fallback. An independent Rustybuzz width loop would need to
reproduce those policies to agree with export. Do not infer that raw shaping of
one chosen face solves all 19 operations.
Sources: usvg 0.47.0 `src/text/layout.rs:28`, `src/text/layout.rs:400`,
`src/text/layout.rs:857`, `src/text/layout.rs:1410`.

The prototype answers only `TitleBBoxX`. Other host requests return `Ok(None)`
and use the existing deterministic fallback. For the supported request it takes
the union of the centered font-metric and ink boxes. This is a conservative
native containment probe, not a claim to reproduce Chromium's exact bbox.
The final SVG is independently converted and rasterized with the same context.
The production typed exporter is unchanged and still needs a context injection
interface. The experiment uses usvg's default selector, not Merman's production
case-insensitive/fallback helpers; production integration must share those too.

### Observed results

Experiment date: 2026-10-09, Windows 11, optimized Cargo release build, one
process/run on the current machine. The actual supplied repository fixture is
`fixtures/flowchart/stress_flowchart_title_padding_subgraph_029.mmd`.

| Title font | Default viewport width | Native-title viewport width | Default horizontal metric overflow | Native metric / ink overflow |
| --- | ---: | ---: | ---: | ---: |
| Arial | 454 px | 454 px | 0 px | 0 / 0 px |
| Consolas | 454 px | 484.9277 px | 30.9277 px total | 0 / 0 px |
| Times New Roman | 454 px | 454 px | 0 px | 0 / 0 px |

**Verified:** the existing public callback can fix a concrete native title
overflow without modifying the core. The current source fixture's default
viewport differs from the historical 376px diagnostic in the September note;
that old measurement must not be presented as the current experiment result.
PNG files were encoded successfully, and the metric and transformed path bounds
above were checked numerically. A visual screenshot inspection was unavailable
because this session's image-view tool rejected image inputs. This is not a
pixel-level visual oracle or a complete text-containment audit.

At 18px, Arial `PostInstall` measured 83.0391px regular and 91.0195px bold;
actual face IDs resolve to ArialMT and Arial-BoldMT respectively. Italic `fj`
selected Arial-ItalicMT and its ink right edge exceeded its 9px metric width.
The mixed CJK sample selected DengXian for the entire observed run, not just its
Chinese characters. Arabic selected ArialMT; emoji selected SegoeUIEmoji.
The combining sequence `e + U+0301` produced one positioned glyph. An unsupported
U+10FFFF produced glyph ID zero: successful parse is not proof of complete glyph
coverage. Empty/all-ASCII-space micro-SVGs produced no text node. Those cases
need explicit adapter semantics, not blanket parse-success handling.

### Performance evidence

| Optimized single-run workload | Time |
| --- | ---: |
| Discover 576 system faces | 429.59 ms |
| 1000 identical probes, original file-backed system database | 24775.97 ms |
| 1000 identical probes, one in-memory Arial face | 135.46 ms |
| 1000 identical probes, full 576-face database with only selected Arial replaced by in-memory data | 139.69 ms |
| 1000 distinct probes, that same full database | 153.89 ms |
| 1000 bbox reads from one already parsed tree | 0.0426 ms |

The retained Arial data is 1,047,208 bytes. Its in-memory and file-source probe
widths match within 0.0001px. The full-database experiment controls database size
and keeps the other system entries; the large reduction supports retaining
selected font bytes rather than reopening file-backed data for every probe.
Fontdb exposes safe `Source::Binary`/`push_face_info` APIs; the prototype does not
use unsafe shared mmap. Font loading and usvg parse caches still need explicit
resource budgets and cancellation in a production adapter.
Sources: `target/native-font-research/src/main.rs`,
`target/native-font-research/results.json`; fontdb 0.23.0
`src/lib.rs:587`, `src/lib.rs:709`, `src/lib.rs:864`;
usvg 0.47.0 `src/text/layout.rs:1206`.

**Inference:** public micro-SVG probing with resident selected-font data is a
credible first implementation candidate. It avoids independently reimplementing
font selection, bidi, fallback, shaping, and outlines. Cache complete results
and keep font bytes resident with a bounded lifetime. Do not preload every
installed font or copy a full database per request. Do not promise the measured
0.154ms/probe for CJK/emoji, variable fonts, large runs, other platforms, or
end-to-end wrapped diagrams: those workloads are not covered here. A direct
shaping fast path should follow profiling, not precede it.

The adapter's three title cases still use file-backed fonts; their native render
times include that cost and do not demonstrate integrated cache performance.
No PDF/JPEG, production font-context injection, full wrapping backend,
cross-platform test, or all-family visual comparison was implemented.

Primary upstream API references:

- [usvg 0.47.0 Text](https://docs.rs/usvg/0.47.0/usvg/struct.Text.html)
- [usvg 0.47.0 Options](https://docs.rs/usvg/0.47.0/usvg/struct.Options.html)
- [usvg 0.47.0 FontResolver](https://docs.rs/usvg/0.47.0/usvg/struct.FontResolver.html)
- [usvg 0.47.0 PositionedGlyph](https://docs.rs/usvg/0.47.0/usvg/layout/struct.PositionedGlyph.html)

The API statements above were verified against locally installed crate source;
the web lookup returned no readable content during this session.

## Smallest Recommended Capability

The following sequence is proposed, not existing behavior:

1. Preserve deterministic defaults; offer native measurement for graphical CLI
   output, with one context shared across measurement, conversion, and painting.
2. Implement only operations whose semantics are actually supported. Decline
   unsupported requests and expose existing fallback provenance.
3. Retain selected font bytes and cache complete effective styles, text,
   operation semantics, wrap mode/width, and context identity. Measure discovery
   separately from warm shaping. The optimized public-API probe is a feasible
   candidate, but benchmark growing-prefix wrapping before default admission.
4. Allow explicit font files and disabled system discovery when reproducibility
   is required. System fonts make layout depend on the installed environment.
5. Repair title vertical measurement before advertising complete native bounds.
   Keep mixed HTML runs and broader CSS support explicit additional work.

**Inference:** this respects ADR-0086: host authority and deterministic fallback
remain, generated browser tables stay removed, and optional native capability
gets measured admission instead of changing the default dependency boundary.
Sources: `docs/adr/0086-deterministic-text-measurement-without-vendored-font-tables.md:36`,
`docs/adr/0086-deterministic-text-measurement-without-vendored-font-tables.md:40`,
`docs/adr/0086-deterministic-text-measurement-without-vendored-font-tables.md:76`.

## Proposed Admission Checks

- Prove identical actual fonts for measurement and painting across regular,
  bold, italic, and missing-glyph fallback runs.
- Distinguish shaped advance, font-metric bbox, ink bbox, and anchor/baseline
  offsets before mapping all protocol operations.
- Cover a long title, wrap threshold, class member, CJK/Latin, combining
  sequence, and unsupported glyph in representative diagram families.
- Report CSS line height and mixed inline styles separately from accurate
  single-run shaping.
- Measure cold discovery, repeated labels, long wrapping inputs, and resource
  and cancellation behavior against deterministic mode.
- Validate PDF separately from PNG.

These are planned checks, not results. This study changes no production code.
