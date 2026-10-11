# Deep diagram lifecycle characterization

This note records the U1 harness for the [approved lifecycle refactor plan](../plans/2026-10-10-2342-refactor-deep-diagram-lifecycle-plan.md). Its subject is whether a host can complete parsing or receive a reported failure on a 2 MiB worker, dispose of library-owned results, and perform another small operation. It does not classify the issue as a security vulnerability or claim whole-pipeline linear complexity.

## Baseline and evidence provenance

The plan's investigation baseline is workspace commit `aa88d63e2`, Merman `0.8.0`, Rust `1.95`, and Mermaid `12.1.0` commit `21f72f07ea22c0af48a3149c550654e80d8e40cb`. The locked Mermaid revision comes from `tools/upstreams/REPOS.lock.json` and `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`. The reference checkout itself can report an older version; source inspection uses `git show` at the locked commit without changing that checkout.

The following observations come from the prior investigation summarized in the plan. They are not fresh runs of the committed U1 harness, and their depths are characterization inputs rather than acceptance limits.

| Family/path | Prior observation |
| --- | --- |
| Flowchart | A 5,000-level facade semantic path aborted; a 10,000-level typed core path and malformed core input aborted. |
| State | A 5,000-level typed parse reported success before disposal aborted; malformed 10,000-level input aborted. |
| ER | Valid 3,000-level nested subgraphs aborted. |
| C4 | Missing the outer closer at 15,000 levels aborted; a valid case timed out. |
| Block | Missing a closer at 10,000 levels aborted; a valid 3,000-level typed case reported success before timeout. |
| Railroad EBNF | A 5,000-postfix `?` chain, around 5,030 source bytes, aborted. |
| Mindmap | Typed parse/drop at 3,000 levels passed; compatibility JSON parse reported success before disposal aborted. |
| Treemap / Ishikawa | Typed parse/drop at 5,000 levels passed. Deep clone safety was not established by those observations. |

A timeout is a cost observation, not evidence of stack overflow. The prior Mindmap source was about 9 MB; the prior Treemap/Ishikawa sources were about 12.5 MB. Their direct Engine paths did not pass through the facade's default 2 MiB source admission policy.

## Input construction

The deterministic generators live in `crates/merman-core/tests/deep_lifecycle.rs`. Brace/end-delimited families use unindented statements, so the generator does not add an avoidable quadratic indentation payload. Indentation remains necessary for Mindmap, Treemap, and Ishikawa because it encodes their hierarchy. The harness reports the generated source's actual byte count before parsing.

Source inspection at the locked Mermaid commit establishes the generator forms:

| Family | Locked source | Generated form |
| --- | --- | --- |
| Flowchart | `packages/mermaid/src/diagrams/flowchart/parser/flow.jison` | `subgraph n0`, nested documents, then `end`; valid leaf `leaf[Leaf]`. |
| State | `packages/mermaid/src/diagrams/state/parser/stateDiagram.jison` | `state s0 {`, nested documents, then `}`; valid leaf `leaf`. |
| ER | `packages/mermaid/src/diagrams/er/parser/erDiagram.jison` and `subgraph.spec.js` | `subgraph G0`, nested documents, then `end`; entity `LEAF`. |
| C4 | `packages/mermaid/src/diagrams/c4/parser/c4Diagram.jison` | `Boundary(b0, "B0") {`, nested boundary statements, then `}`; `System(leaf, "Leaf")`. |
| Block | `packages/mermaid/src/diagrams/block/parser/block.spec.ts` | `block:b0`, nested blocks, then `end`; leaf `leaf`. |
| Railroad EBNF | `packages/mermaid/src/diagrams/railroad/parser/ebnfParser.ts` | `railroad-ebnf-beta` and `rule ::= "a"` followed by repeated `?` postfixes. Upstream postfix transformation wraps the previous node. |
| Mindmap | `packages/mermaid/src/diagrams/mindmap/parser/mindmap.jison` | An unindented root followed by increasing two-space indentation. |
| Treemap | `packages/mermaid/src/diagrams/treemap/parser.ts` | Increasing indentation of quoted section names, then a quoted leaf with value `1`. |
| Ishikawa | `packages/mermaid/src/diagrams/ishikawa/parser/ishikawa.jison` | An indented root followed by increasingly indented cause names. |

Malformed generators retain every completed inner group and omit exactly the final outer closer. Existing shallow family tests define the strict reported-error outcome; the lifecycle test does not replace semantic or error-span parity checks.

## Process and phase boundaries

Each parent test launches its own integration test executable with `--exact lifecycle_child --nocapture` and a local case selector. The selected child creates a worker with `stack_size(2 * 1024 * 1024)`. Only that worker performs the dangerous lifecycle. The facade uses its separate `facade_lifecycle_child` selector.

