# Theme runtime responsibility cleanup on macOS, 2026-10-16

The cleanup removes work that ordinary SVG generation did not need: interpreting the completed
document to certify the renderer's own writes, retaining parallel terminal-coverage ledgers,
scanning for a retired text protocol, and computing identities that consumers did not request.
Theme paints, Mermaid compatibility fallbacks and actual strict/native admission remain owned by
their executing consumers.

This implements the 37 accepted findings in the
[runtime responsibility plan](../plans/2026-10-16-1800-refactor-theme-runtime-responsibilities-plan.md).
D31, D32 and D40 remain the plan's explicit compile-contract/schema deferrals. This checkpoint
does not establish complete recovery to the pre-theme release, browser-visible parity or release
readiness.

## What changed

| Owner | Removed or simplified work |
| --- | --- |
| Ordinary SVG generation | Unconditional final XML/reference interpretation; retained cross-stage XML facts; whole-SVG retired prepared-text scan; non-requested resource/artifact/receipt hashes |
| Class and Flowchart | Duplicate expected/observed terminal values, Class terminal receipt module, overlapping typography sets and cloned child request ledgers |
| State and Sequence | Complete SVG/CSS readback interpreters, default terminal signatures, Sequence baseline synchronization/merge copies |
| Other family writers | ER marker proof maps without a typed request; Mindmap copied palette/font ledgers; Railroad root/CSS recognizer; Ishikawa copied text domain; Block shell/path reverse recognizers; baseline-only completion scans in Wardley, Cynefin, Venn and EventModeling |
| Shared theme model | Full matched-rule history, independent support-manifest mirror, compile-and-discard preset catalog artifacts, constant-zero compatibility bridge fields |
| Facade and native export | Multi-layer digest/self-equality chains, standalone SVG admission constructed before an unrelated native target, unused native-only projections in SVG-only builds |
| Data interfaces | `Renderer::render_svg_data` feeds bindings, WASM and Node directly; strict data uses the same admission classification without freezing public render/session diagnostics |
| Private qualification | SVG observation and actual artifact hashes move to the private acceptance crate; qualification records use schema 2 |

The family owner retains the requested mechanisms. Actual writer branches contribute their
outcomes; absent theme requests do not need a domain that exists only to prove baseline completion.
Source/class/variant/ordinal precedence, unsupported winning facets, Clear semantics and real
fallback outcomes remain observable. This change does not add new ordinal paint support.

The producer boundary needed two concrete fixes once unconditional readback was removed:

- XML text escaping tracks the characters actually retained after invalid scalar removal, so
  `]]` followed by a removed control character and `>` cannot become an illegal `]]>` sequence.
- XYChart avoids intermediate overflow in percentage reservation and extreme finite axes while
  retaining the old arithmetic order for ordinary values. Unrepresentable geometry returns
  `InvalidModel`; finite SVG geometry is checked against native limits only at native consumption.

## Deliberate contract changes

This is a responsibility refactor with explicit contract migration, rather than an assertion that
every historical error or experimental API is unchanged.

- `max_svg_elements` bounds opaque SVG admission, explicit postprocessing and native expanded work.
  Ordinary renderer-owned generation uses source/model/layout/retained-text/output-byte budgets
  and cooperative cancellation. It no longer reparses its final SVG to count elements.
- Native tree and coordinate hard caps belong to native consumption. A strict standalone
  compatibility probe can report backend incompatibility without poisoning an otherwise valid
  standalone operation; explicit caller resource limits remain sticky terminal errors.
- Best-effort ordinary SVG has `Unverified` native status until compatibility is requested.
  `RequirePortable` evaluates the selected bytes and does not silently choose another pipeline.
- Inactive prepared-text/font outcomes, constant-zero bridge projections and experimental
  resource/document/artifact/target digest APIs are removed. Active math, font loading/glyph
  resolution, host measurement and native allocation controls remain.
- Qualification schema 2 owns the actual source/recipe/font/binary/artifact correlation; schema 1
  qualification records require a fresh run. The theme catalog remains schema 1.

See the current [SVG pipeline](../rendering/SVG_OUTPUT_PIPELINE.md) and
[resource integration](../integration/RESOURCE_POLICY.md) guides for these boundaries.

## Matched-output timing diagnostic

The existing native `pipeline` benchmark runs the rich `Renderer::render` SVG operation with a
reused renderer, deterministic measurement, strict parsing and the `trusted_native` profile.
It does not measure the additional data-only interface saving.

Both sides use Rust 1.95.0, the same benchmark and fixture bytes, and `default + svg`: all diagrams,
ELK, Cytoscape and math, without native fonts or raster/PDF features. The optimized bench profile
has opt-level 3, no debug assertions or debug information, no LTO and the default 16 codegen units.
The host is an Apple M4 Pro with 48 GiB RAM.

