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

# Candidate public discovery inventory

The installed Node native package built from `a5e3cd2d6` was queried through its public
operation API, using the family IDs and 47 semantic target IDs returned by its own catalogs.
For each pair, the diagnostic asked about fill, stroke paint and ordinal palette across all
six output IDs. This produced 27918 responses. The 33 catalog family IDs include `error`;
the count must not be equated with 33 fully supported visual diagram families.

For standalone SVG, the results were 188 Conditional, 343 Unsupported, 99 Unverified and
4023 NotApplicable. All 99 Unverified responses were canvas/root queries whose discovery
model is explicitly not yet represented. Browser SVG and each binary export returned
Unverified for relevant visual queries; ASCII returned NotApplicable for this visual subset.
These are coarse discovery classifications, not rendered-terminal findings or preset
qualification. In particular, Unverified PNG/PDF discovery does not mean the exporter cannot
produce an image, and the high NotApplicable count is mostly cross-family target combinations.

The raw queries, reasons, artifact source and summary are retained in
`target/bench/experiments/c7a-a5e3cd2d6/support-inventory.json`. Use these responses to select
terminal probes. Do not convert the response counts into a theme support percentage.

# Published alpha.6 footprint comparison

GitHub's alpha.6 release metadata records publication on September 2, 2026. The named macOS
ARM64 CLI/LSP archives were downloaded and matched their published SHA-256 sidecars. All eight
corresponding npm package versions and the macOS Python wheel were downloaded from their
registries and matched the registry integrity/hash metadata. These are actual published
artifacts, compared with the local clean alpha.7 candidate; they are not matched-toolchain
rebuilds or isolated theme-only experiments.

| Artifact | Published alpha.6 packed bytes | Local alpha.7 packed bytes | Delta |
| --- | ---: | ---: | ---: |
| CLI macOS ARM64 tar.xz | 11470900 | 13499304 | +17.68% |
| LSP macOS ARM64 tar.xz | 3974632 | 4054316 | +2.00% |
| `@mermanjs/web` | 4828999 | 6171330 | +27.80% |
| `@mermanjs/web-analysis` | 1456578 | 1526589 | +4.81% |
| `@mermanjs/web-render` | 4144575 | 5503311 | +32.78% |
| `@mermanjs/web-editor` | 1507960 | 1577497 | +4.61% |
| `@mermanjs/web-ascii` | 1973171 | 2034515 | +3.11% |
| `@mermanjs/node` | 111060 | 113593 | +2.28% |
| `@mermanjs/node-darwin-arm64` | 7785834 | 11068461 | +42.16% |
| `@mermanjs/node-wasm` | 4958170 | 7290115 | +47.03% |
| `merman-python` | 6883138 | 9228164 | +34.07% |

The CLI binary increased from 43330528 to 51398256 bytes (+18.62%); LSP from 17503072 to
17828544 (+1.86%). The CLI's default feature names are unchanged. The published CLI reports
alpha.6 when invoked with `--version`; LSP has no such version output, so its version identity
here comes from the release and verified archive checksum, not that invocation.

For `web-render`, the WASM itself increased from 10420901 to 14053324 bytes. `wasm-tools
objdump` locates 3073883 additional bytes in the code section and 548039 in data; the function
count increased from 16211 to 22916. Final packages omit function-name sections, so this
section-level observation cannot assign individual bytes to the theme compiler, shaping or font
decoding. It does establish that the increase is primarily executable code rather than npm
wrapper files. Renderer manifest changes add the theme contract, font shaping/parsing,
WOFF2/Brotli decoding, Unicode-script handling and digests. These have production consumers,
but their presence alone does not establish that all growth is necessary or optimally compiled.

