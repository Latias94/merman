---
type: "Subagent Finding"
title: "Maintenance native math review"
description: "Subagent Finding for Maintenance native math review."
timestamp: 2026-10-24T05:43:20Z
record_id: "bf027ae9b62c4f6a88b33d3c3198095f"
producer_id: "codex-sdk"
run_id: "headless-sdk-20261024"
subagent_id: "01a16c5e-5053-7120-87c6-b923b22a8732"
related_plan: "docs/plans/2026-10-24-0925-feat-headless-sdk-plan.md"
related_issue: "243"
---

# Finding

Independent native-subagent review of the maintenance-only RaTeX projection found
five defects: output amplification was budgeted after allocation; empty HTML rows
shifted formula content; substring ownership detection rejected ordinary labels;
valid RaTeX glyph-text fallback was rejected; and outline-only edge labels lost
their background owner. These are implementation defects, not requests for a
general HTML/CSS engine or a main-to-maintenance architecture migration.

# Evidence

- Initial module SHA-256:
  `da0f0edad3a36ad21bb381c98599649d25e2b89834d0b5d0d6aa1799d0ead248`.
- Three projection regressions first failed together; the separately added background
  owner regression also failed before its implementation. The final focused run passes
  128 tests, including the actual edge-label `data-id -> use -> defs` background path.
- The fixed module SHA-256 is
  `7634caa6fb48116c6919762b1e3b4b9d5091e69e39878e3491cac7a9dd80d163`.
  Both real runtimes pass the existing SVG/PNG math smoke with this module.
- Additional real wasmtime probes succeed for ordinary marker-name labels,
  `$$\char32 x$$` and CJK glyph-text math. CJK success means the legitimate SVG
  text is retained, not that bundled Cousine gains CJK coverage.
- Plain and four-leading-break formula PNGs are pixel-identical at 168 by 53;
  decoded RGBA SHA-256:
  `3a1d7c71fe35a611777f88f88f024a80303ba791799d0683393cb189e9d7c52d`.
- A 4096-byte font family with 128 alternating text/formula runs produces a
  302131-byte parity SVG. At a 334899-byte ceiling the new pre-write check rejects
  projected 338349 bytes and the same instance subsequently renders successfully.
  The old implementation reported 1116718 projected bytes only after materializing
  the entire projection.

# Recommendation

Admit only actual owned class tokens, retain source-backed zero-height row behavior,
allow the emitter's bounded glyph-text syntax, reuse the existing per-write generated
SVG preflight, and observe owned SVG content through the existing cascade background
owner. Preserve global `htmlLabels: false` literal math and legacy constrained policy.

# Disposition

All five findings are addressed. The reviewer implemented only the disjoint cascade
owner change and its tests, then reviewed the remaining fixes read-only without a new
blocking finding. The integration owner ran all Cargo commands serially; final shared
export/core/FFI/WASI owner tests pass (549, with six explicitly ignored host-managed
font-catalog tests). No review
thread remains open. Full final structure and archive qualification are recorded by
their later verification shard rather than inferred from this review.

# Citations

- Maintenance paths: `crates/merman-render/src/svg/fallback/native_math.rs`,
  `crates/merman-render/src/svg/fallback/cascade.rs`,
  `crates/merman-render/tests/math_native_svg_test.rs`,
  `crates/merman-render/tests/math_edge_background_test.rs`.
- Runtime probes: `platforms/wasi/wasmtime_smoke.py` and `common.py`.
- Local owner logs: `/tmp/merman-sdk-09-native-math-final-green.log`,
  `/tmp/merman-sdk-09-math-final-probes.log`,
  `/tmp/merman-sdk-09-math-final-hosts.log`,
  `/tmp/merman-sdk-09-native-math-final-owners.log`.
