# Flutter Transport Admission

## Decision

Flutter Rust Bridge (FRB) 2.12.0 is **not admitted**. The supported Flutter
transport remains Native ABI 3, raw declarations generated from
`crates/merman-ffi/include/merman.h` by `ffigen`, and the handwritten Dart
facade in `platforms/flutter/lib/src/merman_ffi.dart`.

The private FRB candidate called `merman-bindings-core` directly and therefore
did not weaken the architecture by calling Rust through C and back into Rust.
It demonstrated a useful default async scheduling model, but did not clear the
whole U15 matrix. In particular, it did not provide Merman's synchronous host
text-measurement contract, preemptive cancellation of CPU-bound work, true
incremental output from bindings-core, or an already-owned five-platform
package and CI path. Keeping it beside ABI 3 would create two Flutter transport
contracts to maintain. Replacing ABI 3 would discard the shared compatibility
anchor without a sufficient user-facing benefit.

No FRB crate, Dart dependency, generated bridge, package hook, feature, or
release contract remains in the production tree after this decision.

## Admission Rule

U15 admits at most one Flutter transport. A candidate must preserve the same
request, capability, resource, runtime-policy, error, lifecycle, output, and
callback semantics as ABI 3 and must also justify its additional package,
generator, binary, and CI cost with a material Flutter workflow improvement.
A locally successful build or a more concise generated API is not sufficient.

## Compared Designs

| Property | ABI 3 + `ffigen` baseline | Private FRB 2.12.0 candidate |
| --- | --- | --- |
| Rust ownership | `merman-ffi` delegates to `merman-bindings-core` | Direct path dependency on `merman-bindings-core`; no `merman-ffi` dependency |
| Wire authority | ABI 3 C header, descriptor digest, table sizes, and surface compile-run layout tests | FRB-generated Rust/Dart protocol; deliberately identified as candidate transport version 1, not ABI 3 |
| Dart surface | Raw `ffigen` declarations plus one handwritten policy facade | Generated async methods and opaque Rust object wrappers |
| Native discovery | One exported `merman_get_native_api` symbol | FRB-generated symbols and loader |
| Package ownership | Existing Android, iOS, macOS, Linux, and Windows package metadata and release workflow | No admitted package metadata or target CI; stable FRB 2.12.0 uses copied Cargokit integration |

The candidate was an isolated nested workspace under
`platforms/flutter/spikes/frb_2_12_candidate/`. It pinned both Rust and Dart FRB
dependencies to 2.12.0 and exposed only:

- an opaque, explicitly disposable engine containing `BindingEngine`;
- generic `execute` and SVG byte operations;
- the bindings-core runtime catalog;
- structured status, code name, error kind, capability ID, and message fields;
- an explicit build probe that reported unsupported cancellation, synchronous
  text callbacks, and incremental core output as `false`.

## Behavior Matrix

