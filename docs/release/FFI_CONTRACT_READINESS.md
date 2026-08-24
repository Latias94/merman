# FFI Contract Readiness

This is the final pre-release readiness view for the FFI contract alignment work. It is a
candidate-branch engineering record, not a publication or release approval. The public-native
and experimental public Node lanes are reported separately because their distributed capability
sets and release evidence differ.

## Readiness lanes

| Lane | Status | Contract boundary | Evidence |
| --- | --- | --- | --- |
| `public-native` | green | C ABI 3, Android JNI transport API 2, UniFFI API 5, and the shared default native prebuilt SKU | current artifact-profile dependency claims and the platform verification script |
| `public-typst` | green | Typst plugin ABI 2 with SVG, canonical analysis, and both layout backends | exact `typst-wasm` recipe, import/export validation, package smoke, and size matrix |
| `public-node-alpha` | green, experimental | deterministic static SVG plus metadata/layout operations with both layout backends and no specialist math/export closure | public seven-package contract, generated wire contract, glibc-baseline native builds, target install/render smokes, and verified npm package-group workflow |

The public-native lane does not claim that Android uses C ABI 3: Android consumes its direct JNI
transport API 2. C ABI 3 retains size-tagged discovery and its current wire layout, but historical
partial-table consumers are no longer a supported SDK target. UniFFI is API 5 and includes
operation-scoped cancellation/deadline controls with structured terminal details. API 5 replaces
the API 4 version-probe symbol so stale generated bindings fail before decoding revised records.
Source SDK breaks do not retain compatibility aliases.

## Dependency boundary

The dependency verifier evaluates the current descriptor-owned recipes directly. It checks the
required and forbidden package/feature claims that define each semantic boundary while leaving
package versions and source provenance to `Cargo.lock`, cargo-deny, RustSec governance, and the
checked-in license reports. No opaque transitive-dependency snapshot is treated as a second lockfile.

## Native artifact weight

Native package weight is governed by descriptor-owned feature recipes and explicit dependency
denylists. Default Android, Apple, Python, and Flutter artifacts retain analysis, ASCII, SVG, and
both layout engines while omitting math, binary exporters, and native runtime adapters. The SVG
icon registry stays inside the existing `svg` closure: no `icons` feature, acquisition I/O, or
async runtime is added. Final platform package sizes remain release evidence; compiler- and
linker-sensitive library byte ceilings are not general merge gates.

Typst and Node are audited at their own interfaces rather than inheriting the native list. Typst
retains canonical analysis but has no ASCII or binary-export operation. The experimental public
Node alpha retains only SVG and both layout backends; math remains a typed absent capability until
a distinct Node workflow justifies its closure.

## Clean-build timing

Clean-build timing is intentionally not a merge or release gate. It is machine-sensitive and must
measure the exact revision being discussed, so stale checked-in reports cannot prove the current
tree remains fast. Capture matched base/head timing only when making a build-performance claim and
publish it as non-blocking benchmark evidence.

## Verification entry points

Run the following from the repository root after regenerating owned projections:

```text
python3 scripts/verify_artifact_dependency_closures.py --representative-targets  # PR fast lane
python3 scripts/verify_artifact_dependency_closures.py                         # main/release
python3 scripts/verify-platform-bindings.py
```

The reports and commands above describe readiness; they do not publish packages, move a tag, or
change the Mermaid baseline.
