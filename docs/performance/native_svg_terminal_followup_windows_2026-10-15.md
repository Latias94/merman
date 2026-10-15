# Native SVG terminal follow-up on Windows, 2026-10-15

The preceding repairs are committed on `perf/theme-regression-repair-20261015`:

- `c2e857b9a`: remove quadratic staged provenance scans.
- `70cf1036d`: amortize bounded serialization admission.
- `ab8d96db1`: compact terminal reference validation storage.
- `a56aa0792`: record the Windows attribution and refactor evidence.

This follow-up uses clean `a56aa0792f6d97e976517887cc86cdfe58e46d74` as its product base. It investigates remaining ordinary source-to-SVG latency; it does not close the v0.9.0 regression. No new production change is retained. The [previous checkpoint](native_theme_refactor_windows_2026-10-15.md) remains the source for accepted structural and memory evidence.

## Source-backed exclusions

Default Journey has no complete XML validation facts before standalone finalization. `RootDocument::complete` checks root framing and viewport ownership, not the complete XML grammar. The default root theme produces an empty prelude and no background edit; the default themeCSS branch is an identity. The first complete XML traversal therefore remains the terminal admission. Existing `SvgPipelineBuffer` facts are not being unnecessarily discarded on this default path.

Identity must be proved by the owning operation. The precise root-edit identity is `prelude_len == 0 && background_edit.is_none()`, after existing checks and report completion. Zero growth is insufficient: an equal-length color replacement still changes bytes. Explicit transparent can insert a background rectangle. Nonempty themeCSS must retain parsing/scoping and trust records even if visually inert.

Two full forbidden-scalar stripping calls exist in the explicit postprocessing pipeline. That pipeline is not reached by the default `finalize_standalone(None)` lane. Removing its second strip could be a separate generation-validity experiment, but cannot explain or recover the default operation measured here.

## Exact work-count diagnostic

The ignored work-count probe uses the locked quick-xml 0.42 reader, complete actual SVG bytes and source-matched successful-path bookkeeping. It is not linked to the production validator. Event reads and normalized values are actual parser observations; QName/scalar/checkpoint/reference counts are a source-derived shadow of the successful ordinary XML walk. They exclude callback internals, graph planning, fingerprinting and outer pipeline work. These counts do not establish CPU percentages, correctness equivalence or a speedup.

| Ordinary XML walk work | Class medium | Journey medium |
| --- | ---: | ---: |
| SVG bytes | 77,419 | 12,224 |
| Event reads, including EOF | 1,102 | 184 |
| Elements | 576 | 96 |
| Attributes / normalized_value calls | 1,549 | 360 |
| QName validations | 2,125 | 456 |
| Normalized attribute/text scalars checked | 55,089 | 7,848 |
| Direct checkpoint callbacks | 12,737 | 2,810 |
| Reference observe calls with no field write | 1,155 | 317 |

The scalar loop tests the 64-character checkpoint interval per scalar; it does not invoke the dynamic callback for every character. Class normalization returns borrowed values for every attribute; Journey allocates for seven. Both fixtures have zero prefixed non-xmlns attributes, so the possible duplicate namespace resolution for such attributes is not exercised. Repeated ordinary normalization and prefixed-attribute resolution are not supported as dominant owners by these inputs. Native compatibility has a separate additional traversal and must be measured in its own lane.

## Rejected checkpoint-dispatch candidate

A single isolated candidate changes only the checkpoint type along the complete XML visitor, element validator and character validator from erased `dyn FnMut` to a generic `C: FnMut + ?Sized`. Structure/reference callbacks remain erased, the two large functions keep `inline(never)`, and all algorithms, character loops, cancellation observations and error order stay unchanged. The marker collector uses a temporary reborrow of the same callback.

Independent review finds no semantic blocker. All 76 focused terminal/writer tests pass, including concrete/dynamic callback equality with cancellation injected at every observed checkpoint, Unicode boundaries, forbidden scalars/entities, namespace aliases and duplicate expanded attributes. Clippy passes all renderer targets with all diagram/layout/math features and warnings denied. The two frozen pipeline executables each emit 459 preflights; all 152 render/end-to-end SVG identities match exactly. These checks prove the tested behavior, not a performance win.

The native pipeline executable increases from 40,279,040 to 40,489,984 bytes: +210,944 bytes (+0.52%). History shows callback erasure was deliberate: `dd79ca584` shared XML bodies to reduce Typst/WASM size, as recorded in [the October 10 checkpoint](theme_path_retirement_2026-10-10.md). Its earlier 50,828-byte stripped reduction came from a nine-file combination and cannot be attributed solely to XML callbacks.

