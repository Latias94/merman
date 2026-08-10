# ADR 0079: Default Native Prebuilt Capability SKU

- Status: accepted
- Supersedes: the native prebuilt SKU decision in ADR-0076
- Artifact profiles: `capabilities/artifact-profiles-v1.json`, schema `1`

## Context

The published Android, Apple, Python, and Flutter packages embed native libraries. Shipping every
optional native capability in each package made common installations pay for RaTeX and its fonts,
raster encoders, PDF generation, and host-runtime adapters even when callers only rendered SVG or
used diagnostics. It also made the multi-platform Flutter archive too large for its registry.

The language wrappers already expose one stable operation vocabulary and typed
`missing-capability` failures. A package therefore does not need a second, incompatible API merely
because its prebuilt library selects a smaller capability set.

## Decision

Android, Apple, Python, and Flutter publish one default native prebuilt SKU with these direct Cargo
features:

```text
analysis,ascii,layout-cytoscape,layout-elk,svg
```

The default prebuilt SKU deliberately omits:

- `math`;
- `png`, `jpeg`, and `pdf`; and
- `native-runtime` and its system clock, time-zone, and random adapters.

The bundled runtime is deterministic. Generated language APIs keep the complete operation
vocabulary, so callers can share wrapper code across default and custom artifacts. An unavailable
operation returns the existing typed `missing-capability` error with the required capability ID;
requesting native runtime policy without the adapters returns the existing typed unsupported
operation error. Runtime and presentation catalogs remain the authority for UI discovery.

The source-only `merman-ffi` C ABI reference profile remains complete so the ABI and every output
path continue to receive repository verification. Source consumers may build `merman-ffi`,
`merman-uniffi`, or the Android transport with any valid direct feature set, including math,
binary exports, and native runtime adapters.

The same audit applies to every other cross-language artifact, but it does not force one universal
feature list. Each adapter keeps only the capabilities exercised through its public interface:

| Adapter or package | Selected compiled capability policy | Reason |
| --- | --- | --- |
| Android JNI, Apple UniFFI, Python UniFFI, Flutter C ABI | `analysis,ascii,layout-cytoscape,layout-elk,svg` | These are the common native render, inspection, and text-output workflows. |
| Source-only C ABI reference | Complete native feature set | It verifies ABI stability and every output path; it is not a downloadable default SDK. |
| Typst WASM | `analysis,layout-cytoscape,layout-elk,svg` | The Typst interface exports SVG render and canonical analysis. ASCII and binary export have no callable Typst operation, and math violates the admitted import boundary. |
| Public Node N-API alpha and private Node-WASM comparison transport | `layout-cytoscape,layout-elk,svg` | The distributed interface is deterministic static SVG plus metadata/layout operations. Math, analysis, ASCII, and binary export are not part of the public Node workflow. |
| Browser WASM package group | Package-specific full and slim recipes | Package identity is already the user-visible capability selector; `web-full` and `web-render` retain complete SVG math semantics. |

The Rust facade, CLI, and LSP remain separate products. Their capability sets follow their own
interfaces and are not inferred from a language-binding default.

The size-oriented `native-distribution` Cargo profile is used for distributed dynamic libraries.
Apple static-library slices retain `native-sdk`: although `native-distribution` compressed the
standalone archive slightly better on the measured host, the identical linked Swift smoke binary
was 18.97 percent larger raw and 1.25 percent larger compressed. The complete source-reference C
ABI build also retains `native-sdk`. Artifact profiles, rather than Cargo profile names, own the
capability contract.

## Evidence

Matched host builds on the same revision showed that the selected feature set reduced compressed C
ABI and UniFFI dynamic libraries by about 32 percent compared with their complete native feature
sets. Matched UniFFI static-library experiments showed a reduction of about 39 percent under the
same size-oriented profile. These measurements justify the capability boundary; platform release
workflows remain responsible for final wheel, AAR, XCFramework, and pub archive receipts.

A separate matched Node-WASM experiment removed only the unadvertised math capability from the
private static-SVG recipe. Raw and gzip sizes fell by about 17.3 percent and the resolved normal
dependency set fell from 186 packages to 127 without changing the callable operation set. This
supports interface-shaped recipes rather than one universal cross-language feature list.

## Consequences

- Common native users retain SVG rendering, both supported layout engines, ASCII output, semantic
  operations, diagnostics, validation, and document analysis.
- Typst retains its public analysis interface without paying for an uncallable ASCII or export
  surface; the public Node alpha no longer pays for an unadvertised math closure.
- Math-bearing diagrams and PNG, JPEG, or PDF output require a custom native build or another
  product surface that advertises those capabilities.
- Native clock, time-zone, and randomness require a custom build with `native-runtime`; omission
  never silently changes a request that explicitly asks for native policy.
- Wrapper APIs remain coherent across feature profiles, while runtime discovery prevents a UI from
  advertising unavailable operations.
- Adding another prebuilt native SKU still requires a distinct user workflow, measured final
  package evidence, package naming, legal closure, and release ownership.
