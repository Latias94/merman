# Repository simplification audit

Date: 2026-10-29. First-pass base: `0ce21f1cc`. Follow-through base: `f8bfcc59d`.

Six Codex review lanes covered documentation, verification tools, parser/preprocessing,
SVG rendering, the Rust facade/shared bindings, and ASCII/graph layout. Follow-up reviews
examined the Web/Node boundaries and checked another lane's changes. This is a source-based
audit with scoped implementation and executable verification, not an exhaustive security
certification or a platform release qualification.

The review used the selected Mermaid sources and existing callers to ask three questions:
does an abstraction still have distinct consumers, does a check establish a unique contract,
and does a document still own information that is useful outside its original agent session?
File length alone was not a reason to split a module.

## First-pass implementation

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

## Follow-through implementation

The second pass addressed all nine original priority areas. Six initial implementation lanes,
three additional cleanup lanes, and read-only cross-reviews used disjoint write scopes; Cargo
remained serial under the main agent.

| Area | Follow-through change | Preserved boundary |
| --- | --- | --- |
| Parsing | Ordinary render, controlled render, and compatibility JSON share one controlled kernel; duplicated wrappers and parse flow were deleted. | Parse errors remain suppressible; cancellation remains outside suppression. Caller-owned runtime context, sanitization, source mapping, warning remapping, and timing remain explicit stages. |
| Init directives | Single versus multiple init handling now matches actual execution of the selected Mermaid package, including pre-merge sanitization. | Single legacy `config`, explicit family-key merging, known/detected types, and post-init wrap handling |
| Native fonts | CLI no longer creates a measurement context that the facade immediately forks again. Diagnostics come from immutable completed-output evidence. | Per-operation measurement state, separate export control, raw SVG's explicit fork, quiet behavior, and font budgets |
| SVG | Unchanged prepared-math projection reuses structural metrics; Architecture borrows edge points instead of copying six floats per edge. | Opaque/custom output validation, resource/reference admission, changed math validation, and distinct missing-edge/short-polyline fallbacks |
| Typst/UniFFI | Data convenience methods call the existing data executor, avoiding metadata construction and conversion that were immediately discarded. | Full result metadata, reusable-engine admission, busy/reentry/close errors, control, and resource limits |
| Performance tools | Baseline freezing uses shared `Corpus`/`LaneMetadata` instead of a second schema and a later second load. | Freeze-only canonical paths, unique source paths, file sizes/digests, and report identity |
| FCoSE | Removed always-empty fixed-node storage, dead loops/branch, and unused displacement parameters. | Random order, numerical order, effective constraints, and work accounting |
| ASCII | Removed test-only `from_sections`/`box_content` and their width calculation; existing tests now exercise deferred construction and text replay. | Production geometry, section output, semantic colors, and resource accounting |
| Documentation | Deleted eight completed administrative/duplicate files and the duplicate 0.9.0 release checklists; corrected archived status and references. | Unique design constraints and historical verification in existing design/evidence documents; release acceptance records in Publish Order |
| Playground/Node | Deleted unused preview plan collection/state/internal facade methods and the parse/materialize round trip after encoding controlled JSON. | SDK plan API, recipe validation, native external JSON syntax parsing, UTF-8 and structural wire limits |

### Behavior and API changes

Ordinary custom-parser cancellation previously lost its Parse phase or could become a suppressed
error diagram. Red tests reproduced both cases before consolidation. Ordinary APIs now return
`Error::OperationCancelled`; controlled APIs retain their outer cancellation result, including
callbacks that return cancellation directly without setting the control. Strict/lenient and
known/detected-type regression cases protect this distinction. In-context tests observe the
caller's date and derived ID rather than the engine's different runtime policy.

Mermaid 12.2.0's installed `detectInit` was executed with valid JSON directives, and independently
checked against the selected source at `1fb7869866dd619168d840291c9c54b4f129758b`.
The source map matched the selected `utils.ts`; `sanitizeDirective.ts` was also checked. Contrary
to the initial static hypothesis, merely moving legacy `config` after merging is insufficient:
multiple init arguments are sanitized first, which removes that key.

| Directives | Selected Mermaid result |
| --- | --- |
| One `config: {curve: "step"}` | `flowchart: {curve: "step"}` |
| One init containing that `config` plus `flowchart: {htmlLabels: false}` | `flowchart: {curve: "step"}`; the family object is replaced |
| Two init directives containing only legacy `config` | `{}` |
| Legacy `config` plus explicit `flowchart`, in either order | Only the explicit family configuration remains |
| Two explicit `flowchart` objects | Fields merge; the later value wins for the same field |

Inputs with multiple init directives should use explicit family keys or frontmatter configuration.
Single-init legacy configuration is admitted under the detected type key, with only Mermaid's
`flowchart-v2` to `flowchart` exception. The later family audit below covers keys that Mermaid
filters out. No baseline or comparator was changed to accept this correction.

