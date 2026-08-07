# Rendering Security

Merman is a headless Mermaid renderer. It parses Mermaid source, applies Merman's Mermaid-aligned
sanitization rules, and returns SVG, raster-image, or vector-PDF output. Hosts still decide how that
output is used: downloaded as a file, rasterized, inserted into a browser DOM, or shown in an
editor webview.

## Safe Defaults

Default rendering keeps Mermaid-compatible strict behavior: labels and tooltips are sanitized,
unsafe URL schemes are blocked unless the caller intentionally uses loose Mermaid security behavior,
and returned output carries no Mermaid `bindFunctions` execution hook. A host text-measurement
callback, when configured, still runs synchronously inside the render operation.

`securityLevel` is a sanitization policy in Merman. It is not an execution sandbox. In particular,
Mermaid's browser renderer wraps `securityLevel: "sandbox"` output in a sandboxed iframe; Merman is
headless and does not create an iframe, process sandbox, origin boundary, or content-security policy.
Merman currently applies its strict sanitization path to `sandbox`, `strict`, and `antiscript` text,
but the host must establish any required browser or process isolation itself. Do not interpret a
Merman `sandbox` setting as equivalent to Mermaid's iframe boundary.

For export and raster workflows, prefer the resvg-safe SVG pipeline or a raster output API when the
consumer is not a browser DOM:

```rust
use merman::svg::{ResvgCompatibleSvg, SvgPipeline};
```

The resvg-safe preset always runs after custom draft postprocessors. It tokenizes CSS, removes
active SVG/SMIL content and unsafe attributes, and closes non-navigation rendering resources.
Structural references are limited to same-document fragments; ordinary image resources require an
approved inline PNG/JPEG/GIF/WebP data URL whose encoding is syntactically decodable; `feImage`
accepts either form. This does not prove that the decoded bytes form the declared image container.
SVG, XLink, and XML namespace aliases are checked by local attribute name, matching `usvg`. It then
parses the final XML and returns the sealed `ResvgCompatibleSvg` type used by low-level PNG/JPG/PDF
APIs. `<a>` navigation links remain in the SVG because rasterizers do not resolve them as resources;
they are outside this non-browser raster-resource contract.

Navigation admission is representation-aware. Renderer-created DOM values are serialized first,
then pass through Mermaid's global `decodeEntities` cleanup before the strict-like DOMPurify URI
check. The resvg-safe source scanner consumes serialized attribute text, while terminal XML
validation consumes parsed values; neither layer reinterprets the other's entity encoding.

## Output Resource Boundaries

Output type determines the relevant allocation policy:

- SVG remains vector markup and has no global width or height cap. Normal source, model, label, and
  SVG-byte limits still apply before output is returned.
- PNG/JPG use `RasterOptions`. The default plan limits each side to 4096 pixels and the final image
  to 16,777,216 pixels before allocating the output pixmap. Fit and scale are part of the same
  preflight.
- PDF uses independent `PdfOptions`. Vector page dimensions do not consume the PNG/JPG pixel
  budget. Localized filter images retained by the PDF default to a 33,554,432 aggregate-pixel
  limit, and Merman lowers filter sampling when necessary. This is not a byte-exact upper bound on
  every transient allocation made by third-party conversion backends.
- PNG/JPG and PDF both bound embedded data-URL bytes before `usvg` parses them, then preflight
  embedded PNG/JPEG/GIF/WebP dimensions from image headers. Defaults allow 16 MiB and 16,777,216
  intrinsic pixels per image, with 32 MiB and 33,554,432 pixels in aggregate. Aggregate data bytes
  and resource counts include every same-document `<use>` occurrence that will duplicate an image,
  plus conservative applications of local filter, mask, and clip-path definitions to each
  `<use>`-expanded source element.
- Merman's PNG/JPG/PDF exporters independently disable `usvg` string-href resolution. This defense
  remains required even though the sealed SVG contract removes external image paths, so later
  exporter refactors cannot silently restore host-file reads.
- Both conversion paths also bound recursive SVG work such as isolation depth, filter primitives,
  subroots, and nested SVG images before entering `resvg` or `krilla-svg`. Terminal validation
  resolves the same-document `<use>` graph and rejects cycles, expanded node counts, and expanded
  depth before `usvg` materializes the tree. The sealed XML tree and resolved `usvg` tree retain a
  non-optional 1,000,000-node hard cap and backend depth cap (256 on native, 64 on WebAssembly), so
  an unbounded resource profile cannot bypass third-party recursion safety. Native preparation and
  encoding run on a bounded 8 MiB worker stack.

