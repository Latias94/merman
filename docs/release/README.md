# Releases and upgrades

Use this index to choose an upgrade path by the version you have installed. Merman `0.8.0-alpha.7` was published on 2026-09-30 and is planned as the final alpha in the 0.8.0 cycle. The development branch is preparing stabilization changes; `0.8.0` is not released by this checkpoint.

## Workspace upgrades

| Installed version | Target version | Migration reference |
| --- | --- | --- |
| `0.8.0-alpha.3` | `0.8.0-alpha.5` | [Upgrade guide](ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md): capability selection, Rust APIs, browser packages, and native transports |
| `0.8.0-alpha.4` | `0.8.0-alpha.5` | Runtime contracts are unchanged; alpha.5 follows up the incomplete alpha.4 distribution. See the [alpha.5 changelog](../../CHANGELOG.md#080-alpha5---2026-08-09). |
| `0.8.0-alpha.5` | `0.8.0-alpha.6` | [Upgrade guide](ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md) and [detailed symbol mapping](UNRELEASED_UPGRADE_GUIDE.md): operation-scoped rendering, analysis/editor, and binding changes |
| `0.8.0-alpha.6` | `0.8.0-alpha.7` | [Upgrade guide](ALPHA6_TO_ALPHA7_UPGRADE_GUIDE.md): Mermaid 12 defaults, explicit family features, Agentflow/Usecase, and ASCII/native contract changes |

Apply the intervening guides in order when skipping prereleases. For example, an alpha.5-to-alpha.7 upgrade needs both the alpha.6 and alpha.7 migrations. These are historical release contracts: examples in an older guide target that version, not the current branch. For an earlier release without a dedicated guide, start with its [changelog](../../CHANGELOG.md) and the README at the matching Git tag.

Keep coupled Rust dependencies, generated wrappers, and native or WASM artifacts on one release. Package channels publish independently; a workspace tag alone does not prove that every channel has published. The [dated publication snapshots](PUBLISH_ORDER.md#alpha7-publication-snapshot) record the verified alpha.7 channels and earlier recovery history. Inspect the installed artifact's runtime catalog when its capabilities matter.

`UNRELEASED_UPGRADE_GUIDE.md` is the fixed alpha.6 detailed reference. Its historical filename remains for existing links; it is not a rolling guide to unshipped changes. Development changes belong in the root changelog's `Unreleased` section until a release is selected.

## Independent versions and native protocols

| Contract | Reference | Version boundary |
| --- | --- | --- |
| Tree-sitter grammar and queries | [Query migration](../../distribution/tree-sitter-mermaid/docs/query-migration.md#010-to-020) | `tree-sitter-mermaid` / `@mermanjs/tree-sitter-mermaid` `0.1.0` → `0.2.0`; separate from the workspace version and Tree-sitter language ABI |
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