The only added public Rust API is gated by `native-fonts`:
`RenderEvidence::native_font_diagnostics()` returns an optional `NativeFontDiagnostics` snapshot
with `identity()` and `selected_faces()`. It owns strings rather than services or mutable operation
state. Identity describes the existing context policy/admitted bytes; it is not a cross-process
font fingerprint. No wire schema or native ABI field was added.

### Deliberately remaining work

- Native-memory `discover_executable` still resets the shared bench profile. The runbook and
  historical fingerprint-reuse incident require source-revision evidence; Cargo's executable
  path alone is insufficient. This pass adds no replacement cache/provenance framework and
  makes no timing or allocator-performance claim.
- Computing ASCII group depths once would require a decision about observable work accounting.
  That change and tests coupled to internal callback batching remain separate opportunities.
- Parser cross-review identified a precise diagnostic-span test combining BOM, CRLF, frontmatter,
  and removed comments as useful protection against common source-map drift. Existing
  source-map tests pass, but this combined failure-path test was not added in this pass.

## Independent review

Two read-only cross-reviews examined another lane's parser/render/binding and
ASCII/manatee/graphlib changes. Neither identified a blocking regression. These reviews
supplement the executable checks; they do not claim to have executed them independently.
Follow-through cross-reviews independently checked init execution, cancellation and context
ownership, and the CLI/facade font paths. They found no remaining blocking defect in those diffs;
the newly requested cancellation/context regressions are included. The following first-pass
nonblocking test gaps remain explicit:

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
- No baseline, comparator normalization, ABI, feature recipe, or lockfile was changed. The
  follow-through's additive Rust diagnostics API is described above; the first pass had no
  public API change.

## First-pass verification

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

## Follow-through verification

These results were freshly run after the second-pass changes, separately from the first-pass
receipts above. Cargo reused `target/` with two build jobs and two test threads.

| Check | Result |
| --- | --- |
| Core all features, including direct/callback cancellation and caller context regressions | 2,349 passed |
| Core lean / presentation / compatibility / editor / presentation+editor selections | 42 / 53 / 54 / 65 / 63 passed |
| SVG pipeline without diagram features | 40 passed |
| Pipeline, Architecture edge fallbacks, and compiled math owner tests, all features | 45 passed |
| Facade explicit native-font contexts with PNG/JPEG/PDF | 24 passed |
| Facade native-font contexts without binary codecs | 9 passed |
| Facade render-operation and prepared-source integration | 59 passed |
| CLI native-font, quiet, raw SVG, batch, and codec contracts | 25 passed |
| Facade SVG and CLI SVG/native-fonts independent compile checks | 3 passed |
| Typst official recipe on the host, including data execution | 36 passed |
| UniFFI Apple artifact feature recipe on the host | 66 passed |
| UniFFI data, validation, ASCII, and PNG/JPEG/PDF convenience methods with system fonts | 9 passed |
| FCoSE and Manatee public API | 58 passed |
| ASCII all features after removing alternate test builders | 1,397 passed |
| Performance baseline, allocator, driver, and performance Python contracts | 23 + 51 + 33 + 157 = 264 passed |
| Node wire, API, and loader contracts | 80 passed |
| Web SDK TypeScript compilation with `--noEmitOnError` | Passed; real generated entries and declarations |
| Playground complete runtime suite | 155 passed |
| Playground build-graph suite | 52 passed; 3 blocked on absent WASM glue / Compare runtime assets |
| Playground ESLint, changed JavaScript/TypeScript syntax, and changed documentation links | Passed |
| Changed Rust crates: Clippy all features/all targets, warnings denied | Passed for core, render, facade, CLI, Typst, UniFFI, Manatee, and ASCII |
| Fresh release SVG comparison: all 37 entries, structure/parity/parity-root, three decimals | Passed with existing exact browser-layout/parser-diagnostic receipts |
| Mermaid reference, binding contract, and native ABI projection verification | All passed |
| Final workspace Rust formatting, path budget, and diff whitespace | Passed |

The initial attempt to run all UniFFI tests with only Flowchart and binary codecs selected
failed because existing full-family tests require Architecture/Sequence/Usecase/ER, and binary
output requires a font source. The artifact-owner recipe and the separate data-only binary
selection above passed. No production fallback, fixture, golden, or feature declaration was
changed to make an unsuitable test recipe pass.

For local Playground checks, the existing package-entry generator, installed TypeScript 6.0.3,
and `packageDistClosure` compiled and projected the real full SDK into ignored build directories.
This resolved the previous missing SDK declarations without substitutes. The remaining three
build-graph failures require the actual `artifacts/wasm/merman_wasm.js` and `.runtime` Compare
bootstrap assets. No synthetic WASM declarations, runtime assets, test skips, or graph-policy
exceptions were introduced. WASM/browser qualification and complete Playground typechecking
remain with the Web/Pages owner.

## Contract and configuration follow-through

The third pass reviewed the actual consumers of every remaining contract descriptor, then
cross-reviewed the generator changes. Native ABI and text-measurement JSON remain: they generate
runtime declarations and multiple host-language projections. The rename-policy descriptor also
remains because Rust, TypeScript, defaults, and documentation consume its policy properties.
The WASI JSON was a handwritten packaging mirror; runtime code and real hosts did not derive
their protocol from it.

