---
type: Audit Baseline
title: Modern Mermaid theme capability audit baseline
timestamp: 2026-09-16
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 2e3cb995cb5a2c3a67db370adf10d136d727b65c
tags: theme,preset,modern-mermaid,audit
---

# Corrected reference inventory

Reference: `repo-ref/modern_mermaid` at
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`.
`src/utils/themes.ts` SHA-256:
`9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.
The checkout has an unrelated untracked `pnpm-workspace.yaml`; it was not used or modified.

The earlier indentation-based scan was wrong: there are **24 themes**, not 22. `handDrawn`
and `grafana` have different indentation and were omitted; they are not nested configurations.
The earlier 225-variable, 223,377-byte and selector-category counts are withdrawn. They also
counted JavaScript string characters rather than UTF-8 bytes in some CSS strings.

A read-only TypeScript AST traversal of the exported `themes` object's properties finds
24 `mermaidConfig` objects, 244 `themeVariables` properties, and 240,692 UTF-8 bytes of
`themeCSS` string contents. CSS sizes range from 1,333 to 18,646 bytes. This measures source
configuration size only, not rendered output, capability coverage, or Merman package cost.

The existing [capability corpus](../../../alignment/MODERN_MERMAID_THEME_CAPABILITY_CORPUS.md)
already owns the reviewed 24-theme mechanism and value-level mapping. Its snapshot uses the
same revision and source hash. Reuse that corpus instead of inventing another keyword-based
classification or claiming that CSS filters and backgrounds are inherently unsupported.
Corpus portability is an intended semantic contract, not proof of public preset implementation.

The 24 names are:

```text
linearLight, linearDark, notion, cyberpunk, monochrome, ghibli, spotless, brutalist,
glassmorphism, softPop, darkMinimal, wireframe, handDrawn, grafana, memphis, noir,
material, aurora, win95, doodle, organic, hightech, kawaii, geometricCollage
```

Current Merman has ten catalog entries. Only Brutalist, Spotless and Cyberpunk share reference
names; matching names do not establish equivalent recipes. Font stacks remain host/consumer
inputs. No Inter asset was added. Interactive annotation colors remain application-owned,
consistent with the existing corpus.

# Reference examples

`MERMAID_EXAMPLES.md` contains 34 fenced Mermaid blocks across 11 principal families, including
multilingual and advanced examples. SHA-256:
`c21f39ee68a9537c4651c3e2a54fc491c03a99f178806f0564e97c061d21981f`.

The earlier 88-run smoke used handwritten minimal diagrams; its description as examples
extracted from this file was incorrect. It establishes only successful execution of those
synthetic inputs. The corrected literal-input matrix and parser comparison are recorded in
[the impact audit](2026-09-16-theme-capability-impact-audit.md).

# Size and timing observations retained with limits

Previously measured local files, in bytes (not identical source/profile claims):

| Artifact | Bytes |
| --- | ---: |
| macOS ARM64 CLI executable | 51,379,344 |
| CLI tar.xz | 13,502,080 |
| LSP executable | 17,807,584 |
| LSP tar.xz | 4,050,224 |
| Node N-API darwin-arm64 | 24,548,080 |
| Node WASM | 20,084,758 |
| Web analysis WASM | 3,674,858 |
| Web ASCII WASM | 5,201,971 |
| Web editor WASM | 3,785,997 |
| Web full WASM | 15,937,667 |
| Web render WASM | 14,036,578 |

51,379,344 bytes is about **49.00 MiB**, not the previously reported 51.4 MiB.
Raw files and stripped/compressed metrics must remain separate. Without matched baseline
artifacts and feature profiles, these observations do not establish a size regression.

The earlier `time -p` samples are insufficient for latency conclusions. They mixed launch/font
initialization with rendering and used coarse timing and too few samples. They cannot establish
cold-start cost, throughput, memory, or a speedup. Formal comparison will use the existing
benchmark owners and matched inputs/profiles; no budget was widened from these samples.

# Public Cyberpunk consumer boundary review

The September 16 follow-up inspected current source `81ee5cad0` and the pinned reference recipe.
The external browser comparison used a reused CLI, Mermaid 11.12.1 and no font download; it corroborates missing visible mechanisms but is not an exact-HEAD or all-target qualification run.

The public catalog's recipe builder still returns `ThemeDefinitionV1`, then the existing shared `materialize_spec_wire` path serves both compilation and complete-spec export.
That shared seam already exists; the missing capability is composition of a full built-in recipe there, not a second compiler/export pipeline.
The current Cyberpunk recipe omits the reference's ordered glow and layered grid/radial background despite those concepts being present in the complete model.

