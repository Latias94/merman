# Upgrading from 0.7.0 to the 0.8.0 candidate

This guide compares the stable `v0.7.0` tag, published on 2026-06-09, with the source candidate for the next stable release, `0.8.0`. The latest published workspace release at this checkpoint is `0.8.0-alpha.7` (2026-09-30); this guide does not announce a `0.8.0` publication. The [comparison report](V070_TO_V080_RELEASE_REPORT.md) records measured revisions, evidence, and remaining release checks. The approximately four-month interval includes breaking API and default changes; upgrading the version alone is insufficient for custom integrations.

## Rust applications

| Previous integration | Candidate action |
| --- | --- |
| Parser-only default `merman` dependency | Disable defaults and select `all-diagrams` or the required `diagram-*` features, or use `merman-core` with an explicit family selection. Facade defaults now build a complete SVG product. |
| `render`, `raster`, `ratex-math` features | Use `svg`; independently select `png`, `jpeg`, or `pdf`; use `math` for mathematical labels. Select languages separately. |
| `merman::render::HeadlessRenderer` | Use `Renderer::render`, `RenderRequest`, and the target-local request type. Move reusable parser configuration to `Engine` and per-output settings to the request. |
| `HeadlessAsciiRenderer` or source-to-raster convenience methods | Use the corresponding operation request and typed output artifact; inspect the target's local capabilities. |
| Vendored font metrics or browser-sized snapshot assumptions | Use deterministic measurement or an explicit host text-measurement provider; review output with the final installed font. |
| Narrow feature declaration relying on transitive defaults | Forward the necessary diagram and output features; verify the actual application graph in an independent workspace. |
| Family-specific model imports and exhaustive enum matches | Enable the owner family and review conditional variants and the new operation-scoped semantic/layout types. |

Start with the [Rust embedding guide](../rendering/RUST_EMBEDDING.md) for a runnable request recipe, the Zed-style family selection, layout fallback rules, fonts, and output handling. Use [Features](../FEATURES.md) for the full capability matrix and the [alpha.6 symbol reference](UNRELEASED_UPGRADE_GUIDE.md) for exact replacements introduced during the refactor. That historical reference does not supersede later alpha.7 schema changes.

`layout-cytoscape` and `layout-elk` still imply SVG, but they do not imply diagram families. Use `all-diagrams, svg, layout-cytoscape` to preserve the old broad parser surface of an SVG/Cytoscape embedder. An explicit allowlist can replace `all-diagrams` with its selectors. The complete default includes ELK and math; choosing `all-diagrams, complete-svg` with defaults disabled omits ELK while retaining SVG, Cytoscape, and math. Review the actual resolved graph and include the notices for the artifact being distributed.

## Presentation and snapshots

The source candidate selects Mermaid `12.1.0` at `21f72f07ea22c0af48a3149c550654e80d8e40cb`, compared with `11.15.0` in `0.7.0`; published alpha.7 follows `12.0.0`. The final stable baseline must be verified against the pinned source and refreshed receipts; selection does not certify that transition validation is complete. The [alpha.7-to-0.8.0 guide](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) covers feedback-edge orientation, line hops, Packet/XYChart policies, and direct low-level ELK constructor changes. Mermaid 12 introduces ELK defaults for supported graph families, Redux themes, and Neo appearance. Use top-level layout configuration rather than the removed family `defaultRenderer` settings. Explicit `layout: dagre`, `theme: default`, and `look: classic` preserve the chosen presentation policy, not every historical pixel or SVG byte.

Refresh SVG IDs, DOM selectors, viewBox/geometry snapshots, fonts, and CSS or resvg checks. Default deterministic text measurement no longer ships vendored font metrics. HTML labels and browser measurement remain separate from the headless path. Review ASCII layout, viewport, encoding, and structured-text fallback metadata independently from SVG support; new SVG families do not automatically gain ASCII support.

## CLI and language tooling

Use the owning `render`, `batch`, and compatibility `mmdc` commands for rendering workflows. Narrow CLI and LSP builds require a family selection: select `all-diagrams, analysis` for a lint CLI, or `all-diagrams, stdio` for the standalone LSP. Complete CLI defaults now include capabilities that a narrow host may not need; compare the exact recipe rather than the facade's defaults when reviewing installed size.

The refactor adds dedicated analysis and editor owners. Hosts retain document storage, revisions, scheduling, and cancellation; pass caller-owned source text into immutable analysis/editor snapshots. Use exact lowercase profile/severity values, the current configuration schema, and matching analysis-fact decoders. Follow the [alpha.3-to-alpha.5](ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md), [alpha.5-to-alpha.6](ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md), and [alpha.6-to-alpha.7](ALPHA6_TO_ALPHA7_UPGRADE_GUIDE.md) guides for those versioned boundaries.

## Bindings and package channels

| Candidate contract inherited from alpha.7 | Upgrade requirement |
| --- | --- |
| C/Flutter ABI 3 | Replace the native artifact and matching facade/header together; reject incompatible ABI probes. |
| UniFFI API 7, including Python and Apple | Regenerate wrappers and replace the matching library or XCFramework together. |
| Android JNI transport API 2 | Replace Kotlin projection and all native slices from one matching AAR. |
| Web/WASM transport API 5 | Keep generated glue, package facade, and WASM on one package version. |
| ASCII report schema 3 | Update strict decoders, layout metadata, fallback handling, and output-plan records. |
| Independent Tree-sitter and Typst versions | Select their own releases; a workspace version or tag does not update these packages. |

Query the installed artifact's runtime catalog before enabling optional operations. The current native package recipes and browser/Node packages have different capability sets; use [Package Surfaces](PACKAGE_SURFACES.md) rather than inferring them from Rust defaults. Publication is independent for each channel. Keep using the verified published version until the owning registry or release asset confirms `0.8.0` availability.

## Validate the upgrade

Build the actual dependency declaration outside the Merman workspace, then exercise every host-accepted family with the selected layout and output. Verify explicit rejection of an omitted family, cancellation, resource limits, and unavailable or denied backends. Run the final CSS/font/raster/DOM path and refresh snapshots only after reviewing output changes. Bind performance and size reports to the final reviewed source commit; rerun them if production code, compiler, profile, features, or lockfile changes.

The reusable theme refactor remains deferred beyond this release. The stable candidate's theme compatibility and targeted correctness fixes should not be read as completion of that separate design effort. The [comparison report](V070_TO_V080_RELEASE_REPORT.md) keeps product growth, comparable measurements, and unmeasured surfaces distinct.
