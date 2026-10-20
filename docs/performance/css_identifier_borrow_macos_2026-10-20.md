# Borrowed CSS identifier serialization — October 20, 2026

## Decision

The registered allocation, behavior and validation gates pass after a test-only formatting repair.
This is an adjacent allocation/work-removal experiment without a latency or v0.9.0 convergence claim.

## Change

The shared CSS identifier serializer allocated a temporary `String` even when serialization kept
every input byte. It now returns `Cow<str>`: unchanged input is borrowed, and the first character
requiring transformation creates the output owner and copies the preceding prefix. The original
serializer remains the sole classifier. No family allow-list, caller sanitizer assumption, cache,
extra scan or alternate identifier semantics is introduced. The unchanged-input scan remains O(C).
Transformed inputs can still grow their output allocation more than once.

Unicode scalar positions retain the first/second-character rules; separate UTF-8 byte offsets keep
prefix slices valid. NUL, control characters, XML metacharacters, lone hyphen and punctuation keep
their existing escape order and spelling. XML attributes, fragments, ID projections, writer calls,
resource checks and cancellation remain in their existing consumers. Class, ER and Requirement
only adapt escaped-local accessors to the borrowed-or-owned return type. Sequence and shared CSS
test accessors receive corresponding type adaptations.

The clean base is `7e058230739b75c4fa38313e82e5b2c377f52896`, after the independent scoped appearance
repair. The seven-file patch SHA-256 is
`de5c91a955f615148fdb1419d0bd308f76d3e4199364f3217996c9a82d69b269`.
Four files change runtime code; three other files change tests. Existing hostile-ID integration
coverage gains Packet, Cynefin, ER, GitGraph and Venn rows alongside existing Class and Kanban rows.
Before applying runtime changes, this expanded existing test passed on the original serializer.
The two existing owner tests preserve the original golden bytes and add independent Unicode,
control, XML, position and borrowed/owned assertions; there is no serializer mirror.

## Registered measurement

The operation is complete public `Renderer::render(RenderRequest::svg(...))`, built with all
features in release mode on macOS / Apple M4 Pro / Rust 1.95.0. Compiled themes are prepared before
measurement; one warmup precedes five saved samples. The complete rich SVG result remains owned
at the memory checkpoint. Packet medium / None must strictly decrease allocation count; all four
metrics must not increase on jointly stable controls. Variable controls stay inconclusive.

The 645 site controls, 418 catalog cells, ordered-measurer probes and 26 complete error oracles
retain the current design input identities and measurement windows from the
[scoped appearance checkpoint](scoped_appearance_path_macos_2026-10-20.md). All baseline observations
are freshly captured for this clean revision. Legacy authoring controls remain retired/replaced;
this experiment makes no equivalence claim to those historical themed inputs.

## Results

| Packet medium / None primary | Base | Candidate | Change |
| --- | ---: | ---: | ---: |
| Allocation count | 109 | 108 | -1 |
| Allocated bytes | 19136 | 19117 | -19 |
| Peak growth bytes | 11742 | 11742 | 0 |
| Retained growth bytes | 3951 | 3951 | 0 |

All 418 catalog cells are stable. Of these, 132 improve allocation count and allocated bytes,
286 retain all four metrics, and none increase. Improvements remove one or two allocations and
19–38 bytes; peak and retained growth stay unchanged. The improving fixture aliases cover Class,
ER, Requirement, GitGraph, Packet, Kanban, Venn, Cynefin and all four Railroad aliases.
Warmup result metadata and all five saved sample SVG/report identities match.

All 645 site final SVG/report identities and independent ordered-measurer traces match. Of 504
jointly stable site controls, 50 improve and 454 retain all four metrics; 141 variable controls
remain inconclusive without resampling. All 26 complete errors match. These are allocation
observations, without a measured clock improvement.

## Validation

| Check | Result |
| --- | --- |
| Expanded existing hostile-ID test on original serializer | 1 pass before runtime changes |
| Focused CSS/identifier/theme/resource/cancel/palette release tests | 1644 pass |
| Four-crate all-features release nextest | 8100 pass, seven skipped, including final-format repeat |
| Matching strict release Clippy | Pass with `-D warnings` |
| Full 37-entry DOM matrix | Pass under existing policies; fresh reports archived |
| Optimized full WASM, including wasm-opt | Pass, including final-format rebuild; both archives preserved |
| Release theme acceptance | 34 pass |
| Format, diff and indexed path length | Pass after test-only formatting |

The original serial run passed behavior, tests, Clippy, DOM, WASM and acceptance, then failed
rustfmt because three test expressions needed wrapping. `cargo fmt` changed only two `cfg(test)`
assertions and the Packet fixture tuple. The runtime CSS owner prefix is byte-identical; no
production function, test literal, oracle or allocator sample changes. The original failed run
and archived artifacts remain intact. `format-repair.json` records both changed source hashes;
all five supplemental stages return zero: format, diff, indexed paths, full nextest (8100, seven
skipped), and optimized full WASM. The original session still records its real fmt failure. The
formatting patch SHA-256 is
`bb81c5ce44f574c02a491cdd4861a1326c6ce08a27e2de75af781cce06bd7708`.

Final source verification covers 15,811 files: 15,809 remain byte-identical to the measured freeze,
and the two explicit test-format hash overrides match. All 938 frozen inputs and four baseline
executables remain unchanged. Seven final-format WASM artifacts are separately archived; all
1,217 manifest input hashes match the final source. WASM validation is a build check, without a
browser runtime claim.

The ignored experiment is `target/bench/experiments/css-identifier-borrow-current-20261020/`.
It retains registration and the ownership metadata clarification, 938 frozen input hashes, fresh
baseline and candidate samples, exact SVG/report/error controls, ordered-measurer traces, frozen
baseline executables and independent reviews. Cargo/native stages share the existing target and
run serially. No benchmark dependency, tracing or CI benchmark job is added.

## Remaining work

The latest formal v0.9.0 timing still belongs to `a6742779c`; it does not measure these repairs or
the prepared-render main integration. Refresh all 76 output identities using the reviewed common
public harness, then run fresh A/A and fixed AB/BA on the prospectively selected exact-SVG cohort.
The all-family theme convergence goal remains incomplete.