Child stdout and stderr are redirected to separate files in a unique diagnostic directory under the system temporary directory. This avoids filling a pipe while the parent polls for exit. The parent records exit status, elapsed time, timeout state, and any process-observation error in `status.txt`, then prints the file paths and captured output. Diagnostic files are retained for evidence collection.

Each child has a fixed 60-second parent timeout. A timed-out child is killed and reaped before its parent assertion fails. Process-observation failures also trigger kill/reap. An aborted child fails its parent test while leaving the parent process available for subsequent tests.

Markers are explicitly flushed. Their elapsed times are measured from the start of the worker:

- `parse`: before Engine/facade parsing and after success or a reported error.
- `model`: returned typed/model boundary, or `not-returned` on a reported error.
- `export`: requested only by the small facade projection/export smoke case in U1.
- `clone`: ordinary returned typed-model clone, except the independent State drop probe.
- `clone-drop` and `drop`: before and after ordinary result disposal.
- `after-small-operation`: before and after another small parse and its disposal using the same Engine or Renderer.

Typed ownership probes record `export=not-requested` so compatibility JSON does not obscure the typed boundary. The Mindmap JSON drop probe records clone/export as `deferred-to-U2`; raw `Value` disposal is deliberately measured independently. U1 does not provide a second JSON writer or claim a maintained deep export exists before U2. The small facade case projects compatibility JSON, exports it with serde, clones that JSON, disposes of it and the semantic artifact, then prepares another small diagram.

Railroad's 5,000-postfix case accepts either a returned model with a completed lifecycle or the existing family-depth rejection after the construction-depth repair. A syntax-valid expression exceeding its local implementation boundary is allowed to report that error. The small four-postfix case must parse successfully.

## Test selectors

The core target is gated by `all-diagrams`. The facade smoke target is gated by `diagram-flowchart` and exercises `Renderer::prepare_semantic` without requiring an output feature. The child selectors are ordinary no-op tests when their local environment selector is absent.

Active U1 tests:

- Core: `small_lifecycle_cases_complete_on_host_stack` runs all 14 core scenarios sequentially at depth 4.
- Facade: `small_facade_semantic_lifecycle_completes_on_host_stack` exercises the maintained strict facade semantic path at depth 2.

Deep baseline assertions are initially ignored and must be selected individually. Enable a family's assertions when its repair satisfies the lifecycle contract.

| Exact parent selector | Path and logical depth |
| --- | --- |
| `deep_flowchart_valid_10000` | Typed Engine, valid 10,000 groups, clone/drop. |
| `deep_flowchart_missing_outer_end_10000` | Typed Engine, missing outer `end`, 10,000 groups. |
| `deep_state_typed_drop_5000` | Typed Engine, valid 5,000 groups, independent ordinary drop. |
| `deep_state_typed_clone_drop_5000` | Typed Engine, valid 5,000 groups, ordinary clone/drop. |
| `deep_state_missing_outer_brace_10000` | Typed Engine, missing outer `}`, 10,000 groups. |
| `deep_er_valid_3000` | Typed Engine, valid 3,000 groups, clone/drop. |
| `deep_er_missing_outer_end_3000` | Typed Engine, missing outer `end`, 3,000 groups. |
| `deep_c4_missing_outer_brace_15000` | Typed Engine, missing outer `}`, 15,000 boundaries. |
| `deep_block_valid_3000` | Typed Engine, valid 3,000 blocks, clone/drop. |
| `deep_block_missing_outer_end_10000` | Typed Engine, missing outer `end`, 10,000 blocks. |
| `deep_railroad_ebnf_postfix_5000` | Typed Engine, 5,000 `?` postfixes. |
| `deep_mindmap_json_drop_3000` | Direct Engine compatibility JSON, 3,000 indented descendants, independent drop. |
| `deep_treemap_typed_clone_drop_5000` | Direct typed Engine, 5,000 sections, clone/drop probe. |
| `deep_ishikawa_typed_clone_drop_5000` | Direct typed Engine, 5,000 causes, clone/drop probe. |

Use sequential Cargo work, the shared `target`, and bounded jobs. The ordinary U1 gates are:

```text
cargo nextest run --locked -p merman-core --features all-diagrams,test-support --test deep_lifecycle -j 2
cargo nextest run --locked -p merman --test deep_lifecycle -j 2
```

After building the appropriate core integration artifact, an explicit ignored baseline case can be run directly without rebuilding:

```text
target/debug/deps/deep_lifecycle-<core-artifact-hash>.exe --exact deep_flowchart_valid_10000 --ignored --nocapture
```