The global effect binding matrix admits State/State only.
The nonempty State writer and native filter receipt recognize a single SourceGraphic zero-spread shadow; the exporter also assumes one reference per filter.
State has additional typed Clear routes, so this is not a claim that every other State effect facet is absent.
Sequence width/radius, Flowchart label weight and XY per-series stroke/opacity consumers also need work for the actual reference scenes.
Updating only the effect graph emitter would not close these drawing and native-admission boundaries.

The reference background includes preview-container composition, with two-direction 40px grid and radial Screen layer.
Full-spec root canvas support already exists, but the public recipe does not use it.
C6's hand-built Cyberpunk scenes remain useful mechanism tests and cannot qualify a different public recipe or establish reference reproduction.

The [current product boundary plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md) makes three public-entrypoint scenes—Flowchart, Sequence and XY Chart—required before final C7a freeze, while retaining honest limits for the other themes and families.
No production code or runtime tests were changed or run during this planning review.

# Current-source public-entrypoint baseline

The September 16 U1 run rebuilt source `8dcc6d7d6` with:

```text
CARGO_BUILD_JOBS=1 cargo build --release --locked -p merman-cli --no-default-features --features svg,png,pdf,layout-cytoscape
```

The binary SHA-256 was `0e73af8e999b4de2301bd4df13fdf9a2252e64052009224f6fea1d7d80e416bf`.
This is a bounded native render profile, not the complete shipping CLI profile.
Each committed input in `crates/merman-theme-fixtures/fixtures/public-cyberpunk/` was passed to
`merman-cli render --theme-preset cyberpunk` with a caller-supplied Arial font configuration.
All twelve commands succeeded: three families, each with parity SVG, resvg-safe SVG, PNG and PDF.
All three PDFs opened as single-page documents in macOS PDFKit and produced preview images.

Raw commands, input/output hashes, diagnostics, captures and the browser/PDF preview scripts are in
`target/bench/experiments/theme-public-baseline-8dcc6d7d6/`:

- `render-results.json`: SHA-256 `62577a8fe7767a2425b723ef283f2db3499f7fbd6d40a36b5f2ab0a71ea3f356`.
- `browser-observations.json`: SHA-256 `55694cabc4e866ea91d28c6954a99b0ae563af08558628110e067648aeb2a0dc`.

Browser observation used HeadlessChrome 151.0.7922.77 at 1280×960, DPR 1, without reference CSS or
external network access. Arial was requested through Mermaid configuration; the installed regular
font's path and hash are recorded in the command report. No font resource was embedded. This records
the available host font, not a proof of every resolved glyph or native font face. Reference captures
use the same requested family but retain their own layout and style sizes. Exact geometry, glyph
rasterization and whole-image pixel equality are not acceptance criteria.

| Terminal | Current public preset observation | Required product difference |
| --- | --- | --- |
| Flowchart rectangular nodes | `#0f172a` fill, `#22d3ee` 1px stroke, automatic corner radius, no filter | Declared navy/cyan recipe, 3px stroke, 10px rectangular corners and two ordered glow stages; decision geometry preserved |
| Flowchart labels and edges | Plain labels and edges; native labels remain present | Explicit label weight/color/glow, visible marker paint and edge-label backgrounds |
| Sequence actors | 1px cyan borders, 3px corners, weight 400 labels, no filter | Separate actor geometry/text application, intended border/corner/weight and composed glow |
| Sequence messages | 1.5px cyan strokes without filters; response retains `3,3` dashes | Intended 2px strokes and glow while preserving request/reply distinctions |
| XY bars and lines | Opaque cyan/green bars with zero-width borders; pink/yellow 2px lines, no filters | Recipe-specific series colors, independent bar fill alpha and stroke, glow, and documented cycling |
| XY title | Pale `#e0f2fe`, 20px, weight 400, no shadow | Cyan, 18px, weight 700 and title glow |
| Complete canvas | No tile or gradient definitions; a solid `#020617` root canvas only | Explicit two-direction 40px grid and radial Screen composition |
| XY canvas ownership | A later `.background` rectangle paints white over the root canvas | Default family background yields to explicit typed canvas; explicit caller/source background retains precedence |

The last row is a concrete existing product defect, independent of exact Cyberpunk reproduction.
It occurs in both SVG variants and visibly in native PNG and PDF: pale titles/axis labels sit on
white despite the emitted dark root canvas. The earlier source/mechanism inventories did not catch
this composition error. A root canvas element existing in the document does not prove that a family
has left it visible.

