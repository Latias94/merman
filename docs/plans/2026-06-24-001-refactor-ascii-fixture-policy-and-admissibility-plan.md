---
title: "refactor: ASCII Fixture Policy and Admissibility Rules"
type: "refactor"
date: "2026-06-24"
origin: "direct user request; repo-ref/beautiful-mermaid and repo-ref/mermaid-ascii comparison"
---

# refactor: ASCII Fixture Policy and Admissibility Rules

## Summary

This plan turns ASCII fixture choice into an explicit policy instead of an implicit habit. It keeps copied `mermaid-ascii` fixtures as the exact baseline for simple graph and sequence parity, treats `beautiful-mermaid` as capability prior art, and adds a rule for when complex diagrams should use self-authored fixtures instead of a weak external oracle.

This is a documentation-plus-test strategy change, not a renderer rewrite. The goal is to make future fixture decisions predictable, reviewable, and honest about when the project is measuring parity versus validating its own semantic coverage.

---

## Problem Frame

Current docs already distinguish the narrow `mermaid-ascii` copied corpus from the broader `beautiful-mermaid` reference, but the boundary is still spread across several files and is not yet a usable rule for contributors. That works while the scope is small; it breaks down once the team needs to decide whether a new complex diagram should be compared to an upstream fixture, summarized by a semantic assertion, or authored locally because no meaningful upstream standard exists.

This ambiguity matters most for dense or family-specific diagrams. For simple graph and sequence cases, copied fixtures are a good oracle. For complex class, ER, state, and xychart cases, forcing a copied-fixture mindset can either hide the real behavior behind layout noise or reject a useful regression test altogether. The plan makes that decision surface explicit and keeps the gap registry aligned with it.

---

## Requirements

- R1. The repository must distinguish copied upstream oracle fixtures from self-authored local fixtures.
- R2. Simple graph and sequence coverage must continue to use the copied `mermaid-ascii` corpus as the exact-output baseline when the semantics line up.
- R3. Complex diagrams may use self-authored fixtures when external reference output is a poor oracle or no suitable upstream standard exists.
- R4. Fixture admissibility must be documented as a contributor-facing rule, not left to oral convention.
- R5. `beautiful-mermaid` must remain a capability reference only, not a byte-for-byte output standard.
- R6. The docs and gap registry must explain how to classify a fixture as copied, local, or semantic.
- R7. Validation must keep copied upstream inventory checks separate from self-authored regression fixtures.

---

## Key Technical Decisions

- Keep the narrow copied corpus as the hard oracle for graph and sequence parity. Those cases are stable enough that exact comparison is still the right default.
- Treat `beautiful-mermaid` as reference material for breadth and shape ideas only. It helps with family coverage, but its output is not authoritative enough to become a universal baseline.
- Allow self-authored fixtures when the output question is really "did we preserve the semantic behavior?" rather than "do we match this upstream render exactly?"
- Define admissibility by diagram family and test intent, not by a single global rule. A fixture can be exact-parity, normalized-parity, or locally authored semantic coverage.
- Make the policy visible in docs and test metadata so future contributors do not have to rediscover the same judgment call.

---

## Scope Boundaries

In scope:

- Fixture policy docs and comparison notes.
- Fixture classification rules for copied, normalized, and self-authored cases.
- Gap registry language that distinguishes upstream parity gaps from product-only gaps.
- A small set of representative self-authored complex fixtures for cases where copied upstream coverage is weak or misleading.

Deferred:

- New renderer behavior.
- A wholesale rewrite of the existing fixture corpus.
- Broad fixture generation tooling unless the policy review shows manual metadata is not enough.

Out of scope:

- Treating `beautiful-mermaid` as a strict oracle.
- Replacing copied upstream fixtures where they already serve as a stable exact baseline.
- Rebaselining unrelated diagram families just because the policy exists.

---

## Implementation Units

### U1. Define the fixture policy and admissibility rubric

- **Goal:** Make fixture choice a documented rule instead of an ad hoc judgment.
- **Files:**
  - `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/README.md`
  - `crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`
  - `crates/merman-ascii/README.md`
- **Approach:** Add a short policy section that explains when to use copied upstream fixtures, when to use normalized comparison, and when to author a local fixture because the diagram is too complex or too semantically specific for an external oracle.
- **Test scenarios:**
  - A contributor can tell which source to use for a simple flowchart.
  - A contributor can tell why a dense class or ER diagram may be self-authored.
  - The policy does not describe `beautiful-mermaid` as a byte-level standard.
