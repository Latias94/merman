# 0.7.0 to 0.8.0 candidate: release comparison

Status: preparation checkpoint, 2026-10-04. This report compares the previous stable release, `v0.7.0` (`00a2f291025913d612b6ca1ce9b6df6daf5bef94`, published 2026-06-09), with the source being prepared for `0.8.0`. The inspected stabilization base is `635d62d6c0b28eea260f64879fa2209e39268499`; the candidate has not been published or tagged. The approximately four-month range includes the alpha release history, not just changes since alpha.7. Use the [stable upgrade guide](V070_TO_V080_UPGRADE_GUIDE.md) for required consumer actions and the [Rust embedding guide](../rendering/RUST_EMBEDDING.md) for integration code.

## What changed for users

| Surface | 0.7.0 | Inspected stable candidate | Consequence and evidence |
| --- | --- | --- | --- |
| Rust facade defaults | Parser-only, empty defaults | All families, SVG, Cytoscape, ELK, and math | Parser-only users must explicitly disable defaults and select languages; source declarations in the two tagged/current `crates/merman/Cargo.toml` manifests. |
| Capability selection | Coarse `render`, `raster`, `ratex-math` bundles | Independent output, layout, math, runtime, and positive diagram selectors | A slim host can select its allowlist and outputs; backend features do not select languages. See [Features](../FEATURES.md). |
| Public operation owner | Target-specific headless convenience APIs | `Renderer`, a per-operation request/control, and typed artifacts | Hosts gain consistent cancellation, resource admission, and failure classification; existing renderer call sites need migration. See [embedding](../rendering/RUST_EMBEDDING.md). |
| Mermaid presentation | Pinned Mermaid 11.15.0 | Source selects 12.1.0; validation remains in progress | Review layout, Redux/Neo styling, SVG IDs, and snapshots. The selection is not itself a verification pass; the final pinned baseline and receipts must be reconciled. |
| Language selection | Broad compiled implementations | 34 logical `diagram-*` selectors with output-local availability | Default products keep the full parser set; explicit consumers can narrow it. Alias IDs, Error infrastructure, and admission rows must not be counted as additional logical families. |
| Language and parity growth | Historical 0.7.0 family inventory | Additional language work across the alphas, including Swimlane, Railroad variants, Wardley, Cynefin, Agentflow, and Usecase | Consult the family/runtime catalogs and current [alignment status](../alignment/STATUS.md); parsing, primary SVG admission, ASCII, and editor coverage are separate contracts. |
| Text measurement | Vendored metric path available | Deterministic font-agnostic fallback with an explicit host-provider boundary | Smaller font-data closure does not guarantee the final display font fits; test host titles and labels. See [font-boundary evidence](../alignment/MERMAID_12_TITLE_029_FONT_BOUNDARY_2026_09_30.md). |
| Terminal and editor workflows | Earlier ASCII and embedding APIs | Richer ASCII layouts/viewport/encoding reports; dedicated analysis, editor, and LSP owners | Review report schema 3 and strict decoders; hosts own scheduling and document lifecycle. Follow the versioned migration guides. |
| Distribution | Earlier CLI and native/Web contracts | Separate Web/Node/native package recipes, transport versions, notices, and independent grammar/Typst versions | Upgrade coupled wrappers and artifacts together and query the installed catalog. A workspace tag is not evidence that every channel published. See [Package Surfaces](PACKAGE_SURFACES.md). |

These are consumer-visible consequences, not commit-count claims. The root changelog retains the published alpha sections and credits; its unshipped `0.8.0` section describes stabilization since alpha.7. The larger reusable theme refactor remains deferred.

## Evidence and comparison boundaries

The source-facts collector verifies that `v0.7.0` is an ancestor of the target and reads committed manifests, changed paths, and admission inventories. Its direct-dependency counts are not a resolved normal dependency closure. The 0.7.0 and newer admission tables also use different inventory formats; their row counts are not a like-for-like language coverage metric.

A final release comparison must freeze a clean reviewed source commit after pending renderer/alignment changes have landed. Uncommitted work in another worktree is excluded from this checkpoint. Documentation-only follow-ups may cite an earlier measured code commit; any production, feature, lockfile, compiler, or profile change requires a new measurement.

## Performance and binary size

### Verified historical Windows CLI checkpoint

The GitHub Release Windows x86_64 archives were downloaded on 2026-10-04. Their published SHA-256 sidecars match the downloaded bytes, the release asset byte counts match, and the table reads the actual uncompressed `merman-cli.exe` member. This compares published product defaults at `v0.7.0` and `v0.8.0-alpha.7`, not a rebuild with one compiler/profile or the final stable candidate.

| Unit | 0.7.0 bytes | Alpha.7 bytes | Delta |
| --- | ---: | ---: | ---: |
| Windows CLI ZIP archive | 11,850,128 | 19,534,817 | +7,684,689 (+64.85%) |
| Uncompressed CLI executable | 30,224,384 | 49,761,280 | +19,536,896 (+64.64%) |

The full historical product is larger. New functionality and dependency/default changes are plausible contributors, but this checkpoint does not quantify their individual costs or establish a same-capability regression. Archive bytes also include packaging/compression effects. Attribute the growth using the controlled candidate lanes before choosing a release size verdict.

| Archive | Verified SHA-256 |
| --- | --- |
| `merman-cli-x86_64-pc-windows-msvc.zip` | `51f4898058d7bae48255a15663cafc14fcee3e352f271a916b2c057587070977` |
| `merman-cli-x86_64-pc-windows-msvc.zip` | `7a24ff729ff75409361a5d8c538d74e2a227c75213db67eee1a6a726b206acd7` |