| Area | Simplification | Retained evidence |
| --- | --- | --- |
| Native ABI generator | Exact descriptor-position checks replace redundant contiguous-code sets; renderers borrow validated slices; the only prefix digest operation replaces generic single-caller wrappers. | Name uniqueness, type/layout validation, minimum-prefix bounds, canonical digests, mutation rejection, full generated drift, and real C consumers |
| Text-measurement generator | One vocabulary type and validator covers result kinds; redundant lookups, intermediate collections, and generated-text substring assertions were removed. | Frozen protocol digest, semantic negative cases, complete projections and platform owners |
| Binding generator | One registry projection is shared by all languages; existing runtime-catalog collections replace duplicate registry scans. | Complete operation matrix and eight-artifact drift; Dart executes uint32/uint64 boundary cases instead of checking decoder source strings |
| Resource generator | Identical Python/Kotlin checks cover full sets once; validated UniFFI wire order is emitted directly. | Language-specific collisions, C macro collisions, bidirectional wire membership and append-only ordinals |
| Feature verification | Artifact validation and feature verification share one Cargo metadata load and local feature-group traversal. | Real Cargo builds own dependency resolution; exact recipes and capability/output boundaries remain checked |
| Editor generator | Removed the obsolete semantic-token manifest migration. | Current analysis-setting projection and full manifest drift |
| Documentation | Removed three old migration/admission/readiness documents and consolidated current binding guidance. | Unique memory, discovery, transport, resource, and platform guidance remains with its active owner |
| WASI packaging | Removed `merman-wasi-v1.json` and its mirror-equality test. | Existing `docs/WASI_ABI.md`, actual module, artifact hash, runtime catalog, wire/runtime tests and real-host import/export auditing |

WASI archives no longer contain that JSON path. This changes archive contents, with no wire,
ABI-version, or runtime behavior change. Native SDK archives retain both machine-consumed
descriptors and matching headers.

### Configuration admission and consumption

The public pipeline is shared, but that alone does not prove that every admitted field affects
its diagram. Independent source review identified both admission and consumption defects.

Installed Mermaid 12.2.0 was initialized before executing `detectInit` and `sanitizeDirective`.
Single legacy `config` uses the actual detector ID, except that `flowchart-v2` becomes `flowchart`.
Consequently `flowchart-elk`, `classDiagram`, `stateDiagram`, `xychart`, and `info` legacy keys are
filtered out; explicit `flowchart`, `class`, `state`, and `xyChart` namespaces remain usable.
Known-type parser aliases do not change this source rule. The global canonical-family helper
retains its separate purpose.

A catalog-driven test now exercises site/frontmatter/init precedence, protected fields, explicit
namespaces, known/detected metadata and enabled JSON/controlled-render parsers. It uses existing
legal family fixtures. Selected Mermaid also filters the explicit init namespaces `zenuml` and
`wardley-beta`; the test preserves this distinction from frontmatter admission rather than
assuming that every catalog namespace is an init key.

The consumption fixes are source-backed and checked at their actual outputs:

- Gantt falls back from a nonempty body interval to `gantt.tickInterval`; a UTC cross-month test
  checks the complete top and bottom axis tick labels and body precedence.
- Mindmap roots use `mindmap.padding/useMaxWidth`; tests check graph-relative viewBox margins,
  fixed/responsive attributes, source-over-site precedence, and unchanged explicit defaults.
  Compatibility JSON now clones the final configuration instead of selecting a layout again.
- Sequence combines configured numbering with the body autonumber switch, aligns note text and
  shadow regions, and uses one retained visible model for actor layout and SVG emission.
  Tests cover hidden actors, boxes, mirrored actors, creation/destruction, note/activation-only
  references, all-hidden bounds, self-messages, numbering while autonumber is off, and wrapped
  and unwrapped note alignment. The parser-owned semantic model remains unchanged.

This is not a complete field-by-field qualification for every diagram. Flowchart, State, Class,
ER, Architecture, GitGraph, and XY reviewed paths already consume the effective configuration
or a plan frozen from it; the review found no additional confirmed consumer defect there.

### Mainline integration and fresh evidence

The branch merged `origin/main` at `1c4cd0da9`. All three conflicts preserve both changes:
Mindmap tests, the new 23rd text-measurement operation alongside the simplified generator, and
the updated host measurement guide. The previously failing comparison-project dependency audit
is repaired by mainline's locked `pulldown-cmark 0.13.5`; a fresh local audit passes with the two
existing unmaintained-package warnings. No audit exception was added.

After merging mainline, core plus xtask pass 3,044 tests. Sequence/Gantt/Mindmap pass 253 owner
tests, with one pre-existing ignored test. The real Dart ABI contract entrypoint passes after
the merge. Archive/qualification Python tests pass 45 cases. WASI host tests before the merge
pass all 17 cases, including actual SVG/PNG, ownership, quotas, and error recovery.
