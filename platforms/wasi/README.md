# Real WASI reactor hosts

This directory owns real-runtime qualification of `merman-wasi` scalar ABI v1.
It does not build Rust, parse Mermaid, discover system fonts, or grant guest
filesystem access. The parent build lane supplies the **same reactor bytes** to
both hosts. Neither host installs a preopened directory, inherited environment,
network service, ambient random source, or thread service. Rust libc may import
filesystem/stat functions, `random_get`, `clock_time_get`, and `sched_yield`.
Imports are enumerated from the actual module and checked against the ordinary
`allowed_imports` or the separate, exactly typed `denied_dependency_imports` in
`protocol.json`; a dependency denial is not a filesystem grant.
Both hosts override random bytes with zeroes, time with zero nanoseconds, and
yield with a no-op. These deterministic shims are smoke policies, **not** secure
randomness and not a production cryptographic host. Actual shim calls are
recorded. Path functions see no guest preopens; host-only inputs use raw ABI bytes.

Math/full have a reviewed dependency-export residual: RaTeX's pinned embedded-font
closure retains several wasm-bindgen allocation/closure helpers even without a
JavaScript import. `optional_dependency_exports` records their exact scalar shapes;
they are implementation symbols, not callable Merman SDK APIs. Unknown exports,
changed signatures and any non-WASI import still fail qualification. Standard must
have none of those helpers. Removing that transitive closure requires an upstream
RaTeX font-loading feature split, not a local fake JavaScript host or a second math
implementation. The formula SVG/PNG smokes prove the supported path in both real
non-JavaScript runtimes.

The same pinned RaTeX 0.1.15 font-loader dependency enables `fontdb`'s file/mapping
features transitively in math/full. Embedded fonts cover KaTeX; they do not disable
Unicode fallback. Missing glyphs and CJK can reach `ratex-unicode-font`'s environment
and built-in system-path probes, including `canonicalize` / `realpath` / `readlink`.
This is a reachable dependency path, not merely an unused libc symbol or a proven
Rust-toolchain-only change. Merman's WASI font constructor still admits bytes only,
`usvg/system-fonts` remains forbidden, and both hosts run without preopens or
inherited environment. No system font can be acquired through those probes.

The reviewed residual `path_readlink` is declared only in
`denied_dependency_imports`, with six `i32` arguments
(`fd, path, path_len, buf, buf_len, bufused`) and one `i32` result. Math/full may
link that exact import. Standard rejects it before instantiation, and unknown
imports or changed residual signatures remain errors. Both hosts always override
this import with a counter-only shim returning preview1 `NOTCAPABLE` (76). The
shim receives neither a caller nor a memory/module handle, ignores every argument,
and touches no guest memory or filesystem, including `bufused`. The ordinary
allowlist is unchanged. Standard also rejects the transitive `fontdb` file/mapping
features in its dependency closure. Removing the fallback code itself requires
an upstream font-loading split; this host policy preserves the pinned dependency.

## Workflow interface

Requirements: Python 3.11+ and Go 1.25+. Python dependencies are pinned in
`requirements.txt`; Go dependencies and checksums are pinned in
`wazero/go.mod` and `wazero/go.sum`. Prepare both runtimes without Cargo:

```console
python -m pip install -r platforms/wasi/requirements.txt
go -C platforms/wasi/wazero mod download
```

The workflow entrypoint runs both hosts serially, verifies equal actual catalogs
and decoded PNG pixels, and only then publishes `--catalog-out`:

```console
python platforms/wasi/smoke.py --module target/example/merman_wasi.wasm --flavor standard --catalog-out target/example/catalog.raw.json --output-dir target/example/smoke
```

It uses the fixed subdirectories `wazero/` and `wasmtime/`. Each contains raw
catalog bytes (`runtime-catalog.raw.json`), `smoke.json`, SVG/PNG bytes and
metadata. The output root contains aggregate `smoke.json`. A failed subprocess
does not fall back to a substitute runtime or parser.

The stable CLI for both individual hosts is:

```text
--module PATH --flavor standard|math|full --catalog-out PATH --output-dir PATH
```

