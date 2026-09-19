# Sequence native marker paint

## Scope and cause

This U7 increment starts at `2d35e4b20`. It fixes the gray message arrows observed in the public Cyberpunk Sequence PNG/PDF after the Actor shadow increment. It does not close U7, qualify the public catalog, or freeze C7a.

The Sequence writer supplied the correct `#00f2ff` signal paint through suffix attribute selectors (`[id$="-arrowhead"]`, and related marker names). The locked native stack uses usvg 0.47.0 and simplecss 0.2.2; the latter rejects the suffix operator. Consequently the native marker path inherited the root's gray fill while Chromium applied the cyan rule. A two-case usvg probe with identical geometry produced gray for the suffix selector and cyan for an exact attribute selector.

The production change replaces the two Sequence marker selector lists with exact matches against the already scoped definition IDs. Browser attribute-selector specificity, source CSS ordering, geometry, marker references, and final message paint resolution are unchanged. Native CSS parsing has its own specificity limitations; this change does not certify arbitrary source `themeCSS` across backends. Structured `signalColor` ownership is checked separately. It adds no dependency, public API, font resource, or resource-budget increase.

## Regression coverage

The native regression lives in the existing `merman::theme_composed_effects` integration target. It checks actual PNG pixels after independently removing each message marker reference, so correct line pixels or another correct marker cannot conceal a gray arrow.

- Nine message arrow spellings cover filled, dotted, cross, point, solid/stick half-arrow and bidirectional terminals.
- Transparent message paint leaves no marker pixels.
- Source `signalColor` still overrides typed paint.
- A winning Clear produces the same PNG bytes as the default theme.
- The committed public Cyberpunk Sequence fixture follows the same pixel check.
- Existing CSS and SVG-ID tests retain the scoped-definition contract.

The initial red test observed 0 cyan pixels out of 59 changed marker pixels. An intermediate test used invalid cross-arrow spelling `->x`; correcting the test input to Mermaid's `-x` did not require a production change.

Autonumber's separate zero-length, zero-stroke-width line is not part of the visible message-arrow pixel assertion. Removing its reference yielded no native pixel change; this record does not claim native qualification of its circle background. The sequence-number selector is covered by the existing CSS tests.

## Validation

All Cargo work used the shared target directory, Release profile and `CARGO_BUILD_JOBS=1`.

- Native integration: `cargo nextest run --release -p merman --no-default-features --features svg,png,pdf,layout-cytoscape,embedded-fonts --test theme_composed_effects --no-fail-fast` — **8 passed, 0 skipped**, `/tmp/sequence-marker-native.log`.
- Renderer regression: `cargo nextest run --release -p merman-render --no-default-features --features layout-cytoscape,embedded-fonts --lib --test sequence_svg_test --test svg_internal_id_test --no-fail-fast` — **2,802 passed, 3 skipped**, `/tmp/sequence-marker-renderer-final.log`. The initial run passed 2,801 tests and found one old suffix-selector string oracle in the Actor/Message isolation test. That assertion was updated to the exact scoped selector; its color-isolation and evidence assertions remain unchanged.
- Fresh standalone facade build: `cargo build --release -p merman --no-default-features --features svg,png,pdf,layout-cytoscape,embedded-fonts --message-format=json` — passed. The capture links the actual facade rlib reported by Cargo, not a cached CLI.
- Chromium 151.0.7922.34, 1280×960, DPR 1: before/after public-scene screenshots are byte-identical. Both resolve arrow fill/stroke to `rgb(0, 242, 255)`; a later user suffix-selector rule still overrides both to magenta.
- Fresh public-preset PNG and PDF, plus an actual PDFium raster at 96 dpi: both message arrows are visibly cyan, and the four Actor glows remain present. PNG/PDF admission is **HostDependent**, solely `SystemOrHostFontDependency`; filter receipts remain 4 filters / 4 references / 8 shadow stages. No matched-font reference qualification is claimed.
- `cargo fmt --all -- --check` and `git diff --check` — passed. Scoped Release Renderer Clippy passed with the same 160 existing warnings; `/tmp/sequence-marker-clippy.log`.
- Scoped `ce-simplify-code` completed all three lenses with no worthwhile simplification (`/tmp/sequence-marker-simplify-2d35e4b20.json`). Independent `ce-code-review mode:agent base:2d35e4b20` completed seven applicable lenses with no remaining actionable defect; `/tmp/compound-engineering-501/ce-code-review/20260919-081600-d658dbf5/review.json`. Capacity constraints required serial lens execution in one independent Codex context, not seven independent reviewers. No external-model review was used. The raw-CSS limitation below remains explicit.

Ignored reproduction artifacts are under `target/bench/experiments/sequence-marker-2d35e4b20/`: `selector-probe.rs`, input/expanded usvg probes, `capture.rs`, `capture-contract.json`, `native.svg`, `public-sequence-native.png`, `.pdf`, `public-sequence-native-pdfium.png`, `browser.cjs`, `browser-observations.json`, and `pdfium-observations.json`. The capture contract records the facade rlib/features and hashes. PDFium was an existing local library, identified by path/hash in its observation record; no project dependency was added.

The specificity probe also confirms a native backend limit: with a generated exact-ID attribute rule followed by `#seq marker[markerUnits="userSpaceOnUse"] path { fill:red;stroke:red; }`, Chromium paints red but simplecss retains cyan because it counts ID-attribute specificity differently. This is a raw-CSS limitation, not a promise of native CSS equivalence. See `exact-user-css-input.svg`, `exact-user-css-usvg.svg`, and `specificity-observations.json`.

No full workspace, installed-package/platform matrix, or performance/size benchmark was run for this bounded correction.

## Remaining work

U7 still requires the remaining message/note/text effects, source ownership, bounds, and complete public-scene acceptance across SVG/PNG/PDF. This correction establishes message-marker color, not message glow or full reference-theme equivalence. Broader C7a rollout/profile/installed-consumer and measured performance/size gates remain open.

A related source search also finds suffix selectors in Class and State. Those writers have additional class/inline paint paths, so selector presence alone does not establish the same defect. No blanket rewrite or generic CSS interpreter was introduced.
