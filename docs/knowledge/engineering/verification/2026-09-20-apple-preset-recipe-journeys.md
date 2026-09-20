---
type: Verification Report
title: Apple complete preset recipe and fresh-process consumer journeys
timestamp: 2026-09-20
git_branch: refactor/presentation-theme-model
native_source_commit: 3433717b9
consumer_commit: a0a628094
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
tags: theme,apple,consumer,recipe,c7a
---

# Result

The local macOS SwiftPM consumer now verifies complete preset export, explicit Class paint edits,
fresh-process file import and truthful unsupported-width reporting. It uses a rebuilt
`apple-uniffi-native` XCFramework from native source `3433717b9`, including the accepted SHA-256
upgrade. The existing authoring/support/error goldens and general Apple smoke still pass.

This closes the Apple local preset-export and bounded scoped-edit gap in the earlier audit.
It does not close all six public journeys, visual qualification or final C7a delivery. No public
API, preset, support claim, qualification cell, resource budget, dependency or font asset changed.

# Executed consumer behavior

The existing `MermanAppleSmoke` executable owns the checks in `ThemeRecipes.swift`:

| Check | Executed scope |
| --- | --- |
| Catalog | One-shot and reusable consumers compare the complete preset array against the shared golden, including family designs and empty qualification cells |
| Complete preset export | All ten presets export schema-1 complete recipes through both consumers; 20 comparisons require preset selection and direct recipe import to produce identical State SVG bytes |
| Cyberpunk family exchange | Class plus the existing public Flowchart, Sequence and XY fixtures; eight same-process byte comparisons across both consumers |
| Invalid input | Missing, old and unknown recipe versions, recipe-plus-preset and recipe-plus-spec each reject with `MERMAN_OPTIONS_JSON_ERROR` through both consumers: ten rejections |
| Fresh process | Three separate executable invocations read saved whole recipe files directly through public `options.theme`, with no internal envelope reconstruction or preset regeneration |
| Fresh output and metadata | Twelve SVG byte comparisons and twelve complete operation-metadata comparisons against the reusable parent engine; every child output parses as SVG and the input file remains unchanged |
| Class customization | Append one Class-node rule with fill `#123abc` and stroke paint `#456def`; inspect both actual outer-shape path attributes and require Flowchart, Sequence and XY SVG bytes to remain unchanged |
| Unsupported Class width | Both support queries return `unsupported` with `theme-support.no-supported-route`; a separate appended width-5 rule changes no SVG but yields a source-addressed diagnostic and rejected target admission |

Each run writes to a new directory, including when an output parent is supplied. Existing outputs
cannot satisfy a child that stops writing one of its artifacts. The final run retains three recipe
files, twelve SVGs and twelve operation-metadata files.

The ordinary exported and color-customized scenes report `theme_status: verified` and
`target_status: unverified`. The smoke asserts this distinction for all eleven ordinary cells.
The unsupported Class-width cell reports `theme_status: residual`, `target_status: rejected`,
and `unsupported-geometry` for target `node`, source document `complete_spec`, path `/styles/94`.
Its rendered shape retains the existing width. Successful SVG generation therefore does not
silently become a support or portability claim.

These are explicit paint edits in a distributed complete recipe. They do not establish semantic
brand-color parameters or a safe global replacement of every equal color. The shared catalog still
owns the design-scope explanation, and no catalog cell is promoted by these tests.

# Build and verification

Host: macOS ARM64, Apple Swift 6.3.2, Xcode 26.5. The owner build runs Cargo serially and produces
both `aarch64-apple-darwin` and `x86_64-apple-darwin` static libraries under the `native-sdk`
profile, then combines them into the macOS XCFramework. Only the ARM64 consumer is executed.
Generated UniFFI Swift/header/modulemap files remain identical to the checked-in projection.

```console
CARGO_BUILD_JOBS=1 bash scripts/build-apple-xcframework.sh --macos
MERMAN_APPLE_THEME_SMOKE_OUTPUT=target/bench/experiments/apple-preset-journeys-20260920/complete-artifacts \
  swift run --jobs 1 --package-path platforms/apple/examples/smoke MermanAppleSmoke
git diff --exit-code -- platforms/apple/Sources/Merman/Generated
git diff --check
```