From the repository root, use:

```console
go -C platforms/wasi/wazero run . --module /absolute/path/merman_wasi.wasm --flavor standard --catalog-out /absolute/path/wazero-catalog.json --output-dir /absolute/path/wazero
python platforms/wasi/wasmtime_smoke.py --module /absolute/path/merman_wasi.wasm --flavor standard --catalog-out /absolute/path/wasmtime-catalog.json --output-dir /absolute/path/wasmtime
```

Relative CLI paths are relative to the caller's current directory (for Go's
`-C`, that is `platforms/wasi/wazero`). Use absolute artifact/output paths in CI.
`--output-dir` is a **host-only** output location, never a guest preopen.
Success prints one JSON object to stdout and writes `smoke.json` in the output
directory; failure exits nonzero and prints a JSON failure object. Use separate
output directories per runtime/flavor. The catalog file contains unmodified
runtime catalog bytes, not a feature list inferred from the filename.

`fixtures.json` is the shared input authority for both hosts and a native C
consumer. Source strings are UTF-8 bytes, without newline normalization. Font
paths are repository-root-relative; concatenate their bytes in listed order
for the WASI constructor and describe each slice in its `fonts` array. Native C
passes those same fonts as separate slices. Use `bundled_fonts: false` (native
`use_bundled_fonts = 0`), `engine_options` at construction, and `png_options`
as the PNG operation overlay. This makes font precedence, diagram IDs and 2x
scale identical across transports. No font bytes or PNG bytes use base64.
`engine_options.resources.profile` explicitly selects `interactive` for native
pixel parity. Both WASI hosts additionally construct a math engine without any
options and render a formula without request options, proving default-policy
admission separately from the shared parity inputs.

`basic.svg`, `basic.png`, their metadata JSON, and (for math/full) `math.svg`
and `math.png` are retained for native C pixel comparison. Evidence binds the
module, fixtures and fonts by SHA-256. It distinguishes PNG file hashes from
decoded, unpremultiplied RGBA8 pixel hashes; compare dimensions and RGBA bytes,
not compressed PNG bytes. A successful local adapter unit test is not a
qualification claim for a production reactor. The parent owns final real
artifact qualification and native pixel equivalence.

Math/full additionally submit `math_missing` (a Cyrillic glyph absent from KaTeX)
and `math_cjk` formulas as both SVG and PNG requests. Their source bytes and each
actual output/metadata pair or ordinary structured error are retained. The report's
`dependency_font_probes` distinguishes `rendered` from `owned-error`; successful
SVG evidence records native/unavailable markers without claiming complete glyph
coverage. A successful PNG records whether a successful SVG supplied its viewport
comparison; a declined SVG does not manufacture that evidence. Only
render/unsupported-operation errors are admitted for these probes;
parse errors, host failures and unrelated resource errors fail the scenario.
Both hosts must agree on outcomes, error payloads and decoded successful PNGs.
These dependency probes do not replace the separate visible red-formula oracle.

`denied_dependency_import_calls` records actual shim calls, with per-request deltas
in the probe records. A zero count is valid: libc may fail before `readlink` when
there are no preopens, and RaTeX caches failed discovery. The synthetic regression
calls the real denial directly with valid and invalid pointers, asserting return
76 and unchanged whole guest memory. Production counters are never incremented
artificially. Standard reports these math probes as not applicable.

## Assertions and authority

Both hosts decode ten little-endian u32 fields, call `_initialize` once, audit
actual function exports/imports before instantiation, and release each input
allocation after the synchronous call. The wasmtime audit additionally checks
the complete export table (including optional immutable linker globals) and
rejects imported non-function objects or a non-wasm32/shared exported memory.
Host memory is limited to 512 MiB, calls are bounded by a 120-second instance
deadline, and wasmtime additionally grants five billion fuel units per call.

