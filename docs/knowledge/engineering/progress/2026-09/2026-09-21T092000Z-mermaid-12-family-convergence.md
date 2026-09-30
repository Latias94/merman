---
type: Progress
status: active
---

# Mermaid 12 family convergence and remaining admission

The target remains the complete Mermaid 12 plan and `v0.8.0-alpha.7`; package versions remain
alpha.6 until final release projection. No release, push, or merge is authorized by this checkpoint.

## Implemented and verified

- C4 named arguments now preserve their actual field semantics and source-backed redeclaration
  behavior. Boundary endpoints use the source shape-before-boundary lookup and centered rectangle
  intersections. All 68 focused core C4 tests pass (`target/mermaid12-c4-core-final.log`).
- Non-Layered owner scopes preserve cross-provider edge ownership and missing sections rather than
  inventing routes; SPOrE exports source-local coordinates and label translations. Parent/child edges
  are exported in their owning inner scope with a single final scope offset. Independent review
  confirmed Mermaid-reachable ownership, cancellation and work accounting. The raw Radial hierarchy
  combination remains explicitly unsupported: its cross-scope root selection cannot be represented
  by the flat kernel, and Mermaid removes that container override before reaching the adapter.
  Five focused tests pass, including the Radial outside-first regression and provider peer matrix
  (`target/mermaid12-cross-provider-reviewed-tests.log`).
- State emits source container palette slots, uses the source choice stroke default, retains ELK
  self-loop edges without Dagre dummy nodes, and distinguishes measured from painted container title
  heights. ELK State and Class edge labels normalize Mermaid HTML break tags before Markdown parsing,
  consistently in measurement and painting. Class ELK groups each edge's center and terminal labels
  in source order; Dagre retains its existing order.
- The 36 selected C4/State/Class integration tests pass
  (`target/mermaid12-family-convergence-final.log`). The two shared line-break/State Markdown tests
  pass (`target/mermaid12-edge-break-unit-tests.log`).
- Class SVG wrapping now uses the same source dimension probe as layout, removing the duplicated
  renderer implementation. Its dedicated host probe test passes
  (`target/mermaid12-class-host-probe-tests.log`). The font-size precedence fixture 026 remains a
  bounded deterministic-font wrapping difference: source and local both probe at root 10px and
  paint at theme 24px; independently recomputing the heuristic explains the local line breaks.
- The five deterministic root canaries were checked against completed real Mermaid 12 generation,
  source dimension contracts and canonical artifact hashes. All root signatures and input hashes
  are unchanged. The existing catalog now carries the selected version/commit and actual SVG hashes;
  the comparison revision and gates are unchanged.

## Build and reference state

- The all-4 reference generation remains live in session 33113 / PID 63456. Completed families are
  reviewed against all-3 or unchanged Git HEAD before promotion. Packet's two old error pages were
  proven unchanged from HEAD, then replaced by real Mermaid 12 error SVGs. This is a source version
  change, not an unrelated editor overwrite.
- The running executable was safely renamed within target/debug to
  `xtask-mermaid12-all4-inuse.exe`; the same PID and session continue. This allows a fresh xtask build
  without stopping reference generation. Do not start the Flowchart retry until its existing global
  reference-toolchain lock is released; only the failed Flowchart family needs retry.
- A first native release benchmark build exposed seven package fingerprints tracking sources in
  the old `worktrees/origin-main-receipt` directory despite building the current checkout. That
  reused an obsolete ELK API. Only those seven packages' release artifacts were cleaned with Cargo;
  no source or debug artifacts were removed. Rebuild the benchmark before reporting any timing.
  The observational two-fixture ledger is under
  `target/bench/experiments/mermaid12-default-elk-admission/experiment.yaml`.
- The previously reported Web analysis/ASCII size overruns belong to partial migration builds from
  September 21 at 07:05/07:10 local time. Both still embed Mermaid 11.17.2 reference projection bytes
  and each has 19 changed source inputs. Their profiles correctly exclude ELK. Rebuild current inputs
  before claiming current sizes or optimizing; retain the existing budgets.

