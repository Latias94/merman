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
