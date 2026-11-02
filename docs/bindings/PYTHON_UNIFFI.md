# Python UniFFI Bindings

Status: experimental publishable Python package.

This guide targets `0.10.0-alpha.1` (`0.10.0a1` on PyPI), a five-target ordinary SVG product with math. Published `0.9.2` keeps its original three-target coverage and omits math; use its tagged documentation for that version. Candidate qualification does not establish PyPI availability or change existing artifacts.

This guide owns Python generation, wheel assembly, and release procedures. The
[package generator](../../crates/merman-uniffi/examples/generate_python_package.rs) reads UniFFI
metadata from the matching Rust library and packages the production `merman-uniffi` cdylib. See the
[package README](../../platforms/python/merman/README.md) for application use and the
[UniFFI contract](UNIFFI.md) for shared API and lifecycle rules. The package shape is:

```text
platforms/python/merman/
  pyproject.toml
  src/merman/
    __init__.py
    merman_uniffi.py            generated, not committed
    merman_uniffi.dll           generated/copy on Windows, not committed
    libmerman_uniffi.so         generated/copy on Linux, not committed
    libmerman_uniffi.dylib      generated/copy on macOS, not committed
```

The generated Python module and native library must live in the same package directory because the
UniFFI Python loader resolves the cdylib relative to `__file__`.

## Generate Locally

`scripts/build-python-uniffi-wheel.py` resolves the `python-uniffi-native` artifact profile. It
builds the release library set with the descriptor's native feature list including math, then runs a separate
generator with only `binding-generation`. UniFFI metadata is read from the matching Rust `rlib`,
because fully stripped ELF cdylibs intentionally omit the metadata symbol table; the generated
package still embeds the size-optimized production cdylib. The builder rejects hosts outside the
profile's published target set and replaces the package scaffold's release-set legal report with
the checked-in single-target report before building the wheel.

```bash
python3 scripts/artifact_profile_recipe.py python-uniffi-native --build-host --locked
cargo run -p merman-uniffi --no-default-features \
  --features binding-generation --example generate_python_package -- \
  --metadata-library target/native-distribution/libmerman_uniffi.rlib \
  --cdylib target/native-distribution/libmerman_uniffi.dylib \
  --package-dir platforms/python/merman
```

The default wheel is deterministic and omits `native-runtime`. Custom UniFFI artifacts may enable
that atomic feature to compile the clock, time-zone, and random adapters together. Generated
runtime discovery exposes the concrete `system-clock`, `system-timezone`, and `system-random`
adapter IDs only when present; the Cargo aggregate is not a Python capability name.

On Linux, use `libmerman_uniffi.so` for `--cdylib`; on Windows, use `merman_uniffi.dll`
and run the generator command on one line. `--metadata-library` still selects the matching `rlib`.

## API

The package re-exports the generated UniFFI API:

```python
import json
import merman

api = merman.Merman()
merman.require_text_measurement_protocol_version(
    merman.TEXT_MEASUREMENT_PROTOCOL_VERSION
)
print(api.package_version())
assert api.binding_api_version_v7() == 7
catalog = merman.get_runtime_catalog(api)
capabilities = catalog["capabilities"]
assert catalog["schema_version"] == 1
assert catalog["transport_api_version"] == api.binding_api_version_v7()
assert "svg" in capabilities["capability_ids"]

svg = api.render_svg("flowchart TD\nA[Hello] --> B[World]", None)
ascii_text = api.render_ascii("flowchart TD\nA[Hello] --> B[World]", None)
semantic_json = api.parse_json("flowchart TD\nA[Hello] --> B[World]", None)
layout_json = api.layout_json("flowchart TD\nA[Hello] --> B[World]", None)
document_json = api.analyze_document_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    "file:///tmp/example.md",
    None,
)
document_facts_json = api.analyze_document_facts_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    "file:///tmp/example.md",
    None,
)
validation = api.validate("flowchart TD\nA[Hello] --> B[World]", None)
diagrams = api.supported_diagrams()
ascii_capabilities = api.ascii_capabilities()
themes = api.supported_themes()
theme_catalog = json.loads(api.theme_catalog_json())
family_capabilities = api.diagram_family_capabilities()
lint_rules = api.lint_rule_catalog()

class PreviewMeasurer(merman.MermanTextMeasurer):
    def measure(self, request):
        # Use the final display surface's font API here. Returning None asks
        # Merman to use its operation-specific deterministic fallback.
        return None

services = merman.MermanEngineServices().with_text_measurer(PreviewMeasurer())
engine = merman.MermanEngine(None, services)
try:
    svg_with_host_metrics = engine.render_svg(
        "flowchart TD\nA[Hello] --> B[World]",
        None,
    )
    reusable_document_json = engine.analyze_document_json(
        "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
        "file:///tmp/example.md",
        None,
    )
finally:
    engine.close()
```

