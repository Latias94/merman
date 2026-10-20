---
type: "Session Handoff"
title: "Compatibility review and maintenance family integration"
description: "Session Handoff for Compatibility review and maintenance family integration."
timestamp: 2026-10-20T16:36:19Z
record_id: "c87063bc918942efa74946551f34b556"
producer_id: "codex-windows"
run_id: "host-theme-228-232-20261020"
---

# Summary

Goal remains active. Main branch `fix/host-theme-228-232-20261020` HEAD `9a5bf169a`.
Maintenance `.worktrees/v0.9.2-host-theme`, branch `fix/v0.9.2-host-theme`, still
at v0.9.1 `f6202f702`; all maintenance changes uncommitted and task-owned.

# Verified State

- Maintenance U1 core/Presentation tests 491 passed; U2 export native-fonts/png/experimental
  suite 73 passed with 9 platform skips. C4 full suite 21 originally had one invalid
  missing-role expectation; corrected fallback policy now C4 11 tests pass in later run.
- Old Gantt presets do not contain main's unqualified typed Task bug. All three added
  legacy state/foreground regressions passed; no production Gantt port required.
- Main simplification reviews complete. Implemented shared dotted-path Zen lookup,
  nullable-state cleanup, duplicate C4 marker and Gantt style resolution removal.
  Test-value review found no deletions justified. Preserve proof tests.
- Independent main correctness review found four confirmed P2s: Wardley white arrows
  with typed light text; Zen fragment sequence numbers omitted from counts/receipt;
  Zen unsupported paint reconciliation missing source-owner masks; Gantt active+done
  label classes lost by canonical bar state. All have authored fixes and new tests,
  still awaiting root final runtime verification.
- Main full renderer build hit OS error 1455 (Windows commit/pagefile limit), cascading
  rustc ICE; retry jobs 1 also failed. Use process-local CARGO_PROFILE_DEV_DEBUG=0,
  CARGO_PROFILE_TEST_DEBUG=0, CARGO_INCREMENTAL=0 and jobs 1, same target. First
  reduced-debug run exposed a missed renamed Gantt writer call, now patched.
- Maintenance selected CSS safety fixes 3fe754e8b/070e9889b/916d3c529: all four regressions
  failed before production; after port Block and Requirement pass. Look test expected
  classic incorrectly while old default is neo; fixed test to default neo.
- Maintenance Zen/Wardley implementation authored; first integration run 79/86 pass.
  Fixed Zen stylesheet serialization (raw XML escaping, no Mermaid entity decoding)
  and Wardley derived axis text mistakenly treated as explicit annotation foreground.
  Latest maintenance-render-final.log running at capture; poll live session in thread.

# Compatibility

User explicitly asked whether changed `pub struct MermaidConfig` is breaking. Original
v0.9.1 already exposed this opaque type with private fields, no repr guarantee. Private
implementation is now named `value` plus private `ConfigState` (detector tracking,
paint/font flags, raw Wardley axis owner). Clone, Debug tuple formatting and JSON-only
PartialEq remain. No public constructor signature changed. An independent API review
is active; cargo-semver-checks is installed and must run against v0.9.1. Do not claim
full compatibility from inspection alone. Hidden new cross-crate internal helpers are
additive, no old signature removed.

# Backport Decisions

- Include compatible local look selector, Block inline CSS bypass and Requirement safe
  declaration fixes; preserve old shared parser rather than importing main parser stack.
- PDF-only svgtypes feature omission confirmed old source; red check then add existing
  optional dependency feature and green/real smoke still pending.
- Font budget 16384 is already present at tag, contrary to initial research queue.
- Treemap extreme-size fitting bug cannot trigger in old writer: fitting starts fixed
  38/16, min8/4, shared decrement count at most30/12; CSS font-size affects only output
  suffix. Worker did not edit files. Exclude main bounded-fitting series as absent
  precondition, rather than invent defensive abstraction.
- Other CSS scoping/Unicode/half-arrow/HTML parser changes require release reproduction;
  maintain explicit defer/exclude evidence. No theme-feature or baseline upgrades.

# Next Action

Finish serial main renderer/facade/native export/parity/Clippy/fmt/legal checks; fix any
remaining test owner failures, then focused main renderer commit. Validate maintenance
families/core/facade/font feature leaves, semantic compatibility and independent review;
commit units with originating main hashes. Prepare v0.9.2 version through clean linked
worktree projection, release notes and backport mapping. No push/tag/publish/external
messages. Goal cannot complete until these outcomes are verified or explicit environment
limitations documented. Prior plan and memory are authoritative; do not rewrite plan
as progress ledger. All unrelated user edits remain protected.