| Gate | Baseline | FRB candidate | Admission result |
| --- | --- | --- | --- |
| Header/API drift | `ffigen` regeneration plus a checked diff; no handwritten raw declarations | FRB regeneration changes Rust and Dart glue | Both can detect drift; FRB adds another generator contract |
| Capability and catalog | ABI descriptor and runtime catalog are validated together | Direct bindings-core catalog was exposed under a distinct transport version | Candidate must not claim ABI 3; keeping both duplicates transport ownership |
| Resource/runtime policy | Handwritten facade passes the versioned engine options unchanged | Direct `BindingEngine::from_options` used the same JSON | Equivalent in the bounded candidate |
| Structured errors | Stable ABI status plus kind/capability payload | Custom FRB exception carried the same fields | Equivalent for exercised errors |
| Engine lifecycle | Idempotent `dispose`, use-after-dispose rejection, callback-time disposal rejection | Opaque object generated `dispose`; global `RustLib.dispose` also required | Basic disposal passed, but a rejected post-dispose async call left the Dart process alive past a 5 s bound |
| UI-isolate responsiveness | Calls are synchronous unless the application owns a worker isolate | Default Dart API schedules synchronous Rust work on FRB's native thread pool | Candidate advantage, but implementable above ABI 3 without another Rust transport |
| Queue saturation | No hidden transport queue; synchronous callers own scheduling | Default FRB thread pool uses an unbounded channel; candidate engine serialization adds mutex contention behind it | Candidate fails the bounded-queue gate |
| Cancellation | No false claim of interrupting executing renders | FRB cancellation is user-supplied; the renderer has no cooperative polling seam | No candidate advantage; preemptive cancellation is unsupported |
| Text measurement and reentry | Synchronous `NativeCallable.isolateLocal` callback with explicit same-engine reentry rejection | FRB Rust-to-Dart callbacks are async `DartFnFuture`; no synchronous adapter was implemented | Candidate fails a material ABI behavior gate |
| Large output | Owned output copy, or 64 KiB sink callbacks; core currently materializes the complete output first | The exact default 2.12.0 candidate used SSE: generated Rust encoded every byte into a second buffer and Dart copied it with `Uint8List.fromList` | No low-copy advantage was demonstrated; neither path is true incremental rendering |
| Sink abort/backpressure | Sink can abort while Dart receives bounded chunks | No equivalent bounded sink was implemented | Candidate fails the streamed-output gate |
| Android/iOS/macOS/Linux/Windows | Existing build scripts, package metadata, verifier, and release workflow own all five targets | No candidate target package or CI fixture passed | Candidate fails package delivery and target CI gates |
| Removal | Not applicable | Isolated directory can be deleted without changing bindings-core | Pass; production closure remains single-transport |

## Local Measurement

The bounded run used an Apple M4 Pro host with 48 GiB RAM, Darwin 25.5.0
arm64, Rust 1.95.0, Dart 3.8.1, and Flutter 3.32.1. The baseline resolved
`ffigen` 20.1.1. The candidate generator and runtime were pinned to FRB
2.12.0. The repository base commit was
`cc39fdcd0f0ea6242ddb68c3093859c456c38844`; the report measures the U15
worktree above that base. Both transports used the same direct native SDK
feature set (`analysis`, `ascii`, `svg`, `png`, `jpeg`, `pdf`,
`layout-cytoscape`, `layout-elk`, `math`, and the compiled system adapters),
deterministic runtime policy,
`trusted-native` resource profile, generated flowchart source, and fresh Dart
process boundaries.

<!-- U15_LOCAL_RESULTS_BEGIN -->

| Measurement | ABI 3 + `ffigen` | FRB 2.12.0 candidate |
| --- | ---: | ---: |
| Release dylib, unstripped | 31,158,640 bytes; SHA-256 `0e8931132a38bf2781d66b1212c4d117fdd058f05c24a94c65f221c6c89271b8` | 31,578,992 bytes; SHA-256 `48c9a1b5e2000bc631663377437952e8019e12ccd0ffa5f3b83656e21ead8b93` |
| `strip -x` dylib | 27,370,464 bytes; SHA-256 `69f93c4e6c7b4f40dd5d80bb302ad3b43ec48c4557a5ae841c21cbf30d450730` | 27,688,304 bytes; SHA-256 `ea71145b82e117aa291f98739c300dd7be3d213dff3f739548477811f4a5771d` |
| Exported global symbols | 1 | 27 |
| Transport-generator output | 1 file / 637 lines / 19,188 bytes | 5 files / 1,966 lines / 69,677 bytes |
| All baseline generated helpers | 2 files / 716 lines / 21,942 bytes | Candidate count above excludes any equivalent handwritten policy/resource facade |
| Normalized `cargo tree` output | 561 non-empty lines | 648 non-empty lines |
| Additional normalized crate names | baseline | about 41 bridge/runtime/support crates |
| Large fixture source | 54,885 bytes | 54,885 bytes |
| Large SVG output | 1,166,405 bytes | 1,166,405 bytes |
| Five-run output SHA-256 | `996052b110178a5c43a7485edbd8ecc5de3067f0308749c536be5e8602eb7678` in every run | Same in every run |
| Five-run per-call p50 | 51.623-52.772 ms | 52.877-54.367 ms |
| Five-run per-call p95 | 52.541-166.336 ms | 54.156-71.563 ms |
| Five-run 20-call wall time | 1.029-1.472 s | 1.059-1.138 s |
| Five-run fresh-process wall time | 1.576-2.489 s | 1.877-1.982 s |
| Five-run sampled peak RSS | 375,136-408,832 KiB | 419,280-463,792 KiB |
| Five-run post-run RSS | 384,204,800-417,300,480 bytes | 417,087,488-431,357,952 bytes |
| Five-run 1 ms UI timer ticks | 0 in every run | 983-1,087 |

