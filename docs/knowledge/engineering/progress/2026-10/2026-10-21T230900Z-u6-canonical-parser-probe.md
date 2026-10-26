---
type: Work Progress
title: Migrate the three-family artifact probe to canonical parsing
description: Preserve frozen consumed outputs while removing full parsing from the semantic artifact consumer.
timestamp: 2026-10-21T23:09:00Z
record_id: ccc7e652-2a12-4c18-a3bd-cf667f06b07e
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: dcb45f16d
---

# Probe Migration

`tools/bench/fixtures/semantic-parser/main.rs` now calls `SemanticParser` and
matches `SemanticDiagramModel`. Its runtime input, consumed canonical fields,
ordering, and emitted observations are unchanged. No JSON, SVG, layout, or rich
editor capture is introduced to compare outputs.

The committed baseline remains at source
`e8a2052673d0b269137f45182cee2acfd1444b9a` with probe
`ef27b57b7aebd3fa1650d39d3dad3144b6bc07f6` and receipt
`target/bench/semantic-boundary/size-base-post-pr225/receipt.json`.
Its 2,850,304 stripped bytes remain the required final comparison target.

An independent temporary workspace, outside the repository Cargo workspace,
depends normally on core with defaults disabled and only `diagram-flowchart`,
`diagram-sequence`, and `diagram-state`. Resolved metadata confirms exactly those
three features, with `presentation`, `compat-json`, `editor-facts`, and
`test-support` absent. Its locked offline debug build succeeds using the existing
shared D-drive validation target. Core emits twenty feature-specific warnings;
this is not a strict-lint result.

The actual executable read the three unchanged frozen corpus files. Input hashes,
raw output lengths, and raw output hashes all match the baseline receipt:

| Corpus | Output bytes | SHA-256 |
| --- | ---: | --- |
| Flowchart medium | 2,292 | `612a4aa93c7735a674dd49f8ca4a7d6556ef0c7325d65f68851279caeeff055d` |
| Sequence medium | 1,051 | `c3ded25de6f7ace71f84354fd69043b8db0a76814692a7151e161780c187fc17` |
| State medium | 624 | `9c425f08877aa9424a62e1af3de6641e497d05524f67ca61880291504a12713f` |

The executable's `--families` output also matches the same three-family list.
The migrated probe source SHA-256 is
`57ec6a3d430724754f45e861ab4b2e90d74c86cde1d456f5f18c0ddf0cc2445a`;
the current workspace lock SHA-256 is
`de1fbd7c80a19c446943d97b6dedbf1c43046914bf02e7cb31f8d2b7b269aa3e`.
The temporary consumer and resolved metadata are retained under
`D:/merman-u5-temp/semantic-probe-canonical` and
`D:/merman-u5-temp/u6-semantic-probe-resolved-metadata.json`.

The artifact harness's five diagram-selection unit tests, workspace format check,
and `git diff --check` pass. This validates the API migration and consumed-output
equivalence only. The final committed-source release build, selected dependency
version comparison, matched stripping/profile/toolchain, and strictly smaller
artifact measurement are still required by U9; this debug executable supplies no
size or latency improvement claim.