The captures characterize missing behavior; successful exports do not qualify the preset. Flowchart
also includes unrelated filter definitions, so raw `<filter>` counts must not be treated as applied
theme glow. Later acceptance must inspect actual terminal bindings and native appearance. The scene
README remains the per-terminal source specification. Geometry differences are tolerated when
semantics are preserved; missing labels, family backgrounds covering the canvas, absent requested
paint/effects, clipped glow, and changed relationship/message semantics are not tolerances.

This baseline advances U1. Durable public-preset/browser regression coverage for the full future
recipe is still required alongside U4–U9; neither this capture nor the portfolio probe closes C7a.

## Background fix scope and remaining configuration boundary

The baseline led to a focused fix shared by XY Chart and Wardley: when a root canvas has an explicit
base or at least one layer, the family's default full-size background keeps its DOM terminal but
uses `fill="none"`. Explicit background ownership is checked through the existing configuration
provenance, for both `themeVariables.background` and the family's nested background path. An
unspecified canvas does not suppress the historical background. No preset-name special case,
new dependency or public API is needed.

The regression uses caller/site configuration. Source `themeVariables` directives are protected by
the public facade's existing hardened policy; writing a directive does not establish an effective
source override. Existing layering tests retain that policy check. The unthemed and themed render
must preserve the same resolved paint when caller configuration owns the root background.
Wardley's already-derived nested background can retain white even when a root background variable
is supplied. This pre-existing configuration-resolution behavior is not corrected by the canvas
occlusion fix and must not be described as a newly verified root-to-family color override.

## Next runtime-cost boundary

The U2 source inspection identifies two existing owners to change, without introducing another
proof framework:

- `family.rs::render_svg` currently partitions prepared-label IDs even when the prepared ledger is
  empty. The empty-ledger path can preserve the emitted SVG without parsing it for absent metadata.
- `render.rs::render_svg_target` finalizes every SVG for target admission;
  `StandaloneSvgArtifact::observe_exact` performs native terminal validation even for ordinary
  parity output. Observation demand belongs at this existing facade/finalization boundary.

Strict requests and native export still need actual terminal validation, resource limits and
cancellation. The public admission model currently has no `Unverified` status, so an unobserved
ordinary SVG must not be reported as Portable by accident. Resolve that contract at the existing
admission owner before removing the expensive validation call. Preserve the current invalidation
of prepared/theme evidence after untrusted postprocessing. These are identified implementation
seams, not completed performance improvements.

## Focused fix verification

- Red evidence: the two public-family regression tests failed on the preset case before the fix
  (`white` instead of `none`). Subsequent test development distinguished protected source directives
  from effective caller configuration and recorded Wardley's pre-existing derived-background behavior.
- Green evidence: Release nextest passed 10/10 facade tests across `theme_canvas_background`,
  `xychart_typed_render` and `diagram_theme_layering`, plus 37/37 renderer tests across
  `xychart_svg_test` and `wardley_svg_test`. The new matrix covers nine cases per family.
- The same bounded CLI profile was rebuilt with the working-tree fix. Binary SHA-256:
  `269511080b87d76607408b0aded4cfc4822ed162334908083ca13d818c54addf`.
  The production patch is retained as `background-fix.patch` with SHA-256
  `98a5ac4495843ad9c76a38e3dfce719e5d807fae2298067c900496cf498c5407`.
- Twelve post-fix public renders succeeded: Flowchart, Sequence, XY Chart and Wardley, each as
  SVG/PNG/PDF. Both unthemed XY/Wardley SVG controls are byte-identical to their pre-fix outputs.
  Protected source-directive samples correctly change with the preset; they are not valid controls
  for an effective caller override. Caller override preservation is tested through the Rust facade.
- XY Chart PNG and a PDFKit-rasterized PDF now visibly show the navy canvas behind the pale title and
  axis labels. Wardley also shows the requested canvas, but its current default dark axis/text paint
  is hard to read on navy: this remains a family recipe/consumer gap, not a qualified dark theme.
- `after-fix-results.json` SHA-256:
  `433bb0a10c64b19c0f0a54a5a2e1659681ba3dbc0ae294782613f5a2d4b4b670`.
  The bounded correctness review found no regression in this patch. No full workspace, complete
  browser suite, all-platform build or performance remeasurement is claimed by this verification.

## U2 resource-accounting constraint (2026-09-16)

Further inspection at `94e42be6d` shows that removing native certification is not equivalent to
removing its complete validator. `final_validation.rs::validate_element` currently collects reference
facts while checking native compatibility; its reference planner accounts for `use`, `feImage`,
marker geometry, and filter/mask/clip-path application multiplicity. Ordinary SVG must retain the
currently enforced reference-expansion rejections and cancellation without inheriting native-only
compatibility promises.

