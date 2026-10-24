# Real WASI reactor hosts

This directory owns real-runtime qualification of `merman-wasi` scalar ABI v1.
It does not build Rust, parse Mermaid, discover system fonts, or grant guest
filesystem access. The parent build lane supplies the **same reactor bytes** to
both hosts. Neither host installs a preopened directory, inherited environment,
network service, ambient random source, or thread service. Rust libc may import
filesystem/stat functions, `random_get`, `clock_time_get`, and `sched_yield`.
Imports are enumerated from the actual module and checked against the explicit
allowlist in `protocol.json`; imports are not confused with granted authority.
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

The same pinned RaTeX font-loader dependency enables `fontdb`'s file/mapping
features transitively in math/full. They are not granted a filesystem: Merman's
WASI font acquisition rejects paths, math selects its embedded KaTeX payload,
`usvg/system-fonts` remains forbidden, and both hosts run without preopens.
Standard additionally rejects those `fontdb` features in its dependency closure.
Do not confuse compiled third-party helpers with ambient discovery permission.

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

The small WAT modules in Python tests exercise import/export rejection and
actual no-preopen/no-environment/fixed-entropy host policy, not Mermaid behavior.
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