The serial scenario checks raw catalog capability IDs, separate native-font
provider/constructor-service IDs, all advertised named metadata, real SVG
viewport metadata, 2x PNG dimensions and pixels, constructor font-copy ownership,
parse-error recovery, independent live results, and survival after engine close.
Standard must report structured missing `math` for SVG and PNG. Math/full must
produce native formula SVG and PNG, reject explicit constrained request and
constructor policies, then recover. Standard reports constrained math testing as
not applicable rather than claiming an unavailable renderer passed that test.

ELK follows KTD11's family semantics: standard/math retain a catalog without
`layout-elk`, and frontmatter `layout: elk` must fall back to **byte-equivalent
explicit Dagre SVG**. Full must advertise and plan `layout-elk`. Both use the
real `svg-plan-json` operation; hosts never invent a missing-capability error or
change a Mermaid source to conceal family fallback.

## Direct CLI comparison

`cli_compare.py` consumes a built `cli-release` executable and the actual WASI
module, with identical explicit Cousine bytes, deterministic runtime policy,
interactive resources and diagram ID. It compares byte-identical resvg-safe SVG
and decoded PNG dimensions/RGBA8 at 2x with an explicit background. Inputs cover
the shared basic fixture, the wrapped Swimlane labels from issue #242, and a
formula for math/full. Common cases explicitly select Dagre; full additionally
compares explicit ELK. This avoids equating different optional-loader defaults.
It builds nothing and reads no host fonts implicitly.

```console
python platforms/wasi/cli_compare.py --cli /path/merman-cli --module /path/merman_wasi.wasm --flavor standard --output-dir target/cli-wasi-comparison
```

The output directory must not already exist. Outputs and `comparison.json`
record the exact binary/module/font/source hashes. The SDK release owner builds
the existing CLI release recipe once and runs this check for all three extracted
WASI flavors, separately from the native C comparison and packaged examples.
These are representative source-backed checks, not whole-corpus byte equality
with the CLI or Mermaid.js.

## Native C comparison

`native_smoke.py` drives the parent's already-built C probe. It never builds
Cargo or C. The probe accepts:

```text
PROBE FONT_DIRECTORY SOURCE_FILE OPTIONS_FILE OPERATION OUTPUT_FILE METADATA_FILE CATALOG_FILE
```

The probe exits 0 for success, 3 for an ordinary structured core error, or 1 for
a host failure. `OPTIONS_FILE` contains the shared engine options plus the PNG
overlay where applicable. The optional WASI comparison consumes both fixed
runtime subdirectories and compares raw decoded RGBA8 pixels:

```console
python platforms/wasi/native_smoke.py --probe target/merman-sdk-probe --flavor standard --catalog-out target/native-catalog.raw.json --output-dir target/native-smoke --wasi-dir target/example/smoke
```

## Focused tests

```console
go -C platforms/wasi/wazero test -p 1 ./...
python -m unittest discover -s platforms/wasi -p 'test_*.py' -v
python -m py_compile platforms/wasi/common.py platforms/wasi/smoke.py platforms/wasi/wasmtime_smoke.py platforms/wasi/native_smoke.py
```

The small synthetic modules in Python/Go tests exercise import/export rejection,
flavor and signature restrictions, fixed-failure/no-write dependency denial, and
the actual no-preopen/no-environment/fixed-entropy host policy, not Mermaid behavior.
They are never passed off as production-reactor qualification.

## Dependency provenance

- wazero `v1.12.0`: the published
  [upstream Go module](https://proxy.golang.org/github.com/tetratelabs/wazero/@v/v1.12.0.mod)
  declares Go 1.25 and `golang.org/x/sys v0.44.0`. `go mod tidy` verifies
  downloads against the Go checksum database; no sum verification is disabled.
- wasmtime `50.0.1`: the official
  [PyPI release metadata](https://pypi.org/pypi/wasmtime/50.0.1/json)
  supplies the platform runtime wheel. The smoke records the installed version.
- Pillow `13.0.0`: the official
  [PyPI release metadata](https://pypi.org/pypi/Pillow/13.0.0/json)
  supplies the PNG decoder; this pin requires Python 3.11+.
- Fonts and OFL provenance remain in
  `crates/merman-export/assets/fonts/manifest.json`; fixtures reference those
  production bytes, not test font copies or host font discovery.
