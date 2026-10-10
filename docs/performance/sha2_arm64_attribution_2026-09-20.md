# SHA-256 ARM64 attribution — 2026-09-20

## Finding

The current native SVG profile has substantial SHA-256 work, and the selected dependency backend
is a concrete candidate for further investigation. This checkpoint changes no production
dependency or hashing contract and admits no render-latency improvement.

At source `2db16c6b7`, the default-SVG Cargo lane selects sha2 0.10.9 with only `default` and `std`.
Its pinned source uses the software SHA-256 backend on ARM64 unless the `asm` feature is enabled.
The current root lockfile also contains sha2 0.11.0 through lopdf 0.44.0; that version selects
ARM64 SHA instructions at runtime by default and retains a software fallback.

A fresh 10-second CPU sample of the frozen current Class-medium pipeline executable locates
SHA compression under both the resource fingerprint and the public output digest. These hashes
serve different contracts: the resource fingerprint includes its domain, length-prefixed SVG,
font-catalog identity and font-source priority; the public artifact digest is SHA-256 of output
bytes. The public/native artifact digest is already reused when those byte strings are equal.
The profile does not justify deleting either digest or changing its framing.

## Isolated measurement

An ignored standalone Cargo probe compares the exact cached versions 0.10.9 and 0.11.0 without
editing workspace manifests or lockfiles. It hashes the same 74,203-byte Class SVG retained by the
previous allocation experiment, plus its first 32 bytes. Each input warms both implementations
for 1,000 calls, then executes eight batches in old/new/new/old/old/new/new/old order, 5,000 calls
per batch. Results pass through `black_box`. The values below average four batch means per version.

The same source is rebuilt with `--cfg sha2_backend="soft"` as a registered attribution control;
this selects the software backend in 0.11.0. It does not change 0.10.9's default software path.
Both executables check equal digests for 12 input-prefix lengths, including SHA padding/block
boundaries, and check the standard SHA-256 `abc` value with the old implementation. These are
bounded probe checks, not cryptographic implementation certification.

| Input bytes | 0.10.9 default µs | 0.11.0 default µs | 0.10.9 in control µs | 0.11.0 forced software µs |
| --- | ---: | ---: | ---: | ---: |
| 32 | 0.144 | 0.026 | 0.106 | 0.100 |
| 74,203 | 119.015 | 26.077 | 118.924 | 110.284 |

For the larger input, forcing the new version back to software removes most of its observed
advantage. This supports backend selection as a causal explanation for the isolated hash cost.
The smaller input is sensitive to batching and code generation; its absolute figures are
diagnostic. Neither row can be added directly to a render-time estimate: real digest framing,
other stages, instruction-cache effects and feature unification must be measured together.

Host: Apple M4 Pro, macOS 26.6.2 ARM64, Rust 1.95.0, release profile. Builds and measurements run
serially with one Cargo job. No C/assembly dependency was downloaded or added. The temporary
probe reuses the repository target directory and cached crates.

## Next admission boundary

Evaluate upgrading the workspace-owned SHA dependency to the already-locked 0.11.0 as a new,
separate candidate. Its published crate metadata requires Rust 1.85, below this repository's
1.95 floor, but that alone does not prove package compatibility. Before retaining a change:

- Preserve every existing digest byte and serialized receipt, including theme, document,
  target-admission, binding and resource identities.
- Freeze an adjacent baseline and candidate, then run public Class and cross-family output
  controls, A/A calibration and at least eight balanced confirmation pairs under the existing
  relative/absolute latency gate.
- Review the complete dependency/feature delta, each standalone workspace lockfile, generated
  license projections, and minimal/native/WASM compile surfaces. Keep unexecuted host routes
  explicitly Unverified.
- Measure allocation and artifact-size controls; an isolated hash improvement does not admit
  an application, package-size or cold-start claim.

Enabling 0.10.9's `asm` feature is another possible experiment, but it introduces the optional
sha2-asm dependency and changes the build closure. It was not built or measured here. No forced
CPU target, local cryptographic implementation, cache, weaker digest or budget increase is proposed.

## Evidence

`target/bench/experiments/sha2-arm64-attribution-20260920/` retains the standalone source and
lockfile, preregistration, build logs, both frozen executables, 32 raw batch rows, and summary.
The ledger binds source, input, executable and lockfile digests. Reproduction uses the saved
manifest with `cargo build --release --offline --manifest-path <manifest>` and runs the resulting
probe with the saved Class SVG path; the control additionally sets the Rust cfg above.
The combined summary SHA-256 is
`88d6d6c385e609820d3da6053fcf5c88cca713c6ce2a7c9c70f4cbde69c6aec1`.

The source profile and actual Cargo feature tree are retained in
`target/bench/experiments/prepared-text-direct-copy-20260920/`. That directory also records a
rejected per-tag-copy candidate: Class never enters its changed branch. See the
[attribute-validation checkpoint](svg_attribute_validation_2026-09-20.md) for that applicability
finding and the earlier accepted allocation reduction. The Class release regression and C7a/U10
gates remain open.
