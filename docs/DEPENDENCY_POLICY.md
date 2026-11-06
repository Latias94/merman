# Dependency policy

Merman selects dependencies by the consumer contract they participate in. Compatible Cargo requirements are the default; an exact requirement needs a concrete deterministic, generated-code, ABI, or runtime-admission reason. Lockfiles freeze a build, while manifest requirements must also produce a coherent fresh consumer resolution.

## Workspace and independent packages

Stable coupled workspace crates use ordinary compatible requirements such as `0.9.0`. Prereleases use exact coupled requirements and the existing fresh-consumer admission probe. Internal path dependencies and a green workspace build do not prove registry resolution; use the release owner's consumer checks.

`roughr-merman`, Tree-sitter Mermaid, Typst, and VS Code keep independent version axes. Prepare and publish required independent Cargo packages before the workspace dependents. A patch on a pre-1.0 minor line must preserve its public API; private implementation upgrades do not justify removing types or methods accepted by already-published consumers.

## Exact and compatible requirements

| Dependency surface | Selected policy | Reason and review boundary |
| --- | --- | --- |
| `libm` | Exact 0.2.16 | Iterative layout uses reproducible math instead of host-dependent transcendental results. [The introducing change](https://github.com/Latias94/merman/commit/f5e557f85cfa03992ba367cd970605258e060ea6) ties this choice to deterministic geometry. A change requires source-backed layout and parity evidence. |
| `pulldown-cmark` | Compatible 0.13.6 minimum | Includes the [RUSTSEC-2026-0433 repair](https://rustsec.org/advisories/RUSTSEC-2026-0433.html). Merman selects explicit Markdown options without Wikilinks; keep the patched minimum for fresh source consumers and update each governed lockfile. |
| NAPI runtime / derive / build | Exact 3.16.1 / 3.7.0 / 2.6.0 group | Generated native Node binding code, build integration, and runtime must be admitted together; review package loading and platform artifacts as one group. |
| Tree-sitter CLI / Rust and Web runtime | Exact 0.27.1 group | Parser generation and the selected runtimes are admitted together. Grammar candidate 0.3.3 remains independent; generated language ABI stays 15. Native Node's separate 0.25.x runtime contract remains explicit. |
| `tree-sitter-language` | Compatible 0.1.9 requirement | `LanguageFn` is a public function-pointer adapter, not the generated language ABI. Its API and representation are unchanged from 0.1.8; upstream Tree-sitter uses the compatible requirement too. `Cargo.lock` fixes the audited build at 0.1.9 without forcing other grammars to accept an exact shared adapter version. |
| `ctrlc`, `fontique`, `granit-parser`, `serde-saphyr`, current `svgtypes` | Ordinary compatible requirements | Remove unexplained exact equality. YAML/parser crates select granit-parser 2.1.0 and serde-saphyr 2.1.1; `merman-core/editor-facts` enables `granit-parser/comments` and individual requests select capture so source-config rewrites retain comment evidence. Upgrades require their owning semantic and artifact checks. |
| `roughr-merman` legacy `svgtypes` | Retain 0.11 public compatibility line | Public path types used by already-published Merman consumers must remain compatible. The 0.12.5 arc-rounding fix preserves the legacy API and default features retained by 0.12.4. Current Merman disables legacy defaults and retains explicit randomness. |
| `resvg` / `usvg` | Retain compatible 0.47.0 line | `krilla-svg` 0.8.1 consumes usvg 0.47, and public fontdb 0.23 types cross the shared native font context. Advancing only one renderer would split font types and font/budget ownership. Upgrade the export chain together with a proven adapter or a common admitted dependency line. |

The [published krilla-svg 0.8.1 manifest](https://docs.rs/crate/krilla-svg/0.8.1/source/Cargo.toml) requires resvg/usvg 0.47 and fontdb 0.23. The current [`NativeFontContext` implementation](../crates/merman-export/src/fonts.rs) carries that database and exposes `apply_to_usvg_options`. The compatible resvg/usvg line is not an exact equality requirement. The 2026-11-06 audit observed resvg/usvg 0.49.0, but krilla-svg 0.8.1 is still the latest stable PDF adapter. Freshness alone is insufficient evidence to replace the shared font/export contract. The release audit must retain the upstream versions, manifests, public type boundary, and owner results behind this exception.

An incompatible `usvg::Tree` handoff prevents mixing the 0.49 renderer with the 0.47 PDF adapter. Keep CSS color acceleration local to the ZenUML renderer and TOML writing local to test tooling; parser-only and production read-only consumers should not inherit unused features.

The [upstream language adapter](https://github.com/tree-sitter/tree-sitter/blob/v0.27.1/crates/language/src/lib.rs) wraps the generated entry point with `#[repr(transparent)]`. The [upstream runtime manifest](https://github.com/tree-sitter/tree-sitter/blob/v0.27.1/lib/Cargo.toml) uses a compatible language requirement. Keep generator identity, generated ABI, and public adapter compatibility as separate checks.

Wasmi 2 tooling explicitly enables `validate`, `stable`, `simd`, and `auto-dispatch` while disabling defaults. `auto-dispatch` selects loop dispatch for unoptimized host builds, where native tail-call optimization is unavailable; omitting it can overflow the host stack during ordinary plugin execution. The long-loop interpreter regression and actual Typst package smoke cover this migration.

The 2026-11-06 audit selected JNI 0.22.4, the latest stable release. Its [native-method macro](https://docs.rs/crate/jni-macros/0.22.4/source/src/native_method.rs) emits `AtomicBool::fetch_update`, deprecated on the selected Rust 1.99 toolchain in favor of `try_update`. Keep the dependency's ABI checks enabled and limit the deprecation allowance to the three generated JNI method tables. Android-target Clippy still denies warnings for the transport; remove these allowances when an admitted JNI release updates the generated call.

## Reference tools and JavaScript compatibility

The 0.9.0 preparation audit selected Mermaid 12.1.0. Development selects 12.3.0 through the
[selected reference graph](../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json) and
[development migration](migrations/mermaid-12-3.md); [ADR-0094](adr/0094-mermaid-12-2-upstream-transition.md)
retains the historical 12.2 transition. Published releases retain their original baseline.
The reference host graph is frozen and independently admitted: updating a reference dependency
changes the oracle and requires the Mermaid alignment workflow. ELK follows the exact target
workspace oracle `0.12.1`; the source-backed opposing-loop regression and public worker replay
are recorded in [the engine delta](../crates/merman-elk-layered/MERMAID_12_3_DELTA.md#alpha2-patch-selection-elkjs-0121).
The 2026-11-06 audit observed ZenUML 4.13.0, which falls outside the selected plugin's `^3.47.9`
contract and is not substituted without coordinated admission.

Application TypeScript remains on the compatible 6.0.3 line. The 2026-11-06 audit observed TypeScript 7.0.2; typescript-eslint 8.72.1 still requires TypeScript `>=4.8.4 <6.1.0`, and application scripts use the old compiler API. VS Code and the browser-test workspace independently admit TypeScript 7. Retain this as a documented compatibility exception; do not treat a tool's major version as an unconditional application upgrade.

Playground selects js-yaml 4.3.2, the current `v4-legacy` release. The 2026-11-06 audit confirmed that version 5.5.0 still rejects multi-line flow mappings already accepted by the selected Mermaid configuration contract, such as `config: {\n  theme: neo-dark\n}`. Retain the existing configuration fixture and revisit the major upgrade when the source contract has a compatible parser path; a parser upgrade must not silently reject accepted diagrams.

Mermaid's released hosts also retain their KaTeX, Marked and legacy D3 dependency ranges. The reference lock admits compatible patches, including D3's rounded coincident-arc correction, without flattening distinct parent contracts. Dagre and Graphlib are embedded source baselines in `dagre-d3-es`; installing newer standalone packages does not upgrade that code. A newer major needs a source-backed port and the same parser, render and host admission as a Mermaid change.

Version observations describe the dated preparation or dependency audit, not an evergreen claim about registry latest. Store the complete Cargo/npm candidate audit and compatibility evidence in release artifacts or the release record. Revisit each exception when its blocking public API or companion dependency changes. The policy records selection reasons; it does not assert that a new `cargo deny`, package build, or publication has passed.

For a transitive security advisory, prefer a patched version admitted by the released parent's
dependency range, or a compatible released parent update. When the parent does not yet support
the patched major, retain the supported graph and document the affected path and upstream
removal condition. An advisory alone does not justify forcing an incompatible override. Keep
the unresolved audit result visible; this compatibility policy does not waive an audit gate.

The 2026-11-06 VS Code packaging-tool update removes the old `@azure/msal-node` override. The current parent admits the patched 7.x line directly, so this tooling path again follows the released parent's supported dependency range. It is not part of the bundled extension runtime.

## Node build compatibility

Host and musl builders select Node 26.13.0 and npm 12.2.2. The native GNU builder retains Node 24.13.1 on Bullseye because that container defines the selected glibc 2.31 compatibility baseline; the newer official Node images do not provide that distribution. It copies the Rust 1.99.1 toolchain from the official Bookworm image while retaining the Bullseye target linker and sysroot. The musl candidate uses Alpine 3.24. Keep each container identity and its package-tool provenance with the owner artifact evidence, and verify native loading/package smoke after a builder change. These are distinct preparation choices, not proof that the final package has passed its native owner gates.

## GitHub Actions

GitHub Actions use reviewed full version tags. Source identity remains the immutable commit and tree. Downloaded release tools, staged artifacts, and registry reconciliation retain SHA-256 verification under [Release Preflight](release/RELEASING.md#release-preflight).

## Evidence and release gates

For each exception, retain the selected and observed newer versions, affected public/generator contract, primary source or manifest evidence, owner verification, and removal condition. Keep the evidence small and use existing owner checks rather than building a second dependency resolver or compiler in release scripts.

Run version projection, dependency and legal checks, independent-crate compatibility, fresh registry consumers, artifact closure, and package smoke through their existing owners. A source edit after immutable preflight invalidates that evidence. See [release preparation](release/PUBLISH_ORDER.md#090-preparation) and [release operations](release/RELEASING.md).
