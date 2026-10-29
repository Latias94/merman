# 0.9.3 release readiness

Reviewed on 2026-10-29 in Asia/Shanghai. This is a preparation record, not a publication receipt.

## Decision and source boundary

Do not tag the current candidate as a compatible 0.9.3 patch yet. The maintenance implementation contains a confirmed public Rust enum extension that breaks existing exhaustive matches, and the new SDKs still require complete qualification at the final release commit. Ordinary PR CI is positive evidence, but it does not replace immutable release preflight.

| Identity | Reviewed value |
| --- | --- |
| Previous workspace publication | `v0.9.2`, commit `8e303a43b548c9e40e73f0bc657925d66e7d4fb5`, published 2026-10-22 in Asia/Shanghai |
| Maintenance target | `origin/release/0.9.x`, commit `b1f662e430f16204190fc05c8387573c6d5aea3e` |
| Development comparison | `origin/main`, commit `dc3ca0f22d34bc4306c81b237b7ed5c974d4ca1d` |
| Changelog range | `v0.9.2..b1f662e430f16204190fc05c8387573c6d5aea3e`; the base is an ancestor of the target |
| Preparation branch | `docs/release-0.9.3-readiness-20261029`, based on the reviewed maintenance target |
| Intended version and date | `0.9.3`, intended for 2026-10-29; headings remain `Unreleased` until compatibility is resolved and immutable preflight is selected |
| Compatibility baseline | Mermaid `12.1.0`, source `21f72f07ea22c0af48a3149c550654e80d8e40cb`, Presentation API, Options JSON schema 2 |

GitHub release inspection found no intervening workspace prerelease or 0.9.3 release. SDK preview archives and local qualification are not public SDK releases. The outcome ledger therefore consolidates the four maintenance integrations into their final state since 0.9.2; corrections to newly introduced SDKs are part of their initial feature outcome. Previously published release sections remain unchanged.

## Release blockers

### Public Rust source compatibility

