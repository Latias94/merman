# SVG Output Pipeline

`merman` has distinct SVG output contracts:

- A default typed `RenderTarget::Svg` request returns Mermaid-parity SVG.
- `SvgPipeline` turns that parity SVG into consumer-oriented output for previews, raster export,
  or host-specific cleanup.
- `ResvgCompatibleSvg` is a sealed artifact produced only after the terminal resvg finalizer. It is
  the only input accepted by the low-level raster encoders and cannot delegate non-navigation
  rendering resources to a host file or network resolver.

Default SVG output is not optimized or cleaned by default because parity output is the comparison
surface for upstream Mermaid fixtures. Consumers that need renderer compatibility should opt in to
a pipeline explicitly. Mermaid-parity SVG may contain `<foreignObject>` HTML labels; that is
expected and should not be treated as the export-safe surface.

## Artifact Evidence Lanes

Output contracts and evidence contracts are related but not interchangeable:

| Lane | Proves | Required gate | Does not prove |
| --- | --- | --- | --- |
| Raw/source SVG parity | Emitted bytes or the declared SVG-DOM comparison profile agree with the pinned upstream artifact. | `xtask` raw-byte or SVG-DOM compare. | Browser computed colors, label overlap, edge contact, or raster compatibility. |
| Browser-visible | The pinned browser computes the expected styles and client geometry under the declared viewport, fonts, and runtime graph. | Build-freshness gate followed by browser computed-style/geometry tests. | Exact SVG serialization or `usvg` / `resvg` compatibility. |
| Resvg-safe | The explicit pipeline emits resource-closed consumer SVG that `usvg` / `resvg` can parse and render without resolving host files or network resources. | Pipeline tests plus raster-consumer tests. | Raw upstream serialization, browser presentation parity, or browser DOM safety. |

Theme SVG assertions, Block analytic geometry, and Gantt SVG-DOM comparisons are useful structural
evidence, but they are not browser-visible evidence. Browser-visible claims require the browser lane.

Typical choices:

- Use a default `SvgRequest` when the caller wants the closest Mermaid-compatible SVG string.
- Set `SvgRequest.pipeline = Some(SvgPipeline::readable())` only when the output deliberately needs
  both native `<foreignObject>` labels and SVG text fallbacks. A browser host must select or hide
  one representation or both may be visible.
- Set `SvgRequest.pipeline = Some(SvgPipeline::resvg_safe())`, select a typed PNG/JPEG/PDF target,
  or use `SvgPipeline::process_resvg_compatible()` before calling low-level encoders.
- Use `merman-cli render --svg-pipeline resvg-safe` when you want the CLI to write export-safe SVG
  bytes instead of the default Mermaid-parity SVG contract.
- Use `RenderRequest::png`, `RenderRequest::jpeg`, or `RenderRequest::pdf` when the input is Mermaid
  source and the caller wants the standard render-and-export path. These typed targets select the
  sealed raster-safe path within the same operation.
- Add `SvgPostprocessor` passes when a host application needs product-specific draft styling or
  metadata. The selected built-in preset always runs after these passes.

## Presets

| Preset | Behavior |
| --- | --- |
| `SvgPipeline::parity()` | No post-processing. This preserves the exact SVG string produced by the parity renderer. |
| `SvgPipeline::readable()` | Adds best-effort SVG `<text>` overlays while retaining `<foreignObject>` labels. Consumers that render both representations may display duplicate text. |
| `SvgPipeline::resvg_safe()` | Adds readable fallbacks, strips the original `<foreignObject>` elements, and removes common `usvg` / `resvg` hazards. Structural references are limited to same-document fragments; ordinary image resources require an approved inline PNG/JPEG/GIF/WebP data URL whose encoding is syntactically decodable; `feImage` accepts either form. `<a>` navigation links remain metadata outside the raster-resource contract. |

### Fallback typography boundary

The fallback stage resolves supported typography against the original SVG and XHTML source context
before it removes `foreignObject`. It does not flatten stylesheet class tokens: selector ancestry,
element type, attributes, cascade priority, and inheritance are retained for the admitted subset.
The resolved font size, family, weight, style, fill, and line height are shared by text measurement,
wrapping, placement, and generated `<text>` styling. This keeps a ClassDiagram label measured at
16px from becoming a synthetic 10px label merely because an SVG-only `text` selector exists.

This is a bounded consumer adapter, not a browser CSS engine. Rich-text runs, browser font shaping,
and CSS features outside the admitted subset remain residuals. Hosts that need metric-affecting
styles must inject them before the fallback stage (through the same `SvgPipeline`); styling added
after generated text exists may change paint, but cannot make Merman remeasure wrapping or geometry.
Stable `data-merman-foreignobject` markers and fallback classes remain available to host consumers.