## Next work

Build fresh xtask, rerun affected family comparisons, update C4 semantic and changed layout goldens
through their generators, and finish real SVG promotion. Label/browser residual catalogs still carry
old version-bound evidence and need source review, not bulk hash substitution. The DataId semantic
label adapter is being updated to represent edges with optional labels while still comparing complete
edge identities and required label presence. Final WASM/Playground builds, size checks, reference/legal
projection checks, representative ELK timing, review and focused commits remain outstanding.

## 09:35 UTC continuation

- Fresh xtask built successfully after preserving the live generator executable. Full State
  structure/parity comparison now has 27 issue rows (previously 182), primarily path-command
  differences plus old canary identity evidence. Class has 9 rows (previously 52): the known font
  wrapping fixture, old canary evidence, and five hand-drawn fill mismatches. Counts are diagnostics,
  not pass claims. Logs: `mermaid12-state-family-convergence-compare.log` and
  `mermaid12-class-family-convergence-compare.log` under target.
- The shared DataId adapter now compares all edge identities and actual optional label sets rather
  than assuming every edge owns an empty label wrapper. Its new four-family synthetic regression
  passes; two selected older tests expose the pre-upgrade Requirement anchor and old catalog
  provenance and are being corrected/migrated, not ignored.
- C4 semantic snapshots were regenerated without a remaining delta. The previously missing large
  Flowchart layout golden now generates successfully with the repaired budget accounting.
- Independent review found State's painted title height was not yet used by SVG container painting;
  source-backed HTML/SVG title/body geometry correction is in progress. Class hand-drawn fill weight
  and hachure gap are now source-backed 1.5 rather than the old 4/5.2, including notes; outline styles
  stay independent. Their targeted verification is pending the serial Cargo slot.
- The reference owner reports 17 complete promoted families. Gitgraph automatic commit IDs changed
  because deleting the redundant preliminary parse removed an extra RNG consumption; all fixtures
  with no automatic IDs remained byte-identical, and only label-width/root propagation changed.
  The fresh real runtime output was admitted without rewriting the IDs.
- Candidate collection tooling must stay diagnostic and fail closed. An explicit existing label
  candidate mode is being separated from acceptance receipts so a version upgrade can collect raw
  geometry while old catalogs are stale; normal checks retain source, identity and receipt gates.

## 09:42 UTC continuation

- State painted title geometry and Class source hand-drawn fill now pass all four selected integration
  tests (`target/mermaid12-container-and-handdrawn-tests.log`). Independent review closed both issues.
- The DataId adapter and explicit diagnostic label-candidate path pass all five selected tests
  (`target/mermaid12-semantic-label-adapter-final-tests.log`). Normal acceptance still checks catalog
  and SVG receipts. The candidate path checks registered input identity and all existing label/edge
  semantic checks, collects only review-required output, and always fails final admission even when
  no registered fixture or no DOM comparison is selected. No residual receipt was accepted by this
  change. Old Requirement anchor assertions now derive the world position from the actual signed
  SVG wrapper; pure exact-geometry comparison explicitly takes no residual entries.
- Native default ELK checkpoints completed after repairing stale release cache provenance:
  flowchart_small [554.38, 579.29, 602.82] microseconds; nested-direction Flowchart
  [559.29, 586.52, 614.06] microseconds (Criterion confidence interval/estimate). Both preflight and
  postflight SVG identities match. These are current observational checkpoints, not comparative
  speedup claims. Logs: `target/mermaid12-default-elk-small-bench-final.log` and
  `target/mermaid12-default-elk-nested-bench.log`.