PR [#250](https://github.com/Latias94/merman/pull/250), commit `0a74e782b`, appends `HtmlLineHeight = 22` to the public, exhaustive `TextMeasurementOperation` enum in `crates/merman-render/src/generated/text_measurement_abi.rs`. The owner descriptor is `contracts/abi/text-measurement-v1.json`; adding the enum variant is not made source-compatible by retaining protocol 1 or the existing numeric codes. The [host guide](../bindings/HOST_TEXT_MEASUREMENT.md) already documents the required new match arm.

A Rust 1.95.0 cross-crate probe extracted the exact enum declaration from each reviewed ref and compiled the same consumer matching all 22 published variants. It succeeds against 0.9.2 and fails against the maintenance target with `E0004: non-exhaustive patterns: TextMeasurementOperation::HtmlLineHeight not covered`. This isolates the demonstrated break; it is not a claim that a complete public-API comparison has passed.

For a 0.9.3 release, preserve the published Rust match contract while adapting literal-line measurement internally and retaining the text regressions. If the public variant must remain, select a new minor release and prepare its migration instead. Adding `#[non_exhaustive]` now would itself invalidate existing exhaustive matches; a changelog warning alone does not restore patch compatibility. The pending changelog deliberately exposes the current break until the implementation is corrected.

The public `ALL` array also changes from 22 to 23 elements, and the UniFFI operation enum gains a variant. A mechanical descriptor rollback can restore those declarations, but routing literal decoded text through the old Wrapped operation is not established as an equivalent exact measurement: existing adapters interpret markup differently, including literal `<br>` and zero-size text. Bypassing the selected host for a built-in estimate would change measurement provenance and can lose host-provided geometry. This review therefore does not remove operation 22 or silently downgrade the text fix; a compatible adaptation needs its own source-backed proof and regressions.

### Exact-source release and SDK qualification

The reviewed upstream target still declares workspace version 0.9.2. The preparation branch uses the repository-owned version projector to select 0.9.3; it must be committed together with its generated assets and release notes before freezing a release source. Do not qualify an old 0.9.2 build and relabel it as 0.9.3.

At inspection, `release-sdk.yml` and `release-sdk-publish.yml` each have zero runs. The latest maintenance preflight, [39648535313](https://github.com/Latias94/merman/actions/runs/39648535313), belongs to the immutable 0.9.2 source and predates the SDK matrix. PR #245's six local SDK archives are bound to `addd3fd8dfcbb0f9060223381e64cac9c54e4ce2`; later rendering, HTML5, host-measurement, and configuration changes require fresh qualification.

The final source must pass the 18-archive SDK matrix: native Linux x64/ARM64 GNU, macOS x64/ARM64, Windows x64 MSVC, and WASI, each in standard/math/full form, plus the three native/WASI/CLI comparisons. The producer must exercise extracted C/C++ shared/static and Go/wazero/Python/wasmtime consumers and generate each artifact's actual catalog and legal inventory. Linux's admitted host is Ubuntu 24.04; this does not establish musl or older glibc support.

## Verified outcome ledger

All entries below compare the published 0.9.2 state with the reviewed maintenance target. SDKs and instance-local JavaScript icons are first introduced by this release; their pre-publication fixes are folded into those outcomes.

| Surface | 0.9.2 state -> candidate state | User action or impact | Evidence and disposition |
| --- | --- | --- | --- |
| Native C / WASI SDK delivery | No official archive recipes -> five native targets plus WASI, standard/math/full recipes and extracted consumer qualification | Choose the transport and profile from its catalog; downloads require the separate SDK publisher | [#245](https://github.com/Latias94/merman/pull/245), [issue #243](https://github.com/Latias94/merman/issues/243); Added |
| SDK fonts and metadata | No memory-font constructor -> appended C ABI 3 slot and WASI ABI 1 using shared measurement/painting fonts and dimensions | Supply fonts for missing scripts; SVG viewers still need matching fonts | #245, `merman-ffi` prefix tests and `merman-wasi` runtime tests; Added, same-boundary SDK repairs folded in |
| JavaScript icons | No engine-local loaded packs -> `createRenderSession({ iconPacks })` and `createNodeEngine({ iconPacks })` | Host owns acquisition and credentials; registries are instance-local | [#247](https://github.com/Latias94/merman/pull/247), Web/Node source and contract tests; Added |
| Text geometry | Font/content/whitespace mismatches -> measurement follows emitted text and typography across affected families | Review SVG and export snapshots | #247, [#250](https://github.com/Latias94/merman/pull/250), terminal-consumer and browser typography regressions; Fixed |
| HTML and SVG text | Streaming repair and inconsistent entity stages -> bounded HTML5 recovery and namespace-aware serialization | Class notes remain valid XML; literal text and Kanban breaks display consistently | #247/#250, parser snapshots and text-stage comparisons; Fixed |
| Native formulas | Successful but blank or mispainted output -> visible glyphs/rules with corrected bounds and Sequence placement | Nonempty caller-scoped CSS with native formulas is rejected; omit CSS or use browser parity SVG | #245/#247/#250, CLI formula-presence and independent paint controls; Fixed |
| Swimlane wrapping | Measured lines clipped by nowrap fallback -> complete wrapped native labels | Review affected outputs | #245, [issue #242](https://github.com/Latias94/merman/issues/242); Fixed |
| Family configuration | Ignored Gantt/Mindmap/Sequence settings -> configured intervals, root size, numbering, actor visibility, note alignment and final Mindmap layout | Existing configuration takes effect | [#252](https://github.com/Latias94/merman/pull/252), focused owner regressions and pinned Mermaid source; Fixed |
| Init directives | Legacy single/multiple admission differs -> pinned 12.1 detector/filter/merge order | Prefer explicit namespaces or frontmatter | #252, `init_config_regression`; Fixed |
| Parser cancellation | Lenient errors can suppress custom-parser cancellation -> Parse-phase terminal cancellation survives callbacks | Cancelled work no longer becomes an error diagram | #252, `cancellation_regression`; Fixed |
| Custom measurement | Published operations 0-21 -> literal HTML line height at operation 22 | Exhaustive Rust matches currently need a new arm; blocks compatible patch release | #250, descriptor, generated wrappers, cross-crate probe; Breaking changes pending repair |
| Dependency security | Older selected DOMPurify/Tokio/JavaScript graphs -> reviewed upgrades and legal projections | Rebuild from updated locks; retained exceptions remain governed | #245/#247, dependency audit and owner locks; Security |
| Build and validation maintenance | Old platform locks/tools, Windows Node download handling, memory provenance and fuzz owner issues -> current owner-compatible workflow inputs | Supports package construction and reliable checks; no runtime speed claim | #245/#247/#250, internal-only or folded into delivery/security outcomes |

The complete changed-path inventory contains 2,229 paths. Generated upstream SVG recaptures, parser/DOM signatures, research captures, fuzz seeds, legal mirrors, and tests substantiate the outcomes above; they are not separate product features. Documentation-only 0.9.2 publication commits remain historical receipts. Independent Typst, grammar, Roughr, and VS Code versions are not advanced by this workspace preparation.

## Additional main backports selected during review

The preparation branch adds three bounded repairs beyond the reviewed upstream maintenance target. Their release notes are included in the same pending 0.9.3 section; no new Mermaid baseline or development API is imported.

| Source | Maintenance gap and selected repair | Disposition |
| --- | --- | --- |
| #241, `0704d37c5`, `c8aa6ce42`, final boundary correction `398c61033` | Flowchart still splits legal percent-containing identifiers as comments and lacks final direction/header rules; adapt the final lexer behavior and source-local regressions | Fixed; final-state adaptation rather than sequential cherry-picks |
| #251, `612558b16` | Frontmatter counts brackets in YAML comments/block scalars as nested collections; remove that heuristic precheck while retaining the real YAML parser depth limit, input/materialization budgets, and directive protection | Fixed; preserve regression cases for actual collection depth |
| #241, `64247d7f7` | SDK header macro validation rejects CRLF; use the main regex's optional carriage return and verify matching/mismatched ABI versions for both line endings | Folded into initial SDK delivery; deterministic LF fixtures keep the cross-platform test setup explicit |

The development comparison also inspected #239/#240 publication synchronization, #241 semantic capabilities, #244/#245 SDKs, #246/#247 rendering, #249/#250 text stages, and #251/#252 configuration/cancellation. The maintenance counterparts already contain the applicable font geometry, formula paint, JavaScript icons, HTML5, CSS terminal escapes, Swift projection, and source-order Sequence measurement repairs. Canonical parser accessibility sanitization is present through the maintenance common-db path; the new semantic API is unnecessary for that behavior.

Keep Mermaid 12.2, compiled themes, semantic capability APIs, feature-default changes, new font diagnostics, and metadata/ownership refactors on main. The WASI JSON mirror removal (`a7d769b74`) changes initial archive contents and maintenance responsibility, but does not establish a runtime ABI defect; keep the maintenance guide/catalog/mirror contract together for this release. The graphlib SCC index optimization in `237266bf2` preserves sorting semantics and remains optional performance work.

## CI evidence and its limits

| Integration | Verified head and result | Scope |
| --- | --- | --- |
| #245 SDK | `41ea20969f273cfa01b87fc88103be8f477e351c`, 59 success / 4 skipped; [CI](https://github.com/Latias94/merman/actions/runs/40155781939) | Ordinary PR owners; no complete SDK producer run |
| #247 rendering/icons | `613e6a0b601c28b30fcb642c10b2ae552cb7b67d`, 59 success / 4 skipped; [CI](https://github.com/Latias94/merman/actions/runs/40643570683) | Ordinary PR owners at that source |
| #250 text stages | `774780864448ff6666791955c71c5439e36bebea`, 59 success / 4 skipped; [CI](https://github.com/Latias94/merman/actions/runs/40795860177) | Includes Web and Swift 5.9 owners; no final-release SDK producer |
| #252 config/cancellation | `c942af590966844ae4d7f3df95b56fbf4da1a408`, 15 success / 24 skipped; [CI](https://github.com/Latias94/merman/actions/runs/40809711248) | Three-platform build/test and pr-gate pass; Web/native/package owners are skipped by change selection |

The #252 head and reviewed merge commit share tree `e084b8706d6e5af38c919cbb2bfddf4c5de5008f`, so that run has the same source tree. Its skipped jobs are not passes and its result does not qualify the future version-projected commit. Current C ABI evidence includes the old-prefix consumer and memory-font append checks; WASI native unit tests do not establish actual guest execution in both hosts.

## SDK publication boundary

The normal workspace tag publishes CLI/LSP archives and starts crates.io publication. It does not attach SDK archives. `release-preflight.yml` calls the SDK producer without publishing; `release-sdk-publish.yml` later attaches already-qualified artifacts to an existing GitHub Release.

Keep the producer run's `head_sha`, requested `source_ref`, and eventual tag commit equal. A preflight dispatched from an unrelated workflow ref cannot be used merely because its checkout builds the requested source: the SDK publisher checks the run head against the tag. Freeze the workflow ref at the selected source until the matching run is established.

Require all 18 archives and 18 SHA-256 sidecars, their source/version/profile/catalog identities, and final artifact-specific license reports. The publisher attests and uploads those verified bytes without rebuilding or overwriting. Only after the target Release actually contains those 36 matching assets should release notes claim SDK downloads are available.

The publisher currently fails closed on pre-existing asset names and has no automatic partial-upload recovery. If upload is interrupted, inspect existing assets against the same producer bytes before deciding recovery; do not add `--clobber`. Native final-package smoke concentrates on SVG/PNG despite the native catalog's wider output list; broader codec smoke is useful follow-up, not evidence already supplied by the archive matrix.

## Attribution and local preparation

Verified GitHub metadata identifies `@Latias94` as the author of all four maintenance PRs; the range has no external implementation author or co-author trailer. Issue #242 is reported by `@nerzhulart`; issue #243 is requested by `@mrueg`. The root pending Contributors entry credits each account once. Package changelogs are curated projections of the same outcomes and retain the capabilities and contracts of their own artifacts.

Local evidence is retained under the primary checkout's ignored `target/release-review-20261029/`. The version owner ran with the manifest-pinned npm 12.2.1 through an isolated npx process, leaving the global npm installation unchanged. Its first offline attempt encountered uncached locked packages and left the caller clean; after fetching the exact locked workspace, fuzz, and Node graphs, the same projector succeeded. No unrelated dependency update was performed. The old local `v0.7.0-alpha.1` tag disagrees with the remote tag; it was preserved, and fresh branch refs were fetched without overwriting tags. That historical mismatch does not affect the verified 0.9.2 base.

| Local check | Result and boundary |
| --- | --- |
| Version projection and six changelogs | 0.9.3 preparation checks pass; historical sections and local documentation links are preserved; all entries remain undated for preflight |
| CLI generated assets | Owner write/check succeeds, including both temporal environments and source-package membership; all 15 manual pages carry 0.9.3 |
| Playground catalog | Owner generation and verification succeed at the selected package version |
| Release and SDK Python owners | 140 tests pass across changelog, projection, surface, SDK archive/qualification, preflight-evidence, and workflow-security modules; the initial Windows CRLF failure is repaired |
| Core regressions | The 1,853-test selection completes across 11 binaries with 1,851 passes and two stale YAML diagnostic assertions; after aligning those assertions with main, all four selected real-depth/comment/scalar controls pass. The existing Unicode-header assertion was also aligned with main's required separator and passes in the complete selection. Production guards remain in place. |
| Core lint | `cargo clippy --locked -p merman-core --all-features --all-targets -- -D warnings` passes |
| Formatting and repository checks | `cargo fmt -p merman-core -- --check`, diff whitespace, 160-unit indexed path budget, six-surface contract, and 406 legal projections pass |
| Public enum probe | The unchanged 0.9.2 consumer succeeds against its published enum and fails against the candidate enum with E0004; this remains an intentional failing release-admission result |

Local Cargo work runs sequentially with two build jobs and reuses the primary target directory. This review does not claim a fresh whole-release SVG/root comparison, cargo-deny/audit, final package build/load matrix, or SDK producer run for the prepared source. Those remain final-source preflight gates. A complete post-assertion-change rerun of all 1,853 tests is not claimed; the four focused reruns cover the changed test expectations.

No tag, registry mutation, publishing workflow, or GitHub Release edit is part of this review. After resolving source compatibility, finish and date all release projections, commit the candidate, run immutable preflight from that same commit, and follow the [release operator guide](RELEASING.md) for the explicitly selected publish surfaces.
