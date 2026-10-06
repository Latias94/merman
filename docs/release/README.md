# Releases and upgrades

Use this index to choose an upgrade path by the version you have installed. Merman `0.8.0` Rust crates and CLI/LSP archives are published with Mermaid `12.1.0` compatibility. Other channels remain independent; consult the [publication snapshot](PUBLISH_ORDER.md#080-publication-snapshot) before changing their installed versions.

## Reading the 0.8.0 release notes

The [root 0.8.0 changelog](../../CHANGELOG.md#080---2026-10-06) is cumulative from stable 0.7.0: choose your upgrade path, handle the breaking changes, then review new capabilities, fixes, and bounded performance evidence. Published alpha sections remain unchanged for users tracing an intermediate version. A direct stable upgrade starts with [one consolidated migration guide](V070_TO_V080_UPGRADE_GUIDE.md); it does not require applying superseded alpha APIs in sequence. Users already on alpha.7 should instead start with the [remaining delta](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md).

Package-local changelogs project the release delta onto each SDK's actual APIs, payloads, capabilities, and publication track. Benchmarks retain the exact measured code revision and are not transferred between Rust, CLI, browser, and native binding surfaces.

## Workspace upgrades

| Installed version | Target version | Migration reference |
| --- | --- | --- |
| `0.7.0` | `0.8.0` | [Stable upgrade guide](V070_TO_V080_UPGRADE_GUIDE.md), [Rust embedding guide](../rendering/RUST_EMBEDDING.md), and [comparison report](V070_TO_V080_RELEASE_REPORT.md) |
| `0.8.0-alpha.3` | `0.8.0-alpha.5` | [Upgrade guide](ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md): capability selection, Rust APIs, browser packages, and native transports |
| `0.8.0-alpha.4` | `0.8.0-alpha.5` | Runtime contracts are unchanged; alpha.5 follows up the incomplete alpha.4 distribution. See the [alpha.5 changelog](../../CHANGELOG.md#080-alpha5---2026-08-09). |
| `0.8.0-alpha.5` | `0.8.0-alpha.6` | [Upgrade guide](ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md) and [detailed symbol mapping](UNRELEASED_UPGRADE_GUIDE.md): operation-scoped rendering, analysis/editor, and binding changes |
| `0.8.0-alpha.6` | `0.8.0-alpha.7` | [Upgrade guide](ALPHA6_TO_ALPHA7_UPGRADE_GUIDE.md): Mermaid 12 defaults, explicit family features, Agentflow/Usecase, and ASCII/native contract changes |
| `0.8.0-alpha.7` | `0.8.0` | [Upgrade guide](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md): Mermaid 12.1 semantics, ELK routing/labels, and low-level Rust initializer changes |

Apply the intervening guides in order when skipping prereleases. For example, an alpha.5-to-alpha.7 upgrade needs both the alpha.6 and alpha.7 migrations. Published-version guides are historical release contracts: their examples target that version, not the current branch. The stable upgrade guide consolidates consumer actions across the 0.8 prereleases. For another earlier release without a dedicated guide, start with its [changelog](../../CHANGELOG.md) and the README at the matching Git tag.

Keep coupled Rust dependencies, generated wrappers, and native or WASM artifacts on one release. Package channels publish independently; a workspace tag alone does not prove that every channel has published. The [dated publication snapshots](PUBLISH_ORDER.md#080-publication-snapshot) record verified channels and recovery history. Inspect the installed artifact's runtime catalog when its capabilities matter.

`UNRELEASED_UPGRADE_GUIDE.md` is the fixed alpha.6 detailed reference. Its historical filename remains for existing links; it is not a rolling guide to unshipped changes. Development changes belong in the root changelog's unshipped section. The [alpha.7-to-0.8.0 guide](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) records the Mermaid 12.1 migration; channel availability is tracked separately.

## Independent versions and native protocols

| Contract | Reference | Version boundary |
| --- | --- | --- |
| Tree-sitter grammar and queries | [Query migration](../../distribution/tree-sitter-mermaid/docs/query-migration.md) | Published `0.2.0` → `0.3.0`; separate from the workspace version and Tree-sitter language ABI |
| C and Flutter native integration | [ABI 3 migration](../bindings/ABI3_MIGRATION.md) and [FFI protocol](../bindings/FFI_PROTOCOL.md) | Rolling source protocol; use generated headers and documentation from the same Merman release |
| Typst package | [Package README](../../distribution/typst/merman/README.md) | Independent Typst Universe version; a new workspace plugin crate does not publish a new wrapper |
| VS Code extension | [Extension README](../../tools/vscode-extension/README.md) | Independent extension version and artifact channel |

## Release operations and evidence

| Task | Owner |
| --- | --- |
| Choose a package or delivery channel | [Package surfaces](PACKAGE_SURFACES.md) |
| Prepare, publish, or recover a release | [Release operator guide](RELEASING.md) |
| Inspect publication order and dated channel receipts | [Publish order](PUBLISH_ORDER.md) |
| Operate the independent grammar release | [Tree-sitter Mermaid release](TREE_SITTER_MERMAID.md) |
| Change the Mermaid compatibility baseline | [Mermaid upgrade playbook](MERMAID_UPGRADE_PLAYBOOK.md) |
| Compare releases or justify performance claims | [Comparative release reports](RELEASE_REPORTS.md) |
| Check CLI/LSP platform evidence | [Target admission](CLI_TARGET_ADMISSION.md) |

The [alpha.3-to-alpha.5 refactoring report](ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md) preserves measurements from named historical commits. It is evidence, not a current upgrade procedure. The [documentation archive index](../ARCHIVE.md) explains how other historical plans and reports are retained.