The FRB dylib was 420,352 bytes (1.349%) larger before stripping and 317,840
bytes (1.161%) larger after `strip -x`. Its generated surface was 3.086 times
the raw `ffigen` line count and 3.631 times the raw generated bytes. The
generated-count comparison excludes the handwritten Dart policy facade from
both sides.

Across the five paired runs, the median of each run's p50 was 51.905 ms for
ABI 3 and 53.846 ms for FRB (+3.74%). Median 20-call wall time was 1.038 s and
1.088 s (+4.85%); median fresh-process wall time was 1.630 s and 1.906 s
(+16.91%). Median sampled peak RSS was 379,552 KiB and 437,584 KiB (+15.29%),
and median post-run RSS was 388,169,728 bytes and 426,098,688 bytes (+9.77%).
One baseline run had a scheduler outlier, so the ranges and medians are retained
instead of claiming a general throughput winner. The UI signal was unambiguous:
the synchronous baseline blocked the Dart timer for the complete measured loop,
while FRB kept it ticking approximately once per millisecond.

| Run | Transport | p50 (ms) | p95 (ms) | 20-call wall (s) | Process wall (s) | Sampled peak RSS (KiB) | UI ticks |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | ABI 3 | 51.623 | 52.648 | 1.033 | 1.630 | 377,792 | 0 |
| 1 | FRB | 52.877 | 64.693 | 1.088 | 1.906 | 437,584 | 1,068 |
| 2 | ABI 3 | 51.905 | 52.541 | 1.029 | 1.606 | 375,136 | 0 |
| 2 | FRB | 53.846 | 55.199 | 1.074 | 1.903 | 463,792 | 1,073 |
| 3 | ABI 3 | 52.358 | 52.910 | 1.045 | 1.576 | 379,552 | 0 |
| 3 | FRB | 52.981 | 54.156 | 1.059 | 1.877 | 457,424 | 1,058 |
| 4 | ABI 3 | 51.805 | 53.447 | 1.038 | 1.637 | 392,592 | 0 |
| 4 | FRB | 54.367 | 71.563 | 1.138 | 1.982 | 434,176 | 983 |
| 5 | ABI 3 | 52.772 | 166.336 | 1.472 | 2.489 | 408,832 | 0 |
| 5 | FRB | 54.356 | 55.759 | 1.089 | 1.933 | 419,280 | 1,087 |

The candidate normal smoke completed naturally in 0.641 s and covered bridge
version, direct bindings-core ownership, runtime catalog version, constrained
resource options, SVG output, metadata, structured unknown-output errors,
idempotent disposal, and `isDisposed`. A second process deliberately invoked an
async method after disposal. It received the expected disposed-object error and
printed its success marker, but still had not exited after 5.008 s and was
terminated. The ordinary smoke remained clean, isolating the failure to the
post-disposal request path.

The successful candidate build was warm (Cargo reported 0.89 s). The baseline
build reported 36.17 s under a different cache state, while the candidate's
earlier dependency compilation exceeded one minute before an unrelated source
visibility error. Those samples are not comparable cold-build evidence and are
therefore recorded only as provenance, not as a performance result.

Installing `flutter_rust_bridge_codegen 2.12.0` with its upstream lock emitted
a warning that locked `futures-util 0.3.29` is yanked. The candidate runtime
lock independently resolved `futures-util 0.3.33`, so this is maintainer-tool
provenance rather than a claim that the shipped candidate dylib contains the
yanked release.

