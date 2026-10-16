# Native theme regression attribution on Windows, 2026-10-15

Comparison: `v0.9.0` (`f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`) to merged theme main `b9844db7b747111690bf6369d4536885a85e35a7`.

The regression is reproduced on byte-identical Class and Journey SVGs. The largest isolated new shared owner is ordinary standalone finalization: resource fingerprinting plus XML/reference budget validation, followed by facade artifact/evidence sealing. Family SVG emission also regresses independently. A separate classic-theme provenance path adds deterministic quadratic work for many authored variables. No production optimization is implemented or admitted by this checkpoint.

## Measurement boundary

Host: Windows 11, Intel Core i7-11700, x86_64-pc-windows-msvc, Rust 1.95.0. Both locked bench builds use default features plus `svg`, one Cargo build job, and separate target directories. This lane is warm native Rust, reused Engine/Renderer, strict parsing, deterministic measurement and trusted-native SVG resources. It does not measure CLI startup, native font discovery, raster codecs, Node or browser/WASM.

The pipeline benchmark and fixture files are byte-identical between revisions. `v0.9.0` and the historical main control `3a4b8c393` also have no diff in Cargo.toml, Cargo.lock, or the merman/core/render crates. The earlier [repair closeout](native_theme_repair_2026-10-14.md) is relevant historical evidence on another host, not a substitute for these Windows observations.

Three alternating AB/BA diagnostic pairs use 30 Criterion samples, 2-second warmup and 3-second measurement. Public-operation side values below are arithmetic means of the three independent console point estimates. These are diagnostic observations, not a calibrated confirmation or an admitted optimization. Binary digests and pre/postflight output identity checks pass in the comparable run.

## Public source-to-SVG regression

