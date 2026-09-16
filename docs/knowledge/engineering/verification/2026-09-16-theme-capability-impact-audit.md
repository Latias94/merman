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
contrast failure or a proven new regression. Only these two images were inspected visually in that earlier pass.

The follow-up inspected candidate example 01's native PNG for all ten presets at its original
503 x 955 size. The Chinese labels fit their nodes in this sample, and all ten distinguish the
main text from their canvases. The five dark variants (Editor Dark, One Dark, Gruvbox Dark,
Ayu Dark and Cyberpunk) retain visibly dark arrowheads against their dark canvas; Ayu Dark's
direction markers are particularly hard to distinguish. Light variants expose the marker color
mismatch more clearly without the same dark-on-dark visibility problem. This supports the
already computed-style-confirmed Flowchart gap; it is not an accessibility certification,
quantified contrast result, or all-family visual pass. Retained files are
`c7a-a5e3cd2d6/reference-matrix/01-<preset>.png`.

Additional native PNG spot checks covered Editor Dark Pie (19) and Gantt (17), Cyberpunk
Mindmap (26), and Spotless Timeline (28). The inspected Pie title/legend and Gantt task labels
are visible, and Timeline text fits the displayed boxes. The Mindmap's bright green branch
uses white labels, which merits a computed-color/contrast follow-up before calling Cyberpunk
consistently readable across families. This is a visual concern, not a measured threshold
failure or proof that the refactor introduced it. No new preset is needed to investigate these
existing palette/terminal issues.

| Question | Current evidence | Still needed |
| --- | --- | --- |
| Do representative Merman presets execute on real diagrams? | 480 successful SVG/PNG/PDF outputs; two input-specific failures classified above | Final release-profile replay after the parser correction |
| Do reference theme mechanisms map to typed public behavior? | Existing hash-bound 24-theme corpus | Compare recipe facets with emitted terminals and actual admission |
| Are default, dark, presentation and high-contrast presets usable? | Five presets executed; two dark images inspected | Broader visual checks, arrows/labels, long text and expected palette assertions |
| Do all ten presets satisfy product needs? | All ten executed in the native follow-up | Resolve the arrow-color gap; explicit use-case criteria, broader visual checks and qualification |
| Are clear, transparent and ordinal semantics correct here? | Historical focused tests, not this matrix | Actual authoring specs and negative terminal/admission observations |
| Is arbitrary reference CSS portable? | No such claim is made | Typed mechanism translation and explicitly bounded host CSS residuals |

# Class relation-label readability follow-up

The maintainer identified nearly invisible relation labels in candidate
`reference-matrix/09-cyberpunk.png`. The exact source contains Chinese Class names and four
relation labels (`拥有`, `包含`). Real Chromium inspection of the original SVG reproduces the
problem before PNG export: text is `rgb(224, 242, 254)` while the HTML label background remains
`rgb(236, 236, 255)`. A minimal `classDiagram / A --> B : owns` reproduces the same colors,
excluding Chinese font fallback and rasterization as the root cause.

The preset requests `EdgeLabelBackground.fill`, but Class classified that surface as Unsupported
and retained the default `mainBkg` on the real label elements while applying theme Text.fill.
The old retired projection targeted an unused selector; restoring it would not paint those
terminals. The working repair adds direct static solid/transparent fill on the actual HTML div
and span and native SVG background rect, with existing source/config precedence and completed
writer checkpoints. Unsupported ordinal/value/sibling facets retain residuals. The public support
vector changes to Conditional without increasing the unpublished v1 contract number. The
historical KTD23 retirement record and baseline digest remain intact. The current receipt gate
continues to reject compatibility routing and any compatibility assignments, but allows a later
direct writer; the old permanent-Unsupported assumption incorrectly froze future capability.
Native transparency checks follow the converter: transparent HTML backgrounds omit the fallback
rect, whereas SVG labels retain an explicitly transparent rect.

The new regression first failed on the missing HTML background style and then passed all three
looks with both label modes. The expanded Release run passed all 345 selected Class, mechanism
matrix and support-manifest tests (2,313 unrelated tests filtered out). This includes negative
receipt cases for missing, duplicate, wrong-color and unknown terminals. The original Class input
now exports through all ten presets to SVG, PNG and PDF (30/30), and real Chromium label-color
checks pass for all ten. Cyberpunk now computes `rgb(2, 6, 23)` on both HTML background surfaces;
the four original PNG regions contain dark backgrounds and visible light glyph pixels. These
bounded checks do not establish general accessibility or qualify the entire Class family.
The five native/historical acceptance tests pass without skips, including five base schemes,
three looks, both label modes, static/Default qualifiers and solid/transparent fills. The current
receipt digest was refreshed for the changed live disposition; historical witness and inventory
digests remain unchanged. The C ABI shared support golden also passes all 22 vectors through
the public function table (one selected Release test; 61 unrelated tests filtered out). Formatting
and diff-whitespace checks pass. The old installed candidate remains source-specific and has
not been rebuilt. Diagnostic commands, logs and
artifacts are retained under `target/bench/experiments/class-label-readability/`.

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
rendering path, not just explicit theme authoring. The two diagnostic AB/BA pairs alone were insufficient
to declare confirmed regressions. Independent confirmation has now completed: eight balanced A/A
calibration pairs per executable and eight fresh AB/BA confirmation pairs per fixture. All three
operations are **confirmed regressions** under the registered +10% **and** +50 us thresholds.
The owner reports decision-grade evidence with 95% simultaneous confidence and Bonferroni
adjustment across the six metric components.