<!-- U15_LOCAL_RESULTS_END -->

Timing and sampled RSS are diagnostic, not portable performance promises. The
admission decision does not depend on declaring a latency winner: the candidate
already fails callback, streaming, package, and target-CI requirements.

## Platform Delivery

| Target | ABI 3 package owner | FRB candidate evidence |
| --- | --- | --- |
| Android arm64/x86_64 | Gradle plugin plus packaged `libmerman_ffi.so` slices | Not integrated or tested |
| iOS device/simulator | CocoaPods metadata plus `MermanFFI.xcframework` | Not integrated or tested |
| macOS arm64/x86_64 | CocoaPods metadata plus dylib/XCFramework artifacts | Host dylib only; no package integration |
| Linux x86_64/arm64 | CMake metadata plus packaged `.so` artifacts | Not integrated or tested |
| Windows x86_64 | CMake metadata plus packaged `.dll` artifact | Not integrated or tested |

The baseline release workflow installs the Rust targets, builds and injects
every artifact, runs Flutter analysis and a dry-run publish, then tests the
packed package. The Flutter owner now has distinct `flutter-ios-native` and
`flutter-desktop-native` target-set recipes. The recipe helper projects the
package, manifest, profile, feature, target, crate-type, and target-triple
contract; each Flutter build script consumes those values and rejects a target
outside its declared set. This closes the prior host-profile ambiguity for the
admitted iOS, macOS, Linux, and Windows targets. It is not a candidate advantage
because FRB supplied no target package or CI fixture at all.

Replacing the incumbent path with FRB would still require a complete
replacement matrix. Upstream cross-platform support is not evidence that
Merman's package contains the right feature closure or passes its owner-specific
probes.

## Maintenance And Closure Cost

The baseline generator owns only raw ABI declarations. Resource options and all
Flutter policy remain in explicit Dart code. The comparison separately records
generated lines and bytes rather than treating the handwritten facade as
generator churn.

FRB 2.12.0's default Rust features include its thread pool, Tokio async runtime,
Dart opaque support, logging utilities, and supporting codec dependencies. Its
stable built-in integration uses Cargokit. At the pinned upstream tag, the
checked-in `flutter_package` example vendors 32 Cargokit files totaling 110,489
bytes and 3,645 lines. The 2.12.0 documentation states that Native Assets was
not stable and that Cargokit source had to be copied into the target repository.
The same tag's troubleshooting guide also documents a Cargokit compatibility
boundary at Flutter 3.32.0. These are additional owned build inputs, not free
transport implementation details.

The exact candidate used FRB's default `full_dep: false` mode, which forces SSE
and does not require LLVM-backed `ffigen` for the bridge. Enabling `full_dep`
would select the CST/DCO path advertised for fewer large-byte copies, but would
also add that generator/toolchain dependency. It is a different configuration,
not a benefit observed in this run, and would not repair the callback,
cancellation, lifecycle, bounded-queue, streaming, or package failures.

The default native `SimpleThreadPool` delegates to `threadpool 1.8.1`, whose job
queue is a standard unbounded `std::sync::mpsc::channel`. FRB allows a custom
handler, but Merman would then own another scheduling implementation merely to
restore the explicit bound required by the product contract.

## Source-Backed Semantics

- FRB 2.12.0 source tag and commit:
  <https://github.com/fzyzcjy/flutter_rust_bridge/tree/v2.12.0>
  (`62b9330ed2f900535e34d8443ff82dc54070579a`).
- Synchronous Rust functions use an internal thread pool for async Dart APIs:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/concurrency/sync-rust.md>.
- FRB initialization and disposal are isolate-aware:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/miscellaneous/isolates.md>.
- `Vec<u8>` uses the bridge codec's typed byte representation:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/miscellaneous/codec.md>.
- Cancellation requires an application-provided token and cooperative logic:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/how-to/cancel.md>.
- Rust-to-Dart callbacks are modeled with async `DartFnFuture`:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/direction/rust-call-dart.md>.
- Opaque objects receive generated disposal support:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/guides/types/arbitrary/rust-auto-opaque/dispose.md>.
- Stable built-in integration uses Cargokit:
  <https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.12.0/website/docs/manual/integrate/02-builtin.md>.

