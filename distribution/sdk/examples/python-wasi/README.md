# Bounded Python / wasmtime serial consumer

This is a small, unpublished consumer example, not a new SDK, pool, or service.
It consumes the `merman-wasi` ABI **1** `wasm32-wasip1` reactor using
`wasmtime==50.0.1`. Python 3.9+ is sufficient. The only third-party dependency
is the pinned runtime in `requirements.txt`; the tests use the standard library.
Use an existing environment with that dependency, or install the requirements
in your own environment. This directory does not build or download a module.

The shown PNG options use binding schema 3 (`raster.matte`). Schema 2 artifacts
use `raster.background`; inspect `options_schema_versions` in the loaded catalog.
The client forwards options to core without a schema-migration layer.

The reactor must include `native-fonts`, `bundled-fonts`, SVG, PNG and flowchart
support for these examples and tests. Standard and math-enabled release modules
can both be used; the runtime catalog describes the actual compiled capabilities.
A missing capability remains a structured guest error, not a fallback renderer.

## Python

All paths below are read by Python on the **host**, never passed to the guest.
The client accepts module bytes, optional constructor-options dict, and an
optional list of raw font-file bytes. Font buffers are concatenated with explicit
relative offset/length ranges. Caller fonts precede bundled fonts; bundled fonts
are explicitly requested with `bundled_fonts: true`. No system font lookup occurs.

```python
from pathlib import Path
from client import Client, GuestError

module = Path("merman_wasi.wasm").read_bytes()
source = b"flowchart TD\nA[Hello] --> B[World]\n"
with Client(module, fonts=[Path("Cousine-Regular.ttf").read_bytes()]) as client:
    catalog = client.catalog()                 # decoded runtime catalog dict
    try:
        client.execute("svg", b"flowchart ???")
    except GuestError as error:
        print(error.payload["code_name"], error.payload["message"])
        # The full versioned core error payload, including details, is preserved.

    svg, svg_meta = client.execute("svg", source)
    Path("diagram.svg").write_bytes(svg)
    viewport = svg_meta.get("svg_viewport")    # CSS pixels; not always present
    if viewport is not None:
        print(viewport["width"], viewport["height"])

    png, png_meta = client.execute("png", source, {
        "raster": {"scale": 2.0, "matte": "#ffffff"}
    })
    Path("diagram.png").write_bytes(png)
    plan = png_meta["output_plan"]             # actual raster pixel dimensions
    print(plan["width_px"], plan["height_px"])
```

Omit `fonts` to use bundled fonts only. The optional extra-font test admits a
real font buffer; it does not claim to prove precedence or coverage for arbitrary
fonts. SVG does **not** embed fonts: its eventual viewer needs matching fonts.
An SVG without a root viewBox can omit `svg_viewport`; do not invent dimensions.
The client returns `(raw_data_bytes, metadata_dict_or_None)` without changing
the core metadata. Named catalog `metadata_ids` can be passed to `execute` with
`source=b""` and no nonempty options; their JSON is in data, metadata is `None`.

Constructor resources explicitly default to `interactive`; caller-supplied
resources (including `constrained`) are preserved. Empty request options inherit
the constructor profile. Nonempty request options are forwarded to core as-is;
this example adds no parallel options schema or renderer.

## CLI

Create `diagram.mmd` locally and optionally `png-options.json` containing:

```json
{"raster": {"scale": 2.0, "matte": "#ffffff"}}
```

```console
python render.py --module merman_wasi.wasm --source diagram.mmd --operation svg --output diagram.svg --metadata svg-metadata.json --catalog catalog.json
python render.py --module merman_wasi.wasm --source diagram.mmd --operation png --options png-options.json --font Cousine-Regular.ttf --font Cousine-Bold.ttf --output diagram.png --metadata png-metadata.json
```

