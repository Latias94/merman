# SVG performance refactor directions, 2026-10-13

The recommended direction is to give initialization, each render operation, and file publication
clear ownership of their work. Remove repeated preparation inside those lifetimes before adding
caches, a new daemon, or another validation abstraction. The most concrete candidates are pure
theme calculation with one successful commit, exact-byte SVG fact reuse, and Windows delayed
loading of optional network DLL imports. They improve different workloads and require separate
experiments.

This is a researched direction, not an implemented performance claim. Main is frozen at
`3a4b8c393939b69372a8f05a2b971f7cdf6ae529`; theme PR #178 at
`f47abded02b540d7fe563af7bc75f2fa1bb90c51`. The prior
[investigation](CLI_STARTUP_INVESTIGATION_2026-10-13.md) and this research used isolated worktrees.
No production source, default output policy, existing PR, or CI gate changed in this round.

## Correct workload definition

Default Mermaid-to-SVG uses deterministic text measurement and no native font context. It does
not discover the system font catalog or run PNG encoding. Explicit font/native measurement
requests differ. Compiled PNG/PDF/network capabilities can increase the executable/dependency
footprint without executing those features during ordinary SVG rendering.

The relevant costs are separate:

| Workload | Main costs to investigate | Avoid this inference |
| --- | --- | --- |
| One fresh CLI per SVG | Image/DLL loading, operation setup, file publication | Sub-millisecond parse changes explain tens of milliseconds of startup |
| Reused Engine/Renderer | Theme/config preparation, layout, SVG processing | A process-local cache benefits the first fresh CLI equally |
| CLI explicit parity | Pipeline XML checks and final resource admission | Existing Criterion `end_to_end` uses the same pipeline |
| Library `pipeline=None` | Emission; PR #178 adds terminal admission/evidence | Three parity traversals also occur in this lane |
| PNG/JPEG/PDF | Native painting/fonts plus target-specific work | Its font catalog timings diagnose default SVG |

The existing `parse_cold_engine` benchmark constructs a new Engine inside a warm process. It is
not a fresh-process startup measurement. Existing `profile_render --stage render` measures SVG
emission, not complete terminal processing. These surfaces remain useful when their lifetimes
are stated correctly.

## Recommended ordering

| Order | Candidate | Primary owner | Evidence and expected benefit | Disposition |
| --- | --- | --- | --- | --- |
| 1 | Pure theme resolution, one successful commit | PR #178 core config/theme | Removes a reachable full JSON COW and repeated variable-map copying; latency unmeasured | First implementation candidate |
| 2 | Skip provenance propagation only when ownership proves it is empty | PR #178 config owner | Removes allocations/work without changing values; exact predicate required | Separate small causal candidate |
| 3 | Move terminal facts; reuse unchanged parity validation facts | Existing SVG artifact/pipeline | PR explicit parity has three XML traversals; source-backed opportunity, not a measured speedup | Separate ownership and traversal candidates |
| 4 | Delay selected Windows network DLL imports | Full CLI Windows artifact recipe | Tiny linker control saves 2.6-3.2 ms; full CLI and network behavior untested | Full-capability diagnostic next |
| 5 | Lazy finite initialization-owned theme results | Engine initialization | Avoid repeated theme execution in reused hosts; first-use and retained-memory costs remain | After pure-resolution seam and counters |
| 6 | Use existing host/batch lifecycle for repeated diagrams | Integrations | Amortizes process launch, but request clones/setup still exist | Measure the actual integration first |
| 7 | Separate staging/synchronization from final publication verification | CLI publication/dependency | Better stage ordering; dominant I/O cost not isolated | Research/instrumentation, then narrowly scoped dependency support |

The order is an implementation-dependency order, not a ranking of universal milliseconds.
For editor previews that spawn one CLI per edit, lifecycle and publication may provide a much
larger user benefit than theme work. For native library throughput, the first three rows matter
more. Default SVG does not need a PNG font-cache project.

## Theme materialization: calculate first, commit once

