# mermaid.go WASI integration assessment

Date: 2026-10-24.

Scope: compare issue #243 with the actual downstream API and the locally
implemented SDK. This is source analysis, not a completed downstream integration,
a benchmark, or evidence that the SDK workflows have run remotely.

## Source identity

- Downstream: `repo-ref/mermaid.go`, cloned at
  `01ff8aaa8eb666be632bb33ae7bfb43292f17c59`.
- Request: [Merman issue #243](https://github.com/Latias94/merman/issues/243),
  retrieved through the GitHub API on 2026-10-24.
- Local main implementation: branch `feat/headless-sdk-20261024`,
  HEAD `8bbe76554`, including the existing uncommitted release-tool fixes.
- Local artifact evidence: `target/sdk-refactor-qualified/7c160d53cf8b/`.
  Those artifacts predate the later source-cleanliness guard; they do not prove
  an artifact built from the current dirty tree.

## What the downstream actually does

The project has a browser backend and a CLI backend, with a common `Renderer`
interface for SVG, PNG, scaled PNG, context cancellation and timeout control.
It does not currently implement a wazero backend or a WASM calling convention.
See [renderer.go, lines 19-28](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/renderer.go#L19)
and [merman.go, lines 516-542](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L516).

The CLI configuration consists of executable selection, SVG ID, Mermaid theme,
a Mermaid config-file path and arbitrary extra CLI arguments. It does not
currently submit binding Options JSON or depend directly on schema 2's
`presentation` group. Its configuration file contains Mermaid configuration,
which is distinct from our binding options envelope.
See [merman.go, lines 101-149](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L101)
and [argument construction, lines 372-383](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L372).

Concurrency defaults to `GOMAXPROCS`, with a cancellable queue. Each render has
its own process. A timeout must leave the outer engine usable; `Cancel` closes
the whole engine. See [constructor, lines 182-233](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L182),
[context handling, renderer.go lines 35-99](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/renderer.go#L35),
and [timeout recovery test, lines 541-549](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman_test.go#L541).

PNG `BoxModel` uses image pixel dimensions divided by the requested scale;
integer dimensions are rounded and all four quads describe that logical box.
`WithBundle` embeds source in SVG and is rejected for PNG.
See [merman.go, lines 475-510](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L475)
and [boxModelFor, lines 629-638](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/merman.go#L629).

The checked-out CI installs `v0.8.0-alpha.6`. The current source snapshot
therefore does not establish live acceptance of our maintenance or main SDK.
See [downstream CI](https://github.com/dreampuf/mermaid.go/blob/01ff8aaa8eb666be632bb33ae7bfb43292f17c59/.github/workflows/ci.yml).

## What our artifacts provide

The WASI SDK is the appropriate artifact family: a `wasm32-wasip1` reactor,
independent scalar ABI 1, raw SVG/PNG output, scale/matte options, copied font
bytes, owned structured errors, runtime catalog and serial instance reuse.
The native C SDK would retain the platform/cgo integration that the issue seeks
to avoid. See [WASI ABI](../../crates/merman-wasi/README.md),
[raster options](../bindings/OPTIONS_JSON.md#native-export-options),
and [SDK packaging](../../distribution/sdk/README.md).

Each WASI archive includes `lib/merman_wasi.wasm`, ABI documentation, catalog,
manifest and exact notices. The archive has a SHA-256 sidecar; the module has its
own hash inside `manifest.json`. The module already contains production Cousine
fonts, so the basic runtime payload can be embedded as one WASM byte slice.
Extra font files are optional inputs. See [embedded fonts](../../crates/merman-export/src/lib.rs#L28)
and [package assembly](../../scripts/sdk_archive.py#L437).

| WASI flavor | SVG/PNG | Math | ELK |
| --- | --- | --- | --- |
| standard | Yes | No | No; documented family fallback |
| math | Yes | Yes | No; documented family fallback |
| full | Yes | Yes | Yes |

Standard excludes ELK/EPL and RaTeX/KaTeX. It is not a permissive-license-only
artifact: dependency and font notices still include MPL and OFL. Math also omits
ELK/EPL; full includes the EPL components. See the exact generated notices in the
local archives and [profile authority](../../capabilities/artifact-profiles-v2.json).

## Integration work and remaining acceptance

1. Add a separate WASI-backed implementation of `Renderer` in mermaid.go.
   Keep CLI-specific options such as arbitrary command arguments explicit rather
   than pretending all CLI facilities translate to the binding API. Compile the
   module once; initialize each reactor and copy/free ABI inputs and results.
2. Use a bounded instance pool for parallel callers. Each instance is serial.
   After timeout, cancellation that closes the module, or a trap, replace that
   instance while preserving the outer engine. Ordinary owned parse/option
   errors permit reuse. Keep queue cancellation and whole-engine shutdown.
3. Preserve PNG `BoxModel` semantics rather than returning scaled pixel sizes.
   Map Mermaid theme/config to `site_config`, SVG ID to `svg.diagram_id`,
   and scale/background to raster options. Discover the loaded options schema:
   maintenance uses 2, main uses 3; WASI ABI 1 is a separate version.
4. Preserve existing sentinel error categories and expose structured WASI detail
   through a new Go error type. Do not fabricate CLI exit codes. Source spans are
   parser-provided byte spans with explicit precision; universal exact line and
   column diagnostics are not promised. SVG viewport metadata can be absent for
   responsive roots without a viewBox. See [shared diagnostics](../../crates/merman-bindings-core/src/common.rs#L266)
   and [result descriptor](../../crates/merman-wasi/README.md#result-descriptor).
5. Add direct CLI/WASI acceptance using matching features, input, fonts, config
   and deterministic policy. Current CI compares native C and WASI PNG pixels
   for basic/math fixtures; it does not invoke `merman-cli` for that comparison.
   Include background, scale, extra fonts and the wrapped Swimlane regression.
   See [current parity job](../../.github/workflows/release-sdk.yml#L172).

Default Cousine coverage is Latin/Greek/Cyrillic, not complete CJK/emoji.
Hosts must provide suitable font bytes for other admitted glyphs; SVG display
still needs matching fonts because SVG text does not embed them.
See [font coverage](../../crates/merman-export/assets/fonts/README.md).

Recommendation: retain official WASI artifact/ABI ownership in Merman, provide a
small reusable Go integration example distinct from the qualification harness,
and let mermaid.go own its public Go API and pool. Offer standard by default,
math when formulas are needed without ELK, and full when ELK is required.
Actual remote release assets and downstream integration remain to be verified;
the existing local host qualification does not establish either.