| Confirmed operation | Alpha.6 | Candidate | Relative bounds | Absolute bounds |
| --- | ---: | ---: | ---: | ---: |
| Class tiny | 50.59 us | 188.28 us | +270.28% to +274.05% | +136.33 to +139.31 us |
| Class medium | 600.31 us | 1353.40 us | +124.58% to +126.60% | +744.88 to +765.38 us |
| XY Chart medium | 92.76 us | 244.62 us | +162.04% to +165.64% | +150.03 to +154.03 us |

Raw diagnostic evidence is `end-to-end-elk.json`; independent confirmation is
`end-to-end-confirmation.json`. Stage diagnostics completed separately: parse differences are
approximately +0.4/+1.6/+0.4 us (Class tiny/medium, XY Chart), layout +4.5/+50.3/+3.4 us, and
SVG emission +40.5/+227.5/+53.4 us. These batched stage boundaries exclude setup and cannot
be summed as a complete-operation decomposition. The changes point beyond parsing toward
emission and public-operation finalization; causal profiling remains necessary. It must precede any
optimization or assertion that the added cost is necessary. These results are release-range
regressions, not isolated theme-only causal effects. The coarse historical CLI timings remain
insufficient for performance claims.

CPU sampling now covers seven candidate loops (three complete operations, three prepare-plus-render
loops and a parse-only control) and three alpha.6 complete-operation loops, using the unchanged
`profile_render` example and the same feature recipe. Each loop runs for 20 seconds; macOS
`sample` observes ten seconds at a one-millisecond interval. All ten loops and sampling commands
succeeded. The release binaries retain function symbols without line-level debug information.

| Complete operation, candidate source | Prepared-label token partition | Standalone finalization | Compatibility validation, included within finalization |
| --- | ---: | ---: | ---: |
| Class tiny | 17.0% | 34.9% | 21.7% |
| Class medium | 14.8% | 24.8% | 15.5% |
| XY Chart medium | 15.2% | 27.6% | 18.6% |

These are inclusive sampled-stack proportions, not removable time or optimization admission.
Nested columns must not be added. The corresponding named paths are absent from all three
alpha.6 samples and the candidate parse-only control. Source comparison against branch baseline
`b381b842f` confirms that ordinary SVG without an explicit pipeline previously completed the
family result directly; the current facade invokes standalone finalization and observational
compatibility validation. Token partitioning also accounts for 23.5%–35.8% of the candidate's
prepare-plus-render samples. Its scanner repeatedly allocates KMP prefix tables, including for
single-byte searches. This identifies concrete owners to investigate; it does not justify bypassing
malformed-token/XML checks, cancellation, resource limits or terminal evidence. Raw stacks,
commands, executable hashes and inclusive counts are retained in `cpu-profile-summary.json` and
`experiment.yaml` beside the timing reports.

The maintainer's scope review distinguishes typed theme configuration from native font/resource
and artifact-assurance capabilities. Against `b381b842f`, the renderer adds non-optional direct
dependencies on `rustybuzz`, `ttf-parser`, `unicode-script`, `wuff`, `brotli-decompressor` and `sha2`,
plus the workspace theme contract. Their production consumers are shaping, font metadata,
WOFF2 decoding and fingerprints/receipts; the contract adds canonical JSON serialization.
They are not all necessary consequences of typed colors, borders, typography properties or
rule precedence. Some may already have existed transitively in export-heavy profiles, so this is
not a claim that each is new to every resolved product closure. The next design review must
separate the minimal theme/SVG dependency and execution boundary from explicit font-resource
and native-export guarantees, retaining existing promised behavior until a reviewed replacement
is validated. No dependency removal, new feature split or budget relaxation has been admitted yet.

Manifest inspection finds five added workspace crates: production `merman-doc` and
`merman-theme-contract`, and publish-false `merman-theme-fixtures`,
`merman-theme-authoring-fixtures` and `merman-theme-acceptance`. Both compared workspace
manifests declare Rust 1.95. This is a manifest comparison, not runtime dependency reachability
or byte attribution; the exact-profile verifier remains the owner of dependency boundaries.
A path-grouped release-range diff covers 1,898 changed files, 423,657 added lines and 37,798
removed lines. Of these, 127 integration-test/benchmark/example files account for 86,642 added
lines, and 68 private theme acceptance/fixture files account for 38,234. The 652 remaining crate
files contain 253,851 added lines, including inline tests; this is not a production-only count.
`source-delta.json` retains the complete grouping. These counts include other mainline changes
between releases and cannot establish theme-only cost, binary size or code quality. They do
identify maintenance surface that should be evaluated independently of shipped dependencies.

Cold start, PNG/PDF throughput, large-diagram memory, theme compilation/discovery and same-host
alpha.6 artifact rebuild comparisons remain outstanding. CPU sampling has identified shared
owners; a measured, correctness-preserving optimization has not yet been selected.

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
