# v0.9.2 maintenance candidate

## Review after the maintenance-branch rename

Reviewed on 2026-10-21 against development `3119c614bf8c43d49cfe17ded0a53ec2e7fa983c` and maintenance implementation `ae3299f9a9db517e91c808e63a67cb8d30a1ce37`. PR [#234](https://github.com/Latias94/merman/pull/234) is merged into `main`; PR [#235](https://github.com/Latias94/merman/pull/235) is merged into the maintenance line. The latter branch is now `release/0.9.x`; its rename preserves that commit and the immutable `v0.9.1` tag.

The selected repairs are sufficient for a compatible 0.9.2 patch. No additional development feature or theme-refactor commit is needed to justify the release. The root and coupled package versions already read 0.9.2. The current changelog describes the maintenance implementation's site/source palette contract, which intentionally differs from the compiled-theme implementation on `main`.

| Follow-up candidate | Decision | Evidence |
| --- | --- | --- |
| Look selector, Block CSS, and Requirement CSS fixes | Already included | Maintenance `9f007b96c`, `1fc89439c`, and `c6939dba5`; the selected delta and its regression tests are in #235. |
| Final implicit glyph-fallback repair `0157974b1` | Already included | Maintenance equivalent `741a2e231`, in addition to initial font-selection repair `208cbc5f9`. |
| Certificate dependency refresh `d5097b8d0` | Already included | Maintenance `Cargo.lock` resolves `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10`; the missing user-facing security note is now included. No dependency update is required. |
| Main palette ownership follow-ups `016bbe60d`, `83eb9f98b`, and ER title fallback `3245f102f` | Do not cherry-pick | They resolve compiled-theme paint ownership and terminal receipts. The maintenance Presentation implementation retains its own source/site palette policy and does not contain those typed plans. |
| Playground initialization repair `8250491a0` | Do not cherry-pick | It separates the new workbench's initialization from authored source for compiled host presets. Porting it wholesale would change the maintenance appearance contract. |
| Performance-only PRs #212, #222, #225, #226, and #233 | Exclude from this patch | Their main targets include the unshipped theme architecture; no additional maintenance correctness defect is established by these optimization deltas. |
| Mermaid 12.2 alignment, HTML5 parser migration, prepared-rendering/host APIs, and workbench features | Defer to development releases | These change baselines, behavior, or interfaces beyond a repair-only maintenance patch. |
| Post-release package/version repairs | Already accounted for or owner-specific | The candidate's release projection verifies at 0.9.2, and #235 contains its Typst smoke/runtime-version and generated-asset repairs. Independent channel publication remains separate. |

PR #235 completed its selected checks, including `pr-gate`. Its merged implementation tree is identical to reviewed head `8d27e9e4f8aa35b7095b45a728f5953538cde438`; the earlier local evidence below remains attributable to its recorded sources. Documentation changes after this merge still require a final immutable source SHA for release preflight. PR success, matching trees, and a branch rename are not substitutes for a successful preflight bound to that final commit and 0.9.2.

The remaining release gate is to merge the maintenance documentation, confirm the intended date across the root and package changelogs, freeze the final 40-character SHA, and run full immutable preflight from the maintenance workflow revision. Do not create `v0.9.2` or publish packages before that result. Typst, grammar, Roughr, and VS Code retain independent versions. This review does not claim a 0.9.2 tag, successful immutable preflight, or registry publication.

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