No matching clean WASM base exists locally. Old Web/Typst packages have different source/capabilities and cannot substitute. The canonical Typst size lane additionally requires its pinned Binaryen 133 and wasm-tools strip; these tools are unavailable locally. No speculative pair of cold WASM builds is started for a candidate without dependable native latency evidence.

## Latency evidence is inconclusive

Host: Windows 11, Intel Core i7-11700, Rust 1.95.0, x86_64-pc-windows-msvc. Both frozen binaries use locked default features plus SVG, the identical checked-in pipeline benchmark, strict parsing, deterministic measurement and trusted-native resources. The candidate is the clean product base plus the frozen uncommitted dispatch.patch, not a clean-revision confirmation receipt. Discovery uses three alternating A/B rounds with 30 samples, 2-second warmup and 3-second measurement. Every timed operation preserves the same pre/postflight SVG identity.

The first round set overlaps unrelated Rider/Unity background CPU activity; all raw samples are preserved under loaded-diagnostic/. An earlier unrelated debug Cargo process used E:/github/merman and was left untouched; timing began only after compiler processes stopped. Compile-time deltas are not interpreted because its load overlapped candidate compilation.

A fixed final retry begins with four same-binary A/A pairs per executable. Base pairwise changes in consecutive Journey estimates range from -24.50% to -2.38%; candidate pairwise changes range from -6.92% to -2.40%. This is unstable diagnostic calibration, not the required eight-pair confirmation. The following final three-round means are retained for audit only:

| Fixture | Base (us) | Candidate (us) | Change vs base |
| --- | ---: | ---: | ---: |
| class_medium | 2947.100 | 3201.267 | +8.62% |
| journey_medium | 268.870 | 246.107 | -8.47% |
| class_tiny | 725.830 | 757.350 | +4.34% |
| xychart_medium | 422.650 | 410.497 | -2.88% |
| flowchart_medium | 4461.533 | 5057.733 | +13.36% |
| sequence_medium | 1521.667 | 1361.367 | -10.53% |
| kanban_medium | 361.190 | 353.353 | -2.17% |

Opposing movements and unstable A/A prevent attribution. These data establish neither improvement nor regression of the dispatch candidate. The candidate is rejected for production retention because the observed size cost lacks dependable performance justification. Its exact patch is reversed only after verifying its source and diff hashes; the committed implementation is restored without discarding unrelated work. No scalar validation, XML parsing, policy check or diagnostic is removed from production.

## Next complete ownership boundary

The remaining architectural candidate is a private structured SVG writer whose actual output events can establish XML/reference facts and move them with the immutable byte generation through root/theme/family ownership. Journey is a possible migration fixture, but admission must depend on complete event coverage rather than a family ID. Any raw chunk, opaque postprocessor, root edit or unproven mutation invalidates facts and retains the complete parser.

quick-xml Writer is not a validity proof: arbitrary BytesStart/BytesText/Event values can contain invalid QNames, duplicate attributes, unknown entities or mismatched end tags. Reuse the current namespace/attribute/reference semantics rather than introducing another XML grammar. Draft facts cannot report failure during Emit when the current contract reports it during Postprocess.

Terminal prefix order is an explicit exit gate. With max elements 2, `<svg><g/><rect title="&unknown;"/></svg>` must report the XML attribute error before admitting the third element. If that invalid attribute belongs to a fourth element, the third-element resource failure must win with actual=3. Final counts or a single deferred error cannot replace this sequence. Preserve CR/CRLF/entity normalization, namespace rebinding, duplicate raw/parsed IDs, text `]]>` boundaries, resource error provenance and cancellation. If preserving these obligations requires replaying a full event log with no measured saving, reject the design rather than add a second validation framework.

## Evidence

All exploratory artifacts remain under `target/bench/experiments/svg-terminal-followup-20261015/`: experiment.yaml; cpu-work-count.rs/.jsonl and cpu-work-report/receipt.json; base/candidate frozen executables and receipts; dispatch.patch; focused nextest and Clippy logs; identity.json; loaded-diagnostic/; journey-aa-diagnostic.json; end_to_end-3pairs.json. The isolated dispatch worktree retains the rejected source for inspection. These observations were frozen after the four preceding commits and before this checkpoint was committed. No further production change was retained in that experiment; no push was performed.