- **Verification:** The policy reads as a decision rule, not as a history note.

### U2. Reclassify fixture inventories and gap language

- **Goal:** Keep the existing inventory and gap files honest under the new policy.
- **Files:**
  - `crates/merman-ascii/tests/fixture_inventory.rs`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/SEQUENCE_FIXTURE_GAPS.md`
  - `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- **Approach:** Keep the copied upstream inventory gate for the narrow corpus, but rewrite the surrounding prose so it no longer implies that every meaningful ASCII regression must come from that corpus. Separate upstream parity gaps from locally authored product coverage.
- **Test scenarios:**
  - The inventory test still protects copied upstream counts and provenance.
  - Gap files describe copied-fixture status without pretending to cover all valid ASCII regressions.
  - The gap registry tells contributors where copied parity ends and local coverage begins.
- **Verification:** A reviewer can trace whether a fixture is an upstream oracle, a local regression, or a family-specific semantic check.

### U3. Add representative self-authored complex fixtures

- **Goal:** Prove the policy by example on diagrams where copied upstream fixtures are not the best answer.
- **Files:**
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`
  - `crates/merman-ascii/tests/state_model.rs`
  - `crates/merman-ascii/tests/xychart_model.rs`
  - targeted fixture files under `crates/merman-ascii/tests/testdata/` when a file-backed case is clearer than an inline case
- **Approach:** Add a small, curated set of self-authored cases for dense or family-specific diagrams. Use them to validate semantic behavior, not to chase a copied shape that is noisy or unavailable.
- **Test scenarios:**
  - A dense class relation case uses a local fixture instead of an upstream copy when the upstream shape is not the right oracle.
  - A complex ER or state case checks semantics that are more important than matching a borrowed layout.
  - A chart or family-specific case uses a local fixture when external parity would be misleading.
- **Verification:** The new fixtures read as intentional product coverage, not as fallback copies.

### U4. Publish the selection rule where contributors look first

- **Goal:** Make the new rule easy to find where fixture work happens.
- **Files:**
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/README.md`
- **Approach:** Cross-link the policy, comparison note, and inventory so a future contributor can start from one place and get to the right standard quickly.
- **Test scenarios:**
  - The README explains where to look before adding or rebasing a fixture.
  - The comparison note points at the policy instead of acting like the policy itself.
  - The fixture README clearly says which parts are copied upstream and which parts are locally authored.
- **Verification:** The docs steer people toward the right fixture class without requiring tribal knowledge.

---

## Acceptance Examples

- Given a small graph or sequence case, the team uses the copied `mermaid-ascii` fixture corpus as the default comparison source.
- Given a dense class or ER case, the team can choose a locally authored fixture when the upstream render is not a meaningful oracle.
- Given a family with no good upstream standard, the team can still add a useful regression fixture without pretending it is copied parity.
- Given the docs and gap registry, a new contributor can tell the difference between upstream parity work and local semantic coverage.

---

## Open Questions

- Should the policy stay documentary only, or should it also add a small machine-readable manifest for fixture class and provenance?
- Should the first landing include self-authored examples for class and ER only, or also state and xychart?

---

## Risks & Dependencies

- The main risk is policy drift: if the docs get wordy but the fixture files do not reflect the rules, contributors will keep guessing.
- Another risk is overusing self-authored fixtures and losing the value of copied parity where it is still a strong oracle.
- This plan depends on keeping the current copied upstream inventory stable while the new policy lands.
- The complex-fixture examples must stay small enough that they teach the rule instead of creating a second, competing corpus.

---

## System-Wide Impact

This mostly changes contributor workflow and test interpretation. It does not alter runtime rendering, but it does shape how future ASCII work is validated and how parity claims are made in docs.

---

## Sources / Research

- `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md`
- `crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`
- `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- `crates/merman-ascii/tests/fixture_inventory.rs`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/README.md`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/SEQUENCE_FIXTURE_GAPS.md`
- `crates/merman-ascii/tests/class_model.rs`
- `crates/merman-ascii/tests/er_model.rs`
- `crates/merman-ascii/tests/state_model.rs`
- `crates/merman-ascii/tests/xychart_model.rs`
- `repo-ref/beautiful-mermaid`
- `repo-ref/mermaid-ascii`

