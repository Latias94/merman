---
type: Audit Report
title: Presentation theme capability and impact audit
timestamp: 2026-09-16
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
related_baseline: docs/knowledge/engineering/verification/2026-09-16-modern-mermaid-theme-audit.md
git_branch: refactor/presentation-theme-model
git_commit: 2e3cb995cb5a2c3a67db370adf10d136d727b65c
tags: theme,audit,preset,performance,modern-mermaid
---

# Status and corrections

This is an incomplete audit, not C7a closure or preset qualification. The alpha.6 transition
impact is accepted; final same-source host/artifact qualification and release preflight remain
open. Linux/Windows and Swift compiler-floor execution cannot be inferred from this macOS run.

The earlier report overclaimed its evidence. It counted 22 rather than 24 reference themes,
described handwritten examples as extracted source, and inferred support from keyword searches
and family names. Those mechanism/category counts and support conclusions are withdrawn.
The existing [24-theme corpus](../../../alignment/MODERN_MERMAID_THEME_CAPABILITY_CORPUS.md)
is the source-backed mechanism owner. Typed gradients, filters, ordinal palettes and other
mechanisms must be checked against that owner and runtime admission; they must not be dismissed
as host-only simply because the reference implements them through CSS. A direct typed family
surface also does not establish full support for every facet.

# Latest native follow-up

The clean `a5e3cd2d6` production build now exercises all ten presets: 990/1020 literal-input
outputs succeeded; only the upstream-invalid GitGraph example failed. The Japanese Flowchart
passes all 30 combinations. Archive-bound qualification and replay passed for the existing
18 host-dependent cells. Browser computed styles confirmed a Flowchart arrow-color usability
gap. See the [native candidate record](2026-09-16-c7a-native-candidate-a5e3cd2d6.md) for hashes,
checks and remaining boundaries. The five-preset matrix below is the earlier baseline.

# Literal reference matrix

Source: Modern Mermaid `a021cbce37fc0b07a9f4791c28e983101ea06f2d`, all 34 Mermaid fences
from `MERMAID_EXAMPLES.md`, without rewriting identifiers, labels, syntax or directives.
The [baseline](2026-09-16-modern-mermaid-theme-audit.md) records its SHA-256.

The local CLI comes from the macOS ARM64 cargo-dist build at `6beea1497`; executable SHA-256:
`9bc0aa35f2571fda7fdb0db72d37f8052344955c1c356afae1b51403013f611a`.
There are no changes under `crates/`, `Cargo.toml` or `Cargo.lock` between that build and audit
workspace `2e3cb995c`. This is artifact reuse with a named build source, not a fresh final build.

Five presets were exercised: Editor Light, Editor Dark, Brutalist, Spotless and Cyberpunk.
Every example ran as SVG, PNG and PDF. Each invocation captured its exit status/stderr;
successful files also had byte counts, SHA-256 and format-signature checks.

| Output | Executions | Successful output | Parse failures |
| --- | ---: | ---: | ---: |
| SVG | 170 | 160 | 10 |
| PNG | 170 | 160 | 10 |
| PDF | 170 | 160 | 10 |
| Total | 510 | 480 | 30 |

No input failed only for a particular preset or export target. A timeout is recorded as a failure,
not a missing row. The CLI uses permissive theme admission (`crates/merman-cli/src/config.rs`);
these successes do **not** demonstrate that every requested facet was applied, that strict
admission passed, or that the output is visually correct. Reference theme CSS was not passed
through this matrix: it measures Merman presets against literal reference diagram inputs.

Reproduce with the bounded diagnostic runner (it exits nonzero if any row fails):

```console
python3 tools/debug/render_modern_mermaid_examples.py \
  --reference /path/to/repo-ref/modern_mermaid \
  --cli target/distrib/merman-cli-aarch64-apple-darwin/merman-cli \
  --output target/bench/experiments/modern-reference-new-run
```

The output directory must be new. It retains exact `.mmd` inputs, generated files and
`results.json`. Initial results are under `target/bench/experiments/modern-reference-audit`;
the checked-in runner's confirmation is under `target/bench/experiments/modern-reference-final-confirmation`.
This runner measures execution and artifact signatures, not portability or benchmark timing.

# Failure classification

Both failed inputs were parsed in a real headless Chromium page using the reference checkout's
installed Mermaid 11.12.1 and the project's pinned Mermaid 11.17.2 bundle. Browser execution
avoids the DOMPurify initialization error encountered in a plain Node attempt.

