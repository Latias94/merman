# Repository simplification audit

Date: 2026-10-29. Base: `0ce21f1cc`.

Six Codex review lanes covered documentation, verification tools, parser/preprocessing,
SVG rendering, the Rust facade/shared bindings, and ASCII/graph layout. Follow-up reviews
examined the Web/Node boundaries and checked another lane's changes. This is a source-based
audit with scoped implementation and executable verification, not an exhaustive security
certification or a platform release qualification.

The review used the selected Mermaid sources and existing callers to ask three questions:
does an abstraction still have distinct consumers, does a check establish a unique contract,
and does a document still own information that is useful outside its original agent session?
File length alone was not a reason to split a module.

## Implemented

| Area | Removed responsibility or duplication | Preserved contract |
| --- | --- | --- |
| Documentation | 65 obsolete or duplicate files, including completed workstream administration, superseded session summaries, and duplicate publishing guides; repeated README tables and showcase | The 11 affected workstreams retain their design and verification evidence. Unique rationale was moved before deleting its duplicate owner. |
| CI/scripts | ADR filename/title checks, exact Markdown assertions for fuzzing/Homebrew examples, six retired-import probes, and the obsolete PowerShell CLI packager | Manifest/artifact checks, package exclusions, private-module consumer compilation, real install checks, and workflow permissions |
| Playground | A historical 15-entry validation-migration ledger and its standing existence/description test | The build-graph command retains all 14 actual behavior-test files. Its historical evidence link now points to the original Git revision. |
| Mermaid reference verification | TypeScript layout-registration matching, ELK string-array expansion, and the unused generic extraction entry point | Reviewed inventory structure, source digests, installed-package integrity, and actual Mermaid diagram metadata |
| Preprocessing | The `PublicParse`/`Render` routing enum and alias chains after both paths converged; redundant successful-result packaging | Public APIs, strict/lenient distinctions, original-source mappings, cancellation, and editor failure evidence |
| YAML frontmatter | A text-level nesting scan that counted brackets inside comments and scalar text | Parser input bounds, parser recursion protection, actual collection depth, alias/materialization limits, and small-stack rejection |
| Architecture SVG | A single-implementation model-access trait, borrowed projection types, custom iterators, and request/timing forwarding wrappers | Typed model ownership, declaration/filter order, accessibility, geometry, and SVG templates |
| ER layout | Mutually exclusive forwarding functions, a single-variant authority enum, an impossible missing-seed fallback, and repeated layout selection | Endpoint validation, admission order, operation seed, typography, and work accounting |
| Facade/bindings | An impossible internal `Option`, four unreachable branches, duplicated native-font operation forks, and geometry-specific font selection | Public empty-input behavior and independent control/budget/font state for each output |
| ASCII | An empty label helper, its calls, and unused options threaded through shape painters | Existing unified label emission, transformations, and canvas writes |
| COSE-Bilkent | Duplicate simulation construction and layout sequencing; string IDs stored in simulation nodes | Upstream numerical order, duplicate-ID/edge behavior, endpoint errors, and exact work boundaries |
| Graphlib | Rebuilding and cloning the full node-order index for every multi-node SCC | Node insertion order, cycle ordering, singleton/self-loop behavior, and graph traversal |
| Sequence | An orphan text-emission source file not registered by any Rust module | Active prepared-label emission |

For graphlib, let `V` be the number of nodes and `C` the number of multi-node SCCs.
The removed index construction cost was `O(C * V log V)`, plus repeated node-ID clones.
It is now one lazily constructed borrowed index, `O(V log V)` when needed, with
`O(V)` retained index storage. This describes the changed term, not the complexity
of the entire cycle algorithm or a measured latency improvement. Acyclic graphs
and singleton-only components do not construct the index.

## Follow-up priorities

These findings remain unmodified. They are ordered by the value of clarifying ownership and
behavior before doing further mechanical cleanup.

1. **Unify ordinary and controlled parsing only after specifying cancellation behavior.**
   `crates/merman-core/src/presentation/parse_pipeline.rs` still has separate
   `parse_render_model_controlled_in_context` and `parse_model_in_context` flows.
   The ordinary wrapper converts cancellation to `Error`, which its lenient branch can
   suppress into an error diagram; the controlled route propagates cancellation outside
   parse errors. Static control flow establishes the difference, but a dedicated ordinary
   public-entry regression is needed before selecting a shared kernel.