## Baseline Hardening

The useful FRB observation should be applied without introducing a second Rust
transport:

1. Add an optional persistent Dart worker-isolate actor above the ABI 3 facade.
   The worker owns the dynamic library and engine, exposes `Future` methods, and
   uses a bounded request queue with explicit shutdown.
2. Keep cancellation honest. Queued work may be removed; executing work finishes
   and its result may be discarded until bindings-core and renderers gain a
   cooperative cancellation seam.
3. Keep synchronous host text measurement on the engine's isolate. UI-derived
   measurements should be prepared or cached before rendering rather than
   introducing an async callback into a synchronous layout algorithm.
4. Add large-output copy counters and peak-RSS fixtures. If evidence justifies
   true streaming, add a writer seam in bindings-core so every transport gains
   it; changing only the outer bridge cannot remove the complete core `Vec`.
5. Add worker-isolate responsiveness, queue saturation, shutdown, and
   use-after-dispose tests to the Flutter owner gate.

## Reproduction And Cleanup

The candidate was generated with exact `flutter_rust_bridge_codegen` 2.12.0,
built as a release dylib with `default-features = false` and the direct native
SDK feature set, and compared with a release `merman-ffi` artifact. The
benchmark used identical source and engine options, recorded output SHA-256,
latency distributions, timer ticks, process RSS, artifact bytes and SHA-256,
and generated file/line/byte counts.

The baseline declarations were refreshed from `platforms/flutter`:

```text
dart run ffigen --config ffigen.yaml
```

The generator, builds, and closure reports were run from the repository root:

```text
cargo install flutter_rust_bridge_codegen --version 2.12.0 --locked --force
flutter_rust_bridge_codegen --version
flutter_rust_bridge_codegen generate --config-file platforms/flutter/spikes/frb_2_12_candidate/flutter_rust_bridge.yaml
cargo build --release --manifest-path platforms/flutter/spikes/frb_2_12_candidate/Cargo.toml --no-default-features --features analysis,ascii,svg,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,system-timing
cargo build --release -p merman-ffi --no-default-features --features analysis,ascii,svg,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,system-timing
cargo tree --manifest-path platforms/flutter/spikes/frb_2_12_candidate/Cargo.toml --no-default-features --features analysis,ascii,svg,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,system-timing --edges normal --prefix none
cargo tree -p merman-ffi --no-default-features --features analysis,ascii,svg,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,system-timing --edges normal --prefix none
python3 platforms/flutter/spikes/frb_2_12_candidate/harness.py --incumbent-library target/release/libmerman_ffi.dylib --candidate-library platforms/flutter/spikes/frb_2_12_candidate/target/release/libmerman_frb_spike.dylib --iterations 20 --warmups 3 --nodes 800
```

The candidate package checks were run from
`platforms/flutter/spikes/frb_2_12_candidate/dart`:

```text
flutter pub get
dart analyze bin lib
dart run bin/smoke.dart ../target/release/libmerman_frb_spike.dylib
```

The final harness command was run five times sequentially. The lifecycle probe
used the same smoke with `--probe-use-after-dispose` under a 5 s process bound.
Binary sizes and symbols used `stat`, `shasum -a 256`, `xcrun strip -x`, and
`nm -gU` against the two release dylibs.

After recording results, the entire candidate directory was removed. The final
cleanup gate is:

```text
test ! -e platforms/flutter/spikes/frb_2_12_candidate
! rg -n "flutter_rust_bridge|merman-frb-spike|merman_frb_spike" \
  platforms/flutter Cargo.toml Cargo.lock .github scripts capabilities
```

References in this report and the U15 plan are historical decision evidence and
are intentionally outside that production-closure search.