- Source/oracle investigation distinguishes two State path differences: fixture 015 is reproduced
  by elkjs using the exact 136.375 versus 136 minimum-width transition, creating a 0.046875-pixel
  four-point jog versus a straight section. Fixture 051 exposed a real missing cross-parent
  IncludeChildren policy. Flowchart's existing hierarchy index/path compression is now shared in
  `merman-render/src/elk_hierarchy.rs`, and State applies it before layout. Other families already
  using IncludeChildren or flat graphs do not receive unnecessary new scans. The seven targeted
  hierarchy/source-regression tests are running in session 69169; do not call the refactor verified
  until that result and the independent review are read.
- Generation has promoted 22 complete families; the original process continues. All-4 Flowchart
  still needs its isolated retry after the original toolchain lock is released. The latest standalone
  xtask predates the shared-hierarchy and diagnostic changes: nextest built the test binary only.
  Rebuild the ordinary xtask before fresh source comparisons or candidate collection.

## Shared hierarchy verification result

All seven hierarchy tests pass (`target/mermaid12-shared-hierarchy-final-tests.log`) and independent
source review found no blocker. The first compile identified only a missing Result unwrap in the new
budget test; it was corrected without changing production behavior. The no-default-feature renderer
check and fresh ordinary xtask build are running serially in session 99312, with logs
`target/mermaid12-shared-hierarchy-lean-check.log` and `target/mermaid12-xtask-hierarchy-build.log`.
After they finish, rerun State/Class structure/parity, collect registered-family label diagnostics
with `MERMAN_EMIT_LABEL_RESIDUAL_CANDIDATES=1` only for review (the command must fail admission),
and refresh State/Class layout goldens from the corrected renderer. Keep catalog acceptance distinct
from diagnostic collection; do not bulk re-sign measurement receipts.

## Fresh hierarchy/paint comparison

- The no-default-feature renderer check and ordinary xtask build both passed.
- Fresh State now has 16 issue rows (14 parity path differences plus two stale identity-evidence
  errors), down from 27 before the hierarchy fix. The 051 concurrency case and other corrected
  compound routes no longer appear. Class now has four rows: the one known wrapping fixture in
  structure/parity and two stale identity-evidence errors; all five hand-drawn fill cases converged.
  Logs: `target/mermaid12-state-hierarchy-compare.log` and
  `target/mermaid12-class-handdrawn-compare.log`.
- Diagnostic collection for both registered State/Class label canaries correctly failed closed and
  emitted zero candidates, revealing semantic-gate presentation differences before accepting any
  geometry: upstream Neo path initialization includes stroke-dasharray `0 0 <pathLength> 0` and
  dashoffset 0, while local output omits these full-visible-path decorations. State additionally has
  a stylesheet difference. These require source/normalization ownership investigation before any
  receipt admission (`target/mermaid12-{state,class}-label-candidates.log`).
- The all-4 reference owner now reports 26 complete promoted families. The original generator remains
  live; only the failed Flowchart family should be retried once that run releases its toolchain lock.

## 10:08 UTC continuation

- All-4 finished with 36 complete families / 2563 SVGs. Every completed family was reviewed and
  promoted with matching canonical bytes and manifest/input/SVG hashes. The sole failed Flowchart
  family is retrying in session 10172 using a retained executable
  `target/debug/xtask-mermaid12-flowchart-generator.exe`, output
  `target/mermaid12-upstream-svgs-flowchart-final`; do not restart the complete corpus.
- The 19-family audit is in `target/mermaid12-remaining-family-audit.md`: eleven pass both modes;
  C4 has only stale semantic identity, while ER/Requirement/Sequence/Block have source rendering
  deltas. Shared Neo marker masks were genuinely missing (not animation noise). Production now
  shares Flowchart's static marker mask across Class/State/ER/Requirement, and State CSS consumes
  theme stroke width with the source nullish/truthy distinctions. Verification is pending.
- Sequence basic Neo shadow/participant paints, shared look definitions for Class/Flowchart/Block,
  Block composite palette ordinals, ER Neo markers/root group order, Requirement Neo markers/node
  fill/divider geometry, and Gantt closing-BR section splitting are implemented by family owners.
  They require the pending focused build and fresh family comparisons. A compile caught the missing
  Block renderer-local color-index projection; its owner is fixing that rather than bypassing it.