The ignored raw receipt is `target/bench/experiments/stable-080/published-cli-sizes.json`; each tag directory contains the release API metadata, checksum sidecar, downloaded ZIP, and extracted executable. The experiment registration is `target/bench/experiments/stable-080/experiment.yaml`. Keep this historical receipt separate from later candidate builds.

PE section inspection puts 15,849,584 bytes of the historical executable delta in `.text` (81.13% of the full file delta), 3,182,840 in `.rdata` (16.29%), and 470,940 in `.pdata`. These are section virtual-size differences, not a source-code or dependency attribution. Both enabled capabilities and their implementations compile into `.text`; these two published releases do not control compiler, flags, code paths, or workloads. The inspection receipt is `target/bench/experiments/stable-080/pe-sections.json`.

### Controlled CLI Rustdoc feature decision

At the clean `c45ef5392d8d7447b29197cc133a485c269792f8` code checkpoint, both binaries were built for `x86_64-pc-windows-msvc` using Rust 1.95.0, `cargo build --locked --profile dist -p merman-cli --no-default-features --features <exact recipe> --jobs 1`. The second recipe removes only `rustdoc` from the first recipe's 20 direct features. This is a within-revision, within-profile feature comparison; it does not measure the release-version transition or a compilation-speed improvement.

| Explicit CLI recipe | Uncompressed executable | Normal runtime dependency packages | Difference from full recipe |
| --- | ---: | ---: | ---: |
| Full alpha.7-style release features | 49,723,904 bytes | 303 | Baseline |
| Identical features except `rustdoc` | 48,630,784 bytes | 296 | -1,093,120 bytes (-2.20%); seven fewer packages |

The removed packages are `merman-doc`, `toml`, `toml_datetime`, `toml_parser`, `toml_writer`, `serde_spanned`, and `winnow`. For the same Flowchart, Sequence, Gantt, and Class inputs, SVG output bytes and hashes matched across both binaries. The compiled command catalog includes Rustdoc only in the first binary; the second rejects the missing command. `.text` accounts for 901,936 bytes of the controlled PE section difference. Raw recipes, executable checksums, package closures, output controls, and binary copies are in `target/bench/experiments/cli-rustdoc-default/`. Their build times came from different cache states and are not comparable.

The authoring-only Rustdoc workflow can be explicitly selected with `--features rustdoc`; standard CLI defaults and the official `cli-release` archive omit it. For an installed prebuilt archive, the command cannot be enabled retroactively. Keep the renderer, all diagram families, analysis, ASCII, exports, ELK, math, and local icons available in the standard CLI while attribution for the other choices is still in progress; `network-icons` requires explicit runtime `--allow-network` even when compiled. Retain exact slim source recipes for embedders and lint-only applications. Do not decide to remove a Mermaid-default backend or an advertised CLI output format based on executable bytes alone.

The release campaign has two distinct lanes: **distributed product defaults**, which include each version's declared capabilities, and **equivalent capabilities**, which control the host workload, languages, output, optional backends, runtime policy, and profile. Full-product growth can include newly delivered capabilities; it is not automatically an implementation regression. A parser-only 0.7.0 facade and a default complete-SVG 0.8.0 facade are different workloads.

| Check | State at this preparation checkpoint | Acceptance boundary |
| --- | --- | --- |
| Published 0.7.0 versus alpha.7 CLI archives | Verified Windows checkpoint above | Historical product checkpoint only; retain archive hashes, compressed bytes, extracted executable bytes, target, and release provenance. It does not measure the final 0.8.0 candidate. |
| Final candidate native CLI size | Pending final source freeze | Same host/compiler/target/profile; record raw/stripped/package units separately and the complete recipe. Compare the full product and a controlled capability lane independently. |
| Native Criterion regression confirmation against 0.7.0 | Blocked by historical harness contract | 0.7.0's schema-1 corpus/harness has no current pre/postflight output identity receipts. The current comparator cannot certify comparable rows; do not convert missing evidence into a speedup or a pass. |
| Browser/WASM, Node, mobile, RSS, and installed footprint | Not measured in this checkpoint | Run each owning surface separately with its actual artifact recipe. Native numbers do not certify these targets. |

For native timing, use [the performance runbook](../performance/RUNBOOK.md), [benchmarking](../performance/BENCHMARKING.md), and `tools/bench/compare_self.py`. Resolve the historical harness gap through a reviewed benchmark-only compatibility recipe or an agreed public-operation campaign; retain the original locked production source. Inputs must be byte-identical, operations and engine lifecycles equivalent, and output correctness reviewed despite intentional Mermaid-version presentation changes. Freeze the admission contract before timing, use balanced A/A calibration and AB/BA confirmation, and keep noisy or incomparable results inconclusive. Do not weaken the current output contract merely to produce green rows.

The [2026-09-26 family-selection report](../performance/diagram_selection_native_2026-09-26.md) is earlier controlled evidence for a Flowchart/Gantt subset, not a 0.7.0-to-0.8.0 measurement. The [alpha.3 refactoring report](ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md) likewise keeps its own historical host and commits. Neither supplies missing final-release measurements.

## Remaining stable release checks

Before publication, bind all receipts to the reviewed final candidate: version and changelog projection, clean independent consumers, feature/dependency and legal checks, focused Rust tests, current SVG family admission, and package-owner preflight. Complete the performance/size lanes above, investigate reproduced regressions, and include both measured gains and remaining costs in the final release notes. Run immutable non-publishing preflight only after choosing the release date and source commit; preparation with an `Unreleased` heading is not an immutable release pass.

Publication, tagging, registry uploads, and independent package-manager submissions are outside this preparation checkpoint. Confirm each channel's actual publication before replacing the alpha.7 install instructions with stable commands.
