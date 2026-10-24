# Serial Go / wazero consumer example

This is a small usable example, not a published SDK. It consumes the committed
merman-wasi ABI 1 contract, not the native C ABI. Requires Go 1.25+ and wazero
1.12.0. Obtain an actual packaged `wasm32-wasip1` reactor with native-fonts, SVG,
PNG and bundled fonts (standard, math or full); this example does not build one.
Keep the artifact's notices and bundled font licenses with your application.

The shown PNG options use binding schema 2 (`raster.background`). Main schema 3
artifacts use `raster.matte`; inspect `options_schema_versions` in the loaded catalog.
The raw Execute API does not translate or normalize caller options.

## Run

From this directory, substitute your artifact and source paths:

```console
go run ./cmd/render --module /path/merman_wasi.wasm --source diagram.mmd --operation svg --output diagram.svg --metadata diagram.svg.json --catalog catalog.json
```

Request options are read as JSON, not passed at construction. Options are
operation-specific: the `raster` group applies to PNG, not SVG. For PNG:

Create `png-options.json` containing:

```json
{"svg":{"pipeline":"resvg-safe"},"raster":{"scale":2,"background":"#ffffff"}}
```

```console
go run ./cmd/render --module /path/merman_wasi.wasm --source diagram.mmd --operation png --options png-options.json --font /path/Cousine-Regular.ttf --font /path/Cousine-Bold.ttf --output diagram.png --metadata diagram.png.json
```

All output files are raw bytes, never base64. Parent directories must exist.
The CLI has a fixed 60-second deadline covering initialization and requests;
stdout stays empty and concise completion/errors go to stderr. `--font` is
optional with a bundled-font artifact. Caller fonts are concatenated into one
blob with exact relative offset/length ranges, copied by the reactor, and take
precedence over bundled family/style matches. SVG does not embed these fonts:
downstream display needs matching fonts. An artifact without bundles requires
sufficient caller fonts; capability/font errors remain core-owned errors.

## Embed and reuse serially

Use this pattern in an application package; put the reactor next to its Go
source, and retain required artifact/font notices:

```go
package main

import (
    "context"
    _ "embed"
    "errors"
    "fmt"
    "time"

    wasiexample "merman.local/wasi-example"
)

//go:embed merman_wasi.wasm
var module []byte

func render(source []byte) (output wasiexample.Result, err error) {
    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()
    client, err := wasiexample.New(ctx, module,
        []byte(`{"resources":{"profile":"interactive"}}`), nil)
    if err != nil { return output, err }
    defer func() { err = errors.Join(err, client.Close(context.Background())) }()
    output, err = client.Execute(ctx, "svg", source, nil)
    var owned *wasiexample.OwnedError
    if errors.As(err, &owned) {
        fmt.Printf("%s: %s\n", owned.CodeName, owned.Message)
        // The same client can execute another request after an ordinary error.
    }
    return output, err
}
```

For repeated work, keep the client open around a serial request loop and give
each request its own live deadline. `Catalog(ctx)` returns JSON in `Result.Data`;
`Execute` returns raw payload in `Data` and unchanged core metadata in `Metadata`.
Both include `MediaType`. All bytes are host-owned copies before guest results
are freed. Named metadata operations also put JSON in `Data`. Unknown operation
IDs pass through to core. Request options remain raw JSON (duplicate fields are
not collapsed into a Go map); the core owns option validation and semantics.
Nil constructor options explicitly select the interactive profile; nonempty
constructor options pass through without overlays, so constrained remains
constrained. Empty request options inherit the engine's resource profile.

## Boundaries and lifecycle

- One Client owns one runtime, reactor and engine. Calls, including Close, must
  be sequential; no goroutine safety, locking, pool, retry or fallback is added.
- Standard `wasi_snapshot_preview1` imports are installed, with no preopened
  directories, environment, network access or inherited stdio. No custom random
  or time shims are installed. Core owns deterministic rendering policy.
- Linear memory is capped at 512 MiB. Reactor initialization uses
  `WithStartFunctions()` and calls `_initialize` exactly once, then checks ABI 1.
  Descriptors are decoded as 40 little-endian bytes with checked, canonical
  segments. Core owns parsing, rendering, capability discovery and resource rules.
- Results and allocations are always released while the guest remains usable.
  Cleanup uses `context.Background()` if the request context expires between
  calls. A guest trap or in-flight cancellation closes the runtime immediately:
  no subsequent guest calls, including release calls, are attempted. Replace
  the entire Client with New; never retry on the same trapped instance.
- Already-canceled requests return before touching the guest, leaving a live
  instance usable. Owned core errors are typed with `errors.As`; `Raw` retains
  their full JSON, including optional structured details.
- `WithCloseOnContextDone(true)` enforces live call deadlines. Always supply a
  deadline: cooperative core profiles alone are not hard execution limits.
  This is a linear-memory cap, not a host-process RSS/CPU sandbox. Host copies,
  module compilation and host-side work also consume resources. A production
  pool needs separate serial clients, bounded concurrency, deadlines and full
  instance replacement after traps; those policies are deliberately not built.

## Focused checks

```console
go test -p 1 ./...
MERMAN_WASI_MODULE=/path/merman_wasi.wasm go test -p 1 -count=1 ./...
MERMAN_WASI_MODULE=/path/merman_wasi.wasm MERMAN_WASI_FONT=/path/Cousine-Regular.ttf go test -p 1 -count=1 ./...
```

Integration tests skip when MERMAN_WASI_MODULE is unset. The caller-font case
also needs MERMAN_WASI_FONT. Tests exercise catalog, SVG, PNG scale 2 with white
matte, raw metadata, ordinary parse-error recovery, caller font admission, and
pre-canceled calls that must not poison an untouched instance. A tiny real Wasm
loop separately verifies in-flight cancellation and runtime disposal without
depending on diagram complexity or machine speed. These are example integration
checks, not artifact import auditing or package qualification.