- Current-source slim WASM packages were assembled and measured by the owning size command.
  Analysis raw/stripped/gzip/Brotli: 3632129/3631906/1427400/1089759; all four exceed existing
  limits (3600000/3600000/1375000/1050000). ASCII: 5193276/5193053/1939459/1470599;
  only raw/stripped exceed 5175000. Logs are `target/mermaid12-web-*-baseline-size.log`.
- Theme-storage experiment uses runtime schema 2 exact per-theme set/remove deltas, restores full
  maps once, and leaves audit schema 1 unchanged. Existing gen-theme-snapshot generated it; all
  22 maps match exactly, all 66 oracle cases remain byte-identical, runtime JSON shrinks by 81472
  bytes. Core theme tests pass 37/37. Generator tests are pending the unrelated Block projection
  compile fix. No WASM size win is claimed until both profiles are rebuilt and remeasured.
  Evidence: `target/bench/experiments/mermaid12-theme-storage/equivalence.json` and baseline files.
- State fork/join ELK measurement differs from Dagre: source insertMeasuredNode replaces padded
  shape dimensions with the painted 70x10/10x70 bbox. Root implemented shared bar dimensions with
  ELK consuming painted bounds while Dagre retains padding; new targeted test is not yet run.
- State 007/013/014 residuals also expose a compound kernel mismatch under exact same-size elkjs
  input. The existing phase diagnostic now reproduces After.x expected150.75 versus138.6433333
  (`target/mermaid12-state007-phase.log`); the owner is diagnosing the first differing stage.
  This is not accepted as a font residual. The temporary ignored diagnostic test must be converted
  to a focused regression or removed after diagnosis, never left as hidden permanent debug code.

## 2026-09-27 continuation: ELK boundary rounding and rounded-path attribution

- Recovered renderer state is authoritative. Commit `783504e8a` adds a local scale-aware
  four-ULP boundary tolerance. Mermaid 12's `outsideNode` itself uses exact `>=` comparisons;
  the tolerance compensates for local floating-point boundary drift rather than reproducing
  the upstream predicate literally. A regression covers near-boundary classification and a
  point farther inside the same rectangle.
- The focused pressure fixture `stress_flowchart_multi_direction_graph_011` passes strict
  `parity-root` with that change. `stress_flowchart_shape_mix_009` remains an independent
  residual: upstream retains an approximately `2e-6` initial segment on `L_circle_hex_0`,
  while local output has two points. The full renderer library passes 1,290/1,290 tests.
- A source-backed experiment temporarily disabled and then forced `compact_edge_corners` through
  the actual Flowchart render-config entry. The default comparison path already leaves this product
  presentation branch disabled; forcing it on increases path-command differences, while forcing it
  off is unchanged. No production change was retained. Mermaid 12's `generateRoundedPath` remains
  the applicable parity implementation: fixed radius 5 and `min(radius / sin(angle / 2), len1 / 2,
  len2 / 2)`, with no compact-corner projection or endpoint mask.
- The complete Flowchart comparison was rerun after restoring the production entry. It still has
  53 blocking `parity-root` mismatch rows. Representative routes show that upstream and local
  `data-points` already differ before rounded-path generation, and several node/label widths differ
  by deterministic-font measurement (for example, the long-label fixture's local edge-label widths
  are narrower and the affected node stack shifts by about 5.25px). These differences explain
  missing `L`/`Q` commands as provider/measurement residuals rather than a renderer-side command
  omission. No comparator normalization or residual receipt was added.
- Verification after the experiment: `cargo fmt --all -- --check`; `cargo nextest run -p
  merman-render --lib` (1,290 passed, 0 skipped); fresh `cargo build -p xtask`; full Flowchart
  comparison regenerated with the same 53 mismatch rows. The next investigation should target a
  reproducible browser measurement artifact or the upstream ELK input/provider coordinates, not
  post-process rounded SVG paths.