These limits are deliberately not one switch. Unbounding PNG/JPG output does not unbound PDF
filters or image decoding, and unbounding PDF filters does not disable parser or render resource
profiles. No unbounded option disables the recursive-backend capability cap. Keep each budget
enabled at an untrusted boundary and fit preview images to their actual display size.

`ResvgCompatibleSvg` alone does not prove the declared inline-image container or its decode cost. A
host that bypasses Merman's export APIs and invokes `usvg` or another rasterizer directly must apply
equivalent encoded-byte, container, frame, dimension, and aggregate-pixel checks before decoding.

See [Raster And PDF Output](../rendering/RASTER_OUTPUT.md) for the exact option types and residual
allocator boundary, and [Threat Model](THREAT_MODEL.md) for source/model/render limits. These limits
reduce denial-of-service exposure; they do not replace an operating-system quota for hostile input.

## Browser And Webview DOM Insertion

DOM insertion has a higher bar than file download. Browser and VS Code preview surfaces share the
same generated SVG safety policy:

- Canonical policy: `platforms/web/src/svg-safety-policy.ts`
- VS Code generated copy: `tools/vscode-extension/src/preview-svg-safety-policy.ts`
- Freshness check: `node scripts/check-svg-safety-policy.mjs`
- Regeneration command: `node scripts/generate-svg-safety-policy.mjs`

Use `assertSelfContainedSvgForDom()` from `@mermanjs/web` for a closed preview that must reject both
external navigation and external rendering resources. It accepts same-document fragment references
and narrowly validated inline raster data URLs, but rejects HTTP(S), protocol-relative,
root-relative, and document-relative URLs in links, images, styles, and resource references. VS Code
preview keeps this self-contained policy through the generated policy copy.

Authoring surfaces that intentionally support Mermaid links can instead use
`assertNavigableSvgForDom()`. That policy permits Mermaid-compatible, user-activated navigation on
SVG `<a href>` or `<a xlink:href>` elements: document-relative and root-relative URLs plus the pinned
DOMPurify-safe schemes (`http`, `https`, `ftp`, `ftps`, `mailto`, `tel`, `callto`, `sms`, `cid`,
`xmpp`, and `matrix`). Protocol-relative URLs remain rejected because their capability depends on
the embedding document. The standard Mermaid targets (`_self`, `_blank`, `_parent`, and `_top`) are
accepted as navigation metadata. External images, `<use>` targets, filter images, CSS resources,
tracking URLs, active content, unsafe schemes, navigation downloads, opener relationships, and
named browsing contexts remain rejected. The Web `renderSvgElement()` and `renderSvgToElement()`
convenience APIs use this navigable policy and harden every non-fragment anchor with a new browsing
context plus `noopener noreferrer`. Fragment anchors receive `target="_self"` so a host
`<base target>` cannot redirect them into another browsing context. The SVG string returned by
`renderSvg()` remains unchanged.

The validators return distinct opaque `SelfContainedSvgDomAdmission` and
`NavigableSvgDomAdmission` capabilities, not a claim detached from the host document. They are
bound to the Web package instance at runtime, so a shaped object or structured clone is rejected.
For a closed preview, import the parsed root into the actual owner document and call
`prepareSelfContainedSvgForDomMount()` immediately before insertion. For a navigable preview, use
`prepareNavigableSvgForDomMount()`, which combines the same root/document/base revalidation with
anchor hardening. Browsers resolve `href="#fragment"`
and CSS `url(#fragment)` through `document.baseURI`; an external HTML `<base>` can therefore turn a
source-local reference into a network request. The mount check is conditional: SVG without
fragment references remains admissible in a document with a different base. `renderSvgElement()`
checks the global document, while `renderSvgToElement()` checks the target element's own document.
The Playground combines the shared mount helper with `base-uri 'none'`; the VS Code webview also
declares `base-uri 'none'`.

