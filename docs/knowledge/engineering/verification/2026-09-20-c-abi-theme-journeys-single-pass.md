---
type: Verification Record
title: Native C ABI journeys after XML reference single-pass integration
timestamp: 2026-09-20
source_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Result

The declared `c-abi-native` profile was rebuilt from clean integrated source
`bdb209e1166af960121ce0e2b3231d9929d3bca1` with one Cargo job. The existing independently
compiled C consumer then completed 120 fresh processes: 107 successful operations and 13 expected
errors. Repeated calls in each successful engine returned identical data, metadata and status.
The run covers ten complete preset exports, three customized recipes, four diagram families,
shared support and authoring vectors, caller-supplied fonts, and native PNG/PDF output.

| Artifact | Identity |
| --- | --- |
| Native SDK dynamic library | 36,241,600 bytes; SHA-256 `ca5f4f37a31c0a932d58a1b76ea86f6a5c988c1cda96a3a348ba0ffcb9dfc555` |
| C consumer executable | SHA-256 `d160ad7d64a3d14cb57d8c4bfaac871689e218b8c234ce7c7884511210978977` |
| Consumer receipt | `target/bench/experiments/c-abi-bdb-20260920/receipt.json` |

The C executable negotiates the public ABI header, calls `engine_new`, `execute_collect` and
`result_free`, and closes every engine. A Python driver only prepares files, launches the C
executable and checks returned bytes; it does not substitute an in-process Rust call.

# Scope and limits

The run proves the source-built ARM64 C host profile, including complete recipe exchange, scoped
Class edits, Clear/Transparent behavior, shared diagnostics, missing-glyph rejection and native
PNG/PDF payload construction. It does not rerun the prior PDFKit visual inspection, package the
source crate, or exercise Linux, Windows, Intel or hosted targets. The C ABI remains a source-crate
delivery contract; no generic prebuilt C SDK archive is published. This record renews the C lane
for the integrated candidate but does not close C7a or broader portability gates.

Build and consumer logs, sources, returned payloads and the exact receipt are retained under
`target/bench/experiments/c-abi-bdb-20260920/`. No production API, capability declaration or
font asset changed.