Select the core artifact from Cargo's build output, since the facade target has the same binary name. Record the build command/profile and current workspace revision with the run. Select one named parent case when reproducing a particular failure; keep child runs serial when collecting multiple baseline cases. The prior valid Block timeout does not need to be repeated before U7 solely to establish another timeout under memory pressure.

## Fresh verification

On 2026-10-11, the root implementer reported these fresh baseline checks before running the new harness:

| Command | Reported result | What it establishes |
| --- | --- | --- |
| `cargo nextest run --locked -p merman-core --features all-diagrams,test-support --test nesting_depth_limits -j 2` | All 7 tests passed. | Ordinary existing nesting behavior at `MAX_DIAGRAM_NESTING_DEPTH + 2` (258); this runner-thread gate does not establish the 2 MiB host lifecycle. |
| `target/debug/xtask.exe verify-lalrpop-parsers`, after the root's serial xtask build with `-j 2` | Generated-parser verification passed. | Existing generated grammar artifacts agree with the baseline grammars. |

The U1 implementation agent did not run Cargo builds or tests. The root implementer subsequently ran the new harness on Windows 11 using the shared debug target and reported both active gates passing:

| Gate | Root-reported result |
| --- | --- |
| Core `deep_lifecycle`, with `all-diagrams,test-support` and `-j 2` | 2 active tests passed; 14 deep tests ignored. The small parent exercised all 14 scenarios. |
| Facade `deep_lifecycle`, with `diagram-flowchart` and `-j 2` | 2 active tests passed, including the maintained facade semantic lifecycle. |

The root then selected the ignored core baseline assertions with this exact command:

```text
target/debug/deps/deep_lifecycle-0938f9879f2f5dd0.exe --ignored --nocapture --test-threads=1 --skip deep_block_valid_3000
```

That invocation ran 13 parent cases sequentially in 29.01 seconds. Twelve children exited with `0xc00000fd`; the malformed ER child completed successfully. The parent runner completed normally despite those child failures. All 13 diagnostic status files report `timed_out=false`, `timeout_ms=60000`, and `process_error=None`. No new timeout was observed. The valid Block 3,000-level case was intentionally skipped before its repair to avoid repeating the prior cost/memory-pressure observation.

The following measurements were read from the retained `stdout.txt` and `status.txt` files under `C:/Users/Frankorz/AppData/Local/Temp/merman-core-deep-lifecycle-88484-{case}-{depth}-{sequence}`. All cases used a 2,097,152-byte worker stack and `build_mode=debug`. Typed rows used `Engine.parse_diagram_for_render_model_sync`; the Mindmap row used direct `Engine.parse_diagram_sync`. Their logged profile was `direct-engine-strict`.

| Exact parent selector | Source bytes | Parent elapsed ms | Child exit | Last observed phase |
| --- | ---: | ---: | --- | --- |
| `deep_block_missing_outer_end_10000` | 158,902 | 121 | `0xc00000fd` | `parse=begin` |
| `deep_c4_missing_outer_brace_15000` | 442,809 | 171 | `0xc00000fd` | `parse=begin` |
| `deep_er_missing_outer_end_3000` | 55,901 | 246 | `0` | Parse reported its family error; `drop=done` and `after-small-operation=done`. |
| `deep_er_valid_3000` | 55,905 | 245 | `0xc00000fd` | `parse=begin` |
| `deep_flowchart_missing_outer_end_10000` | 188,910 | 141 | `0xc00000fd` | `parse=begin` |
| `deep_flowchart_valid_10000` | 188,914 | 16,176 | `0xc00000fd` | `parse=begin` |
| `deep_ishikawa_typed_clone_drop_5000` | 25,063,911 | 4,046 | `0xc00000fd` | Parse/model completed at worker elapsed 3,980 ms; `clone=begin`. |
| `deep_mindmap_json_drop_3000` | 9,019,903 | 1,489 | `0xc00000fd` | Parse/model completed at worker elapsed 1,423 ms; `drop=begin`. |
| `deep_railroad_ebnf_postfix_5000` | 5,034 | 673 | `0xc00000fd` | `parse=begin` |
| `deep_state_missing_outer_brace_10000` | 158,909 | 160 | `0xc00000fd` | `parse=begin` |
| `deep_state_typed_clone_drop_5000` | 78,911 | 171 | `0xc00000fd` | Parse/model completed at worker elapsed 110 ms; `clone=begin`. |
| `deep_state_typed_drop_5000` | 78,911 | 170 | `0xc00000fd` | Parse/model completed at worker elapsed 108 ms; `drop=begin`. |
| `deep_treemap_typed_clone_drop_5000` | 25,043,913 | 5,184 | `0xc00000fd` | Parse/model completed at worker elapsed 5,119 ms; `clone=begin`. |

