---
type: Verification Evidence
title: C7a Apple XCFramework and Swift consumer verification
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 6517f6b1f6a1096340ece337f81c4b92f0495b79
tags: theme,c7a,apple,artifacts,verification
---

# Scope and result

The complete `apple-uniffi-native` XCFramework built successfully from the production source
at `6517f6b1f`. All five Rust target libraries were included, the generated Swift binding
remained unchanged, and the Swift package plus macOS ARM64 and x86_64 consumers passed. The release
packaging procedure produced a validated ZIP containing the framework and legal materials.
This is local preparation, not publication or the full Apple compatibility matrix.

The build ran in the primary worktree, initially at `6517f6b1f`. Subsequent commits changed
only plan/verification Markdown; comparison against `6517f6b1f` confirmed no changes in crates,
Apple source or the XCFramework builder. Two pre-existing untracked August engineering-note
directories were retained. This is not independent clean-checkout evidence.

# Executed owners

The host used Rust 1.95.0, Xcode 26.5 (`17F42`) and Apple Swift 6.3.2. Cargo ran with two
jobs and two Rayon threads; Swift compilation also used two jobs.

- `MERMAN_AUTO_INSTALL_RUST_TARGETS=false bash scripts/build-apple-xcframework.sh`: all five already-installed targets built in the existing `native-sdk` profile; UniFFI generated the Swift API 7 projection and `xcodebuild -create-xcframework` succeeded.
- `swift package describe` and `swift build --jobs 2`: package and host module passed.
- `swift run --jobs 2 --package-path platforms/apple/examples/smoke MermanAppleSmoke`: service-backed SVG, ASCII selection metadata, missing-capability handling, resource errors, one-shot/reusable theme-authoring diagnostic envelopes and cancellation passed.
- `swift run --arch x86_64 --jobs 2 --package-path platforms/apple/examples/smoke MermanAppleSmoke`: the same consumer linked the Intel framework slice and passed under host Rosetta; `file` confirms an x86_64 Mach-O executable. This is translated execution, not an Intel hardware runner.
- `git diff --exit-code -- platforms/apple/Sources/Merman/Generated`: no generated changes.
- `python3 scripts/sync-release-legal-materials.py --check`: all 382 legal projections passed freshness checks.
- The existing release ZIP procedure copied `Merman.xcframework`, `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES`; archive membership, ZIP CRCs, and SwiftPM's checksum against an independent SHA-256 calculation passed.

This smoke does not claim the complete shared support/catalog vector suite for Swift itself;
the installed Python and Rust UniFFI consumers separately exercise that shared transport.

# Framework and archive identities

The XCFramework `Info.plist` and `lipo -archs` agree on these slices:

| Library identifier | Architectures | Static library bytes | SHA-256 |
| --- | --- | ---: | --- |
| `ios-arm64` | ARM64 device | 170148640 | `24ac52c0e12dfb326eea3285863d90646959ab3b2baeb5abad7e439fb41917f7` |
| `ios-arm64_x86_64-simulator` | ARM64 and x86_64 simulator | 342018560 | `002f92927d444ae49b19927eaabff8be08d601cd735ea4d368d6019d6a9a4f6b` |
| `macos-arm64_x86_64` | ARM64 and x86_64 macOS | 343432512 | `f5736beff61667f83de442dc6eff16853eca90d851be49d4117de2348ac707c0` |

The local archive is
`/tmp/merman-c7a-6517f6b1f-apple-artifacts/Merman.xcframework-v0.8.0-alpha.6.zip`:
362370110 bytes, SHA-256 `3b005d4e04bbae98fcd1cc1657a353587865ed4ded2d46942fd50536bd0b2fbc`.
The adjacent `.zip.checksum` contains the same SwiftPM digest. Static library and complete
multi-architecture SDK sizes are not final application sizes; no new size budget was set by
this run. The archive was not uploaded and does not replace a published alpha.6 artifact.

Logs are `/tmp/merman-c7a-6517f6b1f-apple-{build,package,swift-build,smoke,intel-smoke,archive}.log`.

# Remaining scope

A Swift 6.3.2 build does not prove the declared Swift 5.9/Xcode 15.2 compiler floor. This
run built iOS/device/simulator libraries without running an iOS application. The macOS
consumer ran natively on ARM64 and under Rosetta for x86_64. Keep the compiler-floor CI lane
and other declared host/device obligations.
Linux/Windows artifact owners, final clean candidate/profile binding and formal C7a
rollout/freeze remain open. No preset qualification cells follow from this SDK smoke.
