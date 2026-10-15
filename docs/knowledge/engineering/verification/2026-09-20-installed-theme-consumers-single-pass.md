---
type: Verification
title: Installed theme consumers after XML reference single-pass integration
timestamp: 2026-09-20
git_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Scope and source

This refresh rebuilds the Python UniFFI wheel and Node Darwin ARM64 and explicit Node WASM
packages from clean integrated source `bdb209e1166af960121ce0e2b3231d9929d3bca1`, which
contains the confirmed XML/reference single-pass SVG optimization. It retains version
`0.8.0-alpha.7`; no package was published and no tag was created.

The host is macOS ARM64 with Rust 1.95.0, Python 3.14.7, Node 24.21.0 and npm 12.0.2.
Cargo builds run serially with one build job. Package profiles and capability declarations are
unchanged from the preceding consumer owners. Optional embedded fonts, math and binary exporters
remain outside these package builds.

# Installed verification

The wheel was generated with `scripts/build-python-uniffi-wheel.py`, installed into a fresh
virtual environment without dependencies, and imported from that environment. The checked-in
Python smoke passes its complete authoring/support/catalog/resource suite: schema, shared vectors,
three families, isolation, scoped rules, cold spec, three direct preset imports, three complete
Cyberpunk scenes, five recipe rejections, 22 support queries and three budgeted operations.

Both Node targets were built by the package-owned `build-candidate.mjs`, assembled with
`assemble-packages.mjs`, and accepted by `verify-packages.mjs`. Each packed group was installed
offline with lifecycle scripts disabled into a fresh project. The installed public entrypoint was
resolved by `smoke-installed-package.mjs`; no source workspace runtime was used.

| Target | SVG renders | JSON operations | Support queries | Authoring diagnostics | Resource checks | Capability rejections | Recipe comparisons |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Darwin ARM64 N-API | 30 | 23 | 44 | 6 | 2 | 8 | 12 |
| Node WASM | 30 | 23 | 44 | 6 | 2 | 8 | 12 |

Each Node consumer also imports three complete recipes in three fresh processes, covering 12
SVG/metadata comparisons across four families. Clear and transparent values retain their expected
support explanations and successful renders do not promote unsupported catalog cells.

# Artifact identities

| Artifact | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-0.8.0a7-py3-none-macosx_11_0_arm64.whl` | 9,052,430 | `22c2a6782ef6776a37f0a309a07f6c49f419ddae63bca693632d60ca00b565d4` |
| `mermanjs-node-0.8.0-alpha.7.tgz` | 113,984 | `2adac267fb5da39f37c8f707a8ffd617f35e3808ae60103731df03424ad5b411` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.7.tgz` | 10,550,040 | `3adb9b368e8019f64c046f2636750d07c00ce2d7753658da45d3f84b728ef310` |
| `mermanjs-node-wasm-0.8.0-alpha.7.tgz` | 7,025,160 | `7ae083bdd1a300694e70fef9002248156e4e91f15467e779638ef037c6558e3f` |

The Node build receipts bind source digest `sha256:f14658ed8d9a90f6acb6a843ff3757ddba9f11d268557d4a09a86c80c18039f2`.
The complete machine-readable identity, receipt hashes, logs and package outputs are in
`target/bench/experiments/installed-theme-consumers-bdb-20260920/current-artifact-identities.json`.

# Cross-process recipe exchange

Python, Darwin ARM64 Node and Node WASM each export and import the complete Cyberpunk recipe.
Every transport compares all three transport origins across Flowchart, Sequence and XY Chart:
27 comparison records across the three consumers and three families. The outputs are identical
within each family: Flowchart 33,350 bytes, Sequence 48,752 bytes and XY Chart 33,712 bytes,
with the existing SHA-256 identities `952eea6b...`, `d17cb376...` and `f5717762...` recorded in
the raw exchange JSON files. No normalization or pixel tolerance is used.

# Limits

This is local macOS ARM64 package evidence. Registry installation, other operating systems,
mobile consumers, portable fonts, binary exporters and matched alpha.6 package deltas remain
unverified. Web, Typst and CLI/LSP records still bind earlier source revisions and must be renewed
before they can serve as final-candidate identities. This record does not close C7a or the broader
U10 footprint matrix.