The root reported these exits as child stack overflow aborts; sampled stderr for the valid Flowchart, State drop, and Ishikawa clone probes also explicitly reported stack overflow. These markers localize the public lifecycle phase; they do not establish the precise internal function or replace a stack trace. Worker elapsed times include source construction and Engine creation; they are not standalone parser benchmarks. Parent times include process launch, worker execution, and process observation. The normal parent completion confirms the isolation mechanism remains usable when its child aborts.

The fresh Treemap/Ishikawa failures occurred during ordinary typed clone after parsing returned. This extends the prior evidence without changing the prior parse/drop successes into failures. U1 uses two indentation spaces per level, producing roughly 25 MB for these sources rather than the prior roughly 12.5 MB sources. The fresh Mindmap probe independently confirms raw compatibility JSON disposal failure; clone/export remain explicitly deferred rather than measured as passing.

Malformed ER at 3,000 levels did not reproduce an abort with this generator. It reported `unexpected end of input`, disposed of the reported error, and completed a subsequent small operation. Its passing malformed baseline does not waive the independently failing valid ER construction case.

U2 will add managed JSON clone/export coverage. Later units will add cancellation/deadline, projection-failure and facade budget cleanup cases, bounded-family accepted-edge coverage, and final repaired-family results. Those cases are outside this characterization-only U1 baseline.

## Canonical ownership and requested output growth

The integrated debug implementation was measured with the existing five family-local accounting
tests in `merman_core-ede88a4848860763.exe`. All five passed. Retained output lives under
`<system-temp>/merman-lifecycle-growth-xzmrtep3`; each log names its exact unit-test selector.
These counters describe construction or retained records, rather than a whole-pipeline benchmark.

| Block depth | Source bytes | Canonical records | Child IDs | Requested legacy JSON bytes |
| ---: | ---: | ---: | ---: | ---: |
| 16 | 335 | 18 | 17 | 17,229 |
| 32 | 671 | 34 | 33 | 58,333 |
| 64 | 1,343 | 66 | 65 | 212,733 |

Block retains each logical block once while preserving the nested legacy `blocksFlat` output.
The output still repeats descendants. Increasing output size is therefore separate from the
removed canonical subtree copies and does not justify changing the JSON representation.

| Grammar workload | Source bytes | Action records | Action links | Additional records |
| --- | ---: | ---: | ---: | --- |
| Class, 1,024 flat declarations | 11,191 | 1,024 | 1,023 | No class-ID carrier records |
| Class, 1,024 declarations in one namespace | 11,207 | 1,027 | 1,026 | 1,024 class-ID records and 1,023 links |
| Class, 1,024 nested namespace declarations | 19,394 | 3,073 | 3,072 | One class-ID record |
| Sequence, 1,024 flat messages | 15,376 | 3,072 | 3,071 | No second action carrier |
| Sequence, 1,024 nested fragments | 14,281 | 2,051 | 2,050 | No second action carrier |
| Sequence, 1,024 nested two-branch fragments | 24,533 | 4,101 | 4,100 | No second action carrier |

The Class and Sequence tests also passed at sizes 8 and 128. Each event is attached once; the
tests independently replay events and check source order. Required qualified namespace strings
remain a separate semantic/output cost.

The Flowchart accounting test passed widths 1, 32, and 1,024, with one stored statement and two
construction steps per declaration. The ER test passed depths 32, 128, and 512, with `depth + 1`
action records, `depth` membership candidates, and `depth` retained completed memberships.
These are assertions over the actual private carriers, rather than timing-based complexity claims.

## Pinned-source Block correction

The flat Block conversion exposed a pre-existing Rust semantic discrepancy. Mermaid 12.1.0
(`21f72f07ea22c0af48a3149c550654e80d8e40cb`) stores the original block object in its map and
uses that same object in parent children. Later style, class, type, and label updates are visible
through both references. The former Rust `clone_block_tree_nonrecursive` froze the nested copy
when a parent completed.