## 2026-09-27 continuation: strict verification and bounded residual attribution

- `target/debug/xtask.exe verify --strict` completed workspace checking, Clippy, all 117 feature-matrix builds, generated-contract validation, alignment evidence, third-party license closure, release legal projections, and both npm license checks.
- Its final workspace `cargo nextest run --workspace` build hit Windows resource exhaustion while compiling tests in parallel: rustc reported allocation failures, `error[E0786]` for an invalid `libtest` metadata mapping, and MSVC `LNK1102: out of memory` / Windows `os error 1455`. No test assertion failure was reported.
- A serial rerun with `CARGO_BUILD_JOBS=1` and `NEXTEST_TEST_THREADS=1` passed `cargo nextest run -p merman-render --lib`: 1290 passed, 0 skipped.
- The Flowchart parity-root comparison remains at 53 strict DOM mismatches. The remaining classes are source-backed browser-font/wrapping drift and provider route coordinates that move with measured node/edge label sizes; no comparator normalization or synthetic path-point postprocessing was admitted.
- The committed ULP boundary correction remains the only production change from this continuation (`783504e8a`).

## 2026-09-27 continuation: boundary experiment correction

- Removing only the four-ULP predicate and its regression did not recover the missing
  `L_circle_hex_0` segment. The exact-predicate build still emitted two local points at
  y=599.8236430254817 and 639.8236430254817; the pinned upstream emitted three at
  y=599.8236409707487, 599.8236389160156, and 639.8236389160156. This rules out the
  ULP predicate as the sole cause; it does not establish the complete provider/measurement
  cause of the difference.
- The same exact-predicate experiment introduced source-end segments on `L_q1_q2_0` and
  `L_q2_done_0` in `stress_flowchart_multi_direction_graph_011`. Their lengths were about
  0.0000118 and 0.0000522 respectively; the pinned upstream has neither. The experiment's
  ten geometry unit tests passed, but both focused strict comparisons failed.
- The experimental removal was discarded by reapplying the committed predicate and test.
  No production change, synthetic route point, comparator normalization, or residual admission
  is justified by this experiment. The earlier statement that local shape-mix output preserved
  the upstream short segment was incorrect and has been corrected above.


## 2026-09-28 continuation: Sequence critical browser evidence

- The pinned Mermaid 12 browser runner was instrumented at the DOM measurement boundary while
  preserving the renderer and SVG serializer. Using the admitted Edge 153.0.4234.48 executable,
  Puppeteer 25.6.0, Mermaid 12.0.0 package SHA-256 `df542ed953a0a169733d7a3d8f339a964d7ae19b196c83254ebfc4599904d333`,
  CLI package SHA-256 `6329cf1c54d9d59b8d94c16ca9f5eb2f394f5e7539b42a11c060f080f8b85078`, and the
  recorded font probe `df16e5d02be72139eddf67ff954b0b112aeabe403e927a42717d8b9efec3d800`,
  the critical label measured and rendered as three rows: `[Establish a`, `connection to the`,
  and `DB]`. The resulting SVG SHA-256 exactly matched the pinned upstream SVG
  `f6fb0ddeb0bc87cfb036e870a252db26de6fba6da0d1ca3634ff9ea09dbd35dd`.
- The existing Sequence family receipt policy now admits only this source as an exact,
  input-bound browser-text residual. The residual catalog binds the fixture input and
  upstream bytes and records local deterministic signature
  `cfd08ae0f3cd7c21e927c905706d04b2048ca4be4f4da7ed2e087c7b61ddd53d`.
  `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagram sequence
  --filter upstream_critical_without_options_spec --diagnostic-browser-text-layout` reports one
  accepted exact receipt; no production renderer or comparator normalization changed.
- The fixture-context contract and all 76 targeted tests (one pre-existing skip) pass under serial
  nextest. This receipt is diagnostic evidence for a browser measurement boundary, not a claim that
  deterministic text metrics equal browser metrics.

