# Sequence Message stroke width

## Scope

This U7 increment starts at `2069e5d85`. It adds the static unqualified and `Default` Message stroke-width consumer, using the same CSS for straight, dotted, and self-message paths. The public Cyberpunk recipe now requests 2px for Sequence messages, matching the pinned Modern Mermaid recipe's width. Message glow, other pending Sequence surfaces, catalog qualification, and the wider C7a contract remain open.

The change adds no dependency, public type, embedded font, or budget increase. The unpublished schema and recipe revisions remain 1; the actual recipe content fingerprint changes to `c982bcac9c51f80245a6eea9fb634f7ecb59a9ec0a41b5573078e3b61c3f982a`.

## Ownership and geometry

A width winner is observed independently of signal-color ownership. Complete writer-owned line receipts are required before marking width Applied. Clear consumes the winning property while restoring the default 1.5px CSS and original viewport. Unsupported sibling facets retain their residual. Message ordinal and non-default variant selectors remain conservatively unsupported, including unmatched selectors; this increment does not expand their support contract.

Typed width needs actual paint bounds. An initial relative-padding approach assumed the layout already contained the default message stroke and markers. Independent review disproved that assumption for `mirrorActors: false` with zero diagram margins: layout edges can describe only the centerline, and self-message writers extend their paths beyond those points. The implementation instead unions actual message geometry and full marker viewport envelopes into the existing root bounds. Self-message control points are shared with the writer. Cross/point markers retain SVG stroke-width scaling; fixed-size markers retain their `userSpaceOnUse` semantics. Rotated marker viewport envelopes are conservative, so large widths can add visible whitespace.

The new bounds pass runs only for an explicit effective width, reuses existing node/edge indexes, checks operation cancellation, and stores a single accumulated rectangle. It does not build a terminal cache or introduce a general drawable/proof abstraction. No performance improvement is claimed without a benchmark.

## Validation

All Cargo runs use the shared target directory and `CARGO_BUILD_JOBS=1`.

- Release Renderer: `cargo nextest run --release -p merman-render --no-default-features --features layout-cytoscape,embedded-fonts --lib --test sequence_svg_test --test theme_support_discovery_test --test svg_internal_id_test --no-fail-fast` — **2,858 passed, 3 skipped**; `/tmp/sequence-message-width-renderer.log`.
- Release native: `cargo nextest run --release -p merman --no-default-features --features svg,png,pdf,layout-cytoscape,embedded-fonts --test theme_composed_effects --no-fail-fast` — **9 passed, 0 skipped**; `/tmp/sequence-message-width-native.log`. New PNG checks independently remove each marker reference, cover 18 zero-margin geometry/width combinations, and measure the cyan pixel rows at straight-message midpoints for 2px and 12px widths.
- Workflow contracts: `python3 -m unittest scripts.test_ci_plan scripts.test_release_workflow_security scripts.test_ci_workflow_android_emulator scripts.test_fuzz_config` — **91 passed**. actionlint **1.7.12** passed for both changed workflows. Local zizmor is unavailable; its pinned CI owner remains responsible.
- Independent `ce-simplify-code` completed three lenses with no necessary simplification. The subsequent `ce-code-review mode:agent base:2069e5d85` completed eight applicable lenses and the two-workflow addendum with no retained finding; `/tmp/compound-engineering-501/ce-code-review/20260919-090531-29790428/review.json`. Capacity constraints required serial lenses in one independent Codex context, not eight independent reviewers. Neither source review substitutes for runtime verification.

The existing native theme CI and release-preflight steps now explicitly select `theme_composed_effects` and its required `layout-cytoscape` feature. There is no new job or script. This makes the composed-effect and marker/width pixel regressions explicit in those gates, rather than relying on workspace feature unification.

- A fresh locked Release facade build produced the actual capture rlib. Ignored artifacts are under `target/bench/experiments/sequence-message-width-2069e5d85/`; `capture-contract.json` binds its features, rlib hash, capture source, and base plus dirty-diff hash. The capture does not use a cached CLI.
- Chromium **151.0.7922.34**, 1280×960, DPR 1: **19 scenes passed**. Computed message widths are 2px/12px. An expanded transparent raster of the actual message terminals, styles and marker definitions found **zero painted pixels outside the production viewport**, with a one-pixel antialiasing edge allowance. This covers the public fixture and 18 zero-margin straight/curved/right-angle cases. `browser.cjs` and `browser-observations.json` retain the reproduction.
- Fresh public Cyberpunk PNG/PDF and an actual PDFium raster at 96 dpi were inspected. Message lines/arrows remain cyan; four Actor filters and eight shadow stages remain present. Both export admissions are **HostDependent**, solely `SystemOrHostFontDependency`. Host fonts are not a matched-font reference qualification. The local PDFium library identity/hash is in `pdfium-observations.json`; no project dependency was added.
- Release Clippy for the Renderer and facade libraries passed; `/tmp/sequence-message-width-clippy.log`. Renderer still reports 160 existing warnings, facade 31. No new warning was introduced by the changed production paths.
- `cargo fmt --all -- --check` and `git diff --check` passed.

The current CI combined command was executed once over all families:

```text
cargo run --release --locked -p xtask -- compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root
```

It found two existing-default-path issues, not a Message-width regression:

- Four Sequence browser-text receipts had become stale after the preceding `2069e5d85` native marker-selector correction. Reversing only its eight selector substitutions in each generated SVG reproduced all four old canonical signatures exactly; input and upstream digests were unchanged. Chromium before/after text attributes, computed styles, geometry, marker styles, and screenshot bytes were identical for all four scenes. Only their `local_svg_signature_sha256` values were refreshed; modes, precision, upstream/input bindings and acceptance conditions were preserved. The focused Sequence comparison now passes all three modes, accepting the same 12 exact comparisons. Reproduction: `/tmp/sequence-browser-receipt-diagnosis-2069e5d85/` and `/tmp/sequence-message-width-sequence-parity.log`.
- Two unchanged Mindmap HTML sanitization fixtures fail final XML validation. The existing HTML sanitizer preserves an unquoted `src=x` attribute, while the Mindmap writer does not complete XHTML void-tag normalization. This is recorded as a separate default-output correction; the full multi-family gate is **not green**. `/tmp/sequence-message-width-parity.log` and `target/compare/mindmap_report_parity_root.md` retain the failures.

The dedicated Release `xtask` root-contract tests passed: **6 passed, 620 skipped by the filter**, `/tmp/sequence-message-width-root-contract.log`. No full workspace, installed-package/platform matrix, or performance/size benchmark is claimed for this increment.

## Remaining work

Finish the Mindmap default-output correction and rerun the complete parity gate. U7 still requires Message glow and the remaining Note, text, Lifeline and Loop surfaces plus source/bounds/native acceptance. This increment does not close U7, qualify catalog cells, or freeze C7a.