| Fixture | v0.9.0 (us) | Merged themes (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 1533.333 | 2519.300 | +64.30% | +985.967 |
| journey_medium | 62.912 | 235.483 | +274.31% | +172.571 |
| class_tiny | 324.807 | 590.027 | +81.65% | +265.220 |

The initial four-case discovery report returned exit 2 because XY Chart SVG bytes differ. It is retained as `end-to-end.json`; it is not the successful comparison receipt. A fresh three-case run, `comparable-end-to-end.json`, returns exit 0 with no output-identity or recipe failure. Correctness fixes remain part of the target implementation.

## Stage attribution

Each stage has three independent observations per side, stable per-side output identities and unchanged frozen executables. Values are arithmetic means in microseconds. Layout projections differ on all three cases; their timings are attribution of current preparation work, not same-output latency admission. Parse and SVG-emission identities match across revisions.

| Stage | Class medium: base / head | Journey: base / head | Class tiny: base / head |
| --- | ---: | ---: | ---: |
| parse | 313.347 / 328.473 | 10.420 / 12.600 | 160.940 / 167.260 |
| parse_cold_engine | 315.640 / 335.653 | 16.610 / 19.568 | 177.973 / 180.140 |
| layout | 635.987 / 685.877 | 8.941 / 14.128 | 50.325 / 74.398 |
| render | 454.127 / 583.497 | 33.477 / 53.812 | 50.734 / 89.299 |

`render` excludes `family::prepare` and layout computation, but includes consuming/destructing the prepared artifact and returned SVG/session. It excludes standalone finalization, facade evidence freezing and admission sealing. Independently batched stages cannot be subtracted from or summed into the complete public operation.

## New shared standalone cost

In v0.9.0, the ordinary `pipeline=None` path returns the emitted string. Current `render_svg_target` calls `finalize_standalone_for_target_admission`, then `StandaloneSvgArtifact::finalize_exact_buffer`, then facade `finish_standalone_svg_target`. BestEffort ordinary output now computes a framed resource fingerprint and performs complete XML/reference resource-budget validation; facade completion freezes reports and constructs artifact/document/admission identities.

The public-API terminal probe prepares/render batches before the timer, then consumes 128 outputs per batch through `finalize_standalone(None)`. It records eight batches after 32 warmups. The timer includes loop/Vec push, temporary validation data and untransferred input-field destruction; returned artifact/session destruction, report freezing and facade admission are outside it. Every timed output preserves SVG bytes/SHA-256, resource fingerprint and terminal status. The status is `Unverified`: this is ordinary budget admission, not full resvg compatibility validation.

The hash probe uses the exact normal-target sha2 0.11.0 rlib linked by merman-render. Its resource-framing source mirror is checked against each real artifact resource fingerprint. Hash phases use eight batches of 1,024 operations. Values below are medians of batch-normalized times; they are different experiments and do not form an additive CPU breakdown.

| Fixture | SVG bytes | Ordinary finalization (us) | Resource fingerprint (us) | Artifact SHA-256 (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 77,419 | 639.221 | 39.313 | 38.808 |
| journey_medium | 12,224 | 104.948 | 6.204 | 5.939 |
| class_tiny | 16,936 | 123.179 | 8.651 | 8.405 |
| xychart_medium | 13,056 | 108.186 | 6.493 | 6.481 |

The terminal call path and the much smaller fingerprint experiment direct the next investigation to XML/reference validation and its data representation. They do not establish exclusive function percentages. Windows ETW profiling was refused with policy error `0xc5585011`; no system profiling policy was changed and no recorder remains active.

Concrete current ownership waste:

- `svg/pipeline/final_validation.rs:752` builds a ReferenceNode for every validated XML element before the no-expansion decision. Nodes retain per-parent children vectors, separate use/parsed ID strings and per-element references. The existing no-expansion optimization avoids the index/DAG only after this collection. Graph-eligible documents construct an additional dependency representation.
- `render/evidence.rs:146` clones RenderSessionReport and then consumes/drops the original family report. Consuming decomposition can move it. Report cardinality is mostly protocol-bounded; this clone is secondary and is outside the SVG-emission benchmark.
- Resource and artifact hashes bind different contracts. They cannot be replaced by a single interchangeable identity. The existing identical public/native artifact digest reuse is already present.

## Independent SVG-emission regression

The theme PR introduced `svg/parity/output.rs` (first introduced in `3ec79e3c8`). Every formatting fragment enters BoundedSvgOutput::record_write, exact append-budget/cancellation checks and capacity handling. XmlTextOutput additionally makes an empty sink write, scans logical CSS bytes for XML escaping, writes bounded UTF-8 slices and checkpoints. These paths are reachable without a selected typed theme in Class, Journey and Kanban.

The ordinary cancellation fast path is an atomic attention check, not a mutex or an unconditional OS-clock call. The variable to measure here is formatting-fragment count as well as serialized bytes. Legal ampersands and split `]]>` require the current XML-text correctness behavior; removing escaping or byte/cancellation admission would undo fixes.

Class also retains binary searches for dense relation indices in its completion domain. Prepared canonical slots can replace repeated lookup while preserving exactly-once, missing, duplicate and foreign-index rejection. Previously repaired Class/Kanban evidence-gating and consuming-seal issues N13-N24 are not newly discovered missing fixes.

## Quadratic classic-theme provenance

This is a separate public source-input problem, not the explanation for default scoped Redux. A default reused Engine parses strict `info` input with YAML frontmatter selecting Dark and E numeric variables named z00000000, z00000001, etc. Frontmatter config is deep-merged without the init-directive generated-key whitelist; numeric values survive source presentation filtering. All five scales parse successfully in both revisions, and the final authored variable is retained.

For this exact input family, a single staged Resolution has an initially empty dependency map. Overlay inserts self-dependencies; no fixed theme calculation reads these unknown keys; replay replaces each self-dependency. remove_dependency_subtree uses BTreeMap::retain. Its exact predicate count is:

`E(E - 1)/2 + A*E + E^2`

A is the number of intervening update assignments. This is deterministic Theta(E^2) work in the new provenance path; it is not an exact count of the whole parse. The probe has Theta(E) added dependency entries at fixed key length, not quadratic peak memory. Those self-dependencies are discarded at final provenance projection. v0.9.0 already paid source parsing, config copying, value calculation and explicit replay; it did not have these dependency-table scans.

The source-backed bound and public reachability were independently cross-reviewed. One base process and one head process each run two warmups and five timed parses per scale, with result destruction outside the timer. These uncalibrated scale diagnostics use median milliseconds:

| Extra variables | Source bytes | v0.9.0 (ms) | Merged themes (ms) |
| ---: | ---: | ---: | ---: |
| 0 | 56 | 0.2308 | 0.6864 |
| 64 | 1,141 | 0.4484 | 1.1064 |
| 256 | 4,405 | 1.2374 | 2.8325 |
| 1,024 | 17,461 | 3.7070 | 14.1641 |
| 4,096 | 69,685 | 15.4135 | 151.2289 |

At E=4,096, 69,685 source bytes trigger at least 25,163,776 retain predicates before update-assignment scans. Source byte and 64-color limits do not remove this higher-order term. Base, Forest and Neutral share the same ownership algorithm; the timing experiment here covers Dark only.

Default scoped Redux materialization exists in v0.9.0 and does not execute this staged program. Explicit typed-theme requests also rebuild immutable Engine compatibility state per request; that is another separately measurable lane. Neither finding justifies attributing ordinary default SVG regression to per-request typed-theme compilation.

## XY Chart disposition

The exact benchmark SVGs were recovered from the frozen release libraries and their SHA-256 values match original preflight. Both contain 90 elements. Only the stylesheet text differs: five Neo filter declarations change from none to drop-shadow. Labels, geometry and all element attributes/text outside that style element are identical. The fixture has no matching Neo node/cluster/icon-shape targets for those changed selectors. This is source/DOM evidence, not a new browser visual-admission claim. Preserve the intended CSS fix; the overall SVG regression is already reproduced by the same-output cases above.

## Refactor direction and exit gates

1. Make the shared terminal data flow the primary workstream. Extend the existing immutable SvgPipelineBuffer/validated-facts ownership rather than adding a generic proof framework. Use one compact reference/edge representation and consuming report transfers. Measure allocation and XML/reference work separately; a flat arena alone is not presumed to recover the entire parse cost.
2. Target elimination of parse-after-generation for owned output through a structured XML writer whose actual events generate validation/reference facts. Keep final admission at its current public boundary. Raw HTML/math fragments use the existing parser; arbitrary string mutation/postprocessors invalidate generation facts and fall back to the complete validator. Do not write a second XML parser or trust family/model counts as actual SVG proof. Start with one representative family to establish the contract and measured benefit before migrating the shared emission boundary.
3. Redesign bounded serialization around one operation-owned ceiling/reservation state and explicit cancellation observation. Remove repeated general-policy work and redundant fragment admission while preserving exact serialized-byte bounds, bounded memory growth, sticky first failure, UTF-8 cancellation intervals, XML escaping and disjoint projected-byte accounting.
4. Repair classic provenance as a separate structural change: retain one source-backed assignment interpreter, separate values from authority, seed compact provenance only on actual owner-bearing reads, and delete exact/prefix descendants in proportion to the affected subtree. Unused z variables must remain in effective config without creating useless derivative self-nodes. Preserve JavaScript `||` / `??` branch and assignment semantics, shallow replay, same-value/different-owner behavior, scoped transitions and color-error order.
5. Keep Class canonical-slot propagation and report moves as secondary owners. Do not substitute these local changes for recovery of the shared terminal and serialization costs.

For each candidate, use an isolated causal diff, affected cargo nextest tests, cargo fmt, Clippy, cross-family SVG/DOM and resource/cancellation/error gates, plus strict verification as the blast radius requires. Latency admission needs fresh balanced A/A calibration per binary and power-sized AB/BA confirmation of at least eight pairs under the existing relative/absolute gate. The provenance repair additionally needs an exact counter or scale test proving removal of the quadratic term. Unbounded or incomplete-key caches, skipped validation, resource relaxation and fixture allowlists are not accepted routes.

## Local evidence and replay

All exploratory source, commands, output identities and raw samples remain under `target/bench/experiments/theme-v090-20261015/`. The base checkout is `E:/Rust/merman-bench-20261015/base-v090`; the base build is its sibling build-base directory. Byte-verified copies of both benchmark executables and matching PDBs are retained under the experiment frozen/ directory, with a frozen-builds.json receipt, so later candidate builds cannot erase this baseline. The tracked diff for this checkpoint contains documentation only. No commit or push was performed.

```powershell
python tools/bench/compare_self.py --base-dir E:/Rust/merman-bench-20261015/base-v090 --head-dir E:/Rust/merman --base-label v0.9.0 --head-label merged-b9844db7b --base-target-dir E:/Rust/merman-bench-20261015/build-base --head-target-dir E:/Rust/merman/target --base-features svg --head-features svg --base-toolchain 1.95.0 --head-toolchain 1.95.0 --preset long --evidence-mode diagnostic --pairs 3 --filter 'end_to_end/(class_medium|journey_medium|class_tiny)' --out target/bench/experiments/theme-v090-20261015/replay.md --json-out target/bench/experiments/theme-v090-20261015/replay.json
```

Replay must use a clean checkout of the measured revision. The local documentation branch has the same production revision but will be dirty until the checkpoint is committed; do not silently use allow-dirty as an admission receipt.

| Evidence file | SHA-256 |
| --- | --- |
| experiment.yaml | `a50fbcde32400d41700501fa795416b956eab5d66aa2d328fa6bcf3a76652ade` |
| comparable-end-to-end.json | `bffa39e8d085a83fae161724ae5cb0a095b2807e0f389449777a06ab06c4d1ad` |
| stages.json | `5ad5c1596a148ab0f91a75a24f2f259c9695fa2385d9559f7caf290d099ed001` |
| terminal-head.jsonl | `3199477f506324718d254adb475cf91990f958ccacc6937a803edb3f32d34d9a` |
| hash-head.jsonl | `5aae354c24666dc933f965dc7130a2a7f667ddf95a2198e0b2ca75d5d99ab667` |
| staged-base.jsonl | `bbf0b7d85f7934c2b9848dd80349b5c61b7980d30e610c762cabcefb670ddad5` |
| staged-head.jsonl | `ae1fe071318dc26130693929c5b7b4212cc87ed833cbed18b77f75579677a71a` |
| xychart-output-diff.json | `b0d01bde65a79fb17ce0e0e59b0606548315034019f58dcc4cc75778a438f54a` |
| frozen-builds.json | `5725ed3918e5d0c4055ac77894791b31d5dc7a4151dbb3e025a50ea5aad46d31` |