## 2026-09-28 continuation: revalidate the existing Sequence wrapping receipt

- Revalidated `upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048`
  with the same instrumented pinned Mermaid 12 / Edge runner. The new browser SVG is byte-identical
  to the admitted upstream SVG (SHA-256
  `90fc3f35949fc9745edb327433733e564aa4617859eacb2f1d56d9bee1792ac9`); the fixture input remains
  `f93593ed1f90823b3a2c7b0d8e1d9a48d07ca7e670528b40fbab1f26ed288a65`.
- The previous local signature was admitted at `0daf31f3c`, before the source-backed wrapped-actor
  correction in `c38856986`. Mermaid's `calculateActorMargins` measures the wrapped description
  with `calculateTextDimensions` before applying `actorStackHeight`. The corrected local actor rows
  are 92px instead of 90px; the fixture's layout-golden delta consists of that 2px row increase,
  resulting vertical offsets, and a 4px total-height increase. Message label widths/heights and
  horizontal geometry are unchanged. The existing host-measurement regression covers this path.
- The browser and deterministic text providers still choose different line breaks for the long
  messages. Browser output is 450x703 and current deterministic output is 454x535. This is the
  existing measured wrapping residual, not geometric parity. Rebind this one existing receipt
  to local signature `a3bf522f3f6e04aff89f087482d165517f36dccabb680b81f4d066c2388db69c`;
  no input/upstream hash, permitted comparison mode, renderer, or comparator is changed.

## 2026-09-28 continuation: Sequence wrapping matrix evidence

- The remaining 24 Sequence parity-root rows were replayed with the pinned Mermaid 12 browser
  runner under the recorded Edge 153.0.4234.48 environment. Every generated browser SVG was
  byte-identical to its checked-in upstream SVG.
- For every row, the local and upstream DOMs retained the same non-text element identities and
  the same concatenated text content. The differences are bounded to browser-versus-deterministic
  line breaking and the resulting text-row coordinates/heights: message, note, loop, and wrapped
  actor labels. No missing model element, endpoint, marker, or route was observed.
- The exact input/upstream-bound receipts now cover these 24 cases with their current local
  canonical signatures. This extends the existing Sequence browser-text measurement policy without
  comparator normalization, synthetic geometry, or a strict-mode exception. The generated browser
  evidence is retained under `target/sequence-critical-measurement/sequence-remaining-browser-evidence.json`;
  the catalog remains the committed verification authority.


## 2026-09-28 continuation: restore exact shared ELK boundary semantics

- The serial workspace nextest run completed with 8,724 passes, two failures, and seven skips.
  One failure used an xtask test binary built before the residual catalog grew from 97 to 122
  entries; rebuilding and rerunning the exact catalog contract passes with the current source.
  The other failure was a genuine State route regression caused by the four-ULP boundary
  tolerance from `783504e8a`, superseding the earlier decision to retain that tolerance.
- With browser-measured label bounds held fixed, edge 6 of
  `upstream_stateDiagram_state_definition_with_quotes_spec` lost a departure segment and changed
  from the pinned `MLQLQLQL` commands to `MLQLQL`. The inward-rounded source port must remain
  inside under Mermaid's exact `outsideNode` predicate. Restoring the upstream `>=` comparisons
  recovers the source route without a family-specific tolerance or a synthetic point.
- The regression now covers the exact boundary and a point one ULP inward on all four rectangle
  sides. Serial renderer library, Flowchart SVG, and State ELK tests pass 1,450/1,450 with
  `layout-elk` enabled. The current catalog contract also passes independently. These targeted
  reruns close both observed workspace failures; they do not claim a new full-workspace run.
- Freshly rebuilt xtask passes State's complete structure/parity/parity-root comparison, including
  22 existing exact browser-text residual comparisons. Flowchart renders 1,153 of 1,157 selected
  fixtures (four established skips), validates 70 existing residual comparisons without signature
  drift, and still reports 45 parity and 45 parity-root fixture mismatches plus one structure
  mismatch. No receipt is refreshed or added by this correction. The restored tspan class order
  is retained; signatures from the earlier experimental binary are not admission evidence.
