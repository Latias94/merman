# Current standalone SVG finalization attribution — Windows, 2026-10-16

## Scope and result

At committed `e56dde946`, measure the public `RenderedFamilySvg::finalize_standalone(None)`
operation independently of parsing, preparation and family emission. This is current-head
attribution only. It does not admit an optimization, non-regression, CPU percentage or v0.9.0
ratio. v0.9.0 has no equivalent ordinary standalone API; its None path returns a String directly.

The Windows 11 / Intel Core i7-11700 probe links the exact Rust 1.95.0 opt-level 3 libraries from
the preceding locked native benchmark build, default features plus SVG. The resource policy is
`trusted_native`, with finite existing ceilings; every effective limit is recorded in the protocol.
No selected typed theme, explicit postprocessor, resvg-safe or raster lane is included. The next
reserved-prefix candidate is excluded from this artifact.

| Fixture | SVG bytes | Mean us | Median us | Min–max us |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 77,419 | 750.012 | 767.397 | 589.356–875.897 |
| class_tiny | 16,936 | 103.920 | 103.023 | 100.816–108.819 |
| journey_medium | 12,224 | 106.658 | 101.406 | 95.331–128.488 |
| kanban_medium | 21,557 | 80.281 | 81.183 | 76.094–82.666 |

There are four batches of 32 operations and 8 warmups per fixture, including the identity reference
as warmup one: 16 samples, 512 timed calls and 32 warmup calls in one fixed invocation. All outputs
match exact SVG bytes/SHA-256, resource fingerprint and Unverified terminal status. Native process
snapshots before and after the invocation contain no Cargo/rustc/link/cl process. They are boundary
observations, not continuous host isolation.

The Class-medium batches vary 589–876 us; Journey varies 95–128 us. Four batches do not establish
stable population estimates. Do not divide these values by an independently measured end-to-end
mean, subtract them from another lane or turn them into exclusive CPU attribution. The evidence
supports investigating standalone finalization as a substantial absolute-work frontier, without
asserting that it is the sole or dominant regression cause.

## Timer boundary

Parse, session creation, parsed-input clone, prepare and family rendering occur before timing.
Both input and output containers are allocated before timing. The timed loop consumes 32
RenderedFamilySvg values and pushes results into a reserved Vec. It includes the public call,
loop/black-box overhead, reserved push and destruction of consumed input fields not transferred
into the result. The exhausted input iterator allocation and all output artifacts/sessions stay
alive until after elapsed time is captured. Hash/identity checks, logging and result destruction
occur outside timing. This is a public-call batch measurement, not callee-only CPU time or facade
completion cost.

## Source-backed next boundary

For default pipeline None and BestEffort, finalization computes the resource fingerprint over
N SVG bytes plus its framing/font/policy values, walks complete XML to collect validation/reference
facts, then consumes those facts for expansion and budget admission. The facade separately computes
an artifact SHA-256 over the same N bytes. These distinct contracts account for 3N complete SVG
stage-input bytes across standalone plus facade; this is not a count of physical memory reads or
internal XML scalar/attribute work. The earlier reserved-prefix observation is outside this timer.
The two digests bind different contracts and cannot be merged merely because both read SVG.

Existing RenderTimings.finalize_svg names family-internal root assembly. It does not time this
public standalone/XML/reference boundary. The existing public render-stage benchmark combines
prepare and emission, while its end-to-end lane includes facade finalization. Future finer
attribution requires a valid sampling tool or explicit opt-in private diagnostics around existing
owners, preserving validation, errors, limits, cancellation and output.

## Evidence and limits

All source, registration, fixture hashes, build command, Cargo-artifact/library identities,
frozen executable, raw JSONL and process snapshots remain under
`target/bench/experiments/terminal-attribution-20261016/`. The source SHA-256 is
`5dc94b02639042607d0e56cb5ebf7455a112b5f81b664ad5756f8dcd0c6947e8` and executable SHA-256 is
`32ef202940d082ca7b2c7d2dbc13d11d471d9d6610fca0b2685ab9da04deaeab`. Independent source and data
review pass. The executed wrapper checks returncode 0 and guard order, but its source was not frozen
in this directory at launch. The later receipt supplement explicitly records that provenance
limit and the observed return code, without rerunning the fixed experiment.
