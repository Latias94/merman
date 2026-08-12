# Mermaid Release Admission Checklist

Read this checklist completely before changing the Mermaid reference descriptor, dependency locks,
or family code. Use it as an evidence index; link durable repository artifacts instead of copying
large logs into the checklist.

## Reference Graph

- [ ] Selected Mermaid tag resolves to an exact source commit.
- [ ] Registry package version, tarball integrity, publish provenance, and source identity agree.
- [ ] The standing bundle contains one selected package/source/runtime graph and no candidate,
      oracle, deferred-major, browser-result, or attestation payload.
- [ ] The selected bundle binds one decision receipt by path and SHA-256.
- [ ] Each plugin's declared dependency range is read from the selected published artifact.
- [ ] The highest stable version inside that range is evaluated as a candidate.
- [ ] A newer stable version outside the range is reported as a separate behavior delta.
- [ ] Candidate admission covers parser, renderer, security, resources, and host integration.
- [ ] Rejected candidates leave the selected graph unchanged and fail closed without semantic
      workarounds or committed deferred placeholders.
- [ ] Generated projections, package locks, runtime labels, and source checkout pins agree.
- [ ] `npm ls --all` in the Playground and reference CLI agrees with the selected descriptor graph.
- [ ] `verify-mermaid-reference --base <trusted-base-sha>` proves the receipt's previous/current
      identities and changed fields match the trusted Git transition.

Evidence table:

| Role | Package or source | Range | Version | Integrity | Commit | Decision and evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Host |  |  |  |  |  |  |
| Oracle |  |  |  |  |  |  |
| Latest-compatible candidate |  |  |  |  |  |  |
| Latest-stable delta |  |  |  |  |  |  |

## Supply Chain and Lifecycle

- [ ] Metadata and archives were acquired in temporary admission state with lifecycle scripts
      disabled.
- [ ] Package name, version, integrity, provenance, and source commit were checked before use by
      official package-manager/Sigstore-capable tooling.
- [ ] Exact Node/npm versions and
      `npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/`
      are recorded with the raw output digest and exit result.
- [ ] Repository Rust code does not parse DSSE, in-toto, SLSA, certificates, or transparency logs.
- [ ] Source checkouts live under ignored `repo-ref/` paths at pinned commits.
- [ ] Package scripts and their dependencies were inspected as untrusted input.
- [ ] Every executed lifecycle action is individually justified and allowlisted.
- [ ] Generated selected-reference projections came from repository tooling rather than manual hash
      edits; candidate reports remain workflow artifacts until a reviewed selection decision.

## Delta Inventory

- [ ] Built-in diagram registrations and aliases were diffed.
- [ ] External diagram and layout registrations were diffed.
- [ ] Grammar, preprocessing, model, config, theme, artifact validation, URL, resource, and DOM
      changes were classified.
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

Parser-only support does not close admission. External output must match a source-observed closed
artifact type and pass its strict validator. A family name or validation failure cannot select an
alternate format. A genuinely new format needs separate validation, presentation, resource, and
security admission; it must not weaken the canonical inline-SVG policy.

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

- [ ] Mermaid reference verification rejects stale selected source, lock, receipt, installed bytes,
      and generated output while still passing when candidate/deferred files are absent.
- [ ] A selected identity change without an exact base-bound receipt fails; an unchanged identity
      cannot replace its receipt except for the explicit initial bootstrap.
- [ ] Bootstrap historical evidence is an ancestor of the trusted base and its Git object digest
      matches the receipt.
- [ ] Official signature-tool nonzero exit fails admission explicitly without interpreting protocol
      internals locally.
- [ ] Every changed LALRPOP grammar was regenerated through `xtask`; checked-in parser output was
      not hand-edited and `verify-lalrpop-parsers` passes.
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
2. Higher stable companion inside the declared range that passes the behavior matrix and produces a
   new base-bound selection receipt.
3. Higher compatible companion that fails one signature, security, or resource test and leaves the
   selected graph and receipt unchanged.
4. Latest companion major outside the plugin range.
5. New built-in diagram with parser and editor grammar.
6. New external diagram whose observed output requires a previously unsupported artifact format.
7. New layout module with a browser-only package.
8. Heavy Rust layout dependency that may justify a feature.
9. Removed syntax requiring recovery and editor changes.
10. Stale selected source checkout, wrong integrity, stale receipt, base mismatch, or stale generated
    projection while no candidate artifacts exist in the repository.

## Handoff

- [ ] Selected and rejected graph decisions are summarized separately.
- [ ] Admission workflow reports and raw-output digests are named without committing completed
      candidate/deferred/attestation artifacts as standing inputs.
- [ ] Behavior ports and residual artifact contracts are named.
- [ ] Feature evidence and the split/no-split decision are recorded.
- [ ] Commands, results, and environment-only skips are listed.
- [ ] Commits are focused and authorized.
- [ ] Push, PR, publication, and release remain outside the handoff unless separately requested.