Reuse the existing reference graph/expansion owner and separate resource accounting from native
compatibility. The browser static validator currently covers `use` expansion only and cannot replace
that accounting wholesale. A follow-up call-chain check found that `foreignObject` and unsupported
native CSS already stop the compatibility pass before expansion planning; ordinary BestEffort
converts that compatibility error into ValidationFailed and still returns the SVG. Output-byte limits
and the initial raw-element preflight are separate from reference planning; they do not establish
complete browser CSS expansion certification.
Preserve expansion rejection for the existing analyzable subset. A new conservative browser CSS or
inherited-marker model would be new scope, not a prerequisite disguised as regression preservation.
The current expansion saturation at
`MAX_RESVG_TREE_NODES + 1` must also become appropriate to the effective budget before an ordinary
SVG path can accept budgets larger than that native ceiling. Keep native backend ceilings for native
export. Cover cycles, duplicate IDs, href/xlink alternatives, marker/path amplification and cancellation.

The public `TargetAdmissionStatus` currently contains Portable, HostDependent and Rejected.
Unobserved ordinary output therefore needs an explicit Unverified state at this existing owner;
absence of observation must not become either Portable or a fabricated native validation failure.
Document/native export and strict qualification still require their actual target observations.
These are implementation constraints, not a claim that certification has already been decoupled.

## U2 prepared-label scan experiment

The adjacent baseline is `94e42be6d`. Before changing production code, Release nextest passed
11/11 token-scanner characterization tests. The candidate checks the existing reserved ID prefix
and fixed foreignObject evidence attribute with the checkpoint-aware substring search. An empty
ledger with neither raw spelling returns the same borrowed SVG; either spelling falls back to the
existing scanner. Nonempty ledgers retain full token validation. This avoids tag/attribute parsing;
both paths remain linear in SVG byte length, and the candidate still performs up to two searches.

Release nextest passed 233/233 SVG pipeline tests after the candidate change. This includes ordinary
and forged token handling, Unicode/quoted attributes, malformed-markup behavior, cancellation,
strict validation, static output and resource-limit regressions. A test initially assumed that a
reserved spelling in plain text was invalid; the baseline characterization contradicted it, so the
new differential assertion was corrected to compare both success and failure outcomes. Production
behavior was not changed to satisfy that incorrect expectation. A bounded independent review found
no missing token recognition or cancellation path. This is not a full branch review.

The same-profile baseline benchmark executable is frozen under
`target/bench/experiments/prepared-label-scan-94e42be6d/` with SHA-256
`e2cd5dc03e39ccd86f00cd3a0ea98713c359eb1b53c88de4e6ec1da7e3450110`.
The revised candidate's clean-source, full-feature confirmation is now complete. The initial
confirmation attempt used only `svg` and correctly failed before timed samples because Criterion
discovered the `flowchart-elk` fixture; it is retained as a contract-failure diagnostic rather than
performance evidence. The successful rerun used `svg,layout-cytoscape,layout-elk`, eight calibration
pairs, eight AB/BA confirmation pairs, 30 Criterion samples, a two-second warmup and a three-second
measurement window. All four fixtures retained byte-identical SVG output and passed postflight
identity checks; the aggregate decision was `confirmed_non_regression`, with one confirmed
improvement, zero regressions, zero contract failures and zero inconclusive fixtures. This closes
the candidate's isolated scan claim, not the alpha.6 recovery gate or U2's ordinary-output admission
split. No native-certification policy changed in this experiment.

The decision-grade fixture rows were (Bonferroni-adjusted one-sided bounds,
95% simultaneous confidence for each decision family):

| Fixture | Baseline (us) | Candidate (us) | Relative change | Reported simultaneous bounds | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Class tiny | 183.306 | 165.075 | -9.95% | [-11.08%, -8.77%] | Non-regression; improvement inconclusive |
| Class medium | 1321.075 | 1193.563 | -9.65% | [-10.11%, -9.10%] | Confirmed improvement |
| XY Chart medium | 239.506 | 218.699 | -8.70% | [-11.00%, -6.63%] | Non-regression; improvement inconclusive |
| Info medium | 32.166 | 31.533 | -1.97% | [-3.05%, -0.76%] | Confirmed non-improvement, no regression |

The exact JSON and Markdown reports are `confirmation-v2.json` and `confirmation-v2.md` in the
experiment directory. The candidate source snapshot used for the clean measurement is the synthetic
commit `8aacba3827f2879b798093df29b2405ca3f2019b`; it is measurement provenance, not a branch or
release commit. No alpha.6 regression closure is inferred from this isolated experiment.