The theme PR protects failure atomicity by cloning the complete configuration before legacy
theme execution. The caller has often already detached its operation config. Mutating the
transaction clone then copies the JSON again. Extended legacy processing also copies theme
variables through multiple intermediate maps.

Use one private materialization module to calculate resolved theme variables and executed
dependency facts from immutable inputs. Every fallible color/evaluation step completes locally.
The configuration owner then applies normalized ownership and final variables once. Existing
pure legacy resolution is a starting seam; default and staged themes retain their distinct
constructor/operator semantics. This replaces the current transaction path instead of wrapping
it with an additional cache or service.

The success-only commit must preserve error atomicity, the final values, exact explicit and
compatibility ownership, detector replay, and relevant mutation-journal semantics. Merely making
the resulting JSON equal is insufficient. Add a genuine extended-legacy late-failure case:
`neo-dark`/`redux-dark` with invalid `cScale2`, plus a null ownership field that normalization
would retire. The existing test named `legacy_theme_failure_is_transactional...` selects `base`,
which now executes staged code; its name does not cover this legacy path.

Legacy provenance propagation can conservatively return early when the owning configuration
proves both `explicit_config_paths.is_empty()` and `theme_compatibility.is_none()`. In this domain
the propagation only produces absent metadata; it does not change values, errors, mutation events,
or controls. This is a property of the config owner, not a diagram-family shortcut. Test explicit
host/source ownership, typed bindings, transferred detector ownership, and equal-value writes as
opposite cases. Do not generalize this proof to staged evaluation without a separate argument.

Only if propagation remains material should the existing fixed dependency ledger become a finite
indexed descriptor, with prepared names/paths and operation-local indexed origin/winner storage.
Preserve dependency order, source-truthy conditions, `IfMissing`, transitive ownership and staged
object paths. Do not build a new theme compiler or derive correctness from a script's source scan.

## Initialization reuse: cache a pure result, not an operation

If exact work counts show material repeated selected-theme execution, share an immutable
initialization environment through Engine clones. It owns raw authored initialization inputs,
the initialized detector baseline, and lazily prepared results for the finite registered theme
set. The operation continues to own filtered source configuration and detector effects.

Detection must still observe initialized global `default` plus source overrides before scoped
appearance selects `redux-color`. A preselected family snapshot would change custom detector
behavior. An ordinary source variable update retains its current after-theme merge semantics;
an explicitly selected registered source theme must calculate from raw initialization plus source
inputs, including when its name equals the initialized theme. Preserve unknown values, JSON null,
the distinct string `"null"`, and JavaScript truthiness/operator behavior.

The reuse key is immutable initialization identity plus exact selected theme. Initialization
identity includes raw values and the applicable authored/compatibility provenance, not just
serialized JSON. Site/compatibility changes replace the environment; old Engine clones retain
their old state. Registry changes need not invalidate a pure theme result that never reads the
registry, because detection and replay still execute on every operation. A final-operation cache
would require a much larger context and is not recommended.