`--options` is a JSON-file path, not inline JSON; it overlays the execution
request, not constructor options. `--font` is repeatable. `--catalog` writes the
runtime catalog alongside the requested render. Output parent directories must
already exist. `--fuel` selects the per-call instruction budget; large rasters
and SVG filters can require more than the default even for modest source text.
Ordinary guest failures emit the full structured error JSON to
stderr and exit 1; host/protocol/runtime failures emit their type and message.
The CLI reads complete local files into host memory before ABI input checks;
it is an example for trusted local inputs, not a bounded host upload service.

## Bounds, ownership and failures

- One instance / one engine, synchronous and **serial only**. Do not share a
  client across threads, call it reentrantly, or mutate its runtime internals.
- Wasmtime fuel consumption is enabled, threads and parallel compilation are
  disabled, and `Store.set_limits` caps guest linear memory at **512 MiB**.
  Each guest export call defaults to **5,000,000,000 fuel**, including initialization
  and release calls. Pass `Client(module, fuel=...)` to select a positive u64
  budget for a larger workload; no automatic retry or budget escalation occurs.
  These are instruction budgets, **not wall-clock timeouts**.
  Compilation and host/WASI work are not bounded by guest fuel. For hard timeouts
  and total CPU/RSS limits, run in an externally supervised, isolated process
  that can be terminated. No hard timeout or process sandbox is claimed here.
- Standard WASI is linked without inherited arguments, environment, stdio,
  preopened directories, or network descriptors. No random/clock overrides or
  smoke-test deterministic shims are installed; standard WASI clock/random
  behavior is retained. This is not an import-audit or hostile-module sandbox.
- `_initialize` runs once, followed by the ABI version check. The 40-byte
  descriptor is decoded as ten little-endian u32 values. Result ranges must be
  canonical, within wasm32 memory and the ABI byte ceilings. Inputs come only
  from live guest allocations, with 32 MiB per input and 1 MiB per envelope.
  Total result bytes, including the descriptor, are limited to 32 MiB.
  Guest registries and native-font limits enforce their additional ABI budgets.
- Results and metadata are copied before `result_free`; all input allocations
  are freed after their synchronous call. No retained Wasmtime memory views
  survive calls. Fonts are copied by the guest constructor before input release.
- `GuestError.payload` preserves ordinary owned errors. After those errors, reuse
  is allowed. `ProtocolError` indicates malformed ABI output, allocation failure,
  or a failed release. A Wasmtime trap/runtime call error marks the client
  unusable: no subsequent guest calls, including cleanup exports, are made.
  Close/discard it and construct a new client; no trap recovery is promised.
- `close()` is idempotent. When healthy, it closes the guest engine first; nested
  `finally` blocks release the store, module and runtime engine even if cleanup
  raises. Use `with Client(...)` or explicitly call `close()`; there is no finalizer.

## Verification

```console
python -m py_compile client.py render.py test_client.py
MERMAN_WASI_MODULE=/absolute/path/merman_wasi.wasm python -m unittest -v test_client
MERMAN_WASI_MODULE=/absolute/path/merman_wasi.wasm MERMAN_WASI_FONT=/absolute/path/Cousine-Regular.ttf python -m unittest -v test_client
```

Without `MERMAN_WASI_MODULE`, the real-runtime tests explicitly skip, even if
wasmtime is not installed. A supplied invalid module or missing runtime fails;
it is not silently skipped. `MERMAN_WASI_FONT` enables the additional font-input
test. Tests assert actual catalog discovery, SVG/viewBox metadata, PNG IHDR and
pixel dimensions at 1x/2x, matte color at the transparent SVG corner, and parse
error followed by successful reuse. No fake runtime or mock-render evidence is
used. A zero-fuel call on the actual reactor verifies trap handling, rejection of
further calls and successful construction of a replacement client. Hostile-module
behavior and OS-level isolation are not qualified by these tests. Syntax checks
or skipped tests are not runtime proof.

The parent integration workflow owns archive inclusion and authoritative runs
against both release lines. This example does not assert those runs occurred.