2. **Check multiple-init merging against the pinned Mermaid implementation.**
   `detect_init_controlled` in `crates/merman-core/src/preprocess/mod.rs` moves each
   directive's `config` into the family namespace before merging. Mermaid's `detectInit`
   in `packages/mermaid/src/utils.ts` merges all directives before assigning `results.config`
   to the family key. Mixed `config` and explicit family keys can therefore differ.
   Build an upstream-backed example before changing existing configuration behavior.

3. **Make CLI native-font diagnostics observe the executing operation.**
   `crates/merman-cli/src/render/execute.rs` forks a font context before the facade
   forks again, then reads `selected_faces()` from the earlier context. The shared
   bindings no longer perform this extra fork. Check actual CLI diagnostics separately
   for Mermaid and raw SVG before choosing one owner for both paths.

4. **Reuse validated structure when prepared-math projection leaves bytes unchanged.**
   `crates/merman-render/src/svg/pipeline/mod.rs` discards the boolean returned by
   `BuiltinSvgStage::PreparedMathProjection` and validates XML again even on its
   explicit no-change path. `preset.rs` already supplies that outcome. Preserve
   opaque/custom postprocessor validation, reference admission, and cancellation;
   byte identity is the useful proof boundary.

5. **Remove unused metadata construction from data-only transports.**
   `execute_typst_operation` in `crates/merman-typst-plugin/src/lib.rs` calls
   `execute_once` and immediately discards the metadata. UniFFI's
   `binary_operation_output` and `string_operation_output` also drop it after
   conversion. Evaluate existing `execute_once_data`/`execute_data` paths rather
   than adding another transport executor.

6. **Consolidate the performance corpus parser and revisit unconditional clean builds.**
   `tools/bench/freeze_perf_baseline.py::_validate_corpus` duplicates the lane schema
   in `corpus_utils.py`, then later calls the shared loader. Keep freeze-specific
   canonical-path and content-identity checks at the freeze boundary.
   `run_native_memory.py::discover_executable` unconditionally cleans the bench profile,
   whereas `compare_self.py` does so for its shared-target freeze case. Verify the
   evidence requirement before changing this behavior; do not replace it with new
   cache/provenance machinery.

7. **Remove unreachable FCoSE constraint state and test-only alternate render paths.**
   `AxisConstraintRuntime::fixed_nodes` in `crates/manatee/src/algo/fcose/mod.rs`
   is always empty and has no population path. Its displacement method also receives
   unused arguments. In ASCII, `relation_graph/model.rs` retains test-only
   `from_sections`/`box_content` construction beside the production deferred path.
   Move tests onto the production path before deleting the alternate implementation.

8. **Retire completed administrative records without erasing engineering evidence.**
   The WASM slimming plan still says `active` while its workstream is closed.
   ASCII architecture deepening says all tasks are complete and `NO ACTIVE TASK`
   while its JSON remains active. The release operator guide also includes completed
   0.9.0 execution checklists already recorded in `PUBLISH_ORDER.md`. Apply the same
   content-ownership test used in this cleanup instead of adding more status indices.

9. **Remove unused work in the Playground and Node wrapper before adding caches.**
   `playground/src/runtime/render-coordinator.ts` calls `collectSvgPlan` again for themed
   previews, while production consumers only retain or copy the result. Confirm the
   whole UI data flow, then remove the unused preview field and operation; the SDK plan
   API and recipe-validation calls have separate uses. In
   `platforms/node/src/errors.mjs::encodeTransportJson`, encoding a controlled copy
   is followed by parsing and rebuilding an object whose result is discarded. Separate
   bounded wire validation from object materialization while retaining depth, token,
   member, string, and UTF-8 limits. Native transport validation remains necessary for
   callers that bypass the JS SDK.

Other bounded opportunities include borrowing Architecture edge geometry instead of copying
six floats per edge, computing ASCII group depths once with explicit work-accounting semantics,
and reducing tests that freeze internal callback batch shapes rather than observable budgets.

