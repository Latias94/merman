---
type: Verification Record
title: Native C ABI recipe, font and export journeys
timestamp: 2026-09-20
source_commit: 25f811599498ae0af56dd60d9319ac1e0f6f9f1e
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Result

An independently compiled C executable now exercises the current `c-abi-native` dynamic library through ABI 3. The run completes 120 fresh processes: 107 successful operations and 13 expected errors. Each successfully constructed engine repeats its operation and requires identical data, metadata and status; constructor failures are checked separately. Both checked-in C examples and the current/alpha.5 C smoke functions also run against this dynamic library.

This is macOS ARM64 source-built consumer evidence. It extends the prior Rust-linked C tests with an external executable, complete recipe editing, and caller-supplied font/export checks. The [declared C ABI distribution](../../../../crates/merman-ffi/README.md#build-from-source) is a source crate with a reproducible host reference profile; no generic prebuilt C SDK archive is published. Packaged source-crate consumption and hosted Linux/Windows/Intel execution remain unverified. The complete C7a matrix remains open.

# Actual consumer paths

| Path | Checked result |
| --- | --- |
| Runtime catalog | ABI 3, package `0.8.0-alpha.7`, and the `embedded-fonts` capability |
| Ten presets | Export each schema-1 complete recipe, save and reload JSON, then compare State SVG bytes from preset selection against direct recipe import in different C processes |
| Public Cyberpunk recipe | Flowchart, Sequence, XY Chart and Class preset/recipe SVG pairs match byte-for-byte |
| Scoped recipe edits | Three saved whole recipes change canvas base, Node border and Class fill; twelve request/constructor pairs compare complete SVG bytes and metadata across fresh processes |
| Clear and Transparent | Class Clear restores `#ECECFF` and reports a source-addressed `unsupported-paint` residual with rejected target admission; Transparent emits `transparent`, while ordinary edits report verified theme execution and Unverified SVG target status |
| Source ownership and isolation | Actual Class outer-shape paths retain source-owned `#334455` fill, `#778899` stroke and width `4`; the other node takes the requested paint. Class fill variants leave the other three families' SVG bytes unchanged |
| Shared support vectors | All 22 results match the complete expected JSON objects |
| Shared authoring errors | All three results match status, resource and structured authoring diagnostics after removing only the nonempty human message; the resource error retains status 10 |
| Native binary exports | Four Cyberpunk families produce eight PNG/PDF files at default limits, each repeated in the same engine with identical bytes and metadata |

The C executable negotiates the current header digest, calls `engine_new`, `execute_collect` and `result_free`, then closes the engine. A Python driver only prepares JSON/input files, launches the C executable and inspects returned bytes. It does not invoke Python bindings or substitute an in-process Rust entrypoint. Constructor options and request options are separate lanes, with identical effective requests. This bounded explicit paint edit does not establish global semantic brand recoloring.

# Caller-supplied fonts

The font cases use the two existing licensed fixture slices, `Excalifont-Regular-Latin.woff2` and `Xiaolai-Regular-CJK-Test.woff2`, supplied through complete-spec JSON. They are not newly bundled product resources. The English scene uses the Latin slice; the unchanged `fixtures/themes/sources/mixed-script-typography.mmd` uses both and the font stack `Excalifont, Xiaolai SC`.

Both scenes succeed as SVG/PNG/PDF through request and constructor options (12 processes). Their metadata says `font_source: embedded` and `theme_status: verified`; SVG remains `target_status: unverified`, while PNG/PDF report `portable` for these specific requests. Native PNG and PDFKit rasterization visibly preserve `Hand drawn`, `测试` and `Portable theme` in the mixed-script scene. This is a local visual check plus runtime admission, not public preset qualification or a multi-host font comparison.

Removing the CJK slice from the same mixed-script request produces six expected `render-error` results (SVG/PNG/PDF × request/constructor). No output payload is produced; the error explains that an admitted font or glyph could not be resolved. Four malformed-WOFF2 cases (spec/complete recipe × request/constructor) return `invalid-argument`, with no missing-capability claim. This differs correctly from the default Node/Apple profiles, which lack embedded-font processing and reject that input at capability admission.

Two resource-free SVG cases name a deliberately absent family followed by `sans-serif`. They preserve the requested name and contain no embedded font face/data. These cases prove the font-reference contract only; they do not measure the absent family's metrics.

PDFKit independently opens all six retained PDFs (four Cyberpunk and two font cases), requires one nonempty page, and writes previews. Only the mixed-script PNG/PDF preview pair was visually inspected in this tranche; page opening does not qualify the other PDF scenes.

# Reproduction and identities

Host toolchains: Rust `1.95.0`, Apple C compiler, Swift `6.3.2`/PDFKit on macOS ARM64. The exact descriptor owner build was:

```console
CARGO_BUILD_JOBS=1 python3 scripts/artifact_profile_recipe.py c-abi-native --build --locked
```

The declared `native-sdk` recipe includes SVG, PNG/JPEG/PDF, embedded fonts, math, analysis, ASCII, both layout engines and native runtime adapters. Unlike the default Apple/Node distribution profiles, this reference build is intended for custom source embedders with these capabilities.

The ignored experiment directory is `target/bench/experiments/c-abi-journeys-20260920/`. It retains `consumer.c`, `run.py`, executables, inputs/options, complete recipes, outputs/metadata, PDFKit previews and `receipt.json`. The preceding build log is `target/bench/experiments/c-abi-current-20260920-build.log`. The C build uses `cc -std=c11 -Wall -Wextra -Werror`, the public include directory and `-L target/native-sdk -lmerman_ffi`. `otool -L` resolves the consumer to `target/native-sdk/deps/libmerman_ffi.dylib`; that file and the top-level library have the identical digest below.

| Artifact | Identity |
| --- | --- |
| Dynamic library | 36,225,200 bytes; SHA-256 `e89fd7c99da39c228e3eb92306a28414972b8daf752a3815f33c2308599383e7` |
| C executable | `e686ed419154a9f4ac85c4d2c43e3b1e2d323d9bc03965e9dbd800f272dfa408` |
| C source | `8046fb6d16ee880cd05c55d86859e1bba6c443d9e01bb86b4b63d4b6e9a50639` |
| Python driver | `970d8be36c7a378f5bf94c95972b650f5346fcd97467064113dda06a7a000679` |
| Result receipt | `5f887c3bad049fa8d5038dc66d4b14e6703b5c705f9149c0bf5525d5a5a31787` |
| PDFKit receipt | `9e55c3600de5ebbebd76aa90626dd952e123274ac519cb10d9de9c5e9ea9526b` |

The receipt binds the lockfile, profile descriptor, public header, font inputs, C/driver source, runtime library and every returned payload. Size is an uncompressed local artifact observation, not a package delta, latency measurement or budget adjustment. No production API, renderer, font assets or capability declarations changed.

# Reference-plan refresh — 2026-09-21

The same descriptor-owned build and unchanged C/Python sources were rerun at
`fee1c1371db8822f1538c07b9634fea5b7c638b7`, which includes the reference-plan allocation change
`b707f6e5b`. The original receipt's source `25f811599` and the optimization's baseline
`7596b61ad` have identical crates, Cargo manifests/lockfile and capability descriptors. This
is therefore an adjacent native-profile renewal, not another alpha.6 comparison.

All 120 fresh-process rows pass again: 107 successes, six invalid-argument results, one resource
error and six missing-glyph render errors. Every row and its complete data, metadata and media
files match the original run byte-for-byte, including the native PNG/PDF and supplied-font cases.
No error-message normalization was used in this cross-build comparison. The source-built
consumer's linked library path resolves to the rebuilt `target/native-sdk/deps/libmerman_ffi.dylib`;
that library, the top-level output and the frozen candidate copy share the recorded digest.
Dynamic-library override environment variables were removed for the consumer run.

| Artifact | Refreshed identity |
| --- | --- |
| Dynamic library | 36,224,928 bytes; SHA-256 `a1c3371fd1810b3f2a15f2d8a69ee7d4168c52a4ad25556f146c0f1bc9e16859` |
| C executable | `5e3499056a18ab5a9b88d09fd3b378db1ebf6b55ade1612725c3a18a11d1e09a` |
| Result receipt | `06a595443d625ef3faa6513ba8cf91178ee7f349fedffb15ffd302f519bcea27` |

The dynamic library decreases by 272 bytes from the original 36,225,200-byte artifact under the
same profile and host. This is a measured uncompressed artifact delta, not a package-size or
latency improvement claim. Both libraries are frozen under
`target/bench/experiments/c-abi-reference-plan-20260921/`, alongside the build log, consumer,
unchanged driver, returned files, `receipt.json` and `renewal.json`. The original C and driver
source hashes remain those in the preceding table.

This refresh did not rerun PDFKit or visual qualification; the returned PDF bytes match those
previously inspected. Packaged source-crate consumption, other hosts and other transport artifacts
still require their own evidence. The complete C7a/U10 matrix remains open.