The base is the previously frozen `3d8a738d43ecc97a235b591cc37cfdf1013f6f39` executable with only
the two recorded removed-accessor build repairs. The candidate is the working-tree implementation
recorded by its saved patch and per-file hashes. Executable SHA-256 values:

```text
base  669e77161c5cd403df9745589e34c016f69374979b9f27aad124d4fc01641193
head  33248cac2edabebc939335bfdce0e4490b9130433fbdc612acadfcb3df468858
```

Two balanced AB/BA pairs use 30 Criterion samples, two-second warmup and three-second measurement
per run. The existing `compare_self.py` scheduler and receipt checks are reused with immutable
executables. Every run passes preflight and postflight; all five output byte digests and element
counts match. No Cargo, profiling or test work runs during timing.

| Fixture | Before, µs | After, µs | Observed change | Identical SVG bytes |
| --- | ---: | ---: | ---: | ---: |
| Class medium | 1,098.200 | 706.240 | −35.69% | 77,419 |
| Journey medium | 100.935 | 37.533 | −62.81% | 12,224 |
| Kanban medium | 136.350 | 81.135 | −40.50% | 21,557 |
| Class tiny | 223.925 | 158.185 | −29.36% | 16,936 |
| Info control | 19.797 | 15.003 | −24.22% | 2,518 |

Values are medians of the two independent run estimates, not confidence bounds. There was no
A/A calibration or power-sized confirmation. These are diagnostic measurements after the
documented contract migration; they are not latency-admission receipts or a claim of complete
release-range recovery. The earlier pre-theme measurements remain in the
[causal attribution checkpoint](native_theme_root_cause_macos_2026-10-16.md).

Fresh macOS `sample` captures use the normal optimized candidate executable: 12-second Class and
Journey profiling loops with five-second, one-millisecond CPU samples. The former general terminal
XML/reference validator and reserved-token scanner do not appear in either capture. The existing
test-only counters separately verify zero general XML/native terminal observation calls for
ordinary generated Class and Journey SVG. Sampling absence alone is not the structural proof.

## Validation and limits

| Check | Result |
| --- | --- |
| Renderer/facade/export/bindings integration, all families + layout engines + math + PNG/JPEG/PDF/native fonts | 6,064 passed; 7 existing skips |
| Private acceptance, enabled through its workspace-only configuration | 34 passed |
| WASM host tests | 47 passed |
| Native Node transport tests | 21 passed |
| Node package/transport tests with the project's npm 12 | 115 passed |
| Web script/contract tests after rebuilding current TypeScript | 149 passed |
| Qualification/archive Python tests plus acceptance-runner tests | 97 passed |
| Existing performance harness contracts | 121 passed |
| Affected public doctests, including compile-fail ownership boundaries | 42 passed |
| Production Clippy for affected crates, all features and targets, warnings denied | Passed |
| Whole workspace, all features | Compiled |
| External production consumer and private acceptance exclusions | Passed |
| Formatting, whitespace and local Markdown links | Passed |
| Generated dependency license reports and release projections | 13 reports and 424 projections checked |

Validation commands, raw logs, source hashes, immutable executables, profiles and the ignored
experiment ledger are retained under
`target/bench/experiments/theme-runtime-responsibilities-20261016/`. No tracing or profiling crate,
new production measurement hook, benchmark framework or CI performance gate was added.

The all-family SVG comparison succeeds for 37 entries, selecting 3,756 fixtures and rendering
3,729, with 27 existing skips. Its 11,187 structure/parity/parity-root comparisons retain the
existing explicitly reviewed parser and browser-layout residuals; this is not a claim of 11,187
pixel-identical browser results.

Actual wasm32 output built with the existing `wasm-size` profile and `wasm-pack --no-opt` executes
eight success/error cases, including strict successful SVG, typed-theme output, malformed options,
deadline and strict rejection. Data/result outputs and errors agree. The normal package builder's
subsequent local Binaryen 133 optimization aborts at `constraint.cpp:1223`; this packaging tool
failure remains recorded. The optimized Web package build is not claimed successful, and existing
publication scripts were not weakened to hide it.

## Remaining work outside this cleanup

D31/D32 retain authoring materialization/serialization costs at the explicit compile boundary.
D40 retains the reachable declarative text-layout capability pending a separate schema decision.
Other live family-specific receipts, including Requirement direct-paint accounting, have real
consumers and were not deleted as dead code. Further latency work should profile the remaining
layout, source-style and bounded writing costs with their own workload registration; the removed
ordinary terminal protocol should not be recreated to satisfy historical white-box tests.
