# Apple Artifact Size And Capability Variants

Date: 2026-10-31 (Asia/Shanghai).
Status: historical v0.9.3 measurements. The main-line product decision is now
[ADR-0100](../adr/0100-prebuilt-product-entry-points.md); its implementation does not change
the published artifact measured here.

## Findings

The published `0.9.3` XCFramework download is 300.9 MB because it contains five architecture
libraries, and the measured iOS ARM64 library carries 97.7 MB of embedded LLVM bitcode.
Removing that bitcode from an experimental copy reduces this library from 140.3 MB to
42.5 MB without changing any linked section's bytes in the tested Swift consumers. A linked
iOS consumer is about 22.8 MB, not 300.9 MB. This is a standalone consumer measurement,
not an IPA, an App Store download estimate, or the incremental size of an existing application.

Prioritize distribution cleanup while preserving the current native code and default
capabilities. Retain the WASI `standard` / `math` / `full` recipes. For Apple, keep the
existing default and consider one optional **Math** artifact before introducing additional
export or lean variants. The current Apple default already includes ELK and does not include
math, so its size cannot be attributed to the formula backend or formula fonts.

## Source And Measurement Boundary

The measured public assets belong to immutable `v0.9.3`, commit
`3b915f69c02d55b873d252cf6c8a6048e7f3427d`. The Apple ZIP SHA-256 is
`b5163a4dcfccc0686f260b7d9f5ad73b6365ba8de06e6110c9afbdf43b1c797a`.
The public archive and checksum were independently verified during the
[Apple backfill](../release/PUBLISH_ORDER.md#093-publication).

All MB values below are decimal: 1 MB = 1,000,000 bytes. Archive sizes, decompressed static
libraries, linked executables and installed application sizes describe different objects.
The investigation downloaded and checked all three WASI archives against their recorded
release digests. Native SDK and CLI download sizes below come from the release asset inventory;
their archives were not downloaded again for this investigation.

## What Is In The Apple Download?

| XCFramework member | Architectures | Uncompressed bytes | Compressed member bytes |
| --- | --- | ---: | ---: |
| `ios-arm64/libmerman_uniffi.a` | iOS device ARM64 | 140,295,520 | 59,945,632 |
| `ios-arm64_x86_64-simulator/libmerman_uniffi.a` | Simulator ARM64 and x86-64 | 281,936,320 | 120,212,826 |
| `macos-arm64_x86_64/libmerman_uniffi.a` | macOS ARM64 and x86-64 | 283,239,824 | 120,597,055 |

The complete ZIP is **300,865,967 bytes**; its file contents total **706,025,724 bytes**.
Headers, module maps and legal material are a small remainder. The archive has no separately
packaged image or font resource large enough to explain the total.

These are five distinct platform/architecture builds. Device ARM64 and Simulator ARM64 are
not interchangeable despite sharing an instruction set. Xcode selects the platform and
architecture required by the consumer. Apple's
[XCFramework guide](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle)
describes the same static-library packaging model and requires separate device and simulator
binaries. Combining them into one fat library would be incorrect.

### The Single iOS Library

Inspection of the original ARM64 `.a` and its Mach-O sections found:

| Content | Bytes | Interpretation |
| --- | ---: | --- |
| `__LLVM,__bitcode` | 97,704,272 | Embedded intermediate representation, 69.6% of the archive |
| All `__TEXT` sections | 22,101,063 | Machine code, constants, strings and unwind data |
| `__DWARF` sections | 5,467,654 | Debug material present in archived objects |
| Symbol and string tables | 6,848,528 | Link-time metadata |
| Relocation entries | 4,469,928 | Link-time metadata |

The remaining bytes include data, compact unwind information, the archive symbol index,
headers and alignment. The largest archived objects are `merman-render` (42.4 MB),
`merman-core` (26.0 MB), `merman-ascii` (15.8 MB), `std` (9.1 MB), and the ELK implementation
(7.2 MB). These object sizes include their bitcode and metadata; they do not measure each
component's final application cost.

The source recipe uses `native-sdk`: optimization level 3, ThinLTO, one codegen unit,
`debug = false`, and `strip = "debuginfo"`. Therefore "a debug build was published" would
be an incorrect explanation. Precompiled dependency objects can still carry debug sections
inside a static archive. Most of the measured overhead is bitcode, not DWARF.

Rust documents [embedded bitcode and its relationship to LTO](https://doc.rust-lang.org/rustc/codegen-options/index.html#embed-bitcode).
In particular, disabling embedded bitcode globally while retaining `-C lto` is invalid.
The experiment below removes delivery-time bitcode from already-built libraries; it does
not disable the Rust optimization pass. Apple deprecated App Store bitcode in
[Xcode 14](https://developer.apple.com/documentation/xcode-release-notes/xcode-14-release-notes)
and removed support in
[Xcode 15](https://developer.apple.com/documentation/xcode-release-notes/xcode-15-release-notes).
That Apple product history does not by itself prove a Rust static-library transformation safe;
the consumer comparison supplies the bounded local evidence.

## Bitcode Removal Experiment

The experiment used matching `v0.9.3` generated Swift and C declarations, the published
static libraries, and `llvm-objcopy` from Rust 1.95's installed LLVM tools:

```text
llvm-objcopy --remove-section=__LLVM,__bitcode --remove-section=__LLVM,__cmdline \
  original.a experiment.a
```

The Swift consumer constructs `Merman`, renders `flowchart LR` with two labelled nodes, and
checks for an SVG result. Compilation uses `-O -whole-module-optimization`, Swift language
mode 5, and linker dead stripping. It includes the complete generated facade and resource
contract. The compiler is Apple Swift 6.4; this experiment does not establish Swift 5.9
compatibility. The iOS build targets iOS 14 with SDK 27.1; the macOS build targets macOS 12
with SDK 27.0.

| Measurement | Original library | Bitcode removed |
| --- | ---: | ---: |
| iOS ARM64 static archive | 140,295,520 | 42,490,152 |
| macOS ARM64 static archive | 140,512,144 | 42,623,512 |
| Linked iOS executable | 22,781,496 | 22,781,416 |
| Linked macOS executable | 22,976,824 | 22,976,744 |
| iOS archive compressed with raw DEFLATE level 6 | 59,908,213 | 14,563,875 |

Every linked Mach-O section payload is byte-identical between original and transformed
inputs on both tested platforms. The complete executable files differ in load-command/link
metadata and symbol material, so they are not claimed to be byte-identical. Both macOS
executables run successfully and produce identical SVG bytes. The iOS executables were
linked and inspected, not executed on a device or simulator. Their Mach-O platform is iOS,
minimum OS is 14.0, and SDK is 27.1; the macOS minimum is 12.0.

The controlled compression row uses identical DEFLATE settings. It is not the existing
ZIP member's exact compressor output, nor a measured replacement for the entire five-build
XCFramework. The result supports a substantial download reduction without promising a
specific future whole-archive size or a smaller final application.

The installed Xcode `bitcode_strip` failed because its internal linker command used an
unsupported bitcode-processing option. No result is attributed to that failed command;
the successful transformation used the pinned Rust LLVM tool above. A future packaging
change must select and pin a supported tool rather than depend on obsolete Xcode behavior.

## Why Other Platforms Look Smaller

| Published `0.9.3` asset | Compressed bytes | Payload boundary |
| --- | ---: | --- |
| Apple XCFramework ZIP | 300,865,967 | Five static architecture libraries, ZIP |
| Native C SDK standard, macOS ARM64 | 61,611,248 | One architecture, static **and** shared libraries, fonts and SDK files, xz |
| Native C SDK math, macOS ARM64 | 67,863,552 | Same SDK delivery boundary, plus math |
| Native C SDK full, macOS ARM64 | 71,920,308 | Same SDK delivery boundary, plus math and ELK |
| CLI, macOS ARM64 | 12,387,252 | One linked executable and distribution files, xz |

Native binding wheels or shared libraries have already crossed the final native link boundary;
they should not be compared directly with a five-architecture `.a` collection. Compression,
architecture count, target instruction set, enabled features and compilation profile also
differ. WASI uses `wasm-size` (`opt-level = "z"`, full LTO, aborting panics) and ships one
linked reactor, whereas Apple ships relocatable native link inputs. These measurements do
not establish that one runtime architecture is intrinsically tens of times less efficient.

The checked WASI archives contain:

| Flavor | Download bytes | Reactor `.wasm` bytes | All file contents, bytes |
| --- | ---: | ---: | ---: |
| `standard` | 3,751,660 | 15,800,961 | 17,152,387 |
| `math` | 4,497,484 | 19,103,971 | 20,545,535 |
| `full` | 4,804,768 | 20,808,399 | 22,280,432 |

The WASI archive includes ordinary-text fallback fonts outside the reactor as well as SDK
and legal files. Compressed download size is not the reactor's memory requirement.

## Capability Variants

The published [artifact descriptors](https://github.com/Latias94/merman/blob/3b915f69c02d55b873d252cf6c8a6048e7f3427d/capabilities/artifact-profiles-v2.json)
are authoritative for these recipes:

| Surface/flavor | Math | ELK | Outputs |
| --- | --- | --- | --- |
| Apple default | No | Yes | SVG, ASCII |
| Native SDK standard | No | No | SVG, PNG, JPEG, PDF, ASCII |
| Native SDK math | Yes | No | Same native SDK outputs |
| Native SDK full | Yes | Yes | Same native SDK outputs |
| WASI standard | No | No | SVG, PNG |
| WASI math | Yes | No | Same WASI outputs |
| WASI full | Yes | Yes | Same WASI outputs |

All recipes include the diagram families and Cytoscape. Semantic/layout/analysis services
are additional operations; the table lists rendering outputs. "Full" adds ELK to the math
recipe; it does not add export formats within either SDK transport. WASI full does not imply
the native SDK's complete output or host-service set.

The three WASI recipes are reasonable, stable delivery choices. Start with standard when
neither math nor ELK is needed; choose math for formulas, or full when ELK is required.
The absent math-off/ELK-on combination is a source-build option. Add another prebuilt
combination only when a concrete consumer needs that tradeoff; do not generate every
feature combination. Standard is not an MIT-only closure: MPL and font notices still apply,
and full additionally includes the ELK/EPL material.

### Original Apple Recommendation and Subsequent Decision

The optional Math variant below was this investigation's incremental recommendation.
The maintainer's subsequent main-line product review selected one recommended complete
SVG product with math, a self-contained local SwiftPM package, and bitcode cleanup instead
of adding another ordinary installation choice. See
[ADR-0100](../adr/0100-prebuilt-product-entry-points.md) and the
[current selection guide](../release/PREBUILT_PRODUCTS.md). Retain the measurements below
as evidence for v0.9.3, not instructions to publish two main-line Apple variants.

Keep **Merman** as the existing default. Removing ELK to rename it "standard" would change
layout behavior and an accepted default capability; see
[ADR-0089](../adr/0089-mermaid-12-default-elk-products.md). Add **Merman Math** as an optional
complete prebuilt runtime if the maintainer selects that new delivery scope: the current
recipe plus `math`, retaining ELK and omitting codecs. The consumer should still use
`import Merman` and install exactly one matching artifact. Do not call this variant "Full",
which would suggest the headless SDK's exports or font services.

The existing UniFFI source supports optional math and codecs. Prebuilt Math delivery still
needs a descriptor-owned recipe, package selection, corresponding legal material and real
Swift consumer evidence. Multiple flavors cannot occupy identical platform/architecture
entries in one XCFramework. They export the same UniFFI symbols and must be mutually
exclusive complete installations, not independently composable libraries. A remote SwiftPM
manifest is a separate consumption improvement; the currently published route is a local
package plus an extracted archive.

An Apple Export product requires more design. Swift services currently expose text
measurement and icon callbacks, not the native SDK's caller-owned font-byte context.
Enabling a codec feature does not add that missing Swift API. Host measurement establishes
geometry but cannot give an encoder the same glyph resources automatically. Formula fonts
are a separate resource set from ordinary-text Cousine fallback, and neither set promises
full CJK/emoji coverage.

## Recommended Work Order

1. Make delivery smaller without changing native code: strip delivery-only embedded bitcode
   after the native build, preserve symbols required by linking, keep Rust LTO, and compare
   actual consumers. Retain the original build inputs when diagnosing a packaging failure.
2. Qualify that transformation across all five architectures, generated bindings and legal
   material. Run macOS and iOS Simulator consumers, check declared deployment targets, and
   measure both the complete ZIP and linked applications. Do not replace an accepted
   `0.9.3` asset; ship a future source-matched release.
3. If Math delivery is selected, publish the two mutually exclusive Apple choices above.
   Test a successful formula, default-artifact capability absence, and constrained-policy
   rejection. Preserve interactive limits and document cooperative cancellation boundaries.
4. Consider iOS-only/macOS-only downloads only after measuring the cleaned combined package.
   Platform bundles and capability choices are independent dimensions; avoid multiplying
   release assets before establishing a useful consumer workflow.

This investigation changes neither published artifacts nor artifact features. The CI runner
migration is tracked separately in the two release-repair PRs; its compiler and Apple SDK
coverage boundaries are documented in [CI](../development/CI.md#apple-binding-checks).
