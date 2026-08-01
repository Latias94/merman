---
name: align-mermaid-release
description: Align Merman to a selected Mermaid release and its companion behavior graph. Use when changing the pinned Mermaid baseline, evaluating compatible or latest companion packages, admitting added or removed diagrams and layouts, porting upstream parser/semantic/editor/LSP/render behavior, updating Playground or reference CLI integration, or deciding whether release dependencies justify a Cargo feature split.
---

# Align Mermaid Release

Treat a Mermaid release as a versioned behavior graph, not a package number. Preserve unrelated
work, keep generated projections derived from repository descriptors, and close every changed
capability across all Merman surfaces.

## Read the Authority

From the repository root, read:

- `docs/release/MERMAID_UPGRADE_PLAYBOOK.md`;
- `tools/upstreams/REPOS.lock.json` and `tools/upstreams/README.md`;
- `docs/FEATURES.md` and `docs/release/PACKAGE_SURFACES.md`;
- the generated Mermaid reference descriptor located by the repository `xtask` help and verifier;
- relevant family alignment notes and architecture decisions;
- [the admission checklist](references/admission-checklist.md) in full.

Record the selected Mermaid release, exact tag and commit, current working-tree state, available
reference checkouts, and requested delivery boundary before changing files. If no release was
selected, finish discovery and ask for that decision before mutating the reference graph.

Completion criterion: the selected source identity, current descriptor, dirty-tree ownership, and
delivery boundary are explicit.

## Establish the Reference Graph

Use the repository generator named by the reference descriptor and exposed by `xtask` help; do not
hand-edit generated projections or invent a second updater. Then run:

```bash
cargo run -p xtask -- verify-mermaid-reference --materialized
```

Resolve three facts for every Mermaid companion:

1. **Oracle**: the exact package, source commit, and integrity selected by the Mermaid workspace.
2. **Latest-compatible candidate**: the highest stable release satisfying the host plugin's
   published dependency range.
3. **Latest-stable delta**: a newer stable release outside that range, reported separately and
   never substituted implicitly.

Treat semver as candidacy, not behavioral proof. Select a latest-compatible candidate only after
its parser, renderer, security, resource, and host-integration matrix has no unexplained delta.
Retain the oracle and fail admission when the candidate cannot prove compatibility. Admit an
outside-range major only as separately scoped behavior-port work with its own evidence.

Completion criterion: the descriptor and locks name one reproducible selected graph, while every
candidate and outside-range release has an explicit admitted, rejected, or separately-scoped
decision.

## Materialize Sources Safely

Materialize Mermaid and behavior-owning companion sources under `repo-ref/` at descriptor-pinned
commits. Acquire registry metadata and tarballs with lifecycle scripts disabled. Before extracting
or executing package code, verify package identity, version, archive integrity, publish provenance,
and source commit against the descriptor and package locks.

Treat lifecycle scripts as untrusted input. If a source package genuinely requires an install or
build action for evidence, inspect the script and dependencies first, record why source inspection
or an existing repository harness is insufficient, and add only the audited action to an explicit
allowlist. Keep downloaded checkouts and caches out of commits.

Completion criterion: every selected package has verified source and tarball provenance, and no
unreviewed lifecycle action executed.

## Inventory the Delta

Derive the delta from pinned source and generated inventories. Account for every added, removed, or
changed:

- built-in diagram registration and alias;
- external diagram and layout module;
- grammar rule, preprocessing rule, semantic model, config, theme, security, resource limit, and
  DOM contract;
- source-backed fixture, example, generated catalog, package lock, runtime label, and provenance
  record.

Classify browser-dependent text measurement, `getBBox()`, `foreignObject`, font, and hand-drawn
effects as explicit artifact contracts. Keep comparator normalization narrow and non-semantic.
Never use a heuristic parser, magic constant, or broad whitelist to conceal a source behavior
delta.

Completion criterion: every upstream registration and behavior delta has one owner and one
admission disposition; no changed surface is merely absent from the inventory.

## Admit Every Capability

Implement each admitted delta through the family-owned path appropriate to its kind:

- **Built-in diagram**: detection, grammar/preprocessing, semantic construction, validation,
  config/theme, layout, SVG, resource limits, editor facts, LSP features, Web bindings, Playground,
  fixtures, and provenance.