Eleven lazy slots bound entry count, not bytes: large host inputs can multiply retained storage.
Register a byte bound or a proven proportional-space bound before implementation, retaining an
uncached fallback. Never prepare unused themes eagerly, retain request controls in shared state,
or cache cancellation/deadline failures. Concurrent first-use, recursion, source overrides,
and policy accounting must be tested. `OnceLock` can block during initialization; choosing it is
not a cancellation strategy. [Rust OnceLock documentation](https://doc.rust-lang.org/std/sync/struct.OnceLock.html).

The entire measured small PR parse operation is about 0.36-0.39 ms. A config-only optimization
has a smaller ceiling and cannot materially recover a roughly 61 ms fresh CLI call. It can be
valuable for a warm sub-millisecond renderer. Share immutable state where appropriate, remembering
that writes to shared `Arc` data can copy the inner value.
[Rust Arc documentation](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut).

## SVG finalization: deepen the existing artifact

The renderer already owns `RenderedFamilySvg`, `StandaloneSvgArtifact` and `ResvgCompatibleSvg`.
Extend their private transfer of exact-byte facts rather than introducing another global cache
or public options bag.

For ordinary best-effort output without custom/inline processors, source inspection gives:

| Frozen path | Complete pipeline/terminal XML traversals |
| --- | ---: |
| Main library `pipeline=None` | 0 |
| Main CLI explicit parity | 2 |
| PR #178 library `pipeline=None` | 1 |
| PR #178 CLI explicit parity | 3 |

These counts exclude emitter work, root-tag scans, character cleanup, postprocessors and private
observations. They describe work, not a measured latency fraction. The omitted-pipeline PR path
already uses a fused ordinary XML/reference-budget walker; do not recreate that completed repair.

First consume `TerminalSvgValidation` and move its reference plan, closure and font seal into
`SvgFinalizationReport`; the current borrowed constructor clones them before dropping the owner.
This is separate from the larger traversal change and primarily benefits paths that produce a
terminal native-validation report, not every best-effort parity call.

Next retain validated structural/reference facts for exact unchanged SVG bytes through the
pipeline. Use them at the terminal resource stage without lexing the string again. Reference
expansion and expanded-budget failure must remain at the current terminal position. The initial
raw/intermediate byte and element checks still protect transforms. A custom processor, CSS/id/
math/icon/root rewrite, or uncertain borrowed output invalidates the corresponding facts.
`Cow::Borrowed` alone does not prove identity: it can refer to different bytes. Preserve existing
prepared-text/math evidence invalidation even if a host callback returns equal strings.

Start by transferring facts from the final pipeline XML check directly into the following
terminal stage; only later consider keeping them across earlier callbacks. Retained facts are an
optional optimization, not new admission authority. Bound their memory; if the internal retention
bound is exceeded, discard them and use the existing terminal scanner. Do not return an early
resource failure: a host processor may shrink an initially permitted SVG with many long ids/hrefs
into a small valid result that the current pipeline accepts. Keep deferred reference/semantic
failures for the existing terminal decision point, rather than changing callback/error order.

XML failures must retain precedence over CSS/native semantics. Keep cancellation checkpoints
around opaque callbacks and the existing sticky terminal failure order. Retaining facts means
retaining their exact validation scope; ordinary parity facts are not native compatibility or
font/resource-closure certificates. Keep DAG counting saturating and bounded by retained raw
nodes/edges, without materializing potentially exponential expanded instances.

Resource fingerprints and raw SVG hashes use different encodings. The former binds a domain,
complete SVG, font catalog and source policy; the latter hashes raw bytes. They cannot be swapped
or derived from one another under the existing digest contract. Document identity already reuses
the raw digest when public/native bytes match. A new encoding needs explicit versioning, not a
silent hash shortcut. Rich private DOM/CSS receipt generation is disabled in ordinary builds and
cannot explain ordinary CLI timing.

A proof-carrying emitter could eventually avoid parsing exclusively safe constructed XML, but
current family output admits strings, HTML, CSS and external/custom projections. That is a larger
cross-family redesign requiring a positive construction domain and validated raw-fragment path.
It is not justified as the first repair. Likewise, exporters already reuse sealed reference and
resource facts; they still need target-specific usvg/font/pixel/filter/encoding admission.

## Windows launch: defer imports without deleting functionality

The added system imports trace through optional network features:

- `network-icons` / Hickory system configuration / `ipconfig` uses `IPHLPAPI.DLL`.
- Reqwest's platform certificate verification uses `CRYPT32.dll`.
- Windows error-string support in that dependency path uses `OLEAUT32.dll` BSTR functions.

This is distinct from PNG system-font discovery. A full CLI can retain network features while
testing MSVC linker delay-loading for these DLLs. Do not replace platform trust roots, remove
features or supply a smaller-capability binary and call that an equivalent optimization.

The root agent built an isolated static-CRT control with the same Rust source, imports and
stdout, varying only linker delay-load arguments. PE inspection confirmed all three imports
moved into delay-import descriptors. Each independent diagnostic batch used three warmups and
64 fresh processes per case, rotating/reversing order with no compiler or profiler active:

| Control | Batch A median | Batch B median |
| --- | ---: | ---: |
| No additional imports | 13.49 ms | 12.21 ms |
| Eager imports | 16.36 ms | 15.26 ms |
| Delayed imports | 13.75 ms | 12.07 ms |

The candidate saves 2.61 and 3.19 ms in the two tiny control batches. It grows the eager control
by 3,584 bytes. Separate eager/delayed smoke programs actually called null-safe BSTR length,
opened and closed an empty memory certificate store, and requested adapter buffer size. Their
stdout matched and all three loaded modules resolved to System32. These tests do not exercise
Merman hostname resolution, HTTPS platform trust, unknown-CA rejection or its dependency error
behavior; the full CLI was not built or admitted as an optimization.

Before full-artifact adoption, examine constructor/import-address assumptions, first-call failure
handling and module search provenance. Delay loading applies to complete DLL function imports,
can change initialization timing and moves load errors into the running operation. Use the standard
helper and a narrowly scoped Windows build setting only after the affected paths pass.
[Microsoft linker delay-load documentation](https://learn.microsoft.com/en-us/cpp/build/reference/linker-support-for-delay-loaded-dlls?view=msvc-170).

Binary size remains a separate target: historical growth is mainly executable code, not merely
embedded assets. A same-capability size/codegen experiment must also control rendering throughput,
compile time, startup and output. Padding controls are not an exact production image-loading model.
Do not change portable renderer architecture solely from this Windows result.

## Publication: attribute first, then decide the stage contract

Current CLI publication stages output, writes it, runs final guard/verifier rounds and calls the
dependency's commit. `atomic-write-file` performs file synchronization inside commit before rename.
Thus synchronization still creates a window after CLI verification; repeated guard rounds do not
remove it. Custom verifier and same-inode mutation tests also make deleting a guard round from
the common helper incorrect.

If segmented I/O measurement shows this path dominates, one later design to evaluate is:

```text
write staging -> prepare/synchronize and prohibit further writes
             -> final identity/snapshot/cancellation verification -> publish
```

The dependency currently lacks that public split. Calling `sync_all` early and then ordinary
commit simply syncs again. Prefer a small upstream capability or an existing suitable primitive
that preserves platform directory-handle behavior; do not replace the dependency with a new
in-repository filesystem transaction framework. Separating callback-capable fix verification
from a controlled render verifier may make one guard round redundant, but requires a precise
type/behavior proof and preserves the final post-callback snapshot check.

Treat this as correctness/ordering and possible I/O optimization, not a promised removal of the
whole measured file-versus-stdout difference. Windows file sync plus rename is not an absolute
portable power-loss guarantee. Frequent flushing can be expensive.
[Microsoft FlushFileBuffers documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers).

Moving sync earlier does not itself reduce sync count and can change failure precedence. If the
target has changed and synchronization also fails, the present path reports the identity failure
first; a naive reordered path reports I/O first. A canceled operation or failing verifier could
also perform synchronization that it previously avoided. Keep this item as attribution/backlog.
A later design must preserve pre-preparation checks/callbacks and final post-preparation
verification where needed, or explicitly propose a changed error contract. Moving sync only
shortens a possible race window; it does not create filesystem compare-and-swap.

An explicit atomic-but-unsynchronized preview policy is a different product contract, not an
equivalent fix. It could be considered later for caller-owned disposable output, with existing
safe default and alias/identity checks, but should not expand this refactor first. Stdout already
lets integrations choose their own storage. Avoid recommending stdout without also accounting
for the caller's eventual write cost in an integration benchmark.

## Host lifecycle: use available reuse before building a daemon

Markdown batch shares a prepared graphical renderer, while Node NativeEngine retains a
BindingEngine with renderer preparation. Async requests still clone and validate request state;
these paths do not imply zero per-request Engine work. They are existing surfaces to measure and
improve, not a new transport requirement.

For a Rust/Node host, own one configured renderer for the appropriate document/session lifetime,
with per-operation source, cancellation and output controls. For an external editor invoking one
CLI on each change, first measure the complete editor route, including process launch, input,
SVG delivery, rasterizer/viewer and UI scheduling. A batch command does not by itself provide
incremental persistent preview. A long-lived CLI protocol would require framing, request identity,
ordering, cancellation, bounded queues, crash recovery and configuration invalidation; do not
build those before demonstrating that existing bindings cannot serve the integration.

Capability matching is mandatory: the shipped Node surface is experimental deterministic
SVG/layout and does not include the complete CLI's math, binary export, analysis, ASCII or
callbacks. Neovim is not automatically a Node addon host. The C ABI exists but has no generic
prebuilt SDK; a LuaJIT FFI integration must own distribution, ABI checks and memory lifetimes.
These are supported architectural options, not interchangeable prebuilt products or a free
migration for inkmd.

## Bounded implementation experiments

Each candidate gets its own diff, diagnostic record and public-operation control. Keep main-only
CLI artifact work separate from theme-PR-only ownership changes. Do not stack an alternative theme
cache architecture onto main while PR #178 changes the same module.

1. **Theme:** exact COW/map-copy/ownership-work counters, then pure resolution and no-owner guard
   independently. Compare full config/provenance/error outcomes and same SVGs against the frozen
   PR. Use class/flow/sequence tiny and mindmap controls, real legacy late failures, authored and
   detector overrides, staged themes, null/unknown values, concurrent/nested/canceled operations.
2. **SVG:** count complete lexical traversals, bytes/events, reference work and copied terminal
   facts. Primary public operation explicitly selects parity; omitted pipeline is a separate
   control. Cover malformed XML, CSS/entity/HTML/Unicode, duplicate ids, use/marker DAG expansion,
   math/custom transforms, resource boundaries and cancellation/error precedence. Confirm exact
   output, digests, status and reason ordering, not just a stage's output length.
3. **Windows:** full `cli-release` recipe, identical capabilities/CRT/link profile/source, frozen
   base and delay-load artifacts. Measure fresh `--version`, stdout/file tiny SVG and a medium
   control; exercise network DNS and accepted/rejected platform TLS plus imported API paths.
   Report binary/import changes and all supported Windows release architectures independently.
4. **Reuse:** measure one genuinely fresh process, new Engine inside a warm process, reused
   Engine and an actual host/batch route separately. Test concurrent first-use and memory retained
   across many initialization environments before admitting a cache.
5. **Publication:** segment writing, guard I/O, synchronization and rename first. Retain alias,
   hardlink/symlink/parent replacement, same-inode edits, canceled staged writes and callback-error
   cases. Test the relevant platform contracts rather than mirroring private implementation.

Build Cargo serially, freeze executables before timing and use the existing harnesses. A traversal
or allocation reduction is a structural result until the complete public operation passes its
registered latency gate. Use calibrated A/A and power-sized AB/BA for admission, with separate
size, retained-memory and complexity bounds. Use scoped nextest, fmt, Clippy, feature/profile and
Mermaid parity checks for a retained implementation. Do not add expensive benchmark jobs to every
PR or redesign ordinary CI during this investigation.

## Research record

Three subagents independently researched CLI lifecycle/publication, config/theme ownership and
SVG finalization. Their detailed notes and official references are under the isolated worktree's
ignored `target/bench/experiments/svg-refactor-directions-2026-10-13`. The root owns serialized
compiler and timing work. That directory retains two raw loader batches, PE imports, API smoke
source/results and the experiment ledger. No full production candidate, calibrated speedup,
cross-platform performance result, commit or publication is claimed by this document.

A fourth agent challenged the combined direction. Its two concrete counterexamples narrowed
the first SVG experiment to final-check fact transfer with bounded optional retention/fallback,
and kept sync-stage reordering outside equivalent performance implementation until error and
callback ordering are decided. The initial implementation can be three focused reviews: theme
resolution/provenance repairs against PR #178, SVG terminal ownership/final-check reuse against
the chosen renderer baseline, and full-capability Windows startup experiments. Finite theme
reuse, a persistent CLI protocol and publication API changes stay behind those results.