| Input | Merman | Mermaid 11.12.1 | Mermaid 11.17.2 | Finding |
| --- | --- | --- | --- | --- |
| Example 3, source line 58: Japanese Flowchart node IDs (`開始`, `注文`) | Rejects `Unexpected character at 17` | Parses | Parses | Confirmed at the baseline; corrected by the follow-up below |
| Example 22, source line 588: unquoted Chinese GitGraph branch names | Rejects invalid reference | Rejects lexer/parser input | Rejects lexer/parser input | Reference example is invalid for both tested upstream versions; do not change Merman to accept it merely for a green matrix |

The browser observations are retained in `reference-parse.json` beside the initial results.
Both failures occur before theme evaluation. Neither is an Unsupported/Unverified theme
admission observation; the earlier proposed conflation of parse failure with theme support is
incorrect.

# Preset usability observations

Visual inspection of `01-editor-dark.png` (Chinese login Flowchart) and `05-cyberpunk.png`
(Chinese payment Sequence) found visible Chinese labels on dark canvases. Arrowheads appear
considerably darker than the corresponding lines, making direction harder to see. This is a
usability concern requiring terminal-color and pixel/contrast investigation, not yet a quantified
contrast failure or a proven new regression. Only these two images were inspected visually.

| Question | Current evidence | Still needed |
| --- | --- | --- |
| Do representative Merman presets execute on real diagrams? | 480 successful SVG/PNG/PDF outputs; two input-specific failures classified above | Final release-profile replay after the parser correction |
| Do reference theme mechanisms map to typed public behavior? | Existing hash-bound 24-theme corpus | Compare recipe facets with emitted terminals and actual admission |
| Are default, dark, presentation and high-contrast presets usable? | Five presets executed; two dark images inspected | Broader visual checks, arrows/labels, long text and expected palette assertions |
| Do all ten presets satisfy product needs? | All ten executed in the native follow-up | Resolve the arrow-color gap; explicit use-case criteria, broader visual checks and qualification |
| Are clear, transparent and ordinal semantics correct here? | Historical focused tests, not this matrix | Actual authoring specs and negative terminal/admission observations |
| Is arbitrary reference CSS portable? | No such claim is made | Typed mechanism translation and explicitly bounded host CSS residuals |

# Cost and architecture audit still open

The baseline record retains exact local file sizes and withdraws the incorrect MiB conversion.
No equivalent pre-refactor/alpha.6 artifact comparison has been performed in this audit. The
coarse CLI smoke timings do not establish performance. Use existing Rust pipeline, memory,
Node and WASM benchmark owners with fixed revisions, features, fonts and inputs before deciding
whether growth is justified. Python/Typst/native sizes, cold start, throughput, large diagrams,
theme compilation and discovery costs remain outstanding.

Keep production family ownership and acceptance-only historical authority. No new generic proof
engine, bundled Inter resource, or budget increase is justified by the observations above.
C7b mechanism breadth and additional presets require explicit findings; zero Legacy routes and
many successful outputs do not close that work.

# Unicode parser correction follow-up

The Flowchart lexer now recognizes the exact 409 `UNICODE_TEXT` entries from the pinned
Mermaid 11.17.2 `flow.jison` at source commit `dcb694ddb58dc5ad3502e7e903cac05fd812eac3`.
It uses character-range matching without a new runtime dependency. Unsupported-character
recovery advances by a complete UTF-8 scalar, and the obsolete byte-only helper was removed.

The 197 selected Flowchart core tests pass, including the Japanese sample's nodes/edges/labels,
mixed Latin/Greek/Korean/CJK IDs beside edge operators, rejected out-of-range IDs, and recovery
after emoji. Real browser checks against both Mermaid versions accepted the mixed-ID case and
rejected emoji, combining-accent, U+9FCD and non-BMP letter IDs, matching the new regression cases.

A newly built development CLI (`--no-default-features --features svg,png,pdf,layout-cytoscape`)
reran exact reference example 3 with all five audited presets and three export targets: **15/15**
successful outputs with valid signatures. CLI SHA-256:
`3b343c1ab6454c14bbc804563f6096f55d4aca6cadb67fca260354f04c702f13`.
The input hash matches the original example; results and output files are retained under
`target/bench/experiments/modern-reference-unicode-repaired`. The third-party license contract
also passes. This is a targeted development-profile check, not a rebuilt release archive or a
new full 510-row matrix. The invalid GitGraph reference remains a reference-input limitation.
