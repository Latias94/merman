# Mermaid Release Admission Checklist

Read this checklist completely before changing the Mermaid reference descriptor, dependency locks,
or family code. Use it as an evidence index; link durable repository artifacts instead of copying
large logs into the checklist.

## Reference Graph

- [ ] Selected Mermaid tag resolves to an exact source commit.
- [ ] Registry package version, tarball integrity, publish provenance, and source identity agree.
- [ ] The Mermaid workspace lock is recorded as the companion oracle.
- [ ] Each plugin's declared dependency range is read from the selected published artifact.
- [ ] The highest stable version inside that range is evaluated as a candidate.
- [ ] A newer stable version outside the range is reported as a separate behavior delta.
- [ ] Candidate admission covers parser, renderer, security, resources, and host integration.
- [ ] Rejected candidates leave the oracle selected and fail closed without semantic workarounds.
- [ ] Generated projections, package locks, runtime labels, and source checkout pins agree.
- [ ] `npm ls --all` in the Playground and reference CLI agrees with the selected descriptor graph.

Evidence table:

| Role | Package or source | Range | Version | Integrity | Commit | Decision and evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Host |  |  |  |  |  |  |
| Oracle |  |  |  |  |  |  |
| Latest-compatible candidate |  |  |  |  |  |  |
| Latest-stable delta |  |  |  |  |  |  |

## Supply Chain and Lifecycle

- [ ] Metadata and archives were acquired with lifecycle scripts disabled.
- [ ] Package name, version, integrity, provenance, and source commit were checked before use.
- [ ] Source checkouts live under ignored `repo-ref/` paths at pinned commits.
- [ ] Package scripts and their dependencies were inspected as untrusted input.
- [ ] Every executed lifecycle action is individually justified and allowlisted.
- [ ] Generated provenance came from repository tooling rather than manual hash edits.

## Delta Inventory

- [ ] Built-in diagram registrations and aliases were diffed.
- [ ] External diagram and layout registrations were diffed.
- [ ] Grammar, preprocessing, model, config, theme, sanitizer, URL, resource, and DOM changes were
      classified.
- [ ] Removed syntax and changed recovery behavior were classified.
- [ ] New and changed upstream fixtures were mapped to source provenance.
- [ ] Browser-dependent residuals have an explicit artifact contract.
- [ ] Every delta has an owner, admission state, and verification route.

## Capability Admission

For every admitted diagram or layout, close every applicable row. Mark a row not applicable only
with a reason.

| Surface | Evidence |
| --- | --- |
| Detection and aliases |  |
| Preprocessing and exact source mapping |  |
| Parser, recovery, and diagnostics |  |
| Family-owned semantic model and validation |  |
| Config, theme, security, and resource limits |  |
| Layout and render artifact contract |  |
| Editor lexemes and non-overlapping semantic tokens |  |
| LSP completion, hover, symbols, references, and rename |  |
| WASM bindings and typed runtime requirements |  |
| Monaco, VS Code, and Playground behavior |  |
| Reference CLI registration |  |
| Public ABI and editor/facts schema constraints |  |
| Positive, negative, recovery, resource, and security fixtures |  |
| Generated catalogs, status, and provenance |  |

Parser-only support does not close admission. External rich output must use its explicit sanitized,
sandboxed artifact path; it must not weaken the canonical inline-SVG policy.

## Feature Decision

- [ ] The behavior was first evaluated against existing semantic features and presets.
- [ ] Before/after `cargo tree` evidence covers affected packages and feature combinations.
- [ ] Native and WASM target support was verified.
- [ ] Dependency licenses and distribution obligations were reviewed.
- [ ] Clean-build timing was measured with an isolated target directory rather than deleting shared
      artifacts.
- [ ] Browser and Typst size matrices were compared with existing budgets.
- [ ] Public package surfaces and preset combinations were assessed.
- [ ] Browser-only lazy modules remain typed runtime capabilities unless Rust ownership proves a
      separate dependency boundary.
- [ ] The final record explicitly says either `no split` with reasons or names the justified feature
      and its ownership boundary.

## Verification Evidence

- [ ] Mermaid reference verification rejects stale source, lock, provenance, and generated output.
- [ ] New or removed registrations cannot disappear from parser/editor/render/Playground inventory.
- [ ] Focused family, parser, editor, LSP, Web, Playground, CLI, and security tests pass as applicable.
- [ ] Workspace `nextest`, formatting, clippy, strict verification, and alignment gates pass.
- [ ] Structure, parity, and parity-root comparisons pass without semantic whitelists.
- [ ] Web contracts, build, smoke, and browser tests use a fresh WASM artifact.
- [ ] Dependency and feature changes pass target and WASM size matrices.
- [ ] Documentation and generated status/provenance are readable and reproducible.
- [ ] Every skip is environment-only, named, and accompanied by the strongest available substitute.

## Forward Scenarios

Use these scenarios when changing the skill or alignment machinery. The workflow must produce a
closed decision for each without relying on a hardcoded release number:

1. Core-only patch with no companion delta.
2. Higher stable companion inside the declared range that passes the behavior matrix.
3. Higher compatible companion that fails one security or resource test and retains the oracle.
4. Latest companion major outside the plugin range.
5. New built-in diagram with parser and editor grammar.
6. New external rich-output diagram.
7. New layout module with a browser-only package.
8. Heavy Rust layout dependency that may justify a feature.
9. Removed syntax requiring recovery and editor changes.
10. Stale source checkout, wrong integrity, or stale generated projection.

## Handoff

- [ ] Selected and rejected graph decisions are summarized separately.
- [ ] Behavior ports and residual artifact contracts are named.
- [ ] Feature evidence and the split/no-split decision are recorded.
- [ ] Commands, results, and environment-only skips are listed.
- [ ] Commits are focused and authorized.
- [ ] Push, PR, publication, and release remain outside the handoff unless separately requested.