For Mermaid 11.16 Quadrant, parity output intentionally retains upstream's invalid
`hsl(..., NaN%)` point presentation attributes. A browser ignores them and uses the SVG initial
black fill with no stroke. The typed Quadrant resvg-safe path explicitly emits
`fill="#000000" stroke="none"`; this keeps the export visible without teaching the raw comparator
that invalid HSL and RGB are equivalent.

## Rendering With A Pipeline

```rust
use merman::svg::SvgPipeline;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

let output = Renderer::new().render(RenderRequest::svg(
    "flowchart TD; A[Layer 7\\nHTTP]-->B;",
    OperationControl::new(),
    SvgRequest {
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..Default::default()
    },
))?;
let RenderOutput::Svg(Some(svg)) = output else {
    return Err("no Mermaid diagram detected".into());
};
# let raster_safe_svg: &str = svg.svg();
# let _ = raster_safe_svg;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Runnable example:

```bash
cargo run -p merman --features svg --example custom_svg_pipeline > out.svg
merman-cli render fixtures/flowchart/basic.mmd --output out.svg --svg-pipeline resvg-safe
```

The high-level source-to-output boundary is `Renderer` with a typed target request. Low-level
callers that already own SVG can use `SvgPipeline` and `finalize_resvg_svg(svg, session)`; the
latter is the explicit sealed boundary for existing SVG text.

The source-string helpers extract root SVG attributes only as descriptive metadata. They never
promote `aria-roledescription` or any other SVG text into the closed `RenderFamilyKind` capability.
Only the typed family render operation can retain that capability through postprocessing. A
diagram-type string alone does not authorize a family-specific fallback, and
`finalize_resvg_svg(svg, session)` deliberately performs only family-agnostic cleanup.

## Host Postprocessors

Applications can append product-specific draft passes. The postprocess context includes preset,
pass ordering, diagram type, diagram title, and root SVG id:

```rust
use merman::svg::{
    RenderResult, SvgPipeline, SvgPostprocessContext, SvgPostprocessor,
};
use std::borrow::Cow;

struct AddComment;

impl SvgPostprocessor for AddComment {
    fn name(&self) -> &'static str {
        "add-comment"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> RenderResult<Cow<'a, str>> {
        Ok(Cow::Owned(format!(
            "{svg}<!-- type={} id={} -->",
            ctx.diagram_type().unwrap_or("unknown"),
            ctx.svg_id().unwrap_or("unknown"),
        )))
    }
}