The retained metadata, verified archives, component sizes/hashes and section dumps are under
`target/bench/experiments/theme-alpha6-impact/`: `published-release.json`,
`published-native-artifacts.json`, `published-package-artifacts.json`,
`package-size-comparison.json` and `wasm-attribution/`. Both published and candidate Node native packages were also installed and queried. Their
capability IDs remain `layout-cytoscape`, `layout-elk` and `svg`, and both expose only SVG
output. The candidate adds materialization, preset export and theme-support operations plus
the theme catalog. Thus the size comparison is not explained by adding PNG/PDF or math to
Node; it includes substantial new theme/font behavior inside the existing SVG capability.
`node-runtime-capabilities.json` retains both actual runtime responses. Node transport and
Python growth warrants further attribution despite passing the current artifact gates. Passing the September 14 WASM
budgets does not mean there was no increase from the published release. No additional budget
relaxation follows from this comparison.

# Cost and architecture audit still open

The baseline record retains exact local file sizes and withdraws the incorrect MiB conversion.
The [candidate record](2026-09-16-c7a-native-candidate-a5e3cd2d6.md) now adds same-source Python,
Typst, native SDK, Flutter and Node artifact sizes, plus final Web/Typst strip/compression
measurements. All 24 current WASM budget checks pass without changing their limits, and all
61 exact-profile dependency observations pass. These results establish current artifacts and
registered boundaries. The published-artifact comparison above supplies release footprint
deltas; neither result establishes minimum possible size.

The first release-range experiment is registered under
`target/bench/experiments/theme-alpha6-impact/experiment.yaml`: local tag `v0.8.0-alpha.6`
(`d529f858ea3d337a1bdc8fe12e44e1403ededf2e`) versus `a5e3cd2d6`, Rust 1.95.0, the unchanged
complete-SVG pipeline harness and 35 byte-identical standard fixtures. Each checkout retains
its own lockfile; any eventual delta includes dependency changes and cannot be attributed solely
to theme implementation. The diagnostic uses two AB/BA pairs with the long preset. Initial discovery failed on both
versions because the shared list also preflights ELK fixtures. The corrected experiment enables
`svg,layout-elk` on both sides; it retains the initial failure and does not bypass preflight.
The corrected diagnostic completed 68 timing invocations across 17 byte-identical SVG-output
fixtures. The other 18 fixtures have changed output hashes and were excluded from timing, so the
whole report correctly exits with `contract_failure`; it is not a passing 35-fixture comparison.
No output normalization or admission relaxation was applied.

The comparable subset raises a broad default-rendering regression signal. Representative
per-operation estimates are:

| Complete SVG operation | Published alpha.6 source | Candidate source | Diagnostic change |
| --- | ---: | ---: | ---: |
| Class tiny | 50.29 us | 187.71 us | +273% |
| Class medium | 597.67 us | 1341.20 us | +124% |
| XY Chart medium | 93.38 us | 244.12 us | +161% |

These operations do not select an explicit theme. Their output hashes match and both versions
use the same harness, toolchain and capability recipe. This therefore concerns the ordinary
rendering path, not just explicit theme authoring. The two diagnostic AB/BA pairs are insufficient
to declare confirmed regressions. An independent confirmation on these three fixtures is running
through the existing owner's balanced A/A calibration and power-sized AB/BA procedure, with the
registered +10% **and** +50 us thresholds. Raw diagnostic evidence is `end-to-end-elk.json`;
confirmation belongs to `end-to-end-confirmation.json`. Stage attribution and causal profiling
must precede any optimization or assertion that the added cost is necessary. The coarse
historical CLI timings remain insufficient for performance claims.

Manifest inspection finds five added workspace crates: production `merman-doc` and
`merman-theme-contract`, and publish-false `merman-theme-fixtures`,
`merman-theme-authoring-fixtures` and `merman-theme-acceptance`. Both compared workspace
manifests declare Rust 1.95. This is a manifest comparison, not runtime dependency reachability
or byte attribution; the exact-profile verifier remains the owner of dependency boundaries.
Cold start, stage attribution, throughput, large-diagram memory, theme compilation/discovery
and same-host alpha.6 artifact rebuild comparisons remain outstanding.

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
