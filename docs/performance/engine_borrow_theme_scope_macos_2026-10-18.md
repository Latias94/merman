# Engine borrowing and theme performance scope — macOS, 2026-10-18

## Decision

Commit `3bf737d25e5fcb264a805be0a805519478365f78` makes the internal synchronous parse operation
borrow `Engine`. Ordinary public requests remove two Engine clones, explicit request themes remove
one, and `prepare_semantic` removes one. The engine is not part of the returned `SemanticArtifact`.
Default requests borrow the renderer; explicit themes still install compatibility on a private clone.
The one captured runtime context, parsing/config copies, admission and terminal-error order remain.

This is an ownership refactor, not a heap-allocation or calibrated latency improvement. Engine
registries/configuration already share Arc-backed storage. The implementation adds no cache,
persistent state, public lifetime, feature or dependency. A successful themed request followed by
an ordinary request, then a cancelled themed request followed by an ordinary request, now has one
public regression test. Existing tests retain artifact use after a temporary renderer is dropped.

## Evidence

Adjacent base is `df425947a` (runtime equal to `ca17e5228`). The completed frozen root-viewport
baseline probes match that runtime. Across 178 public rich SVG operations, output bytes, theme/
resource reports and ordered custom-measurer traces match. All 152 pipeline SVG identities match.
172 inputs have stable five-repeat memory samples on both sides, with all four metrics unchanged;
six Flowchart inputs remain memory-inconclusive with all samples kept. The original 86 core
inputs remain stable. No allocation saving is claimed.

Fixed adjacent diagnostics use two AB/BA pairs for emission/E2E of five families, 30 Criterion
samples, 2 s warm-up and 3 s measurement: 40 invocations with matching preflight identities and
postflight verification. Every selected E2E row passes the preregistered >10% AND >1 us material
regression screen. These are uncalibrated diagnostics.

| Family | Public E2E, base → head (us) | Change |
| --- | ---: | ---: |
| Zenuml | 23.7570 → 23.8985 | +0.60% |
| Packet | 11.1700 → 11.0525 | -1.05% |
| Timeline | 43.5305 → 43.3990 | -0.30% |
| State | 433.4850 → 432.2550 | -0.28% |
| ER | 413.9800 → 409.1850 | -1.16% |

The separate four-invocation v0.9.0 same-SVG Zenuml context observes 20.3045 →
23.9505 us (+17.96%). Different error/resource contracts and the absence of A/A
calibration prevent a theme-only cause, all-family gap or confirmed recovery claim. Do not combine
the current estimate in that lane with adjacent-pair estimates.

Final source passes 8,253 native tests (13 skipped), Clippy with warnings denied, the 80-test focused
selection and the strengthened eight-test layering selection, one semantic-only/no-SVG test, 34
private theme tests, 47 WASM host tests, optimized full WASM/37-scene smoke, all 37 selected DOM
comparison groups in three modes, minimal SVG features and formatting. Read-only consumer,
correctness and security reviews find no remaining issues. The full native run precedes only the
final test strengthening and a comment clarification; runtime function bodies remain unchanged.

The initial candidate had one Clippy needless-borrow failure; it was corrected before the final
build, output captures and fixed timing schedule. Preliminary timing logs were overwritten and
are not included as final evidence. A copied preceding-experiment assertion requiring a Zenuml
allocation decrease was also removed: it was inconsistent with this experiment’s preregistered
clone-work claim. The original nonincrease controls remain. The isolated probe executables were
temporarily made writable for replacement after two overwrite attempts failed; the frozen base
was not changed.

Raw preregistration, final source/library/executable hashes, five-repeat memory captures, ordered
traces, diagnostics and validation records are under
`target/bench/experiments/engine-operation-borrow-20261018/`. Some checks were captured in the
session tool output rather than full raw log files. Cargo, memory captures and timing ran serially
on Apple M4 Pro/48 GiB/macOS aarch64/Rust 1.95.0. No full strict release/transport sweep is certified.
Final pipeline SHA-256: `a89c74fb68c95d4df086063c30d03d34722224579091fcb32c2097e147a16866`.

The new immutable goal registration records scope and the active next action. Whole-bundle memory
validation is blocked by pre-existing historical shards without required frontmatter and legacy
rollups; this checkpoint does not migrate those unrelated records or claim bundle-wide validity.

## Scope of the continuing goal

Use `DiagramFamilyId::all()` as the logical catalog: 35 families including Error, 34 public family
Cargo selectors, 43 parser variants and 47 authoring headers. The existing cross-family lane has
38 scenes (35 logical families plus three Railroad syntax variants). The 37 DOM groups omit Zenuml;
they are not 37 logical families. Existing native corpus fixtures cover every logical family.

The 10 public compiled presets are editor-light/dark, one-dark, gruvbox-light/dark, ayu-light/dark,
brutalist, spotless and cyberpunk. The four `family_designs` entries are design targets, not the
complete support scope. Existing pipeline measurements use no compiled request theme. Compiled
theme coverage must therefore be captured separately, compiling once outside per-render measures,
then distinguishing actual support, partial application and fallback for every family.

The frozen tag discovery currently matches 19 fixtures across 15 logical families; 57 renderable
fixtures change SVG and three frontmatter-only cases are not rendering operations. Keep these
lanes separate. Changed SVG and newly introduced theme/native functionality cannot silently enter
a same-work release recovery ratio. The source-backed changed-output audit remains applicable;
unexplained changed output remains unresolved.

Next: measure the independent root style temporary, then explicit root-report inventory/coverage
rebuilding. The strip-important plus scoped-CSS composition is a separate configured policy
candidate with ordering counterexamples, not a default-theme cost. Preserve source/config fallback,
actual writer facts, error/cancellation/resource contracts and host measurement work.
