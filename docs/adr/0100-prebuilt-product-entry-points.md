# ADR 0100: Prebuilt Product Entry Points

- Status: accepted for the main development line on 2026-11-01
- Amends: the Apple binary-product decision in ADR-0079 and ADR-0097
- Does not change: source feature defaults, maintenance releases, transport ABI versions

## Context

Source consumers can deliberately select Cargo features. A prebuilt consumer instead needs a
package that handles ordinary documents through the chosen host interface. Mathematical labels
and an explicitly requested ELK layout are document semantics; users should not discover their
absence only after installing the recommended SVG renderer.

The Apple size investigation found substantial embedded LLVM bitcode in each of five static
architectures. Its removal from distribution copies preserves the linked consumer while reducing
the download. A large XCFramework is therefore insufficient evidence for removing math from the
recommended product. The existing complete native and WASI SDK recipes already include math and
ELK and need a clearer entry point rather than another capability tier.

## Decision

Pilot task-oriented binary delivery through Apple Swift, native C and WASI:

| Host interface | Recommended recipe | Advanced selection |
| --- | --- | --- |
| Swift | `apple-uniffi-native`, including explicit `math` | Custom source build |
| Native C | `sdk-native-full` | Existing `math`, `standard`, or custom source build |
| WASI | `sdk-wasi-full` | Existing `math`, `standard`, or custom source build |

The existing artifact descriptor remains the build and expected-capability authority. The
`artifact_profile_recipe.py` entry point offers list, description and recommendation queries
without invoking Rust. The recommendation policy selects one recipe per pilot interface;
capabilities, output formats, target triples and diagram families come from that recipe.
Unsupported output or target requests fail with the supported alternatives. Discovery describes
source recipes, not whether a particular version has been published.

Apple's complete SVG product selects math explicitly alongside its existing diagrams, analysis,
ASCII, SVG, Cytoscape, ELK and fixes. It does not add raster/PDF encoders or ambient native runtime
adapters. Rust defaults, convenience aggregates, other language packages, and the meanings of SDK
`standard`/`math`/`full` remain unchanged. Runtime catalogs still own callable capability discovery,
and constrained resource policy still rejects math even when the artifact includes it.

Font acquisition, text measurement and output encoding remain separate concerns. Swift hosts can
supply the existing text-measurement callback. They do not acquire the native C/WASI memory-font
service through a recipe change. No transport ABI or ownership protocol changes are required.

Clean only distribution copies of Mach-O/ELF static libraries with the active pinned Rust
toolchain's LLVM object tool. Retain original build outputs and optimization/LTO settings. Preserve
Windows COFF archives byte for byte: the pinned tool cannot process required short import members,
and an unmeasured download benefit does not justify an archive parser/reassembler. Compare complete
same-source Apple payloads and linked consumers separately; an archive size is not an app size.

The Apple producer emits a complete local Swift package containing the matching generated facade,
resource declarations, XCFramework, manifest and legal material. Consumers can use `.package(path:)`
without a Rust checkout or compiler. A separate binary ZIP and remote URL/checksum manifest
projection prepare an immutable release handoff. The remote manifest must refer to the binary ZIP,
not the complete local-package ZIP. Public remote installation remains unverified until published
and resolved through SwiftPM; this pilot neither creates an external package repository nor
publishes assets automatically.

## Consequences

Ordinary pilot users choose a host and required output before seeing build details. Experts retain
positive features and explicitly reduced SDKs. The recommended product is complete for its
interface rather than a universal all-features build.

Apple's feature change must update legal closure and real consumers together. Qualification covers
ordinary and formula-bearing SVG, constrained-policy rejection, codec absence, all five Apple
architectures, macOS execution, simulator execution, and Swift 5.9 source compatibility. Installed
consumers verify package contents independently of the repository layout.

The policy is main-only. Published v0.9.x assets keep their original capabilities and packaging.
Other host packages require their own product decision and installed-consumer evidence before
changing defaults.
