# Standalone SVG terminal phase attribution — Windows, 2026-10-16

## Result and scope

The complete XML walk and reference-fact collection is the largest of the three instrumented
segments on all four default fixtures. Investigate its attribute processing next. This is
successful-fixture wall-clock attribution, not an optimization, exclusive CPU profile,
end-to-end percentage, v0.9.0 comparison or non-regression admission.

The temporary worktree is based on `e56dde946`, using Windows 11 / Intel Core i7-11700,
Rust 1.95.0, native default features plus SVG and the locked opt-level 3 benchmark build.
The unchanged public `finalize_standalone(None)` probe uses deterministic layout and the
finite `trusted_native` resource profile. There is no selected typed theme or explicit
postprocessor. Only private timers and stderr events are added; these hooks are not a
production change.

| Fixture | SVG bytes | Elements | Fingerprint mean / median us | XML + facts mean / median us | References mean / median us |
| --- | ---: | ---: | ---: | ---: | ---: |
| class_medium | 77,419 | 576 | 42.920 / 39.200 | 487.363 / 461.950 | 21.276 / 19.150 |
| class_tiny | 16,936 | 95 | 9.165 / 8.800 | 77.659 / 74.300 | 6.380 / 5.900 |
| journey_medium | 12,224 | 96 | 6.995 / 6.400 | 83.573 / 80.400 | 3.671 / 3.000 |
| kanban_medium | 21,557 | 98 | 11.563 / 11.000 | 61.009 / 59.150 | 0.228 / 0.200 |

Each cell summarizes 128 calls after eight warmups, grouped into four batches of 32 per
fixture. The run contains 512 timed calls, 32 warmup calls and 1,632 phase events. Class-medium
XML batch means are 478.578, 475.944, 472.028 and 522.900 us; individual calls range
419.2–673.9 us. Keep raw variation with the means rather than interpreting them as stable
population estimates. This one fixed invocation does not establish a scaling law or attribute
the XML segment's time to a specific internal helper. Kanban reference observations of
0.1–0.9 us are near timer granularity and support only a small-order-of-magnitude observation.

## Instrumentation boundaries

- Resource fingerprinting includes the existing SHA initialization, domain framing, SVG/font/
  policy inputs and finalization inside `fingerprint_svg_resources`.
- XML + facts surrounds the existing complete
  `visit_well_formed_svg_with_controls::<true>` call. It includes XML event/attribute validation,
  callbacks and reference-fact observation. Initial container construction and subsequent local
  container destruction are outside this segment.
- Reference analysis surrounds the successful ordinary
  `analyze_svg_reference_facts_with_checkpoints::<false, _>` call. The later final checkpoint and
  structure-budget check are outside this segment.

Elapsed time is captured before writing each JSON event to stderr. Timer overhead, changed
code generation and intervening stderr writes can perturb surrounding work and subsequent
segments. Do not compare the instrumented outer operation to the earlier normal-binary
[standalone measurement](native_standalone_attribution_windows_2026-10-16.md), divide either
by independently measured E2E time, or use their difference as a CPU percentage. Error, cycle
and cancellation paths may return before a successful phase event; missing events are not
zero-time observations. Instrumentation is unsuitable for measuring production cancellation
latency.

## Correctness and provenance

The original XML, security, error, checkpoint and resource-budget order is unchanged. All four
SVG byte strings, source/SVG SHA-256 values, resource fingerprints and Unverified terminal
statuses match the normal-head probe. Each call produces the expected ordered event triple;
XML byte counts and element counts remain consistent. The process exits zero. Native
Get-Process snapshots before and after the invocation contain no cargo/rustc/link/cl process;
these boundaries do not provide continuous host isolation.
Independent source and data review pass, including recomputation of every phase summary and
verification of contiguous fixture/event order rather than byte-length grouping alone.

The source, patch, optimized library identities, executable, frozen runner, raw stdout/stderr,
summaries and snapshots remain under
`target/bench/experiments/svg-terminal-phase-profile-20261016/`. The probe source SHA-256 is
`5dc94b02639042607d0e56cb5ebf7455a112b5f81b664ad5756f8dcd0c6947e8`; the instrumented executable
SHA-256 is `662c6754f2b7bc451bc38c6b1c0834e7d50c567549f93d0b76b15051a7aa7eb8`. The initial artifact
selector fails before compiling or executing the probe because Cargo paths contain mixed
slashes. Its failure is retained. The corrected selector chooses the exact sha2 0.11.0
opt-level 3 runtime artifact; this is setup correction, not a measurement rerun.

The next experiment is exact-name routing inside the reference-attribute owner, preserving
complete preceding XML validation and marker checkpoints. It is a constant-factor latency
hypothesis; fewer no-op calls alone cannot admit it. A broader event-derived writer/facts
boundary remains separate work requiring a complete XML proof and mutation invalidation.
