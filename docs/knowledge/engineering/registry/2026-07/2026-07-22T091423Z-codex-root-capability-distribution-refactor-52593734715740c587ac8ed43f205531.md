---
type: "Work Registration"
title: "Capability-driven feature and distribution refactor"
description: "Work Registration for Capability-driven feature and distribution refactor."
timestamp: 2026-07-22T09:14:23Z
record_id: "52593734715740c587ac8ed43f205531"
status: "active"
producer_id: "codex-root"
run_id: "019f5be4-4389-72e0-9695-99fe14830072"
related_plan: "docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md"
git_branch: "refactor/family-owned-architecture"
git_commit: "4e2a2d6c6"
registration_id: "codex-root-capability-distribution-refactor"
latest_link: "U13 now uses a semantic capability catalog plus exact Cargo artifact recipes; interface contracts remain at their natural owners."
---

# Scope

Execute the capability-driven feature, artifact, interface, ABI, package, dependency, and
verification redesign through U1-U16 without retaining obsolete alpha compatibility.

# Current Claim

U1 and U2 are committed. U13 is ready to commit: the capability descriptor owns semantic IDs and
additive presets, while the artifact descriptor owns exact Cargo recipes. C ABI, UniFFI, LSP, Web,
Typst, and package contracts remain at their natural owners. Neither descriptor embeds plan units,
documentation paths, release status, or manual evidence. The fixed-point review has 18 confirmed
findings: 3 already closed and 15 tracked by U16. Dependency and feature findings are assigned to
U3/U4/U5/U8/U9/U12.

# Latest Links

- U13 semantic capability and exact artifact-recipe contracts are locally verified.

# Handoff

Commit U13, then execute U16 in risk order: memory/resource safety, parser/semantic correctness,
then CI/interface/current-facing documentation fixes. Continue with environment/output/CLI/ABI/package
feature migration and dependency cleanup. Use the related plan as authority and record verification
in successor memory shards and commits.

# Citations