- **External diagram**: typed runtime requirement, exact plugin graph, cold and reused loading,
  semantic/headless behavior that Merman must own, source-observed closed artifact type and strict
  validator, isolated execution when package code is untrusted, Playground and reference CLI
  registration, failure projection, and security tests. A validation failure may not infer a second
  artifact format; a genuinely new format requires separate admission before selection.
- **Layout module**: typed selection and registration, config propagation, deterministic fallback,
  resource enforcement, reference CLI and Playground integration, and source-backed layout parity.
- **Removed or changed syntax**: parser recovery, diagnostics, editor lexemes, semantic tokens,
  completions, hover, symbols, rename/reference behavior, fixtures, and migration documentation.

When a changed syntax path uses a checked-in LALRPOP grammar, treat the `.lalrpop` source and its
generated Rust parser as one atomic projection. Run `cargo run -p xtask -- gen-lalrpop-parsers`;
never hand-edit `crates/merman-core/src/generated/lalrpop/*.rs`; then prove freshness with
`cargo run -p xtask -- verify-lalrpop-parsers`.

Port observable dependency behavior needed by headless Merman; do not port implementation-only data
structures that add no behavior. Parser-only support is incomplete. Keep one parser-derived fact
stream for semantics, editor services, LSP, Monaco, and VS Code rather than adding a regex or
transport-specific grammar. Preserve the repository's declared public ABI and editor/facts schema
constraints unless a separate architecture decision explicitly changes them.

Completion criterion: each admitted capability has source-backed positive, recovery, resource,
security, and cross-surface evidence, and every exposed surface agrees on its support level.

## Decide Feature Boundaries

Prefer the existing semantic capability that already owns the behavior. Diagram count alone is not
a feature boundary, and a browser-only lazy companion is a typed runtime capability rather than an
automatic Cargo feature.

Before adding a feature, record before/after evidence for:

- `cargo tree` on affected packages and feature combinations;
- supported native and WASM targets;
- dependency licenses and distribution obligations;
- clean-build time and cache-independent dependency cost, measured with an isolated
  `CARGO_TARGET_DIR` rather than deleting shared artifacts;
- browser and Typst artifacts against `docs/release/WASM_SIZE_BUDGETS.json`;
- every public package surface and build preset affected by the split.

Add a feature only when a genuinely optional dependency has platform, license, clean-build, or
artifact-size cost that cannot fit an existing semantic capability. Record a no-split decision with
the same clarity when the evidence does not justify one.

Completion criterion: the feature decision is evidence-backed, all affected target combinations
compile, and package-surface documentation matches the resulting graph.

## Verify and Hand Off

Start with focused family/parser/editor/browser tests, then run the applicable strict gates:

```bash
cargo run -p xtask -- verify-mermaid-reference
cargo run -p xtask -- verify-playground-example-catalog
cargo run -p xtask -- verify-web-diagram-catalog
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify-lalrpop-parsers
cargo run -p xtask -- verify --strict
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The strict command owns release DOM comparisons in `structure`, `parity`, and `parity-root`, Web
contract/build/smoke/prepack, Playground unit/lint/build/browser, and VS Code package gates. Run
reference CLI, LSP, platform, and extension-host matrices whenever those surfaces changed. Run the
WASM size matrix and target build matrix whenever dependency or feature ownership changed. Use
`npm ls --all` in the Playground and reference CLI to prove that materialized companion graphs match
the descriptor. Use `cargo nextest` for Rust tests.

Update the upgrade playbook, relevant ADRs and family/editor records, package surfaces, Playground
design, generated status, and provenance. Report selected versus rejected companions, admitted
capabilities, residual artifact contracts, feature evidence, commands and results, and any
environment-only skips. Prepare focused Conventional Commits only when the task authorizes commits.
Treat push, PR creation, package publication, and release as separate authority.

Completion criterion: all checklist rows are closed with reproducible evidence, strict gates pass
without unexplained exceptions, generated files are clean, and the handoff contains no external
delivery action outside the request.

When changing this skill, run:

```bash
python3 .agents/skills/align-mermaid-release/scripts/validate_workflow.py
```

Also run the installed `skill-creator` `quick_validate.py` against this skill directory. The
workflow validator enforces the stable headings, repository references, verification commands,
handoff boundaries, and publication-command ban. Validate release-specific evidence by running the
repository commands above against the selected source graph; do not replace that evidence with a
synthetic fixture.
