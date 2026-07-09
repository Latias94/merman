# Rendering Security

Merman is a headless Mermaid renderer. It parses Mermaid source, applies Merman's Mermaid-aligned
sanitization rules, and returns SVG or raster output. Hosts still decide how that output is used:
downloaded as a file, rasterized, inserted into a browser DOM, or shown in an editor webview.

## Safe Defaults

Default rendering keeps Mermaid-compatible strict behavior: labels and tooltips are sanitized,
unsafe URL schemes are blocked unless the caller intentionally uses loose Mermaid security behavior,
and renderer output does not execute callbacks.

For export and raster workflows, prefer the resvg-safe SVG pipeline or a raster output API when the
consumer is not a browser DOM:

```rust
use merman::svg::pipeline::SvgPipeline;
```

The resvg-safe pipeline is designed for SVG-to-raster tools. It is not a general sanitizer for
arbitrary user-supplied SVG.

## Browser And Webview DOM Insertion

DOM insertion has a higher bar than file download. Browser and VS Code preview surfaces share the
same generated SVG safety policy:

- Canonical policy: `platforms/web/src/svg-safety-policy.ts`
- VS Code generated copy: `tools/vscode-extension/src/preview-svg-safety-policy.ts`
- Freshness check: `node scripts/check-svg-safety-policy.mjs`
- Regeneration command: `node scripts/generate-svg-safety-policy.mjs`

Use `assertSafeSvgForDom()` from `@mermanjs/web` before inserting raw SVG strings into a browser
DOM. The web helper entry points that mount SVG into an element apply the same policy before DOM
insertion. VS Code preview uses the generated policy copy so the extension does not drift from the
browser package.

If an application bypasses these wrappers and inserts `renderSvg()` output directly with
`innerHTML`, the application owns that DOM trust decision.

## Loose Security Settings

Mermaid's loose security mode exists for compatibility with diagrams that intentionally contain
custom links or callback metadata. Treat loose mode as trusted-input behavior. It is appropriate for
local authoring previews or controlled documents, not for untrusted multi-tenant input.

## Host Responsibilities

Hosts should:

- keep untrusted authoring and preview surfaces on strict/default settings;
- use `assertSafeSvgForDom()` or the VS Code preview policy before DOM insertion;
- avoid postprocessing that reintroduces scripts, event handlers, external loads, or unsafe links;
- prefer raster or resvg-safe output for downloads in environments that cannot inspect SVG safety;
- run `node scripts/check-svg-safety-policy.mjs` when changing the shared policy.

For parser and sanitizer design context, see `docs/adr/0020-sanitization-and-security-level.md`,
`docs/adr/0023-url-sanitization-braintree-port.md`, and
`docs/adr/0024-dompurify-default-allowlists-and-generation.md`.
