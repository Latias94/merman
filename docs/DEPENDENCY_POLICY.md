# Dependency policy

Merman selects dependencies by the consumer contract they participate in. Compatible Cargo requirements are the default; an exact requirement needs a concrete deterministic, generated-code, ABI, or runtime-admission reason. Lockfiles freeze a build, while manifest requirements must also produce a coherent fresh consumer resolution.

## Workspace and independent packages

Stable coupled workspace crates use ordinary compatible requirements such as `0.9.0`. Prereleases use exact coupled requirements and the existing fresh-consumer admission probe. Internal path dependencies and a green workspace build do not prove registry resolution; use the release owner's consumer checks.

`roughr-merman`, Tree-sitter Mermaid, Typst, and VS Code keep independent version axes. Prepare and publish required independent Cargo packages before the workspace dependents. A patch on a pre-1.0 minor line must preserve its public API; private implementation upgrades do not justify removing types or methods accepted by already-published consumers.

## Exact and compatible requirements

| Dependency surface | Selected policy | Reason and review boundary |
| --- | --- | --- |
| `libm` | Exact 0.2.16 | Iterative layout uses reproducible math instead of host-dependent transcendental results. [The introducing change](https://github.com/Latias94/merman/commit/f5e557f85cfa03992ba367cd970605258e060ea6) ties this choice to deterministic geometry. A change requires source-backed layout and parity evidence. |
| NAPI runtime / derive / build | Exact 3.14.2 / 3.6.12 / 2.6.0 group | Generated native Node binding code, build integration, and runtime must be admitted together; review package loading and platform artifacts as one group. |
| Tree-sitter CLI / Rust and Web runtime | Exact 0.27.1 group | Parser generation and the selected runtimes are admitted together. Grammar package 0.3.1 remains independent; generated language ABI stays 15. Native Node's separate 0.25.x runtime contract remains explicit. |
| `tree-sitter-language` | Compatible 0.1.9 requirement | `LanguageFn` is a public function-pointer adapter, not the generated language ABI. Its API and representation are unchanged from 0.1.8; upstream Tree-sitter uses the compatible requirement too. `Cargo.lock` fixes the audited build at 0.1.9 without forcing other grammars to accept an exact shared adapter version. |
| `ctrlc`, `fontique`, `granit-parser`, `serde-saphyr`, current `svgtypes` | Ordinary compatible requirements | Remove unexplained exact equality. YAML/parser crates select 1.3.0; upgrades still require their owning semantic and artifact checks. |
| `roughr-merman` legacy `svgtypes` | Retain 0.11 public compatibility line | Public path types used by already-published Merman consumers must remain compatible. Private rand 0.10.3 / getrandom 0.4.3 changes in candidate 0.12.4 do not remove this surface. Current Merman disables legacy defaults and retains explicit randomness. |
| `resvg` / `usvg` | Retain compatible 0.47.0 line | `krilla-svg` 0.8.1 consumes usvg 0.47, and public fontdb 0.23 types cross the shared native font context. Advancing only one renderer would split font types and font/budget ownership. Upgrade the export chain together with a proven adapter or a common admitted dependency line. |

The [published krilla-svg 0.8.1 manifest](https://docs.rs/crate/krilla-svg/0.8.1/source/Cargo.toml) requires resvg/usvg 0.47 and fontdb 0.23. The current [`NativeFontContext` implementation](../crates/merman-export/src/fonts.rs) carries that database and exposes `apply_to_usvg_options`. The compatible resvg/usvg line is not an exact equality requirement. Newer 0.48.1 was considered for preparation, but freshness alone is insufficient evidence to replace the shared font/export contract. The release audit must retain the upstream versions, manifests, public type boundary, and owner results behind this exception.

The [upstream language adapter](https://github.com/tree-sitter/tree-sitter/blob/v0.27.1/crates/language/src/lib.rs) wraps the generated entry point with `#[repr(transparent)]`. The [upstream runtime manifest](https://github.com/tree-sitter/tree-sitter/blob/v0.27.1/lib/Cargo.toml) uses a compatible language requirement. Keep generator identity, generated ABI, and public adapter compatibility as separate checks.

Wasmi 2 tooling explicitly enables `validate`, `stable`, `simd`, and `auto-dispatch` while disabling defaults. `auto-dispatch` selects loop dispatch for unoptimized host builds, where native tail-call optimization is unavailable; omitting it can overflow the host stack during ordinary plugin execution. The long-loop interpreter regression and actual Typst package smoke cover this migration.

## Reference tools and JavaScript compatibility

The 0.9.0 preparation audit selected Mermaid 12.1.0. Development now selects 12.2.0 through [ADR-0094](adr/0094-mermaid-12-2-upstream-transition.md); published releases retain their original baseline. Its reference host graph is frozen and independently admitted: updating a reference dependency changes the oracle and requires the Mermaid alignment workflow. ZenUML 4.6.0 falls outside the selected plugin's `^3.47.9` contract and is not substituted without coordinated admission.

Application TypeScript remains on the compatible 6.0.3 line. TypeScript 7 does not provide the old compiler API and peer contract required by the selected TypeScript/ESLint stack. Retain this as a documented compatibility exception; do not treat a tool's major version as an unconditional application upgrade.

Playground selects js-yaml 4.3.2, the current `v4-legacy` release. Version 5.4.3 rejects multi-line flow mappings already accepted by the selected Mermaid configuration contract, such as `config: {\n  theme: neo-dark\n}`. Retain the existing configuration fixture and revisit the major upgrade when the source contract has a compatible parser path; a parser upgrade must not silently reject accepted diagrams.

These version observations describe the 0.9.0 preparation audit, not an evergreen claim about registry latest. Store the complete Cargo/npm candidate audit and compatibility evidence in release artifacts or the release record. Revisit each exception when its blocking public API or companion dependency changes. The policy records selection reasons; it does not assert that a new `cargo deny`, package build, or publication has passed.

## Node build compatibility

The native GNU builder retains Node 24.13.1 on Bullseye because that container defines the selected Linux compatibility baseline; a newer host tool does not by itself admit a newer glibc requirement. The musl candidate uses Node 24.21.0 on Alpine 3.23. Keep each container identity and its package-tool provenance with the owner artifact evidence, and verify native loading/package smoke after a builder change. These are distinct preparation choices, not proof that the final package has passed its native owner gates.

## GitHub Actions

GitHub Actions use reviewed full version tags. Source identity remains the immutable commit and tree. Downloaded release tools, staged artifacts, and registry reconciliation retain SHA-256 verification under [Release Preflight](release/RELEASING.md#release-preflight).

## Evidence and release gates

For each exception, retain the selected and observed newer versions, affected public/generator contract, primary source or manifest evidence, owner verification, and removal condition. Keep the evidence small and use existing owner checks rather than building a second dependency resolver or compiler in release scripts.

Run version projection, dependency and legal checks, independent-crate compatibility, fresh registry consumers, artifact closure, and package smoke through their existing owners. A source edit after immutable preflight invalidates that evidence. See [release preparation](release/PUBLISH_ORDER.md#090-preparation) and [release operations](release/RELEASING.md).
