---
type: Verification Evidence
title: Web WASM size gate after theme revision 82 rebuild
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: web,wasm,performance,verification
---

# Result

The release size gate was run against the freshly rebuilt Web package artifacts. It failed closed
because every Web profile exceeds the checked-in budget. No budget was changed.

| Profile | Raw | Stripped | Gzip | Brotli | Budget raw / stripped / gzip / Brotli |
| --- | ---: | ---: | ---: | ---: | --- |
| web-analysis | 3,674,988 | 3,674,723 | 1,412,187 | 1,074,050 | 3,600,000 / 3,600,000 / 1,375,000 / 1,050,000 |
| web-ascii | 5,198,118 | 5,197,853 | 1,914,936 | 1,446,190 | 5,125,000 / 5,125,000 / 1,875,000 / 1,425,000 |
| web-editor | 3,786,123 | 3,785,858 | 1,456,749 | 1,104,910 | 3,775,000 / 3,775,000 / 1,450,000 / 1,100,000 |
| web-full | 15,857,592 | 15,857,327 | 5,919,707 | 4,360,154 | 14,800,000 / 14,800,000 / 5,550,000 / 3,980,000 |
| web-render | 13,972,486 | 13,972,221 | 5,277,719 | 3,887,239 | 12,625,000 / 12,625,000 / 4,800,000 / 3,425,000 |

The command exited with `WasmSizeMatrixFailed`; this is an expected release-blocking result until
measured size is reduced or a separately reviewed budget decision is made. Existing size budgets
remain authoritative. The theme revision 82 support-query change is included in the rebuilt
artifacts, but this run does not attribute the full regression to that change.

# Reproduction

```text
cargo run --locked -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

Log: `/tmp/theme-web-size-20260913.log`. The size command rebuilt `xtask` but did not mutate
tracked package sources. Future optimization should use the performance workflow and retain the
same capability contracts; weakening the gate or raising all limits without attribution is not an
acceptable fix.