## Independent review

Two read-only cross-reviews examined another lane's parser/render/binding and
ASCII/manatee/graphlib changes. Neither identified a blocking regression. These reviews
supplement the executable checks; they do not claim to have executed them independently.
The following nonblocking test gaps remain explicit:

- Prepared native-font output cancellation racing a measurement-limit failure lacks a
  direct error-priority test.
- The new source-map test checks Unicode mapping, but does not directly combine entity
  replacement mapping with the comment-preserving buffer's `maxTextSize` fallback.
- Named-graph COSE-Bilkent lacks a direct mixed invalid-model/insufficient-budget test.
  Its two public surfaces now share an implementation, so their comparison cannot detect
  common drift; retain independent upstream coordinates and add a fixed non-forest
  coordinate/work reference if this algorithm is changed further.

## Boundaries deliberately retained

- Prepared outputs must re-admit source/model facts against the current request's limits.
- Child controls isolate terminal state between independently requested outputs.
- Raw options and merged options are different validation boundaries.
- SVG validation at opaque/custom transformation boundaries is still necessary.
- FFI pointer, ownership, and lifecycle checks protect a distinct trust boundary.
- DOMPurify policy generation still needs its narrow literal-list reader.
- Diagram families remain independently selectable; optional layouts and math remain explicit.
- No baseline, comparator normalization, public API, ABI, feature recipe, or lockfile was changed.

## Verification

Cargo work runs sequentially with two build jobs and two test threads, reusing `target/`.

| Check | Result |
| --- | --- |
| Repository Python contracts, Python 3.13 | 832 passed |
| Changed workflow actionlint and offline zizmor | Passed; no high-severity findings |
| Final core all features, including YAML follow-up | 2,344 passed |
| Independent core lean / presentation / compatibility / editor selections | 42 / 50 / 50 / 65 passed |
| Architecture/ER owning render tests and typed family API, all features | 381 passed |
| ER layout without ELK | 4 passed |
| Facade render-operation and prepared-source tests with PNG/JPEG/PDF and system fonts | 59 passed |
| Explicit native-font binding, prepared-source, and geometry tests | 17 passed |
| Manatee and graphlib suites | 228 passed |
| ASCII all features | 1,397 passed |
| xtask suite | 690 passed |
| Release SVG comparison: all 37 entries, structure/parity/parity-root, three decimals | Passed with existing exact diagnostic receipts |
| Mermaid reference verification | Passed |
| Production package exclusions and independent consumer boundary | Passed with every producer feature; both private imports remain inaccessible |
| Independent facade/shared-binding feature checks | 8 passed: facade empty/ASCII; bindings empty/SVG/native-fonts/native-fonts+PNG/native-fonts+JPEG/native-fonts+PDF |
| Playground build-graph suite | 52 passed; 3 could not resolve the locally unbuilt `@mermanjs/web` package |
| Workspace Rust formatting and diff whitespace | Passed |
| Changed Rust crates: Clippy all features/all targets, warnings denied | Passed for core, render, facade, shared bindings, ASCII, manatee, graphlib, and xtask |

The first facade export run without any enabled font source correctly returned
`FontSourceUnavailable`; the font-capable test recipe above passed. The first complete Python
run used Apple's Python 3.9 in a nested shell; rerunning with Python 3.13 consistently on `PATH`
passed. Neither result justified a production fallback.

The YAML follow-up initially exposed two tests tied to the removed scanner's exact wording.
The parser still rejected both hostile inputs on their small-stack threads. Those tests retain
the typed frontmatter-error and small-stack requirements while allowing the owning YAML parser's
recursion-limit diagnostic.

The complete SVG comparison used the existing Linux owner command:

```console
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 \
  --diagnostic-browser-text-layout --report-root
```

The three Playground failures occur in unchanged source-graph tests while resolving package
exports. The local workspace dependency points to `platforms/web/packages/full`, whose built
`dist/package-entries/full.d.ts` is absent. No test skips, substitute declarations, or fake
package artifacts were introduced. The Web/Pages owner remains responsible for the complete
package build and these dependent checks.

Cross-platform binary distribution and browser-runtime qualification remain with their existing
CI owners; no publication is part of this audit.
