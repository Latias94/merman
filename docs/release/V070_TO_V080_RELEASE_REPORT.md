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

These are consumer-visible consequences, not commit-count claims. The root changelog summarizes the cumulative stable-to-stable migration and retains the published alpha sections and credits as history. The alpha.7-to-0.8.0 guide isolates the remaining prerelease delta. The larger reusable theme refactor remains deferred.

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

| Release | Archive | Verified SHA-256 |
| --- | --- | --- |
| `v0.7.0` | `merman-cli-x86_64-pc-windows-msvc.zip` | `51f4898058d7bae48255a15663cafc14fcee3e352f271a916b2c057587070977` |
| `v0.8.0-alpha.7` | `merman-cli-x86_64-pc-windows-msvc.zip` | `7a24ff729ff75409361a5d8c538d74e2a227c75213db67eee1a6a726b206acd7` |

The ignored raw receipt is `target/bench/experiments/stable-080/published-cli-sizes.json`; each tag directory contains the release API metadata, checksum sidecar, downloaded ZIP, and extracted executable. The experiment registration is `target/bench/experiments/stable-080/experiment.yaml`. Keep this historical receipt separate from later candidate builds.

PE section inspection puts 15,849,584 bytes of the historical executable delta in `.text` (81.13% of the full file delta), 3,182,840 in `.rdata` (16.29%), and 470,940 in `.pdata`. These are section virtual-size differences, not a source-code or dependency attribution. Both enabled capabilities and their implementations compile into `.text`; these two published releases do not control compiler, flags, code paths, or workloads. The inspection receipt is `target/bench/experiments/stable-080/pe-sections.json`.

### Controlled CLI Rustdoc feature decision

At the clean `c45ef5392d8d7447b29197cc133a485c269792f8` code checkpoint, both binaries were built for `x86_64-pc-windows-msvc` using Rust 1.95.0, `cargo build --locked --profile dist -p merman-cli --no-default-features --features <exact recipe> --jobs 1`. The second recipe removes only `rustdoc` from the first recipe's 20 direct features. This is a within-revision, within-profile feature comparison; it does not measure the release-version transition or a compilation-speed improvement.

| Explicit CLI recipe | Uncompressed executable | Normal runtime dependency packages | Difference from full recipe |
| --- | ---: | ---: | ---: |
| Full alpha.7-style release features | 49,723,904 bytes | 303 | Baseline |
| Identical features except `rustdoc` | 48,630,784 bytes | 296 | -1,093,120 bytes (-2.20%); seven fewer packages |

The removed packages are `merman-doc`, `toml`, `toml_datetime`, `toml_parser`, `toml_writer`, `serde_spanned`, and `winnow`. For the same Flowchart, Sequence, Gantt, and Class inputs, SVG output bytes and hashes matched across both binaries. The compiled command catalog includes Rustdoc only in the first binary; the second rejects the missing command. `.text` accounts for 901,936 bytes of the controlled PE section difference. Raw recipes, executable checksums, package closures, output controls, and binary copies are in `target/bench/experiments/cli-rustdoc-default/`. Their build times came from different cache states and are not comparable.

The stable source default and official `cli-release` archive retain Rustdoc, as the published alpha.7 artifacts do. Its 1.09 MB / seven-package cost is small relative to the complete native CLI, and installed prebuilt archives cannot add the command later. In 48 alternating cold-process pairs at this same code checkpoint, `--version` took 30.61 versus 30.45 ms (Rustdoc on/off paired ratio 1.009; off/off A/A ratio 1.003), and the selected Flowchart SVG took 52.98 versus 52.63 ms (paired ratio 1.007; off/off A/A ratio 1.013). Output hashes matched. The differences overlap observed process noise: there is no resolved startup penalty on this host; this is not a compilation-time, memory, or broad-rendering measurement. The ignored preregistration and raw samples are `target/bench/experiments/cli-rustdoc-default/cold-cli-experiment.yaml` and `cold-cli-results.json`.

### Controlled Rustdoc-off CLI feature attribution

At the clean code commit `c1643c7377eddaa7c2ae7e803f3b87cd8294bea1`, five sequential Windows x86_64 builds used Rust 1.95.0, locked Cargo dependencies, the same `dist` profile, and the then-descriptor-owned 19-feature `cli-release` recipe, with Rustdoc off. Each alternative removes only the named capability from that 19-feature recipe, except the Zed-style row, which selects 13 explicit families in place of `all-diagrams`. The target, compiler, sources, and remaining feature flags are identical within this table. Normal dependency packages come from `cargo tree --edges normal,no-proc-macro` for each recipe; package counts are not compiled byte counts.