Neither policy admits external rendering resources. For diagrams that require them, embed or
rewrite the resources, keep the SVG as a download, or build a host-specific policy with an explicit
URL allowlist, CSP, and isolation model. Do not expose a broad "allow external resources" switch to
solve a navigation-only requirement.

Inline raster validation runs before `DOMParser`, `innerHTML`, or decoded-byte allocation. The shared
policy applies these independent limits:

| Resource | Per image | Aggregate SVG |
| --- | ---: | ---: |
| Base64 payload | 24 MiB | 44 MiB |
| Decoded file bytes | 16 MiB | 32 MiB |
| Intrinsic canvas pixels | 16,777,216 | 33,554,432 |

The enclosing SVG is also limited to 64 MiB of UTF-8 and UTF-16 source representation, and one raw
attribute value is limited to 25 Mi UTF-16 code units. Base64 is parsed canonically; MIME and file
signatures must agree. PNG, GIF, JPEG, and WebP dimensions come from bounded container/header
scans. The scanners accept only a static single-frame subset: all APNG control/frame chunks,
multi-image or application-controlled GIFs, WebP animation flags/chunks, independently compressed
PNG metadata, unknown PNG/WebP chunks, and inconsistent container dimensions fail closed. See the
[PNG](https://www.w3.org/TR/png-3/),
[GIF89a](https://www.w3.org/Graphics/GIF/spec-gif89a.txt), and
[WebP container](https://developers.google.com/speed/webp/docs/riff_container) specifications for
the structures used by these checks.

This is deliberately not an image decoder. It does not decompress PNG image data, GIF LZW data,
JPEG scans, or WebP bitstreams, and it does not infer frame counts from file names or browser timing.
An accepted payload can still be rejected by the browser's decoder. The policy proves the encoded
and decoded file-byte bounds, declared canvas bounds, and absence of recognized or unsupported
animation/frame containers before the browser sees the SVG; it does not claim an exact browser heap
allocation bound.

If an application bypasses these wrappers and inserts `renderSvg()` output directly with
`innerHTML`, the application owns that DOM trust decision.

Neither DOM assertion nor strict sanitization creates Mermaid's sandboxed iframe. A host that needs
origin isolation must insert the validated result into a host-owned sandboxed iframe or another
equivalent isolation boundary.

## Loose Security Settings

Mermaid's loose security mode exists for compatibility with diagrams that intentionally contain
custom links or callback metadata. Treat loose mode as trusted-input behavior. It is appropriate for
local authoring previews or controlled documents, not for untrusted multi-tenant input.

## Host Responsibilities

Hosts should:

- keep untrusted authoring and preview surfaces on strict/default settings;
- choose `assertSelfContainedSvgForDom()` for closed previews or `assertNavigableSvgForDom()` for
  authoring surfaces with explicit link navigation;
- carry the returned admission to the real mount boundary and call
  `prepareNavigableSvgForDomMount()` for navigable SVG or
  `prepareSelfContainedSvgForDomMount()` for a closed preview with the target's `ownerDocument`;
- avoid postprocessing that reintroduces scripts, event handlers, external loads, or unsafe links;
- run all trusted SVG/CSS postprocessors before the resvg-safe terminal stage; modifying the sealed
  string invalidates its evidence;
- prefer raster or resvg-safe output for downloads in environments that cannot inspect SVG safety;
- keep PNG/JPG, PDF-filter, and embedded-image budgets independent and enabled for untrusted input;
- use `RasterOptions::with_fit_to` for previews instead of rasterizing an intrinsic oversized SVG;
- bound source/model/render work with an appropriate `RenderResourceProfile` and apply host process
  memory/time quotas at hostile boundaries;
- treat host text measurement as synchronous untrusted work: bound input and caches, avoid UI-thread
  deadlocks, and follow the [host measurement lifecycle](../bindings/HOST_TEXT_MEASUREMENT.md);
- run `node scripts/check-svg-safety-policy.mjs` when changing the shared policy.

For parser and sanitizer design context, see `docs/adr/0020-sanitization-and-security-level.md`,
`docs/adr/0023-url-sanitization-braintree-port.md`, and
`docs/adr/0024-dompurify-default-allowlists-and-generation.md`. The public capability split between
Mermaid sanitization, headless SVG output, raster-resource closure, and browser navigation is
recorded in `docs/adr/0078-headless-svg-security-capability-boundaries.md`.
