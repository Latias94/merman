# v0.9.2 maintenance release audit

## Follow-up commit and changelog audit

Reviewed through 2026-10-22 against development `3119c614bf8c43d49cfe17ded0a53ec2e7fa983c` and the maintenance delta from immutable `v0.9.1` (`f6202f702dbde0a75049b1a62b3124feae709b3d`). Merged [#235](https://github.com/Latias94/merman/pull/235), source `ae3299f9a9db517e91c808e63a67cb8d30a1ce37`, contains the initial maintenance repairs. Follow-up [#237](https://github.com/Latias94/merman/pull/237) adds the independently applicable fixes identified below. Its base is `release/0.9.x`; the published tag, channel receipts, and final source are recorded in [PUBLISH_ORDER.md](PUBLISH_ORDER.md). The published root and SDK 0.9.2 changelogs record the release date 2026-10-22.

The earlier review missed independent parser, CSS, Python packaging, and release-gate fixes. It is superseded by this audit. The selected repairs still fit a 0.9.2 patch: Presentation, Options JSON schema 2, Mermaid 12.1.0, and public API signatures remain unchanged. Main's compiled theme API, schema 3, and Mermaid 12.2.0 are not backported.

The maintenance release comparison is `v0.9.1..#237`. Main development is inspected through first-parent integrations from `v0.9.0`, with fixes already published in 0.9.1 excluded from new outcomes. The 0.9.1 maintenance tag is not an ancestor of main; a raw missing-commit list would confuse adapted equivalents and old branch history with missing behavior. Decisions below compare final source and owning tests, not commit dates or subjects alone.

### Selected outcome ledger

| Surface and audience | Published baseline -> candidate | Evidence and disposition | Changelog home |
| --- | --- | --- | --- |
| Native font selection | Implicit generic and missing-glyph fallback can select symbolic faces -> symbolic faces excluded, explicit named fonts retained | `208cbc5f9`, `741a2e231`; #235, issue #228; included | Root 0.9.2 Fixed; custom-native SDK projections; Node omitted |
| C4 rendering | Filled boundaries can cover descendants and host relationship paints are incomplete -> background-first layering and actual host paint ownership | `282497485`, `4b4927959`, `31a8add27`; #235, issue #229; folded into one outcome | Root and coupled SDK 0.9.2 Fixed |
| Wardley rendering | Host anchor and structural colors incomplete -> explicit/evolved colors preserved while host fallbacks reach terminals | `3b38d4178`; #235, issue #230; included | Root and coupled SDK 0.9.2 Fixed |
| ZenUML rendering | Host colors/typography incomplete -> matching measurement/paint typography and explicit namespace precedence | `3b38d4178`; #235, issue #231; included | Root and coupled SDK 0.9.2 Fixed |
| Authored Mermaid palettes | Host-derived colors leak into selected themes -> full explicit site/source palette retained, independent fonts available | `0576442c2`; #235, issue #232; included; main intentionally has a different source-only palette policy | Root and coupled SDK 0.9.2 Fixed |
| Look selectors | Arbitrary look strings can reach selectors -> supported-look whitelist and normal fallback | Main `3fe754e8b` -> `9f007b96c`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Block source CSS | Rejected declarations can be restored into SVG -> rejected declarations dropped, valid siblings retained | Main `070e9889b` -> `1fc89439c`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Requirement source CSS | Shared validation bypassed and quoted priorities truncated -> safe parsing with declaration order and quoted values preserved | Main `916d3c529` -> `c6939dba5`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Flowchart style declarations | Repeated `classDef` overwrites the prior declaration -> source-ordered accumulation | Main #227 `ef5fa90dd`; maintenance `semantic.rs` retained the overwrite and receives the same fix/tests in #237 | Root 0.9.2 Fixed and SDK parser item |
| Flowchart error recovery | Unsupported multibyte characters advance one byte -> whole-character diagnostic spans and safe continuation | Main #178 `a5e3cd2d6`; only the remaining lexer/recovery delta is backported in #237; Unicode identifier ranges are already present in the legacy parser | Root 0.9.2 Fixed and SDK parser item |
| Pie/Info strict and lenient parsing | Invalid known-type headers can succeed as error-shaped JSON -> normal parse failure, recovered editor facts, and lenient error diagram | Main #227 `ef5fa90dd`, characterization `d5a4e8643`; maintenance uses the same family parsers; adapted inline characterization matrix in #237 | Root 0.9.2 Fixed and SDK parser item |
| XYChart empty input | Semantic/render/editor paths construct different empty models -> one configured default model | Main #227 `ef5fa90dd`; same state constructor exists on maintenance; included in #237 | Root 0.9.2 parser outcome and SDK parser item |
| SVG CSS cleanup | EOF-terminated strings/comments/URLs can survive one cleanup pass -> reject incomplete tokens and retain safe neighboring CSS | Main #227 `ef5fa90dd`; shared sanitizer exists on maintenance; same token and idempotence controls included in #237 | Root 0.9.2 Fixed and SDK CSS item |
| Python wheel construction | Hardcoded repository `target` ignores Cargo configuration -> locate both cdylib and metadata under Cargo's reported target directory | Main #178 `c7f661394`; reproduced by source/path contract and included with owner tests in #237 | Root and Python 0.9.2 Fixed; other SDKs omitted |
| Workspace crates publisher | Preflight runs independently versioned Typst wrapper tests against a new workspace plugin -> current-plugin examples and separate wrapper owner | Main #210; maintenance still lacked `--examples-only`; workflow and its contract test included in #237 | Root 0.9.2 Fixed; SDK runtime projections omitted |
| Tree-sitter edit fuzzing | Fresh/incremental recovery ranges are incorrectly required to match -> per-tree geometry plus inverse-edit restoration, following Tree-sitter 0.27.1 | Main #227; identical legacy fuzz target receives the standalone repair, regression tests and exact LF/binary seeds in #237; no parser API/grammar import | Root 0.9.2 developer-workflow item; SDK runtime projections omitted |
| Certificate verification dependencies | Distrusted emSign paths admitted by affected locked versions -> `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10` | Main `d5097b8d0` / maintenance `7d13eff7d`; already included by #235; RustSec links verified against the advisory database | Root 0.9.2 Security; SDKs without this TLS dependency closure omitted |
| Compatible dependency refresh | Re-resolve the three release-owned Cargo locks against the current index | #237 updates the workspace, fuzz and standalone Node locks; fixed math/Tree-sitter/NAPI versions and the shared export type boundary remain pinned | Root 0.9.2 Changed; generated legal reports and exception paths refreshed |

Release-version, legal, and evidence commits in #235 (`4be24ad63`, `8bd23504a`, `748cf9662`, `8f1d9e73d`, `7d13eff7d`, `ca7f8cc04`, `20fe1abaf`, `8d27e9e4f`) are internal preparation folded into their owning outcomes. Capture/documentation commits `f46c42500` and `dedf759dc`, binding regression `fdb0c9636`, and Gantt controls `cc7491584` provide evidence rather than separate product fixes. Gantt's legacy state colors already passed; do not announce a new maintenance Gantt repair. Typst smoke runtime-version repair `27d4bfa6b` supports the current-plugin gate outcome without changing the independent wrapper version.

### Other inspected main integrations

| Follow-up candidate | Decision | Evidence |
| --- | --- | --- |
| Main palette ownership follow-ups `016bbe60d`, `83eb9f98b`, and ER title fallback `3245f102f` | Do not cherry-pick | They resolve compiled-theme paint ownership and terminal receipts. The maintenance Presentation implementation retains its own source/site palette policy and does not contain those typed plans. |
| Playground initialization repair `8250491a0` | Do not cherry-pick | It separates the new workbench's initialization from authored source for compiled host presets. Porting it wholesale would change the maintenance appearance contract. |
| Performance-only PRs #212, #222, #225, #226, and #233 | Exclude from this patch | Their main targets include the unshipped theme architecture; no additional maintenance correctness defect is established by these optimization deltas. |
| #195 CLI distribution and CI restructuring | Exclude features and CI relocation | New distribution channels and broad job placement changes are not maintenance runtime repairs; the existing 0.9.x publisher owners remain authoritative. |
| #196 Mermaid 12.2.0 alignment | Defer | Changing parser/render baselines, fixtures and companion behavior is a development release, not this patch. |
| #178 theme migration | Select independent repairs only | Whitelist/CSS, remaining UTF-8 recovery, and Python target-path fixes are selected explicitly above. New theme APIs, schemas, presets, proof receipts and broad resource/DOM rewrites stay on main. |
| #203 HTML5 fragment parser migration | Defer | This replaces the HTML parser/dependency and malformed-label behavior across render paths. Existing resvg transform/paint and root-label-ID fixes are already in 0.9.1; no new HTML parser is needed for the selected maintenance repairs. |
| #204 optimization and native-font recovery | Already present or development-only | Native session and ER measurement equivalents are in 0.9.1; subsequent compiled-theme ownership and performance work does not belong to this patch. |
| #206 lexer progress and #209 font recovery | Already published in 0.9.1 | `17ab67c5c`, `101c01111`, `60bebfa20`; do not announce these as new 0.9.2 fixes. |
| #208 WinGet installed smoke | Independent channel CI | Does not change the selected workspace runtime; channel submissions remain separately owned. |
| #211 version/provenance and #213 post-release contracts | Select applicable equivalents | 0.9.2 projection and Typst runtime-version checks already exist; #210 publisher selection is additionally backported. Mermaid 12.2 fixture provenance and main's theme-dependent legal closure do not apply to 12.1 maintenance. |
| #223 theme workbench and #224 prepared-rendering/host APIs | Defer features | Do not import new public APIs or their test framework. #227's runtime fixes are ported to the existing maintenance parsers and inline characterization matrix instead. |
| #227 shared parse-fuzz framework | Keep the maintenance harness | Production fixes, source-local regression tests and the independent Tree-sitter oracle are selected; the new main-only shared projection framework/API adapters are not prerequisites. |
| Sequence half-arrows `91ac6c32c` and UTF-8 probes `cbac63276` | Already superseded | The legacy actor scanner already calls safe `signal_type_at`, and message rendering uses shared source/target marker semantics; no missing marker repair is imported. |
| ELK endpoint projection removal `783694dd2` | Already superseded | The final `align_elk_endpoint_adapters_to_route` implementation is shared by both branches. Their current file delta concerns typed edge-key lookup, not the historical unsafe projection. |
| Treemap extreme-size fitting, PDF-only `svgtypes`, and unsupported Treemap format rejection | No additional maintenance port | Legacy bounded fitting does not have the typed extreme-size trigger; existing PDF-only compilation succeeds without that dependency; format-error policy is a separate compatibility change. |
| DOM identity/scoped-CSS and standalone-resource refactor groups | Defer associated rewrites | These change prepared text namespaces, duplicate-edge occurrence keys, scoped CSS policies, and resource phase ownership. Existing root-scoped HTML-label repairs remain present; importing the development ownership chain is outside this patch. |

### References, credit, and formatting

GitHub metadata confirms that the inspected implementation PRs are authored by `@Latias94`. The entire `v0.9.0..main` author inventory contains only that maintainer and Dependabot, with no `Co-authored-by` trailers. The requested author credit names `@Latias94` once in each pending root release section; bots are omitted. Issues #197-200 and #228-232 are reported by `@nerzhulart`, credited once in the pending Contributors sections for the font and host-theme reports. The 0.9.1 issue #207 credit is corrected to verified reporter `@000Janela000` beside #209. Existing older contributor credits are preserved.

Each new 0.9.2 fix links either merged #235 or its original merged PR plus maintenance #237. Main's Unreleased entries link the main owner rather than claiming maintenance implementation details. Root and five coupled package changelogs use one physical line per prose paragraph/list item. Published sections receive only whitespace reflow and the verified 0.9.1 attribution correction; tables, code fences, independent package versions and prior wording remain intact.

### Validation and publication boundary

#### Dependency review on 2026-10-22

Fresh `cargo update` runs changed 40 workspace packages, 22 fuzz packages, and five standalone Node packages within their declared requirements. Selected updates include `aws-lc-rs 1.18.2` / `aws-lc-sys 0.46.0`, `rustls-native-certs 0.8.5`, `json5 1.3.3`, and the Ratex `0.1.15` family. The certificate fixes from #235 remain at `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10`; this review does not claim another newly fixed certificate advisory. The lockfile under `docs/assets/issues-228-232/` is frozen reproduction evidence and is not a release dependency authority.

`cargo-deny 0.19.9` fetched RustSec database revision `4b587f00996ebe5c05a73eaef105ef241e876414` (2026-10-21). The locked workspace's ordinary `advisories bans licenses sources` policy passed. Fuzz and Node each passed the additional all-features advisory check. Existing governed informational advisories for `ttf-parser 0.25.1` (RUSTSEC-2026-0192) and `rustybuzz 0.20.1` (RUSTSEC-2026-0206) remain; the current export dependency family has no compatible replacement. Their exact Ratex dependency paths are updated for the new lock, without changing their review deadline or broadening the ignore policy.

The CI-pinned `cargo-audit 0.22.2` independently audits all four tracked locks against the same freshly fetched database, including the unchanged reproduction lock. Each exits successfully with zero vulnerability entries, no ignored advisories, and no severity or platform filters. The workspace lock reports the three unmaintained advisories described here; fuzz and the reproduction lock report the two font-stack maintenance advisories; Node reports no advisories. Raw local JSON receipts are retained under the maintenance worktree's ignored `target/security-review-20261022/` directory. These results preserve informational warnings rather than claiming an advisory-free dependency graph.

The supplemental workspace `--all-features` advisory check still reports RUSTSEC-2026-0374: unmaintained `basic-toml 0.1.10`, reached through `merman-uniffi/binding-generation -> uniffi_bindgen 0.32.2 -> askama 0.16.1`. The fresh compatible resolution offers no fixed release; this is an informational maintenance advisory in the optional binding-generation toolchain, not a new runtime vulnerability. It is recorded here rather than globally ignored. The same supplemental license check rejects the existing UniFFI generator's MPL-2.0 packages; the root policy is scoped to the ordinary workspace graph, and generated license reports separately include the all-features inventory. Applying that workspace license policy to the standalone fuzz graph also rejects its pre-existing libFuzzer NCSA/private-package entries and unused allowances. These supplemental results do not constitute clean all-features/all-manifest policy receipts.

#### Follow-up implementation identity

Maintenance preparation head `85d5cfef90a341644fecb438e684afc2c5dec658` contains six separate commits: parser/CSS repairs `2daa82708`, the edit-fuzz oracle `f617cd170`, Python/Typst owner fixes `8568547f1`, dependency/legal refresh `2fcbeb9d3`, release notes `07b19b741`, and the LALRPOP artifact refresh `85d5cfef9`. The RaTeX 0.1.15 source identity is verified from the published crates' VCS metadata. All 22 bundled font/resource files match 0.1.14 byte-for-byte; the new source's MIT license and third-party notices also match the retained files. The source contract records the new commit and Cargo checksums rather than asserting that the old source identity describes the new packages.

The first #237 CI run at `07b19b741` caught stale generated parser artifacts after LALRPOP advanced from 0.23.1 to 0.23.2. The existing `gen-lalrpop-parsers` command regenerates all five files; their grammar hashes and parse tables remain unchanged. Generated changes add the version/features header, lint allowances, and cloning of the existing `usize` location values. `verify-generated` passes after materializing the locked DOMPurify 3.4.16 reference source at `b9b9d80f7e401771c2ccaef5f45def7eec8f27d7`. No source grammar, Mermaid baseline, or public API changes accompany the refresh.

At `85d5cfef9`, the complete `verify-feature-matrix` passes its profile/structure checks, isolated consumers, and 42 build probes. The actual Node N-API transport configuration also passes `cargo check`; the crate intentionally rejects `--all-features`, which selects both mutually exclusive transports and attempts a WASM transport on a native target. Validation selects the real transport instead of weakening that guard.

After regeneration, `cargo nextest run --locked -p merman-core --all-features --lib` passes all 1,790 tests at `85d5cfef9`, with zero skips. This provides a fresh semantic regression receipt for the updated generated parser bodies in addition to the earlier 4,731-test dependency-refresh suite.

The exact full SVG gate also passes at `85d5cfef9`: `cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout`. It retains the existing exact browser-text-layout and parser-diagnostic residuals; parser diagnostics are not DOM parity. No golden fixtures, residual catalog, comparator, or normalization rules change. Local comparison reports are under the shared build cache's `target/compare/` directory.

After the dependency refresh at `07b19b741`, the core/render/analysis/editor regression suite passes 4,731 tests across 104 binaries, with two intentional skips, using Rust 1.95, all diagram families, both optional layout engines, and math rendering. The all-features/all-targets CLI check and the standalone fuzz library's 16 regression tests also pass with the refreshed dependencies. Local legal validation passes: 13 freshly generated Rust dependency reports, 394 synchronized release-material projections, the third-party source/license contract, both governed exception paths, and 24 generator/projection/exception tests. The focused packaging/workflow/fuzz/release Python suite runs 109 tests: 108 pass and one platform-specific test is skipped. The pinned Rust 1.95 formatter passes each of the 15 manually maintained Rust files individually; the five generated parsers use their owner generator's exact formatting. Full `cargo fmt --all -- --check` hits Windows command-line error 206, so the Linux CI owner remains required for that command. `actionlint`, changelog/version/surface checks, whitespace checks, and the indexed path budget pass; `zizmor` is unavailable locally and remains a CI-owned check.

PR #235 completed its selected checks, including `pr-gate`. Its merged implementation tree equals reviewed head `8d27e9e4f8aa35b7095b45a728f5953538cde438`; those earlier receipts remain attributable to that source. #237 changes runtime code and publisher behavior as well as documentation, so it needs its own source-scoped checks and CI. The historical green #235 result is not evidence for the new head.

For the published 0.9.2 source, the immutable preflight and channel receipts are recorded in `PUBLISH_ORDER.md`; this main-branch synchronization keeps the same date across root/package changelogs. Typst, grammar, Roughr, and VS Code retain independent versions. This review does not replace the immutable release receipts or independently versioned publication records.

## Original local preparation snapshot

The remainder records the earlier pre-PR local preparation, including its original source identities and receipt paths. Its statements about unpublished branches and local-only operations describe that historical checkpoint, not the current repository state.

Status: local v0.9.2 candidate prepared and locally release-ready; not published. Immutable remote
preflight, tag creation, registry upload, and external communication remain maintainer-owned steps.

The maintenance base is immutable tag `v0.9.1` (`f6202f702dbde0a75049b1a62b3124feae709b3d`).
Implementation lives on `fix/v0.9.2-host-theme` in `.worktrees/v0.9.2-host-theme`; its final local
candidate metadata/security commit is `7d13eff7d`.
Main repairs live on `fix/host-theme-228-232-20261020`, based on `a933b02c1976a5034350d96574aea048e393c04e`.
The main lock/legal security refresh is `d5097b8d0`; the implementation head immediately before it
is `1f01c31b7`.
No push, tag creation, registry upload, or external message is part of this preparation.

## Compatibility boundary

Maintenance retains the Presentation API, binding Options JSON/FFI versions, and Mermaid 12.1.0 pin.
Main retains its independent theme architecture and Mermaid 12.2.0 baseline. No main theme-feature
migration is a maintenance prerequisite. Final SVG changes correct paint ownership, layering and
font selection; consumers with byte snapshots may need to update those snapshots.

`MermaidConfig` remains an existing opaque public Rust type with private storage. Its constructors,
methods, Clone, JSON-value PartialEq and tuple-shaped Debug output remain available. Private
metadata is excluded from JSON and Debug. The original type has no fixed-layout representation,
and FFI does not pass it by value. A direct rustc probe confirmed that the old private tuple
constructor also prevents downstream `MermaidConfig(..)` patterns. An independent API review found
no source-breaking change; default-feature and feature-bearing cargo-semver-checks passed all 223
checks for each of core, render and export. The explicit feature sets were `all-diagrams` for core,
`all-diagrams,layout-cytoscape,layout-elk,math` for render, and
`native-fonts,png,jpeg,pdf,experimental-system-font-query` for export. These checks compare the actual
0.9.2 candidate against `v0.9.1` with `--release-type patch`.

The same old-public-API consumer compiled and rendered an EditorDark Sequence SVG in three fresh,
initially lock-free resolutions: published 0.9.1 alone, local candidate 0.9.2, and published
`merman =0.9.1` with local 0.9.2 implementation siblings. Lockfile inspection confirmed the registry
facade and local sibling versions in the mixed lane. The program checks configuration mutation,
clone-on-write, equality, tuple Debug and Send/Sync/Unpin/UnwindSafe/RefUnwindSafe as well as the
legacy Presentation render path. It uses only `svg,diagram-sequence`; it does not replace the
feature-bearing API comparison or prove every downstream feature combination.

The [Cargo compatibility guide](https://doc.rust-lang.org/cargo/reference/semver.html#struct-tuple-normal-with-private)
classifies converting a nonempty tuple struct with entirely private fields to a named-field struct
as compatible. Added hidden cross-crate helpers remain additive public items; their hidden
documentation does not make them private. Engine Debug includes additional internal diagnostic
fields, so its complete debug string is not byte-identical to the tag.

## Issue repair mapping

| Behavior | Main | Maintenance equivalent |
| --- | --- | --- |
| Authored palette and derived ER/Quadrant colors (#232) | `4c70b8909`, `633b45ff0` | `0576442c2`: separate raw initialize/source layers from Presentation defaults; retain typography, actual terminal fallback chains and detector mutation semantics. Includes Timeline scale regression. |
| Symbolic implicit font selection (#228) | `9a5bf169a` | `208cbc5f9`: same admitted-byte classification across closed, bounded, legacy system and optional platform discovery; explicit named fonts remain selectable. |
| Filled C4 boundaries hide descendants (#229) | `0903b2c94`: family painter background before descendants, foreground after | `282497485`: adapt the iterative old painter; transparent baseline preserved. |
| C4 relationships, labels and arrowheads (#229) | `0903b2c94`, `1f01c31b7`: actual typed terminal roles and receipts | `282497485`, `4b4927959`, `31a8add27`: existing native C4 fields and HostTheme projection; explicit site/source owners win; exact materialized host line provenance retains native shared markers without an actual host paint request. |
| Gantt state and label paints (#230) | `0903b2c94`: state-qualified presets and terminal foregrounds | `cc7491584`: existing old surface/muted/alt states already pass new regression coverage; no typed-theme feature port. |
| Wardley anchors (#230) | `0903b2c94`: text/node-label adapter with independent annotation and arrow pairs | `3b38d4178`: legacy final palette fallback, explicit anchor/axis values and evolution retained. |
| ZenUML host colors and typography (#231) | `0903b2c94`: unified family preparation and honest terminal receipts | `3b38d4178`: namespace-first legacy adapter; matching measurement/paint fonts; plain companion baseline retained. |

Maintenance binding regression and user documentation are in `fdb0c9636`. The Cargo-owned fuzz
lock repair is `8bd23504a`; the release-owned 0.9.2 version projection is `748cf9662`. Independent
review confirmed that this projection changes only coupled versions, preserving dependency
identities, independent package versions, ABI and schema. Main binding integration is `a8adfde7e`.
The pre-security legal projection was `8f1d9e73d`; final legal material is included in maintenance
candidate commit `7d13eff7d` and main commit `d5097b8d0`, with the updated lock hash in every
generator-owned report.

After that projection, both branches also lock `rustls-webpki` to `0.103.16` and
`webpki-root-certs` to `1.0.10`, resolving RUSTSEC-2026-0382 and RUSTSEC-2026-0383 without changing
the direct dependency contract. The regenerated candidate reports contain the exact new lock hash;
`cargo deny check advisories bans licenses sources` passes on both branches, and full workspace
`cargo check --locked --workspace --all-features` passes on both branches.

## Docgen assessment

The reported [docgen commit](https://github.com/josselin-pujo/docgen/commit/608646f8dd7708f277986b061bb9521453d2e743)
uses 0.8.0-alpha.7. Its 800,000/1,000,000 interactive/trusted-native layout ceilings were already
recalibrated to 14,100,000/15,000,000 on both selected branches before this work. The exact failing
graph was not supplied, so no claim is made that it fits either current limit. This candidate does
not increase budgets or select an unbounded default. The facade already re-exports the resource
policy; an additional direct `merman-render` dependency is unnecessary. See the
[source-backed diagnosis](../research/2026-10-20-docgen-and-host-theme-defect-diagnosis.md) for the
version distinction and measured-policy boundary.

## Additional fix decisions

| Main commit/group | Decision | Release-source evidence |
| --- | --- | --- |
| `3fe754e8b` look selectors | Include adapted fix | Old DiagramLook accepts arbitrary nonempty selector values. New regression failed before whitelist. |
| `070e9889b` Block CSS | Include adapted fix | Old writer restores declarations rejected by its safe parser. Malicious URL regression failed before removing fallback. |
| `916d3c529` Requirement CSS | Include adapted fix | Old writer bypasses shared validation and truncates quoted `!important`; use existing safe parser and normalize only terminal priority suffix. Unsafe and valid sibling controls failed before repair. |
| `ec421dc91` font budget | Already present | Tag uses 16,384 default probes, matching CLI help and 500-table ER regression. Do not repeat the change. |
| Treemap fitting series ending `a675fdc80` | Exclude: triggering path absent | Old fitting starts at fixed 38/16, minimum 8/4. Width and height share at most 30/12 decrements; CSS font-size only changes an output suffix. Main's typed extreme-size precondition does not exist. |
| `de977a8b4` PDF dependency | Exclude: dependency not required by this old path | Old sole svgtypes use and its root-metadata caller are gated to PNG/JPEG. Locked PDF-only cargo check passes. |
| `c7f661394` Python wheel target path | Defer to Python owner | Independently packaged wheel is outside the selected workspace maintenance publication surface; requires installed-wheel validation when selected. |
| `f88e38a99` lexer progress | Already present | Equivalent release fix `17ab67c5c` and fuzz seeds are present. |
| `cbac63276` UTF-8 signal probe | Already superseded | Old released scanner delegates to safe `signal_type_at` and `input.get(position..)`. Patch-ID difference alone does not imply missing behavior. |
| HTML label transforms/IDs, rustls and native font sessions | Already present | Release backports `e4cea5f9b`, `1325e3bc3`, `80abdcb68`, `101c01111` and limit option `60bebfa20`. |
| `44fccd368` Zen stylesheet XML | Fold into issue repair | Original old CSS was literal; the new namespace adapter needs exactly one raw XML serialization boundary, without Mermaid entity decoding. |
| RustSec `RUSTSEC-2026-0382` / `RUSTSEC-2026-0383` | Include lock-only security fix | Both workspace locks now use `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10`; cargo-deny advisories pass and regenerated legal reports match the new locks. |
| Sequence half-arrows, Unicode identifiers, endpoint projection, CSS scoping, HTML5 parser group | Defer independent release-local reproduction | These span parser/geometry/CSS contracts and moving main interfaces. No reproduced maintenance issue currently justifies importing the associated feature/refactor chain. |
| Unsupported Treemap format rejection | Defer | Observable error-policy change requires its own maintenance compatibility decision; unrelated to reported issue repairs. |
| New diagrams/looks, theme contracts, schema increments and pure performance work | Exclude | Outside this patch release's repair scope. |

## Evidence and remaining gates

Local ignored diagnostics are under `target/diagnostics/issues-228-232-20261020/` in the main checkout.
Earlier red logs are historical observations, not the current result. Main font tests passed 120;
maintenance font tests passed 73 with nine platform-only skips; maintenance core/render/Presentation
passed 3,443 after the final C4 provenance change. Maintenance binding owner features passed 220 tests
again on that source.
The main broad owner run passed 7,622 of 7,627 tests; the five failures were repaired and covered by
a passing 142-test final delta. A later authored-palette plus empty-theme Wardley regression was
reproduced separately; its final Wardley owner passed all 20 tests after repair. Linux Fontconfig integration is
executable but has not run on this Windows host.

Both branches produced the original six inputs through the public binding SVG/PNG/PDF functions
with no preset, editor-light and editor-dark: 54 artifacts per branch. The final current-head
receipts are `main-native-final1f01.log` and `maintenance-native-final092.log`; each has 18 PNG/PDF/SVG
triples, an inspection manifest, and a contact sheet. Each PNG was decoded and checked for nonempty
painted content, and each PDF parsed as one page. Earlier contact sheets and ZenUML dark PDF
rasterizations were inspected. Maintenance ZenUML's original quoted-font stylesheet bug was
reproduced in native output and its corrected foreground/background pixels were rechecked.
Transparent dark previews were composited onto the host canvas only for inspection; this does not
change the legacy root-background contract. Deterministic text measurement versus installed-font
paint can still produce font-dependent label clipping; these artifacts do not prove pixel parity.
Representative artifact dependency closure checks passed on both branches, including their
declared WASM and platform dependency graphs; these are metadata checks, not cross compilations.

Main full SVG corpus gates passed structure, parity and parity-root with the existing exact
browser-text receipts. The first structure run found four real C4 marker regressions; both branches
received failing no-host/empty-theme controls before repair. Final C4 SVG/geometry owners passed
58 tests on main and 24 on maintenance. No comparator, baseline SVG or residual catalog was changed.
Main feature-matrix validation passed its isolated public consumers and all 45 build cases.
Warnings-denied all-target Clippy passed both final implementation heads, including the final C4
changes (`main-clippy-final6.log` and `maintenance-clippy-final092.log`).

Maintenance's final full SVG structure gate is `maintenance-svg-structure-final092.log`; its parity
and parity-root run is `maintenance-svg-parity-final092.log`. Both include the six math fixtures with
the installed pinned Node KaTeX backend and report all blocking checks passed. Main's structure,
parity, and parity-root receipts remain `main-svg-structure-final2.log` and
`main-svg-parity-final.log`; main deliberately defers its existing math fixtures, so that bounded
main residual is recorded rather than overclaimed.

Maintenance's final feature matrix is `maintenance-feature-matrix-final092.log` (42/42 build cases
and 16 isolated consumers). Its release-owned asset receipts are
`maintenance-cli-assets-write092.log`, `maintenance-playground-catalog-write092.log`, and
`maintenance-playground-catalog-check092.log`; all generated assets are now versioned 0.9.2 and
their source checks pass. The candidate's 13 Rust legal reports and 394 projections pass after the
security lock update; main's corresponding checks cover 13 reports and 424 projections.

The final source heads have been compiled with all workspace features after the lock update
(`main-cargo-check-security-final1f01.log` and `maintenance-cargo-check-security-final092.log`). The
focused issue regressions, public binding contracts, semver checks, old-consumer lanes, formatting,
warnings-denied Clippy, native artifacts, full SVG gates, legal projections, feature matrices, and
independent review are complete. The only intentionally unexecuted evidence is Linux Fontconfig
process integration, because this run is on Windows; its executable owner test and deterministic
font classification coverage remain present for the Linux CI owner. No comparator, baseline SVG,
or residual catalog was changed. Publication remains outside this local preparation.