The next ordinary wheel includes math and omits PNG, JPEG, and PDF. Codec methods remain available
for a custom current-contract library and otherwise raise `MermanError.Binding` with
`MermanErrorKind.MISSING_CAPABILITY` plus the exact capability ID. Math input is still rejected
under the `constrained` resource policy because that policy excludes the native math backend.
The ordinary interactive profile can render `flowchart TD\nA["$$x^2$$"] --> B` without adding
source features. Published `0.9.2` also omits math.

Errors are exposed through the generated `MermanError` type. `MermanError.Binding` carries the
underlying status code/name, `MermanErrorKind`, optional `capability_id`, optional
`MermanResourceErrorDetails`, optional `MermanDiagnosticErrorDetails`, optional
`MermanIconRegistryErrorDetails`, optional `MermanCancelledDetails`, and message from
`merman-bindings-core`. `UNKNOWN_OPERATION` has no
capability ID; `MISSING_CAPABILITY` preserves the exact descriptor ID. Resource failures expose the
stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase, actual value, effective maximum, and selected profile. Consumers should not
parse the message to distinguish these cases.

Generic operations may attach a caller-owned `MermanOperationControl`:

```python
control = merman.MermanOperationControl(timeout_ms=250)
request = merman.MermanOperationRequestV4(
    operation_id="svg",
    source="flowchart TD\nA --> B",
    uri=None,
    options_json=None,
    control=control,
)
result = api.execute(request)

# Retain `control` and call this from another thread when the request becomes stale.
control.cancel()
```

Cancellation is cooperative. A cancelled operation raises `MermanError.Binding` with code name
`MERMAN_CANCELLED` and `cancellation.reason` (`requested` or `deadline_exceeded`) plus the observed
checkpoint `phase`. It is distinct from resource rejection and returns no partial output. Opaque
callbacks and single-call encoders can only be checked before and after invocation; use worker or
process isolation for hard preemption.
The optional `options_json` argument uses the shared contract documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).
`ResourceOptionsBuilder` emits Options JSON schema `3`; omit its profile for a reusable request that must inherit the constructor ceiling, and use `ResourceOverrideId` rather than the full catalog-only `ResourceLimitId` when adding overrides.
`Merman.lint_rule_catalog()` returns structured analyzer rule metadata, including evidence
references and policy tags, for editor settings, diagnostic explanations, or LSP rule
configuration.

