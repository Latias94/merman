# Fuzzing

Merman uses `cargo-fuzz` and libFuzzer for panic and sanitizer coverage across the parser, the
headless renderer, and the `resvg-safe` SVG pipeline.

The fuzz project is intentionally an independent Cargo workspace under `fuzz/`. Keep its
`Cargo.lock` committed and separate from the root workspace so nightly-only fuzz dependencies do
not change the stable public crate lockfile.

## Toolchain

Use the same versions as CI unless a local investigation needs a newer nightly:

```sh
rustup toolchain install nightly-2026-07-01 --component rust-src
cargo install cargo-fuzz --version 0.13.2 --locked
```

The repository root stays on stable Rust. Invoke `cargo-fuzz` with the nightly toolchain explicitly.

## Targets

| Target | Surface | Seed corpus | Dictionary |
| --- | --- | --- | --- |
| `parse_mermaid` | Semantic JSON, typed render model selection, and lenient recovery | `fuzz/seeds/mermaid` | `fuzz/dictionaries/mermaid.dict` |
| `render_mermaid` | Strict parse, layout, SVG render, and `resvg-safe` output | `fuzz/seeds/mermaid` | `fuzz/dictionaries/mermaid.dict` |
| `svg_pipeline` | Raw XML SVG through `SvgPipeline::resvg_safe()` | `fuzz/seeds/svg` | `fuzz/dictionaries/svg.dict` |
| `ffi_api` | C ABI status, buffer ownership, reusable engine calls, and host text-measure callback handling | `fuzz/seeds/ffi` | `fuzz/dictionaries/mermaid.dict` |

`ffi_api` keeps the text seeds above readable, but random inputs use a small binary frame so
options, document URI, and source bytes can evolve independently:

```text
selector options_len options_bytes [uri_len uri_bytes] source_bytes
```

The optional URI field is present when the selector's high bit is set. Otherwise the harness uses a
fixed default URI and treats the remaining bytes as Mermaid source.

## Local Smoke

Run a fast smoke before changing fuzz harnesses:

```sh
cargo +nightly-2026-07-01 check --manifest-path fuzz/Cargo.toml --locked
mkdir -p fuzz/corpus/parse_mermaid fuzz/corpus/render_mermaid fuzz/corpus/svg_pipeline fuzz/corpus/ffi_api
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address parse_mermaid fuzz/corpus/parse_mermaid fuzz/seeds/mermaid -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address render_mermaid fuzz/corpus/render_mermaid fuzz/seeds/mermaid -- -runs=64 -timeout=10 -max_len=32768 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address svg_pipeline fuzz/corpus/svg_pipeline fuzz/seeds/svg -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/svg.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address ffi_api fuzz/corpus/ffi_api fuzz/seeds/ffi -- -runs=64 -timeout=10 -max_len=16384 -dict=fuzz/dictionaries/mermaid.dict
```

On macOS, local `cargo-fuzz` installations may default to the wrong host target if the binary was
installed under Rosetta. In that case, reinstall `cargo-fuzz` natively or add the explicit target
triple for the local host. The CI authority is Linux x86_64 with ASan.

## Sanitizer Policy

CI uses AddressSanitizer because it catches the most relevant native memory faults with reasonable
signal-to-noise for this codebase and dependency graph. `cargo-fuzz` also supports leak, memory,
thread, and no-sanitizer modes, but those are investigation tools rather than required release gates.

All UTF-8 SVG inputs, including malformed XML, pass through the pipeline for panic and sanitizer
coverage. For well-formed input, the assertions mirror the documented `resvg-safe` structural
contract: output must stay XML-parseable, remove active elements, event-handler and unsafe URL
attributes, and `foreignObject`, while preserving safe local fragment references and safe raster
image data URIs. `resvg-safe` is a raster-compatibility pipeline, not a general sanitizer for
arbitrary host CSS; browser and webview consumers must follow `docs/security/RENDERING_SECURITY.md`.

When a crash is found, minimize it before promoting it into a regression test:

```sh
cargo +nightly-2026-07-01 fuzz tmin --fuzz-dir fuzz <target> fuzz/artifacts/<target>/<crash-file>
```

If the minimized input exposes a public API bug, add a focused stable test under the affected crate.
Keep the fuzz corpus for exploration, not as the only regression proof.
