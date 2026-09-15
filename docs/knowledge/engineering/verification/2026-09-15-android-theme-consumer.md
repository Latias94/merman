---
type: Verification Evidence
title: Android theme authoring consumer smoke
timestamp: 2026-09-15
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,android,authoring,verification
---

# Result

The Android public consumer smoke now exercises `materialize-theme-json` through both the static
one-shot API and a reusable `MermanEngine`. Each path must return the structured
`theme_authoring` envelope for an invalid empty font stack, including schema version, diagnostic
code, and JSON path. The smoke also applies a one-byte `max_theme_encoded_bytes` limit through
both consumers and requires the theme-specific resource error with authoring details.

The instrumentation test source also uses the current static runtime-catalog validator API. It
builds successfully with the Android debug test artifact.

# Validation

```text
platforms/android/gradlew -p platforms/android testDebugUnitTest --stacktrace
BUILD SUCCESSFUL

platforms/android/gradlew -p platforms/android assembleDebug assembleDebugAndroidTest --stacktrace
BUILD SUCCESSFUL
```

The CI Android emulator job runs `connectedAndroidTest` through
`python3 scripts/verify-platform-bindings.py --only-android-instrumentation-smoke`. This local
host does not have an Android emulator, so device execution remains CI-owned evidence.

# Boundary

This proves the Kotlin/JNI consumer code compiles and that the real instrumentation smoke contains
both authoring paths. It does not claim a local device execution, other Android ABIs, or C7a
contract freeze.
