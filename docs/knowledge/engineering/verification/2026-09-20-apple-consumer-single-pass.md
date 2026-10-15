---
type: Verification Record
title: Apple Swift consumer after XML reference single-pass integration
timestamp: 2026-09-20
source_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
---

# Result

The macOS Apple XCFramework was rebuilt from clean integrated source
`bdb209e1166af960121ce0e2b3231d9929d3bca1` with the repository owner
`scripts/build-apple-xcframework.sh --macos`. The generated ARM64/x86_64 macOS framework was
consumed by the existing Swift smoke package with one Swift job.

The smoke passed all current owner lanes: two materializations, 22 support queries and five
structured errors per one-shot/reusable consumer; ten preset exports per consumer; four
Cyberpunk families; ten malformed recipe rejections; three fresh-process recipe imports; ten
Class boundary cases; strict Clear and missing-family checks; six font capability rejections; and
dense Class/XY scenes. The complete output and metadata comparisons remained exact within the
smoke contract.

| Artifact | Identity |
| --- | --- |
| macOS XCFramework static library | 341,420,504 bytes; SHA-256 `49bb27e6443b1576d88e250bde771bcb4ba199eb34b11094b85c83020bd61148` |
| Generated `Merman.swift` | 192,841 bytes; SHA-256 `2fbaf99f0817e544e4682dc62b5c6639ebfb1e8adb0d0660303ad89f6a8485cd` |

# Evidence and limits

The build and Swift smoke logs plus retained recipe/boundary artifacts are under
`target/bench/experiments/apple-bdb-20260920/`. This is local macOS ARM64 execution using the
macOS framework's universal slice. It does not prove iOS, Intel runtime execution, Swift 5.9,
installed release archive consumption, visual qualification or hosted CI behavior. No generated
binding or production source changed, and no publication was performed.