let pipeline = SvgPipeline::resvg_safe().with_postprocessor(AddComment);
# let _ = pipeline;
```

Custom postprocessors run in insertion order, then the selected built-in preset runs as the terminal
stage. The resource policy checks output size after every custom pass. A `resvg_safe` pipeline then
tokenizes and sanitizes CSS, strips active SVG constructs and unsafe attributes, parses the final
XML, removes external image, paint, filter, cursor, and structural references, and validates the
residual compatibility contract. The source scanner treats attribute text as serialized XML and
normalizes retained anchor navigation once; the terminal validator treats parser output as a DOM
value and does not decode entities again. Attributes in the SVG, XLink, and XML namespaces are interpreted
by local name, matching `usvg`; namespace aliases therefore cannot bypass the terminal contract.
The terminal validator also resolves the same-document `<use>` graph before `usvg`, rejecting
cycles and expanded node/depth limits while retaining occurrence counts for inline-image export
budgets. It uses the same `svgtypes` IRI grammar as the pinned `usvg` dependency for local
fragment references. Local `feImage` and marker references contribute to that graph; local
filter, mask, and clip-path definitions are conservatively charged once per `<use>`-expanded
source element so attribute, inline-style, and stylesheet effect selection cannot bypass
embedded-image budgets.
Custom pass errors are surfaced with the pass name attached.
Finalized output is represented by `ResvgCompatibleSvg`; callers cannot construct that type from an
arbitrary string.

Do not modify the SVG string after terminal finalization and continue describing it as resvg-safe.
Any later XML, attribute, or CSS rewrite invalidates the sealed artifact's evidence. Put host
styling and structural passes inside the same `SvgPipeline`; if an external component must rewrite
an already finalized string, pass the result through `finalize_resvg_svg` again before rasterizing.

This contract targets `usvg` / `resvg`. It does not claim browser DOM safety. Browser embedding must
choose the Web package's `assertSelfContainedSvgForDom()` or `assertNavigableSvgForDom()` interface
to match the host's navigation capability, carry the returned admission through the matching
`prepareSelfContainedSvgForDomMount()` or `prepareNavigableSvgForDomMount()` helper on the actual
parsed root and owner document, and enforce the surrounding CSP/sandbox policy. This final root and
mount check prevents unchecked source/tree substitution and keeps fragment-only resource
references local when an HTML document defines a `<base>` URL.

Parity and readable output retain Mermaid's external-resource references. A host that intentionally
supports external images should resolve them under its own root, protocol, byte, and pixel policy,
inline the accepted bytes as an approved data URL in a custom draft postprocessor, and then let the
terminal resvg-safe preset validate the result. Do not re-enable a generic `usvg` string resolver on
untrusted output.

The sealed type proves resource location and data-URL syntax, not that the decoded bytes form the
declared image container or that decoding is cheap. Hosts that send `ResvgCompatibleSvg` directly
to a third-party rasterizer must enforce byte, container, dimension, frame, and aggregate decode
budgets for inline images. Merman's PNG/JPG/PDF APIs perform those checks before their backend
parses the SVG.

## Built-In Host Styling Blocks

Host styling should use product-neutral postprocessors rather than modifying `resvg_safe` itself:

```rust
use merman::svg::{
    CssOverridePolicy, RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline,
    SvgRenderOptions,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

let pipeline = SvgPipeline::resvg_safe()
    .with_postprocessor(RootBackgroundPostprocessor::new("#0f172a"))
    .with_postprocessor(
        ScopedCssPostprocessor::new(
            r#"
.node rect {
  stroke: #2563eb;
  stroke-width: 2px;
}
.merman-foreignobject-fallback-text {
  fill: #111827;
}
"#,
        )
        .with_override_policy(CssOverridePolicy::StripExistingImportant),
    );

let output = Renderer::new().render(RenderRequest::svg(
    "flowchart TD; A-->B;",
    OperationControl::new(),
    SvgRequest {
        pipeline: Some(pipeline),
        options: SvgRenderOptions {
            diagram_id: Some("host-diagram".to_string()),
            ..Default::default()
        },
        ..Default::default()
    },
))?;
let RenderOutput::Svg(Some(svg)) = output else {
    return Err("no Mermaid diagram detected".into());
};
# let _ = svg;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ScopedCssPostprocessor` injects a `<style>` element under the root `<svg>` tag and prefixes normal
selectors with the root SVG id. When the SVG already has style elements, the injected style is placed
after them so host rules follow Mermaid defaults in cascade order. `CssOverridePolicy::StripExistingImportant`
is opt-in because it changes cascade semantics. Generated `<foreignObject>` fallback text keeps useful
classes and inline font/fill hints so host CSS can target readable fallback output. When the same pipeline
feeds raster export, keep injected CSS in the `usvg` / `resvg` supported subset; browser-only features
such as CSS custom properties are better reserved for inline-only SVG pipelines or resolved by the host
before rasterizing.

Product-specific rules still belong in host code. For example, Zed-style accent token assignment,
theme color selection, and diagram-family-specific color semantics should be implemented as custom
`SvgPostprocessor` draft passes. `RootBackgroundPostprocessor` is the
narrow exception for a common host canvas need: it rewrites only the root `<svg>` inline
`background-color`, preserving all Mermaid-owned diagram colors.

Binding consumers can pass external Mermaid defaults through `options_json.site_config` without
embedding an init directive into the diagram source:

```json
{
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc"
    },
    "themeCSS": ".node rect { stroke-width: 2px; }"
  }
}
```

Binding consumers can also inject host-owned scoped CSS through `options_json.svg.scoped_css`:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "diagram_id": "host-diagram",
    "scoped_css": ".node rect { stroke: #2563eb; stroke-width: 2px; } .merman-foreignobject-fallback-text { fill: #111827; }",
    "css_override_policy": "strip-existing-important",
    "root_background_color": "#0f172a"
  }
}
```

The injected CSS is scoped to the root SVG id and inserted after Mermaid CSS. With
`pipeline="resvg-safe"`, merman runs the built-in CSS sanitizer after injecting host CSS so the
binding preset does not silently lose its raster-safety contract. Hosts still own the trust and
compatibility policy for the CSS they provide.

`svg.root_background_color` is a narrower host-owned option that sets the root SVG canvas color
without relying on CSS cascade over an inline style. Passing `"transparent"` keeps the canvas
transparent for hosts that composite diagrams over their own background.

Binding consumers can opt into generic duplicate-fallback cleanup without writing a Rust
postprocessor:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "drop_native_duplicate_fallbacks": true
  }
}
```

`resvg-safe` includes structural cleanup for generated fallback groups tied to native SVG
`<switch>` text fallbacks. `drop_native_duplicate_fallbacks` is the broader opt-in cleanup for
additional native/fallback duplicate surfaces, and it also works with `readable` when a host keeps
the original `<foreignObject>` labels. It does not apply host palette replacement.