| CLI recipe | Executable bytes | Change from Rustdoc-off recipe | Normal packages | User-visible tradeoff |
| --- | ---: | ---: | ---: | --- |
| Rustdoc-off candidate at `c1643c737` | 48,630,784 | Baseline | 296 | All 34 families, ASCII/SVG/PNG/JPEG/PDF, both layout backends, local and authorized remote icons. |
| Without `network-icons` | 43,000,320 | -5,630,464 (-11.58%) | 233 | Local icon packs remain; no HTTP(S) icon-pack acquisition or `--allow-network` option. |
| Without `layout-elk` | 45,995,520 | -2,635,264 (-5.42%) | 294 | Removes an upstream-default graphical backend and changes Flowchart/Class SVG outputs in the selected controls. |
| Without `png,jpeg,pdf` | 42,999,808 | -5,630,976 (-11.58%) | 245 | Drops all three advertised binary output formats and their `mmdc` compatibility modes. |
| Thirteen Zed-style diagram families | 42,434,048 | -6,196,736 (-12.74%) | 295 | Removes 21 less-common diagram families from the CLI; appropriate for a known host allowlist, not arbitrary CLI inputs. |

All four selected Flowchart, Sequence, Gantt, and Class inputs rendered successfully in every recipe. Their SVG bytes matched the Rustdoc-off recipe for the network, binary-export, and 13-family alternatives; removing ELK changed the Flowchart and Class SVG hashes. These controls do not prove that every family or non-SVG operation behaves identically. Each ablation starts from the same 19-feature Rustdoc-off recipe: dependency closure, dead-code elimination, and feature interactions overlap, so **do not add these savings**. Build durations came from different cache states and provide no compile-time claim. Raw recipes, SHA-256 digests, executable copies, section sizes, closure inventories, and selected-output controls are under `target/bench/experiments/cli-defaults/`, with the registration in its `experiment.yaml`. These values attribute the *former Rustdoc-off candidate* at the specified commit, not the new 20-feature default or the historical 0.7.0-to-0.8.0 increase: the 0.7.0 CLI already linked `reqwest`, for example. Rebuild the final full recipe and its selected ablations after the feature decision before quoting a final-release saving.

The stable CLI default and the official archive will keep all diagram families, SVG/ASCII and PNG/JPEG/PDF, ELK/Cytoscape, math, local icons, `network-icons`, and the Rustdoc authoring command. The general CLI must accept arbitrary diagrams, ELK changes default rendering, and dropping binary formats would remove documented output workflows. Remote icon packs also worked in the 0.7.0 CLI; an installed archive cannot later turn on a missing feature. The former Rustdoc-off recipe saved 5.63 MB without network support; the current 20-feature recipe saves 5.53 MB, as measured below, while actual network access still requires `--allow-network` (and private destinations require a second permission). An offline source build can omit `network-icons` without losing local packs. Rustdoc remains available in prebuilt artifacts. Source builds that do not need it can still select a smaller explicit capability recipe. Smaller family allowlists belong in embedding products whose inputs are known, as illustrated in [Features](../FEATURES.md) and the [Rust embedding guide](../rendering/RUST_EMBEDDING.md). Reconsider an offline CLI archive if user demand justifies a separately documented and tested distribution; this checkpoint does not create an extra SKU.

### Current 20-feature CLI default versus an offline build

After restoring Rustdoc to the `cli-release` descriptor, both Windows `dist` executables were rebuilt sequentially from the same PR working tree with Rust 1.95.0, the same locked dependencies, target, and compiler profile. The full 20-feature recipe and the 19-feature alternative differ **only** in `network-icons`. This is a new controlled candidate receipt; the source is identified by the parent commit and production-input diff SHA-256 in the ignored `target/bench/experiments/cli-final-defaults/results.json`. A final release source freeze still requires a fresh artifact receipt.

| Exact recipe | Executable bytes | Normal runtime packages | Difference from full CLI |
| --- | ---: | ---: | ---: |
| 20-feature `cli-release` with Rustdoc and network icons | 49,723,904 | 303 | Baseline |
| Same recipe without `network-icons`, retaining Rustdoc and local icons | 44,196,352 | 240 | -5,527,552 bytes (-11.12%); 63 fewer packages |

