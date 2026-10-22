---
type: "Work Progress"
title: "Family alias retirement and final evidence source"
description: "Eighty pure family aliases are removed with focused passing checks; final artifact and performance evidence must use the resulting commit."
timestamp: 2026-10-22T16:46:00Z
record_id: "518d6638f05045a89794e96a26e197b4"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "881b2be0816920f6ed5d141df2cd94c002765427"
supersedes: "e01c395b38404ca5a4ddf1f5b3163fdb"
---

# Verified State

The complete approved objective remains active. No push, PR, merge, publication or release.
The R16 audit found 74 remaining pure family type aliases and six State renamed reexports.
Seventy-two type aliases came from this migration; Cynefin and Railroad were older redundant
spellings. They have no independent state or presentation identity. All 80 old identifiers
are removed from the 1,802 tracked Rust files, and current migration documentation lists
every canonical replacement. Actual full presentation carriers, fields, serialization,
feature gates and pairing remain unchanged.

Worker `01a16472-10d9-7692-b79c-048692727281` prepared the patch in the isolated
`canonical-family-retirement` worktree. Reviewer `01a1647d-3e7b-7940-ae11-f9aa8ddae23c`
independently found no non-mechanical behavior or sidecar changes. Both agents are closed.

Fresh integrated patch checks:

- Core/render/ASCII/xtask all-target/all-feature Clippy with warnings denied passed.
- Projection/request/typed-family/State SVG and ASCII nextest: 1,494/1,494 passed.
- Isolated lean core doctest: 8/8 passed.
- Format, diff whitespace and indexed path budget passed.
- First nextest selection failed before execution because `corpus_golden` is not a binary;
  preserve `target/semantic-family-retirement-tests.log`. The corrected selection and results
  are in `target/semantic-family-retirement-tests-corrected.log`.

# Completed Pre-Cleanup Evidence

At committed source `881b2be08`, the renewed external evidence queue passed:

- Five Web artifact builds, TypeScript contract checks, package smoke and DOM safety.
- Mounted-diagram browser suite: 10/10, including the prior stale-provenance failure.
- Parser differential: 728 cases, 512 accepted, 216 rejected, zero mismatches.
- Exact CLI process matrix: 24/24, including source defaults and explicit release math.

These receipts are frozen under
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/renewed-20261022/`.
The older strict run remains exit 1 with 187 Chromium passes, one stale-WASM failure and nine
not run. The rebuilt mounted suite supplies its scoped recovery evidence, not a rewritten
strict result. VS Code separately passed 255/255.

# Queue And Measurement Boundary

The integration process (PID 57700, session 68994) finished its CLI child and then stopped on
an exclusively reserved `semantic-size-u1.log` guard. No size or installed-host measurement
from that queue ran. The memory/timing waiter (PID 66076, session 99029) was stopped while
only waiting and had no build children. Do not restart either original queue blindly.
The hold file records why final artifact work waits for this cleanup commit.

External Python wrappers exist for integration, allocation and timing. They wrap existing
owners rather than adding repository infrastructure. `run_allocations.py` and `run_timing.py`
have not sampled anything; their integrated source still names the earlier candidate and
must be advanced before use. Historical U3/U4 pairs and thresholds stay fixed.

The clean timing adapter head currently matches `881b2be08` outside four bench/corpus files;
its source adapter was reviewed. Apply this final cleanup to its product tree and freeze
the new committed identity before measurement. Do not weaken byte-equality, caps or timing
calibration to accept a result. Preserve inconclusive rows.

Windows Dart's `x86_64-pc-windows-gnu` standard library was installed successfully for the
pinned Rust 1.95.0 toolchain. MinGW and Dart 3.11.6 are available. Node/Typst/Dart fresh
installed-product checks remain outstanding, along with final semantic/SVG size and all
renewed adjacent allocation/timing evidence.

# Next Action

Commit this verified breaking cleanup. Pin that commit for final size and host artifacts,
advance the integrated benchmark-only control, and continue the serialized existing checks.
Do not repeat the 661-build strict matrix for this identifier-only patch. Use current output
identities and focused owning checks to bridge unaffected evidence; no whole-plan completion
is established yet.