- Independent Standards and Spec reviews of this focused diff reported no code defects or scope
  expansion. Spec review explicitly retains the family parity and integrated U15 closeout gates.
  Logs: `target/mermaid12-exact-boundary-tests.log`,
  `target/mermaid12-state-exact-boundary-compare.log`, and
  `target/mermaid12-flowchart-exact-boundary-compare.log`.


## 2026-09-29 continuation: Flowchart measured ELK and endpoint admission

- The pinned Mermaid 12 / Edge 153.0.4234.48 runner replayed all 45 outstanding Flowchart
  fixtures byte-for-byte against their admitted upstream SVGs. The committed matrix
  `fixtures/_verification/flowchart-elk-browser-measurements.json` retains 395 measured
  node records and 373 provider routes. A permanent xtask test checks each input and
  upstream SHA-256 against the current repository. JSON `float_roundtrip` is enabled only
  in the renderer's dev-dependencies so the captured IEEE-754 coordinates survive decoding.
- Equal browser node/label dimensions reproduce all 373 routes through the real Flowchart
  adapter and pure Rust ELK provider, with matching point counts and coordinate error at
  most 1e-8. This isolates the deterministic-versus-browser measurement boundary rather
  than substituting the reference algorithm or copying routes into production.
- The controlled final renderer uses captured centers, bounds, label boxes and provider
  routes. All 45 fixtures preserve DOM identities, edge kinds, looks, markers and path
  command sequences. All 370 non-hand-drawn edges also match every final path coordinate
  within the writer's three-decimal serialization precision (0.000501px). The three
  hand-drawn edges retain command/marker checks and the separate numeric provider check;
  their RoughJS control points remain a bounded browser/shape-path residual.
- The earlier roughly 0.06px endpoint difference in `newshapesset6_lr_md_html_false_094`
  was a test measurement gap: SVG Markdown requests its complete plain label after
  wrapping, not the original Markdown string. The captured height is 62.20000076293945px;
  the test's deterministic fallback had supplied 61.6px. Reusing the captured full-label
  box at that request closes the endpoint difference without a production tolerance.
  The fixture's wrapped SVG labels retain the same text and escaped markup content.
- Source review exposed real endpoint defects before residual admission. Circle and
  double-circle intersections now retain the radius captured by their shape renderer,
  rather than treating browser bounds as an ellipse. Anchor, filled circle, crossed circle,
  state start/stop aliases and choice follow their pinned shape intersections. Crossed
  circle's source radius is shared by measurement, paint and intersection. Exact-center
  circle intersection preserves upstream NaN semantics. Eight measured fixtures now pass
  without any residual receipt.
- The remaining 37 exact receipts bind the current fixture, upstream SVG and full local
  signature. All admit only `parity` and `parity-root`, except 094's measured text wrapping
  also requires `structure`. Together with the existing catalog this is 159 entries.
  No comparator normalization, global tolerance, missing-element exception or synthetic
  geometry was introduced. These receipts describe measured browser differences, not
  pixel-perfect deterministic rendering.
- Current-source verification: the 1,357-test renderer library run passed; the provider
  and controlled-renderer regressions passed; all four catalog/source-binding tests
  passed. The full primary-family `compare-all-svgs --check-dom --dom-modes
  structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout` completed
  successfully. Flowchart rendered 1,153 fixtures with four established skips, checked
  3,459 mode comparisons, and admitted 145 exact residual comparisons. Independent Spec
  review found no remaining code defect; Standards review requested this durable residual
  explanation. This is not the complete U15 audit: new-family checks, fresh consumer
  artifacts, affected size/performance gates and final release-surface verification remain.

Evidence logs: `target/flowchart-browser-evidence/final-all-comparison.log`,
`renderer-lib.log`, `endpoint-numeric-proof.log`, and `final-xtask-tests.log`.
