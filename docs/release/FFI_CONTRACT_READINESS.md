# FFI Contract Readiness

This is the final pre-release readiness view for the FFI contract alignment work. It is a
candidate-branch engineering record, not a publication or release approval. The public-native
and private-Node lanes are reported separately because the Node candidate remains private even
when its transport checks are green.

## Readiness lanes

| Lane | Status | Contract boundary | Evidence |
| --- | --- | --- | --- |
| `public-native` | green | C ABI 3, Android JNI transport API 1, UniFFI API 4, and the one full native SDK SKU | current artifact-profile dependency claims and the platform verification script |
| `private-node` | green, private | deterministic, static-SVG, text-only candidate; not an admitted or publishable native SDK surface | current artifact-profile dependency claims and the Node package contract tests |

The public-native lane does not claim that Android uses C ABI 3: Android consumes its direct JNI
transport API 1. C ABI 3 retains size-tagged discovery and its current wire layout, but historical
partial-table consumers are no longer a supported SDK target. UniFFI uses API 4. Source SDK
breaks do not retain compatibility aliases.

## Dependency boundary

The dependency verifier evaluates the current descriptor-owned recipes directly. It checks the
required and forbidden package/feature claims that define each semantic boundary while leaving
package versions and source provenance to `Cargo.lock`, cargo-deny, RustSec governance, and the
checked-in license reports. No opaque transitive-dependency snapshot is treated as a second lockfile.

## Native artifact weight

Native package weight is governed by descriptor-owned feature recipes and explicit dependency
denylists. The SVG icon registry stays inside the existing `svg` closure: no `icons` feature,
acquisition I/O, async runtime, or second native SKU was added. Binary size remains useful evidence
for focused optimization work, but compiler- and linker-sensitive byte ceilings are not merge gates.

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