The final run also passes the prior 58 authoring/support/error calls, SVG/ASCII, host text callback,
icon registry, missing PNG/JPEG/PDF/math capability, resource-limit and cancellation checks.

The universal XCFramework static library is 341,278,504 bytes with SHA-256
`8bee9b52cad86cb6c99583a21f2edca9b262fdae177ecfa6f6541b9b9b4fb084`.
The library copied into the executed Swift build has exactly the same hash. These are uncompressed
local static-library bytes, not installed or archived package size and not a historical comparison.

`target/bench/experiments/apple-preset-journeys-20260920/receipt.json` records the native source,
lockfile, profile descriptor, build owner, consumer sources, generated bindings, both target
libraries, final executable, fixtures and all 27 artifacts. `build.log` and `smoke-complete.log`
record successful execution. Native source and build inputs remain unchanged from the named
revision; the additional Swift smoke source is recorded by hash. The final executable SHA-256 is
`a64114c72119caa1fff8fb6e69f19c0fab62f0e9dfc320be176b7c39c1c514c2`; the receipt SHA-256 is
`d6fa883325585def0d162e7ac12f30905c9dd2adc164997dbe804930b74def51`.
This is not an immutable same-source release archive receipt.

# Findings and remaining boundaries

The first test assumed all presets lacked Mermaid compatibility settings. That was too broad:
only the three native presets have that restriction. The test now follows the existing Node/Python
consumer contract. A second test expected Class node width to change; source inspection and the
real public metadata show that this is explicitly unsupported. The request remains as a negative
case, and the successful color edit is exercised separately. Neither finding required a renderer
change or weakened support assertion.

One trial Swift invocation coincided with the owner's XCFramework replacement and failed before
compilation. The final consumer build and run occur after packaging completes; that failed attempt
is retained in the experiment logs and supplies no validation evidence.

Still unverified in this slice: Swift 5.9/Xcode 15.2, native Intel execution, iOS build/runtime,
installed release archive provenance, visual/readability qualification, controlled or missing-font
behavior, dense Class and extended-series XY, and
PNG/PDF workflows in export-capable profiles. The default Apple package deliberately lacks native
binary exports, embedded fonts and math. Existing missing-capability checks do not substitute for
those separate workflows. The complete six-journey requirement and C7a remain open.

# Paint and font boundary follow-up

The final ARM64 SwiftPM smoke also executes `ThemeBoundaries.swift`. Across the one-shot and
reusable entry points it exercises ten Class cases: omitted, explicitly cleared and transparent
fills, each against source-owned fill/stroke styles and an independently overridden stroke. Source
ownership remains per facet. A Clear fill masks the earlier paint without becoming transparent,
retains a source-addressed unsupported-route diagnostic, and is rejected by strict portability;
transparent and ordinary typed paints render and remain `target_status: unverified`.

The same consumer renders a deliberately absent host font family through the one-shot API, a
reusable engine and constructor options. The family name and fallback survive in the SVG without
an embedded asset or `@font-face`; this proves the resource-free contract, not host font metrics.
Two complete-spec wire shapes carrying a valid WOFF2 magic prefix but no body are rejected through
all three entry points with the default artifact's `embedded-fonts` missing-capability error,
including constructor admission before rendering. The runtime catalog also confirms that the
default Apple artifact does not advertise that capability.

The final command was:

```console
MERMAN_APPLE_THEME_SMOKE_OUTPUT=target/bench/experiments/apple-theme-boundaries-20260920/final-artifacts \
  swift run --jobs 1 --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

It passed the existing authoring and preset checks plus `Apple theme boundaries passed: 10 Class
cases, 2 strict Clear rejections, 6 missing-family renders, 6 font capability rejections`.
The final log SHA-256 is `93fa616c89b493db39e369aaa8c5cdb4dc70b471e260f47221bda6a9c3201232`.
The run produced 57 retained local artifacts under the ignored experiment directory. This remains
macOS ARM64 with Swift 6.3.2/Xcode 26.5; Swift 5.9, Intel, iOS, browser/PDF font metrics and
visual qualification remain unverified.