The direct UniFFI binding API is `7`, independently versioned from the native C ABI and the
text-measurement protocol. `get_runtime_catalog()` reads one atomic catalog, validates
flat schema `1`, artifact identity, sorted stable IDs, and local output/operation and
adapter/capability relations before returning it. Do not infer availability from Cargo feature
names or copy an ID table into Python; inspect the loaded catalog instead. Preserve unknown
discovery IDs. The [UniFFI contract](UNIFFI.md#public-model) owns version checks and the open
`MermanOutputPlan` record. Result-returning binary methods preserve operation metadata; inspect
`output_plan.kind` and optional format payloads, and retain `raw_json` for future kinds.

## Text Measurement

The Python UniFFI package uses Merman's built-in headless text measurer by default. This is the
right default for CLI tools, documentation generation, tests, and server-side batch rendering
because it is deterministic and does not require GUI or browser dependencies.

Python GUI or WebView hosts that need label geometry to match their own font stack place a
`MermanTextMeasurer` in `MermanEngineServices` and pass that value to the direct
`MermanEngine(options_json, services)` constructor. The callback is immutable for that engine;
construct another engine to change it or return to the built-in measurer. Inspect `request.phase`
for the routing stage and `request.operation` for the exact platform primitive. Return a record
tagged with the matching
`MermanTextMeasurementResultKind`: metrics, length, horizontal extents, or wrapped metrics with raw
width. `None`, wrong-kind or invalid results, and Python exceptions reported through UniFFI's
generated callback trampoline use the operation's deterministic fallback for that request instead of
failing the reusable render/layout call. Merman does not catch arbitrary foreign unwinds that
bypass that generated boundary. Follow
[`HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md) for the
shared callback rules around caching, natural width, and avoiding async UI-thread blocking.
Text-measurement protocol 1 exposes 23 operations with contiguous codes 0 through 22.
`CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET` returns the signed `length` for Architecture's
`createFormattedText(...)` bbox y under inherited `dominant-baseline="middle"`.
`CREATE_TEXT_B_BOX_Y_OFFSET` remains the ordinary createText probe; the two operations are not
interchangeable and both may return a finite negative value. `RAW_B_BOX_HEIGHT` returns the
non-negative height from a direct raw SVG `<text>.getBBox()` probe.
`RAW_B_BOX_Y_OFFSET`, `RAW_MIDDLE_B_BOX_Y_OFFSET`, and `RAW_HANGING_B_BOX_Y_OFFSET`
return signed direct-text bbox y offsets at origin `(0, 0)` under ordinary, middle, and hanging
baselines respectively. They have no formatted-text offsets and must not be substituted for the
two createText probes.

The generated callback method is `measure(self, request)`, not `measure_text`. One-shot facade
methods receive `options_json` explicitly (`api.render_svg(source, options_json)`). Generic
operations put the same value in `MermanOperationRequestV4.options_json` and call
`api.execute(request)`. A reusable engine accepts baseline options at construction and
request-local overrides on each operation, such as `engine.render_svg(source, options_json)`;
those overrides are deeply merged without changing the baseline or its runtime policy. Do not
estimate width from character count in production; return `None` when the host cannot reproduce
the requested operation with the display surface's real font API.

Callback-free reusable engines admit concurrent calls. A callback engine serializes operation
admission and raises typed `BUSY` for a competing call. Same-engine entry while the callback is
active raises typed `REENTRANT_CALL`, including an attempted call dispatched from the callback to
another thread. A close from the callback also raises `REENTRANT_CALL` and preserves the complete
engine for retry. `close()` is explicit and idempotent; always call it for a callback-owning engine
because the callback may capture the engine and form a reference cycle. There is no callback
mutation lifecycle.

## Build And Verify A Wheel

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The script builds `merman-uniffi`, copies the checked-in Python package scaffold into a temporary
staging directory, generates UniFFI Python files only in that staging directory, and builds a
platform wheel under `target/python-wheels`. With `--run-smoke`, it installs the wheel into a
temporary venv without dependency resolution and copies only the standalone
[wheel consumer](../../platforms/python/merman/examples/wheel_smoke.py) outside the checkout.
The consumer runs with `python -I`, requires the imported package to come from that venv, compares
the wheel's distribution version with the native library, validates the runtime catalog, and
checks positive formula SVG geometry, constrained math rejection, and absent PNG/JPEG/PDF codecs.
The separate [source smoke](../../platforms/python/merman/examples/smoke.py) retains callback,
theme fixture, and reusable-engine coverage; owner-local Rust and generated-binding tests carry
the exhaustive operation and protocol contracts. The build fails when generated support files differ from their checked-in projections or
when setuptools emits a universal `py3-none-any` wheel, because the package carries a native
library.

`--expected-target <rust-triple>` additionally requires the actual Rust host to match the selected
matrix member. It does not turn a cross compile into execution evidence. macOS builds use matching
compiler deployment targets and single-architecture wheel tags: `11.0` for ARM64 and `10.13` for
x86-64. Current hosted macOS execution does not establish execution on the oldest macOS tag.

## Linux Baseline And Matrix Qualification

The release owner builds exactly these targets:

| Rust target | Wheel platform | Native runner |
| --- | --- | --- |
| `aarch64-apple-darwin` | `macosx_11_0_arm64` | `macos-15` |
| `x86_64-apple-darwin` | `macosx_10_13_x86_64` | `macos-15-intel` |
| `aarch64-unknown-linux-gnu` | `manylinux_2_28_aarch64` | `ubuntu-24.04-arm` plus pinned container |
| `x86_64-unknown-linux-gnu` | `manylinux_2_28_x86_64` | `ubuntu-24.04` plus pinned container |
| `x86_64-pc-windows-msvc` | `win_amd64` | `windows-2025-vs2026` |

The two Linux members use official PyPA AlmaLinux 8 images tagged `2026.05.07-2`, pinned by
architecture-specific manifest digest in the workflow. The digests were obtained from the
official [x86-64 tag metadata](https://quay.io/api/v1/repository/pypa/manylinux_2_28_x86_64/tag/?onlyActiveTags=true&specificTag=2026.05.07-2)
and [ARM64 tag metadata](https://quay.io/api/v1/repository/pypa/manylinux_2_28_aarch64/tag/?onlyActiveTags=true&specificTag=2026.05.07-2).
The job asserts the actual libc is `glibc 2.28` and the CPU matches the matrix member, then builds
the production cdylib and metadata rlib there. `auditwheel==6.8.2` audits the binary before and
after repair, explicitly repairs with `--plat manylinux_2_28_<arch> --only-plat`, and the same
baseline environment installs and executes the repaired wheel. Relabeling a wheel built on a
newer Ubuntu host is not a baseline qualification.

Every target uses separate installed consumers for Python `3.14` and the declared minimum `3.9`.
The `python-qualification-<rust-triple>` workflow artifacts retain consumer JSON; Linux also
retains the image/source identity and audit/repair output. The exact final wheels are retained as
`python-wheel-<rust-triple>`. These declarations describe the owner recipe; actual qualification
requires a successful run against the exact candidate commit.

The aggregate check runs `python scripts/python_wheel_licenses.py --require-release-set` on all
final wheels. It verifies the exact descriptor target set, rejects duplicate targets even when
the file count is five, requires the declared platform tags, compares embedded WHEEL/METADATA
identity, and checks each target's exact generated license report. Unsupported musl, universal2,
and Windows ARM64 tags cannot stand in for GNU or single-architecture wheels. There is no
qualified sdist; the repository helper remains the explicit source-build route.

## Release

`release-python.yml` defaults to a nonpublishing candidate. Supply the matching version's `v*`
release tag and `source_ref` equal to `main`, an exact 40-character commit, or the matching tag.
For a main/commit candidate the tag need not exist. The workflow resolves one immutable source
commit and tree before building all five wheels and retaining the qualification artifacts.
`publish=false` guards both GitHub Release mutation and PyPI publication;
`publish_to_pypi=true` without `publish=true` is rejected.

A later authorized release sets `publish=true` and requires `source_ref` to be the matching
immutable release tag. After qualification, the owner attaches wheels to the existing GitHub
Release. `publish_to_pypi=true` additionally publishes through Trusted Publishing, retaining the
existing exact-checksum reconciliation before and after publication. Both mutation boundaries
recheck the remote tag. Dispatching an updated workflow definition never changes the selected
artifact source.

Configure the PyPI project `merman` with a Trusted Publisher for this repository and
`.github/workflows/release-python.yml`. No PyPI API token is required for the OIDC path.