The four selected Flowchart, Sequence, Gantt, and Class SVG outputs were byte-identical between these builds. The feature still changes the compiled command's network permission surface, so this is not a claim of complete capability equivalence. With five warmups, 48 alternating cold-process pairs, and eight no-network A/A control pairs, the `--version` operation measured 27.67 versus 24.28 ms (network on/off paired ratio 1.161; A/A 1.004), establishing a cold startup cost for this release recipe on this host. The small Flowchart render measured 50.93 versus 46.40 ms (paired ratio 1.139), but its A/A ratio was 1.181; **render latency in this new run is inconclusive**. The earlier Rustdoc-off run above had a cleaner SVG control, but do not transfer its timing precision to a different recipe. Neither lane measures warm renderer parse/layout/render stages. Raw builds, executable digests, selected output checks, and startup samples remain in the ignored `target/bench/experiments/cli-final-defaults/` folder.

### Reference Mermaid CLI installed-size context

On 2026-10-04, `npm pack @mermaid-js/mermaid-cli@12.0.0` produced a 26,224-byte package tarball (84,181 bytes unpacked). This is *only the CLI package*, not an installed executable. A fresh Windows Node 24.21.0 / npm 12.0.2 installation of that exact CLI plus its `puppeteer@25.6.0` peer, using `npm install --ignore-scripts --no-audit --no-fund` and `PUPPETEER_SKIP_DOWNLOAD=1`, resolved `mermaid@12.1.0` and occupied 377,877,174 bytes of regular files in `node_modules` (the sum of file lengths, not filesystem allocated blocks). The ignored experiment registration, receipt, and full transitive lockfile are in `target/bench/experiments/ref-mermaid-cli-size/measurement.json` and `package-lock.json`.

**The browser is excluded** from that 377.9 MB: normal Puppeteer rendering also needs a compatible Chrome/Chromium, whether downloaded by its install hook or supplied by the host. A cached Chrome Headless Shell with a matching build number on this machine occupies 283,816,260 bytes, but it is an illustrative separate component, not a measured default-install payload or proof that the CLI will use that particular browser. Npm tarball bytes cannot be compared to the native CLI executable alone, and native size does not imply rendering feature, platform, output, or latency equivalence. This context shows that retaining a ~1 MB Rustdoc subcommand is not a material installed-footprint decision relative to the complete reference toolchain; native CLI size should still be tracked against its own previous stable version and embedded-host budgets.

### Native library rendering checkpoint (2026-10-05)

The [matched-Dagre library report](../performance/library_dagre_v070_to_01cc4562f_2026-10-05.md) compares v0.7.0 with merged product source `01cc4562f` using complete `merman` crate SVG calls inside reused native processes and engines, excluding CLI startup. Both versions explicitly select Dagre/classic, the default theme, and matching Flowchart settings. On six selected fixtures, current rendering shows approximately **15%-33% lower paired-median time**; the four stress cases previously slower under changed release defaults now show 15%-21% lower costs. Five controls use the scoped investigation and one nested-cluster control retains its earlier receipt. These Windows-host observations illustrate improvement under matched backend settings, but **statistical confirmation remains outstanding**: both same-executable A/A calibrations failed their registered noise gate. They do not establish a whole-library speedup, identical output work, or the contribution of a particular alpha optimization.

The comparison excludes ELK because v0.7.0 has no Flowchart ELK implementation baseline; alpha.7 is not needed or measured. The [initial](../performance/library_rendering_v070_to_01cc4562f_2026-10-05.md) and [expanded](../performance/library_rendering_expanded_2026-10-05.md) reports preserve broader historical observations and unresolved rendering/identity signals. Their release-default ratios are not a like-for-like Dagre regression table. Receipts preserve paired estimates, source and executable hashes, settings, output checks, raw-sample provenance, and failed calibration outcomes. No production renderer change is made for this comparison.

### Diagnostic cold CLI latency; native regression still unconfirmed

A separate Windows diagnostic ran the verified published `v0.7.0` CLI and the earlier Rustdoc-off 0.8.0 candidate on the **same input bytes and host**, spawning a new process for each SVG render and capturing stdout. Each operation had three warmups, 24 alternating base/candidate pairs, and eight candidate/candidate A/A control pairs. All four commands succeeded and returned well-formed, deterministic SVG, but their output bytes differ across versions: Flowchart SVG grew from 9,869 to 29,109 bytes in this small sample, for example. Both the Mermaid baseline and published-vs-local compiler/build context also changed. These are descriptive *full-product cold process* measurements, **not** matched-output renderer latency or an admitted 0.7.0-to-0.8.0 performance regression.

| Tracked fixture | 0.7.0 median | Candidate median | Paired candidate/base median | Candidate A/A median ratio |
| --- | ---: | ---: | ---: | ---: |
| Flowchart tiny | 24.10 ms | 34.05 ms | 1.42× | 1.04× |
| Sequence tiny | 24.25 ms | 34.20 ms | 1.40× | 1.01× |
| Gantt medium | 25.26 ms | 33.73 ms | 1.33× | 1.02× |
| Class tiny | 25.39 ms | 35.91 ms | 1.39× | 1.04× |

