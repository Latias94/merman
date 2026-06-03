# HPD-080 - Info Raster Font Fallback

Date: 2026-06-03

## Trigger

Ubuntu CI reported:

`fixtures/info/upstream_cypress_statediagram_v2_spec_v2_should_render_a_simple_info_001.mmd:
rasterized PNG appears blank or all background-colored`

The fixture is a frontmatter-wrapped bare `info` diagram with `fontFamily: courier`.

## Diagnosis

The `info` entrypoint is not metadata-only. Mermaid 11.15's
`diagrams/info/infoRenderer.ts` calls `configureSvgSize(svg, 100, 400, true)` and appends visible
version text. Local output intentionally mirrors that browser SVG shape: `width="100%"`,
`max-width: 400px`, no root `viewBox`, and `<text class="version">...`.

The blank PNG path therefore pointed at raster integration, not at the source-content gate. The
existing raster backend loaded system fonts and set the default family to `Arial`, but `fontdb` does
not implement browser-like font-family fallback. On a Linux environment without `Courier`/`Arial` or
usable generic aliases, `usvg` can drop the text node instead of falling back to another installed
face as a browser would.

## Change

- Centralized PNG/JPEG `usvg::Options` setup in `configure_usvg_options_for_raster(...)`.
- Kept system font loading, but now binds missing generic `sans-serif`, `serif`, and `monospace`
  aliases to actual loaded faces when possible.
- Replaced the plain default resolver with a browser-like resolver that:
  - tries `usvg`'s default exact-family query first;
  - falls back to `monospace -> sans-serif -> serif` when the requested family looks monospace;
  - falls back to `sans-serif -> serif -> monospace` otherwise;
  - finally uses any loaded face instead of dropping text when no named/default family matches.
- For no-`viewBox` SVGs with `max-width: Npx`, set `usvg`'s default viewport width to that max-width
  before parsing. This matches the browser intent of Mermaid's `configureSvgSize(..., true)` path
  better than the previous hardcoded `100x100` relative-size default.
- Added a unit regression that rasterizes a no-`viewBox`, `width="100%"`, missing-font text SVG and
  asserts visible non-background ink.

## Verification

- `cargo test -p merman --features raster render::raster::tests -- --nocapture`
- `cargo nextest run -p merman --features render,raster --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`

## Residual

This is still a system-font-backed raster path, not a fully vendored-font renderer. The fix makes
font-family fallback browser-like when at least one system face is available. A future deeper raster
workstream could bundle an explicit fallback font if host-font independence becomes a release gate.