The source authority is [`blockDB.ts`](https://github.com/mermaid-js/mermaid/blob/21f72f07ea22c0af48a3149c550654e80d8e40cb/packages/mermaid/src/diagrams/block/blockDB.ts):
style/class updates at lines 61–87; insertion and duplicate type/label handling at 157–175;
child references at 186–191; and direct `getBlocks`/`getBlocksFlat` access at 305–337.
The [Block grammar](https://github.com/mermaid-js/mermaid/blob/21f72f07ea22c0af48a3149c550654e80d8e40cb/packages/mermaid/src/diagrams/block/parser/block.jison)
creates a fresh shallow object for each composite declaration. Replaying a repeated composite
populates that fresh object, while the first registered object's children and columns remain intact.
The retained flat model implements these two rules without historical subtree copies.

A Node probe transpiled the actual pinned TypeScript DB source in memory. Its
`post-completion-metadata` case confirmed identical object identity for nested and flat `A`, with
later circle/type, label, styles, and class updates but the original width and directions. Its
`repeated-composite-columns` case confirmed `G` retains columns 2 and child `A`, while the later
child `B` is still registered in the flat map. Only unrelated imports were stubbed: ASCII label
sanitization, empty config, logging/clear hooks, and an unused shallow space clone. This is a
DB source probe, not a complete browser renderer run. The reproducer and JSONL result are
retained at `<system-temp>/merman-u7-pinned-blockdb-12.1.0-20261011-probe.cjs` and its
`.stdout.jsonl` sibling.

The existing `xtask update-snapshots --diagram block` changed exactly four Rust semantic goldens:
`upstream_docs_block_introduction_to_block_diagrams_001`,
`upstream_docs_block_text_on_links_045`,
`upstream_examples_block_basic_block_layout_001`, and
`upstream_html_demos_block_block_diagram_demos_001`. Each change adds the already-declared
styles to three nested views of `B`. No upstream SVG baseline or comparator policy changed.
The focused tests independently check live metadata and repeated-composite children/columns.

## Integrated family comparison

A fresh debug `xtask` compared 17 affected or bounded families against the committed Mermaid
12.1.0 SVGs using `--check-dom --dom-mode parity --dom-decimals 3`. Block, C4, Mindmap, Ishikawa,
ER, Swimlane, Railroad and its EBNF/ABNF/PEG dialects, TreeView, and Usecase returned success
without the browser-text-layout diagnostic flag. Existing family fixture policies still apply,
including the explicitly named hand-drawn Ishikawa fixture's structure profile.

The unflagged runs for State, Sequence, Class, Treemap, and Flowchart returned their established
DOM differences. The repository CI gate was then run with `--diagnostic-browser-text-layout`.
All five passed verification of the existing exact input, upstream, and local SVG signatures:

| Family | Existing browser-text-layout receipts accepted |
| --- | ---: |
| State | 11 |
| Sequence | 9 |
| Class | 1 |
| Treemap | 14 |
| Flowchart | 61 |

Treemap also accepted its one existing exact parser-diagnostic receipt. That diagnostic result
is not strict DOM parity. The catalog and normalization rules were unchanged; a changed local
signature would fail this gate. Flowchart uses `compare-all-svgs --diagram flowchart` for this
CI check because its specialized command does not accept the diagnostic flag. The other four
use their corresponding `compare-<family>-svgs` command with that flag. Raw command results and
logs are retained under `<system-temp>/merman-lifecycle-family-parity-o6hd4t91`, in `results.json`
and `ci-residual-results.json`. These results establish the existing CI comparison boundary,
not elimination of registered browser/layout differences. ZenUML has no command in this SVG
comparison catalog; its semantic, typed-wire, and facade SVG lifecycle suites cover its change.

## Repaired debug host lifecycles

The integrated debug run `658c1eef-3014-49b7-9d67-37e07914dd6e` completed all fifteen deep
core child cases. Every case used a 2,097,152-byte worker stack, exited with code zero, recorded
`timed_out=false` and `process_error=None`, and completed both ordinary disposal and a subsequent
small diagram. The fixed child timeout was 60 seconds. These are direct strict Engine operations;
large indentation-based sources are intentionally separate from facade source admission.

| Child selector | Logical depth | Source bytes | Parent elapsed ms |
| --- | ---: | ---: | ---: |
| `block-missing-outer-end` | 10,000 | 158,902 | 85 |
| `block-valid` | 3,000 | 46,906 | 55 |
| `c4-missing-outer-brace` | 15,000 | 442,809 | 129 |
| `er-missing-outer-end` | 3,000 | 55,901 | 192 |
| `er-valid` | 3,000 | 55,905 | 200 |
| `flowchart-missing-outer-end` | 10,000 | 188,910 | 98 |
| `flowchart-valid` | 10,000 | 188,914 | 16,394 |
| `ishikawa-typed-clone-drop` | 5,000 | 25,063,911 | 3,981 |
| `mindmap-json-clone-export` | 3,000 | 9,019,903 | 1,677 |
| `mindmap-json-drop` | 3,000 | 9,019,903 | 1,483 |
| `railroad-ebnf-postfix` | 5,000 | 5,034 | 658 |
| `state-missing-outer-brace` | 10,000 | 158,909 | 128 |
| `state-typed-clone-drop` | 5,000 | 78,911 | 149 |
| `state-typed-drop` | 5,000 | 78,911 | 118 |
| `treemap-typed-clone-drop` | 5,000 | 25,043,913 | 5,282 |

The retained status/stdout/stderr files are in `<system-temp>/merman-core-deep-lifecycle-<pid>-<selector>-<depth>-0`.
The parent PIDs for these rows are 2172, 22920, 25168, 43324, 53444, 58920, 67564, 68724,
74504, 82900, 83152, 84320, 87684, 87880, and 91596. Parent times include launch and observation;
worker times include source construction and Engine setup. Neither column is a parser benchmark.
The 5,000-postfix Railroad case reports the established local construction-depth outcome safely.
The passing malformed ER case remains a passing baseline rather than a newly repaired abort.

Additional core/facade suites exercise cancellation after nested completion, a real deadline after
export begins, projection failure, default model-budget rejection, repeated IDs with shallow
canonical depth, overlay/snapshot ownership, SVG/ASCII adapters, and maximum accepted bounded
families. ZenUML's accepted 256-level semantic walk initially exposed a real overflow and was
replaced locally by iterative construction; its parser boundary and bounded public AST remain.

## Integrated debug quality checks

On Windows 11 / Rust 1.95, `cargo nextest run --workspace --all-features --locked -j 2
--no-fail-fast --status-level fail --final-status-level fail` completed run
`658c1eef-3014-49b7-9d67-37e07914dd6e`: all 9,450 tests across 255 binaries passed; seven existing
skips remain. Test execution took 441.088 seconds. The configured all-target/all-feature Clippy
command for core, render, facade, bindings-core, CLI, and xtask passed with `-D warnings` on the
finished production change. `cargo fmt --all --check`, `git diff --check`, and fresh xtask
`verify-lalrpop-parsers` also passed. Cargo builds used `CARGO_BUILD_JOBS=2` throughout the final
runs, separately from nextest's two test jobs.

Earlier integrated run `88b2471d-6af9-4ecc-b3b4-2d35bf5ce0ab` passed 9,447 tests and exposed
the one old Block golden mismatch described above. A subsequent rebuild was interrupted by
disk exhaustion before tests began. Only task-created incremental compiler caches were removed
from verified `target/debug/incremental`; this freed 28.703 GiB and retained sources, fixture
baselines, and test executables. Neither build interruption is recorded as a successful test run.

Feature/public API checks additionally passed: the core infrastructure-only managed JSON suite
(12 tests; run `c4d354ef-7358-4d37-b094-c5412743b761`), reduced State/Treemap/Ishikawa/ZenUML
core compilation, the public native ASCII closure for core/facade/bindings/UniFFI/Wasm libraries,
the reduced ASCII CLI, and wasm32-unknown-unknown ASCII compilation. The Rust example in the
migration guide was extracted, compiled against the migrated core library, and executed; retained
compile/run logs are under `<system-temp>/merman-migration-example-3miiedih`. All-target checking
also compiles the migrated examples and pipeline benchmark. No dependency, manifest, FFI ABI,
or transport-schema revision was needed by this ownership change.

## Release host lifecycles and selected Wasm targets

Release core run `b365f049-7955-43b1-9253-53d356b52f7d` passed all 34 tests across the deep
lifecycle, managed JSON, Block lifecycle, and bounded lifecycle integration binaries in 2.875
seconds. Release facade run `2460ce6c-f02b-4e1e-b3ee-bb078f3c5b83` passed all 13 deep/bounded
lifecycle tests in 0.527 seconds. Both used two Cargo build jobs and two nextest test jobs;
their worker stack assertions remain 2,097,152 bytes.

The preceding core release run passed 33 tests and failed one before creating its worker:
Windows returned `AlreadyExists` while the parent created a retained diagnostic directory.
The affected case passed an exact retry (`e66c8c4b-8f27-4405-a085-271bda8ced40`) without source
changes, followed by the clean full 34-test run above. PID-based names can collide with retained
artifacts when Windows reuses a process ID. The existing core/facade/managed-JSON diagnostic
names now also include an epoch-nanosecond suffix and creation errors display the actual path.
No old diagnostic directory was removed. The interrupted run is not counted as a passing suite.

The explicitly selected extended Flowchart release case completed at 30,000 levels with a
588,914-byte source, a 2 MiB worker, exit code zero, and no timeout. Its parent elapsed time was
14,676 ms against a fixed 180-second limit. It completed typed parse, ordinary clone/drop, and
a subsequent small operation; compatibility export was not requested in this ownership probe.
Logs and status are retained under
`<system-temp>/merman-lifecycle-flowchart-30000-release-9grdnr4r`. This is lifecycle evidence,
not a linear-time parsing claim.

The affected-family `merman-wasm` closure also compiled for `wasm32-unknown-unknown`, without
defaults, with both SVG and ASCII enabled and Flowchart, Swimlane, ER, State, C4, Block, Class,
Sequence, Mindmap, Treemap, Ishikawa, Railroad, ZenUML, TreeView, and Usecase selected. This
compile gate establishes the feature/API closure; it is not a Wasm runtime or browser test.

## Additional completed-fragment cancellation coverage

Debug run `7549b292-3148-4a0f-b6e1-acb4c5e85dc3` passed six named cancellation tests across
three binaries in 7.142 seconds. Three new integration cases use the existing explicit child
processes, 60-second watchdogs, and 2 MiB workers. State and Block project 3,000-level models
and cancel during ascent, with approximately 1,600 levels of completed inner JSON still owned.
The State probe keeps only the outer state record so map iteration order cannot determine its
exit point. ZenUML confirms a complete 256-level first branch and cancels at the second statement
task, after that deep branch enters the managed completed collection. Checkpoint counts are
derived from these confirmed structures and their documented traversal steps. All preserve the
original terminal phase and then process a small diagram.

The other two upgraded cases exercise C4 closure and replay cancellation at 15,000 levels in
explicit 2 MiB workers. Their exact private construction checkpoints retain the existing
derivation, terminal assertions, and subsequent small diagram. These named unit cases use
nextest's per-test OS process isolation rather than the integration harness's separate parent
watchdog. The sixth selected test is the existing State inner-document cancellation regression.

The root additionally ran both upgraded C4 unit selectors directly from the current debug test
executable under an external Python `subprocess.run(timeout=60)` watchdog. Closure and replay
completed with exit code zero, no timeout, and parent times of 83 ms and 7,516 ms, respectively.
The existing unit assertions verify the 2 MiB worker, original terminal, cleanup, and subsequent
small operation. Python's timeout path kills and reaps the child; this verification adds no
production worker or persistent runner framework. Logs and status are retained under
`<system-temp>/merman-c4-cancellation-watchdog-debug-2fq7kq4t`.

## Requirement evidence map

| Requirement | Implementation and evidence |
| --- | --- |
| R1 | Named core/facade deep and bounded lifecycle cases use 2 MiB workers; debug and release results are recorded separately above. |
| R2 | Parser carriers and projection slots retain non-recursive or verified locally bounded ownership through malformed input, partial-completion cancellation, projection errors, deadline expiry, and model-budget rejection. |
| R3 | State/Block/Treemap/Ishikawa canonical relationships use IDs; deep returned model and managed-JSON clone/drop cases use their ordinary destructors. |
| R4 | Source-backed family tests, shallow typed-wire oracles, exact existing family SVG gates, and the Rust migration guide retain the pinned Mermaid contract; the Block correction has separate pinned-source evidence. |
| R5 | Managed compact/pretty writers export deep JSON; generic serde preflights actual emitted containers at 128, independently of model policy. |
| R6 | Flat complexity calculations match the former typed-wire meaning; facade tests preserve default limits, override provenance, suppression behavior, and original sticky operation phases. |
| R7 | Repeated-ID Flowchart cases separate deep syntax from shallow canonical depth; no new global syntax-depth ceiling is present. |
| R8 | Flat families retain their models; Railroad constructed depth and ZenUML/TreeView/Usecase accepted/rejected edges have explicit lifecycle cases. |
| R9 | Carrier, retained-membership, canonical-record, and output-byte growth checks are reported above, separately from required legacy output duplication and remaining parsing/layout cost. |
| R10 | Family-specific grammars, records, replay, editor facts, and render adapters remain the authorities; managed JSON is a narrow owner over the existing value type. |

All eleven active units are represented by these changes. The plan's separately deferred release
selection/publication, Holt upgrade recommendation, generic policy redesign, and independent
layout optimization remain outside this implementation. The evidence does not claim arbitrary
raw `Value` operations, caller-defined recursive models, unlimited allocation, or Wasm runtime
execution.

## Completed review and caller-owned resolution

The actual `ce-code-review` run `20261010-185836-59455179` completed all eight selected local
reviewer lenses and independent finding validation. Its original verdict was `Ready with fixes`,
with one confirmed P2 finding (#1): a caller's writer could observe and latch cancellation, then
return an I/O error that masked the original terminal. The root implementer accepted and applied
#1 inline. A narrow read-only control accessor now replays an already observed cancellation with
its original phase and reason. It does not observe a newly requested cancellation, check the clock,
consume a checkpoint, or change resource-terminal behavior.

The discriminating regression initially failed against the former implementation (run
`8eab3d59-6879-478f-bf99-ec223ea71fd4`). After the fix, all eight managed-JSON tests passed
in run `b7eb91c9-4332-4732-8eb9-9c4fb0e1748b`. The regression covers compact and pretty output,
each with both already observed cancellation and requested-only cancellation followed by I/O
failure. The migration guide states this precedence explicitly.

Two separate read-only caller follow-ups then reviewed only the production fix and the later
test/diagnostic-directory differences. The reliability follow-up closed #1 with no findings or
testing gaps. The testing follow-up closed both original coverage gaps, confirmed the completed
State/Block/ZenUML fragments and deep C4 workers, and returned no findings or remaining testing
gaps. These follow-ups did not rerun or replace the original full review. The original immutable
receipt retains its pre-fix R6/U2/U11 status; caller resolution closes those items with the explicit
follow-up and execution evidence.

Artifacts are retained under
`<system-temp>/compound-engineering-Frankorz/ce-code-review/20261010-185836-59455179/`:
`review.json`, `caller-writer-fix-review.json`, and `caller-test-delta-review.json`. Reviewers did
not run Cargo; the root executed verification. All reviewers used Codex subagents as requested;
different-model independence is not claimed. No actionable finding was skipped or deferred.

## Final checks after review resolution

After applying #1 and completing the independent test-delta reviews, final all-workspace
all-feature run `9cd54652-015e-451e-8ebb-d7a03503ff0d` passed all 9,454 tests across 255 binaries,
with seven existing skips, in 428.249 seconds. This includes the three additional completed-fragment
cancellation cases and the discriminating writer regression. The final configured Clippy command
for core, render, facade, bindings-core, CLI, and xtask also passed with all targets, all features,
and `-D warnings` in 1 minute 26 seconds. Both commands used two Cargo build jobs; nextest used
two test jobs. No production source changed between these gates.

Final release core run `cbb5175f-b10c-4c5c-8994-3146690d1169` passed all 40 selected lifecycle
tests across five binaries in 3.649 seconds, after a bounded two-job release build. Its 1,763
excluded unit tests were outside the explicit selection, rather than ignored lifecycle failures.
This run includes the final writer precedence regression, all three new completed-fragment
integration cases, and both 15,000-level C4 unit cases. The post-directory-fix facade release run
`4cb65355-81a2-4fe9-8fab-9c4c867a231b` had already passed all 13 cases in 0.530 seconds; the final
all-workspace debug gate additionally covers facade behavior after the writer precedence fix.

Both release C4 unit selectors also passed a direct external 60-second watchdog run: closure in
31 ms and replay in 1,197 ms, both exit code zero and no timeout. Logs and status are retained under
`<system-temp>/merman-c4-cancellation-watchdog-release-gha87kzg`. This complements the independently
bounded debug executions recorded above; the checked-in unit harness still uses nextest isolation.

Final infrastructure-only core run `e19da38b-8b3d-4135-8d23-8f05fdff50b2`, without default
features, passed all 13 feature/managed-JSON tests in 0.329 seconds. The selected 15-family
Wasm SVG/ASCII closure was rechecked after the fix and passed in 7.06 seconds. This remains
compilation evidence, not Wasm runtime execution. Earlier native/reduced-family, generated-parser,
formatting, migration-example, canonical-growth, and 17-family CI comparison results remain
applicable: the review fix changes only precedence for an already observed cancellation followed
by writer I/O failure, and leaves successful output, grammars, comparison policies, and examples
unchanged. No required implementation or validated review finding remains unresolved.


## Follow-up scope and API review

A second independent Codex review audited the complete implementation range
`aa88d63e2..542c58038` for architecture, API contracts, correctness, simplicity, and test scope.
The review found no semantic regression or over-broad shared abstraction. It did identify four
owning typed-model serde adapters as unused Rust-wire compatibility code: State's importer and
wire structs, Block's owning serde path, and Treemap/Ishikawa's nested-tree import/export paths.
They were removed together with their round-trip tests. Mermaid compatibility projectors,
canonical flat records, State document projection, and family-specific resource complexity
calculators remain the only production paths.

The review also found two ordering regressions in the new projectors. State now accumulates
managed records in input order before assembling its JSON object; Treemap and Ishikawa insert
`root` at the former field position. A public complexity count now saturates on a serializer that
refuses the generic 128-container boundary instead of panicking; exact deep JSON counts use
`from_json`. The grammar-carrier and Block lifecycle diagnostic directories now include epoch
nanoseconds and report their actual path on creation failure, so retained Windows artifacts cannot
collide on PID reuse.

After this follow-up delta, `cargo nextest run --locked -j2 -p merman-core --all-features` passed
1,856/1,856 tests in 74.493 seconds. The focused migration set passed 21/21 tests, including
5,000-level Treemap/Ishikawa export and Block lifecycle cases. `cargo check --locked -p
merman-core --all-features`, `cargo fmt --all --check`, and `git diff --check` also passed. The
old typed-wire contract is intentionally removed; the migration guide now directs Rust consumers
to canonical records and `compatibility_json()`.