A non-rendering `--version` control also differed across the published 0.7.0 binary and this candidate (20.05 versus 27.80 ms median); thus the cold-render gap cannot all be assigned to layout or SVG emission. To isolate one feature within the earlier Rustdoc-off source/profile, 48 alternating pairs compared that candidate with an otherwise identical build without `network-icons`; both produced exactly the same bytes for `--version` and the selected Flowchart SVG. Rustdoc-off candidate versus no-network median times were 27.35 versus 23.65 ms for `--version` (paired ratio 1.16×, candidate A/A 1.01×) and 33.48 versus 29.55 ms for the SVG operation (paired ratio 1.12×, candidate A/A 1.04×). The network-capable executable also imports more Windows system DLLs and includes the larger HTTP/TLS/DNS closure; this is feature-cost attribution for *cold CLI process use on this host*, not proof that an individual library accounts for the full delay. The official CLI retains network icons as described above. Raw samples, input/output hashes, commands, and registration are in `target/bench/experiments/stable-080/cli-cold-render-diagnostic.*`, `cli-startup-control.*`, and `target/bench/experiments/cli-defaults/startup-*`.

The release campaign has two distinct lanes: **distributed product defaults**, which include each version's declared capabilities, and **equivalent capabilities**, which control the host workload, languages, output, optional backends, runtime policy, and profile. Full-product growth can include newly delivered capabilities; it is not automatically an implementation regression. A parser-only 0.7.0 facade and a default complete-SVG 0.8.0 facade are different workloads.

| Check | State at this preparation checkpoint | Acceptance boundary |
| --- | --- | --- |
| Published 0.7.0 versus alpha.7 CLI archives | Verified Windows checkpoint above | Historical product checkpoint only; retain archive hashes, compressed bytes, extracted executable bytes, target, and release provenance. It does not measure the final 0.8.0 candidate. |
| PR candidate native CLI size | Current 20-feature `dist` executable measured above; final source freeze pending | Same host/compiler/target/profile; record raw/stripped/package units separately and the complete recipe. Compare the full product and a controlled capability lane independently. |
| Published-CLI cold render diagnostic | Four source-identical inputs and a `--version` startup control measured above | Product observation across different compilers, SVG outputs, and Mermaid baselines; not a renderer regression gate. |
| Native library release observation / strict confirmation | Six matched-Dagre controls observed on 2026-10-05; statistical confirmation remains outstanding | Explicit Dagre/classic settings and complete library SVG calls are reported separately from CLI timing. Both A/A calibrations failed; changed revision-specific outputs and the bounded fixture selection prevent a full strict-regression pass. ELK and alpha.7 are excluded. |
| Browser/WASM, Node, mobile, RSS, and installed footprint | Not measured in this checkpoint | Run each owning surface separately with its actual artifact recipe. Native numbers do not certify these targets. |

For native timing, use [the performance runbook](../performance/RUNBOOK.md), [benchmarking](../performance/BENCHMARKING.md), and `tools/bench/compare_self.py`. Resolve the historical harness gap through a reviewed benchmark-only compatibility recipe or an agreed public-operation campaign; retain the original locked production source. Inputs must be byte-identical, operations and engine lifecycles equivalent, and output correctness reviewed despite intentional Mermaid-version presentation changes. Freeze the admission contract before timing, use balanced A/A calibration and AB/BA confirmation, and keep noisy or incomparable results inconclusive. Do not weaken the current output contract merely to produce green rows.

The [2026-09-26 family-selection report](../performance/diagram_selection_native_2026-09-26.md) is earlier controlled evidence for a Flowchart/Gantt subset, not a 0.7.0-to-0.8.0 measurement. The [alpha.3 refactoring report](ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md) likewise keeps its own historical host and commits. Neither supplies missing final-release measurements.

## Remaining stable release checks

Before publication, bind all receipts to the reviewed final candidate: version and changelog projection, clean independent consumers, feature/dependency and legal checks, focused Rust tests, current SVG family admission, and package-owner preflight. Complete the performance/size lanes above, investigate reproduced regressions, and include both measured gains and remaining costs in the final release notes. Run immutable non-publishing preflight only after choosing the release date and source commit; preparation with an `Unreleased` heading is not an immutable release pass.

Publication, tagging, registry uploads, and independent package-manager submissions are outside this preparation checkpoint. Confirm each channel's actual publication before replacing the alpha.7 install instructions with stable commands.
