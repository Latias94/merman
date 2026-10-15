---
type: Verification Evidence
title: C7a Android archive and device verification
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 6517f6b1f6a1096340ece337f81c4b92f0495b79
tags: theme,c7a,android,artifacts,verification
---

# Scope and result

The Android owner built the complete `android-native` AAR and local Maven publication at
`5e76f9835`. JVM contracts passed 46/46 tests. At `6517f6b1f`, the instrumentation owner
passed 9/9 tests on an API 36 ARM64 emulator, including the public authoring/support/error
and resource-budget consumers. No tests failed or skipped in these final XML reports.
This closes the local Android package/device tranche, not the complete release matrix or
C7a contract freeze. The CI API 29 x86_64 device lane was not executed locally.

Both runs used the primary worktree with clean tracked source and the two previously
present untracked August engineering-note directories. They are not independent clean
checkout evidence. Commit `6517f6b1f` changes only the instrumentation negative fixture;
it does not alter the native library, Android production wrapper or package contract.

# Executed owners

Toolchain: Rust 1.95.0, NDK 29.0.14206865, JDK 17.0.20, Gradle 9.4.1 and AGP 9.2.0.
Cargo used at most two jobs and two Rayon threads. The `android-native` distribution
contains both `aarch64-linux-android` and `x86_64-linux-android` libraries.

- `python3 platforms/android/build-android.py --assemble-aar --ndk-home /Users/frankorz/Library/Android/sdk/ndk/29.0.14206865`: both native targets, symbol contract and AAR content validation passed.
- `platforms/android/gradlew -p platforms/android testDebugUnitTest --max-workers=2 --stacktrace`: 46 tests across five suites passed.
- `platforms/android/gradlew -p platforms/android publishReleasePublicationToLocalStagingRepository --max-workers=2 --stacktrace`: local Maven staging passed; no registry publication occurred.
- `python3 scripts/verify-platform-bindings.py --verify-android-maven`: staged Maven package validation passed.
- `python3 scripts/verify-platform-bindings.py --only-android-instrumentation-smoke`: nine tests passed after the fix, then passed again at committed `6517f6b1f`.

The existing `Medium_Phone_API_36` image ran read-only, without snapshot writes, as
`emulator-5556`. Its reported SDK was 36 and ABI was `arm64-v8a`. The task-owned emulator
was shut down after validation; the saved AVD state was not changed.

# Failure and repair

The first device attempt stopped at a Gradle dependency TLS download failure. A process-local
proxy allowed the same owner to proceed; TLS verification remained enabled. The first
executed device suite then exposed a stale negative fixture: it attempted to replace
`transport_api_version: 1`, while the Android transport already advertises 2. The replacement
did nothing, so the validator correctly accepted the unchanged handshake.

Commit `6517f6b1f` derives both malformed representations from
`Merman.TRANSPORT_API_VERSION`. The test still rejects a quoted numeric version and a
floating-point version. The production Android transport remains 2 and the first-release
theme protocols remain 1. The final committed run succeeded without the proxy after the
dependencies had been cached.

# Artifact identities

| Artifact | Source | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `platforms/android/build/outputs/aar/merman-android-release.aar` | `5e76f9835` | 19638230 | `b30ebb2c73365a64b8e61dbfacedf733a6ba7664faa28c466e633a4c392e2a4f` |
| `platforms/android/build/outputs/apk/androidTest/debug/merman-android-debug-androidTest.apk` | `6517f6b1f` | 43761613 | `f1c776550f97a8ebb5e5eecb6ce07978eb7e95a1a1001867a12441692ea37faf` |

The local Maven repository is `platforms/android/build/repo/io/merman/merman-android/0.8.0-alpha.6`.
This is candidate preparation under the existing development package number, not a replacement
of any published artifact.

Logs are `/tmp/merman-c7a-5e76f9835-android-{build,unit,maven-build,maven-verify}.log`,
`/tmp/merman-c7a-5e76f9835-android-device{,-proxy}.log`, and
`/tmp/merman-c7a-android-device-{fixed,committed}.log`. Final JUnit reports are under
`platforms/android/build/test-results/testDebugUnitTest` and
`platforms/android/build/outputs/androidTest-results/connected/debug`.

Apple XCFramework/Swift consumers, Linux/Windows artifact owners, final profile/catalog
binding and the formal candidate/rollout/contract decisions remain separate obligations.
No new preset qualification cells or portability claims follow from this Android smoke.
