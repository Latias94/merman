# Optional Embedded Font Capability (U3)

Date: 2026-09-18. Base: `f0f06b364`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U3 / KTD2.

## Boundary

`embedded-fonts` selects caller-supplied font decoding, metadata parsing and native prepared
shaping. The existing implementations move behind this one feature. It introduces no dependency
or font bundle. Basic SVG keeps typography values, font-family names, complete-spec exchange,
canvas layers and supported effects. Font-dependent compilation returns a missing-capability
outcome after applicable input admission. Ordinary render options continue to use their existing
resource schema; fine-grained theme limits belong to the theme compiler/authoring contract.

The renderer compiler restriction is monotonic. Bindings and CLI also restrict it using their
own feature and artifact selection, so a richer Cargo sibling cannot widen an artifact contract.
Native export keeps its ordinary system-font dependencies; export's weak feature forwarding does
not implicitly turn on an output backend or WOFF2 decoding.

Six profiles opt in: CLI release, C ABI native, Rust all/native SDK/bindings SDK, and Web full.
Other profiles retain resource-free rendering and advertise no embedded-font capability. This is
an unpublished alpha.7 contract selection, not a claim that font names are disabled.

## Executed verification

Rust regression tests below use `--locked` Release builds. Targeted Cargo runs use serial
compilation; feature-matrix verification used its default build scheduling. Build checks and
generator runs are listed separately and are not Release runtime evidence.

| Check | Result |
| --- | --- |
| Initial feature-off renderer regression, before production gate | Expected failure: embedded font compilation succeeded |
| Renderer lib + font capability + Flowchart effects, `embedded-fonts,math,layout-cytoscape` | 2762 passed, 2 skipped |
| Renderer lib + font capability, no default features | 2515 passed, 2 skipped |
| Renderer mixed integration targets, feature off | 166 passed, 1 skipped |
| Same integration targets, embedded-fonts on | 178 passed, 1 skipped |
| Bindings feature off: rejection and named-font render | 2 passed |
| Bindings feature on: isolation, discovery and strict native render | 4 passed |
| Facade basic SVG: public Cyberpunk canvas/glow, wire forwarding, unthemed label | 3 passed |
| CLI feature off with bindings-core font feature explicitly enabled | 1 passed |
| Facade PNG/PDF: effects, labels, render operation | 51 passed |
| Isolated export PNG/JPEG/PDF, feature off / on | 87 / 108 passed |
| Fresh dependency license report generation | 13 reports regenerated; third-party contract passed |
| WASM `svg` / `svg,embedded-fonts` checks | Both passed |
| Renderer/bindings/export library Clippy | Passed with existing warnings |
| Native macOS + WASM minimal SVG normal dependency trees | All five optional font-processing packages absent |
| ELK feature-off test-target compilation | Passed |
| cargo fmt and git diff checks | Passed |
| Python profile, closure, license recipe and Web package tests | 96 passed |
| Workflow contract tests | 39 passed |
| Web capability descriptor tests | 10 passed |
| Fresh xtask capability/binding/Typst generation and descriptor validation | Passed; 34 profiles |
| Feature matrix | 11 build checks passed |
| Exact closure: rust-svg-basic, web-render, typst-wasm, web-full, cli-release | Passed |

The host-profile closure uses the verifier's Linux reference target; Web and Typst use
`wasm32-unknown-unknown`, and the selected CLI representative is `aarch64-apple-darwin`.
These are dependency observations, not cross-platform execution or package-size measurements.

The mixed renderer integration targets are `diagram_theme_test`, `flowchart_node_effects`,
`state_svg_test` and `sequence_svg_test`. Real font fixtures require the feature; common admission,
projection, XML ownership and requirement identity tests remain in the base configuration.

A review caught a CLI feature-unification leak, now covered by an explicit ambient-feature
regression. Test development also exposed invalid WOFF2 magic and unsupported ordinary resource
limit overrides in the new test inputs; the corrected tests retain the intended error-order checks
without changing the public resource schema. Both `theme.spec` and complete-spec recipe selections
exercise one-shot, constructor and reusable binding entry points.

## Review and limits

Completed review receipt:
`/tmp/compound-engineering-501/ce-code-review/20260918-203621-54922567/review.json`.
Correctness, testing and API-contract passes completed independently. Other lenses and the export
increment used disclosed inline review because subagent capacity was exhausted. The simplification
review's selected model failed upstream; its three read-only lenses were completed locally.
No remaining source finding was retained. Running checks are owned by this verification record,
not inferred from the review verdict.

U3's optional-font dependency and compilation boundary is closed by the checks above. This does
not establish fresh installation or package-size evidence for every shipping profile.
No final artifact-size, throughput or memory improvement is claimed here. U10 remains the owner
of measured cost admission; the overall theme delivery goal and C7a contract remain open.