The first candidate used two substring searches. Two balanced AB/BA pairs per fixture (30 Criterion
samples, 2-second warmup, 3-second measurement per invocation) produced the following diagnostic
means of two point estimates. Every invocation's exact SVG byte length, element count and SHA-256
matched the baseline, including its postflight check. These are working-tree diagnostics under
background host load, without noise calibration or independent confirmation:

| Public end-to-end fixture | Baseline (us) | First candidate (us) | Diagnostic difference |
| --- | ---: | ---: | ---: |
| Class tiny | 184.185 | 176.700 | -4.1% |
| Class medium | 1367.900 | 1314.900 | -3.9% |
| XY Chart medium | 249.230 | 233.980 | -6.1% |
| Info medium | 33.096 | 35.3385 | +6.8% |

Do not accept the first candidate on these numbers: the Info control increased by about 2.24 us.
The fixed evidence attribute itself contains the reserved ID prefix, so the second full search is
redundant. The revised candidate searches that attribute only if its constant spelling no longer
contains the prefix. This keeps the exact-absence proof valid if those owners change independently.
The first candidate executable, patch and `diagnostic.json` remain in the experiment directory;
results from it cannot be attributed to the revised candidate. No alpha.6 regression is closed by
this experiment.

The revised single-search candidate again passed 233/233 Release SVG pipeline tests. Repeating the
same diagnostic schedule against the original frozen baseline yielded:

| Public end-to-end fixture | Baseline (us) | Revised candidate (us) | Diagnostic difference |
| --- | ---: | ---: | ---: |
| class_tiny | 184.050 | 163.940 | -10.9% |
| class_medium | 1333.100 | 1195.650 | -10.3% |
| xychart_medium | 241.380 | 213.175 | -11.7% |
| info_medium | 32.377 | 31.971 | -1.3% |

All 16 invocations retained identical per-fixture SVG preflight identities and passed postflight
identity checks. The first candidate's Info increase did not recur in this diagnostic. The revised
executable SHA-256 is `cfff79e557f3941633f0672191d81d28e6597af04cda05ea535a92ada1a061fd`;
raw results are `diagnostic-single-search.json` and adjacent logs. The production patch and file
hashes are retained as `single-search-code.patch` and `single-search-context.json`.

The earlier working-tree diagnostic pairs are historical measurements and are superseded for this
isolated scan claim by the clean-source confirmation above. They do not establish an alpha.6 recovery
gate, and they do not close U2's ordinary-output admission split or U10 impact closure. The
strict/native observation boundary remains active.

`cargo fmt --all -- --check` passed.
`CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman --features svg --lib` exited successfully;
it still emitted warnings, including large error values
and unused helpers. This run is not a completed workspace-wide warning classification or a
warning-free gate. No complete workspace, browser matrix or installed-package rebuild was run for
this isolated candidate.

On September 17, the current production files matched the clean confirmation snapshot exactly.
`CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render --lib prepared_text` passed
47/47 focused tests. An initial command targeted the facade crate and selected zero tests; a
second incorrectly supplied its `svg` feature to the renderer crate and failed before compilation.
Neither unsuccessful command is counted as verification.

## U2 reference-budget correction (2026-09-17)

A new negative test reproduced a resource-accounting defect before implementation: an ordinary
Parity SVG with a 20-level branching `use` DAG returned `ValidationFailed` instead of rejecting
its explicit two-million-element budget. The existing graph saturated at the native backend's
one-million-node ceiling plus one, so the compatibility-only error hid the larger resource limit.
The raw document contained only 65 elements; no expanded SVG was allocated by the test.

Reference counts and marker multiplicity bounds now use saturating machine-word arithmetic,
independently of the backend ceiling. The existing graph traversal, cycle detection and native
checks remain in place; resource counts can reach the requested budget before native compatibility
is classified. This adds no dependency and does not materialize expanded nodes. Separate tests
exercise arithmetic overflow and marker bounds. This is a prerequisite to U2's observation split,
not completion of that split or proof of arbitrary browser CSS expansion accounting.

Verification: the new above-ceiling budget test failed before the change by returning an observed
artifact, then passed after the change. `CARGO_BUILD_JOBS=1 cargo nextest run --locked
-p merman-render --lib svg::pipeline` passed 239/239 tests in the development test profile, including
static and strict native validation, cycles, cancellation, prepared labels and foreignObject
characterization. `cargo fmt --all` and `git diff --check` passed. The foreignObject characterization
still records the current `ValidationFailed` state; U2 must replace this with the planned unverified
ordinary-output state during the observation split. No performance claim or complete export-matrix
claim is made for this resource-budget correction.
