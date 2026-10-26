---
type: "Work Progress"
title: "Final size, installed hosts and native inspection fix"
description: "Final semantic and SVG sizes and installed Node, Typst and Dart pass; Web integration and registered allocation/timing remain open."
timestamp: 2026-10-22T17:46:00Z
record_id: "c0f69db0ebed44bfa82ce80a59231731"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "3c858bdfbc4372b0253144a98c4df47bd3ab72da"
supersedes: "518d6638f05045a89794e96a26e197b4"
---

# Verified State

The complete approved goal remains active. U1-U8 implementation and final alias removal
are committed; U3/U4 performance admission and U9 completion remain open. No push, PR,
merge, publication or release is authorized or performed.

Final raw evidence is retained outside Cargo targets at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-d0d26db24-20261023/`.
The product revision is `d0d26db24baa5823e235553c9a9251e3f70815a2`.

- The isolated three-family semantic consumer decreases from 2,843,648 to 2,126,848
  stripped bytes, or 25.2071%. Runtime-fed consumed outputs match exactly. Defaults are
  disabled; the core closure contains only Flowchart, Sequence and State. Common resolved
  external versions/sources match, although workspace lockfiles differ. Five normal
  external packages and their additional build dependency disappear, with no additions.
- The same-capability SVG control decreases from 13,202,944 to 13,162,496 bytes, or
  0.3064%, with exact output equality for the three fixed fixtures. External packages and
  features match; explicit math is present on both sides. Neither size result proves latency.
- Fresh Node N-API and Node-WASM artifacts passed package assembly/verification, local
  tarball creation, offline installation and installed-package smoke. The native target
  is Windows x64 MSVC. `node-installed-verification.json` freezes the successful steps.
- Typst 0.15.1 compiled 23 package examples with nine expected failures. The 110 retained
  artifact files and hashes are listed in `typst-retained-files.json`. This is local preflight.
- Windows GNU native packaging and standalone Dart 3.11.6 ABI 3, example and theme-authoring
  smoke passed. `dart-host-verification.json` binds logs and identical original/packaged DLL
  hashes. This is not actual Flutter desktop UI or other-platform runtime coverage.

The main performance document records the final size recipes, limits and receipt hashes.

# Native Inspection Correction

The first Dart native build succeeded but its symbol gate read the MinGW import library,
misclassifying linker metadata `merman_ffi_dll_iname` as an exported C API. The actual DLL's
PE export table contains only `merman_get_native_api`.

Commit `3c858bdfb` reads DLL exports using sibling `llvm-readobj --coff-exports` and resolves
the Rust LLVM executable suffix on Windows. The ABI allowlist is unchanged. The 17 shared
symbol-contract tests, 19 Android builder tests and real DLL export check pass. The original
`dart-native.log` failure remains retained; `dart-native-pe-exports.log` supplies recovery.

Only `platforms/flutter/build-native.py`, `scripts/native_symbol_contract.py` and its test
changed after the product revision. `integration-python-only-bridge.json` records the exact
inventory. Successful size/Node/Typst checks need no repeated Rust builds for this correction.

# Runtime At Capture

The initial integration session 75405 terminated on the Dart symbol false positive. Its
waiter 9022 stopped before allocation/timing, as required. Do not restart those queues.

Session 81275 runs the new external `run_remaining.py` serially. The first four Dart steps
passed. Web full and analysis have finished; render is building at capture. Remaining steps
are final Web smoke and mounted-diagram, followed by unchanged `run_allocations.py` and
`run_timing.py` schedules. Read the actual logs/process state before resuming. Existing logs
are opened exclusively; never overwrite them or rerun successful schedules blindly.

The runner checks main HEAD against `3c858bdfb` before every remaining integration gate.
Wait until those gates finish before another documentation commit, or explicitly record a
new source bridge. Do not modify product inputs while a WASM build or browser test runs.

# Measurement And Completion Boundary

All five allocation checkouts and all six timing checkouts are clean at registered revisions.
Runner and receipt hashes match `size-and-runner-freeze-v2.json`. The integration timing
adapter head is `1401e2d17b0c322e4ae9088176e7ccd6dcf98559`; its production tree equals
`d0d26db24` outside the documented benchmark adapter inputs. Fixed caps, scales, samples,
groups, fixtures, A/A procedure and AB/BA pair budget are unchanged. No renewed memory or
timing samples have run at this capture. Historical U3 timing remains NOT ADMITTED.

`completion-audit-after-dart.json` is a successor to the earlier in-progress audit, with
`goal_complete=false`. The old strict result remains exit 1; its earlier rebuilt 10/10 mount
suite is scoped recovery, not a rewritten full strict pass.

The recorded strict matrix has 661 build cases: 512 diagram singletons and 108 capability
leaves dominate, followed by 27 exact artifact checks. Command keys are already deduplicated;
the identified excess is overlapping coverage across wrapper layers. No broad xtask redesign
was mixed into this work. Do not repeat that complete matrix for this Python-only correction.

Finish final Web/mount, inspect every renewed allocation/timing classification, reconcile
R1-R16 and AE1-AE12, then commit task-owned evidence/docs. Keep the original untracked
`crates/xtask/tests/fixtures/diagram-features-NqjeLP/` untouched and unstaged.

# Integration Completion Before Commit

At 2026-10-22T17:52Z, all five Web WASM and TypeScript package builds, source/artifact
identity checks, 37-family package smoke and DOM safety completed successfully. Chromium
desktop mounted-diagram passed 10/10. `final-host-integration-receipt.json` freezes all seven
resumed Dart/Web steps, their log hashes and the five Web input manifests. Its scope preserves
the older failed strict result rather than reporting a new full strict pass.

The original integration HEAD checks have all finished. Session 81275 now runs the unchanged
allocation schedule, currently building the U3 baseline with one Cargo job. It will continue
to timing after all five allocation batches and freezes finish. Documentation-only commits
are now safe: measurements use their separate clean registered checkouts. The remaining
scope is fresh allocation/timing admission, final audit and evidence/documentation commits.
