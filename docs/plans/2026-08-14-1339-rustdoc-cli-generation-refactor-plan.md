---
title: "Rustdoc Dual-Path Integration Refactor - Plan"
date: 2026-08-14
type: refactor
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
deepened: 2026-08-14
---

# Rustdoc Dual-Path Integration Refactor - Plan

## Goal Capsule

**Objective:** Add a checked `merman-cli` generation workflow that removes Rustdoc rendering from dependency-sensitive consumers' Cargo graphs while retaining `merman-rustdoc` as an independent one-step integration.

**Authority order:** Product Requirements own observable behavior. Key Technical Decisions own implementation mechanisms. Implementation Units may refine neither without updating the owning ID.

**Execution profile:** Execute in goal mode from an isolated worktree based on current `origin/main`. Use incremental Conventional Commits at coherent unit boundaries. The executor owns implementation, migration, verification, review fixes, cleanup, and final commits.

**Stop conditions:** Do not declare completion while the generated-fragment consumer fixture depends on `merman-rustdoc` or Merman renderer crates, while generated artifacts can be stale without `rustdoc check` failing, while source Markdown or Mermaid files can be overwritten, while partial publication can escape after an error, while the retained proc macro is coupled to the CLI, or while abandoned WASM experiment code remains in the product diff.

## Product Contract

### Summary

Add `merman-cli rustdoc build` and `merman-cli rustdoc check` to generate committed, self-contained Markdown fragments with inline static SVG. Rust crates consume those fragments through Rust's built-in `include_str!`. Keep `merman-rustdoc` as a separate, explicit choice for authors who value one-step `cargo doc` over dependency cost; do not make either integration invoke, discover, or fall back to the other.

### Problem Frame

`merman-rustdoc` is a proc-macro crate that links the renderer into the host compilation graph. Its default `complete-svg` feature enables SVG, Cytoscape, ELK, and math. A feature gate around `cfg_attr(doc, ...)` controls macro expansion, but Cargo has already selected and built enabled dependencies by then. Current locked closure probes report 214 normal packages and 222 normal-plus-build packages for the default crate. The `svg`-only path still reports 142 and 149 packages.

The heavy graph is therefore a placement defect, not a default-feature tuning defect. Browser-rendered alternatives avoid the graph but give up Merman's distinguishing properties: deterministic static SVG, offline operation, build-time diagnostics, strict validation, fixed Mermaid semantics, and immediate light/dark switching without JavaScript.

The CLI already owns Merman's full native renderer, Markdown scanners, resource controls, and transactional publication. The new workflow should place the expensive renderer there once, keep Rustdoc consumption native to Rust, and preserve the existing static-output advantages.

### Key Decisions

- **Use checked CLI pre-generation as the zero-dependency Rustdoc path.** (session-settled: user-approved - it is the only route that removes the renderer and integration crate from that consumer's Cargo graph.) Governs R1-R14, R20.
- **Retain `merman-rustdoc` as an independent one-step path.** (session-settled: user-updated - chosen so users can trade Cargo cost for automatic `cargo doc`; retention means product choice, not a CLI subprocess or fallback.) Governs R14-R16.
- **Keep static, offline SVG as the product identity.** (session-settled: user-approved - chosen over browser Mermaid.js: browser execution would discard deterministic, no-JavaScript, and fail-early behavior.) Governs R4-R10.
- **Treat WASM as a bounded evidence task for a later `merman-rustdoc` optimization.** (session-settled: user-updated - this plan records whether a lightweight one-step path is viable but does not ship an unproven backend.) Governs R18.

### Actors

- A1. A crate author writes or updates external Rustdoc Markdown and runs the generator locally.
- A2. CI runs `rustdoc check` and rejects missing, stale, tampered, or unsafe generated artifacts.
- A3. Cargo and docs.rs consumers of the CLI path consume committed fragments without invoking Merman, a proc macro, a build script, a browser, or an external command.
- A4. A Merman release maintainer updates capability recipes, generated CLI assets, documentation, and package checks when the Rustdoc workflow changes.
- A5. A crate author may instead choose `merman-rustdoc` for one-step generation and knowingly accept its separately documented Cargo closure.

### Requirements

#### Consumer and command surface

- R1. A crate that consumes generated Rustdoc fragments must add no `merman-rustdoc`, `merman`, layout, math, proc-macro, or renderer package to its Cargo normal or build dependency graph.
- R2. The CLI must expose `merman-cli rustdoc build [--config PATH] [--quiet]` and `merman-cli rustdoc check [--config PATH] [--quiet]`; `PATH` defaults to `merman-rustdoc.toml` in the working directory.
- R3. Configuration schema 1 must declare stable fragment IDs and local `.md`, `.markdown`, `.mmd`, or `.mermaid` sources relative to the configuration root.
- R4. Generated fragments must be self-contained Markdown with inline static SVG and no JavaScript, network fetch, browser runtime, Node.js, Chromium, or remote include.

#### Rendering and determinism

- R5. Generation must use the pinned Mermaid semantic baseline, complete SVG capability, Cytoscape, ELK, math, readable SVG processing, a deterministic runtime, Rustdoc light/dark themes, and strict SVG validation.
- R6. Markdown inputs must preserve all non-Mermaid bytes while replacing Mermaid fences and supported local `include_mmd!` directives; raw Mermaid inputs must produce one diagram fragment.
- R7. Stable SVG and wrapper IDs must derive from the logical source path, Mermaid source digest, and same-source occurrence so unrelated fragments or preceding diagrams do not churn IDs.
- R8. Identical source, configuration, includes, and Merman version must produce byte-identical fragments and receipt data across repeated runs on supported hosts.

#### Safety and publication

- R9. The generator must reject absolute paths, root escapes, symlink escapes, source/output overlap, duplicate IDs, output collisions, remote includes, invalid UTF-8, malformed fences, invalid SVG XML, scripts, event attributes, and external SVG resources.
- R10. Resource limits must bound source bytes, diagram count, renderer work, staged output bytes, and scheduling weight through the existing CLI resource-policy model.
- R11. `build` must preflight and render every fragment before mutation, then publish the complete managed file set and a portable receipt with crash-recoverable all-or-recover transaction semantics; unchanged files must retain their modification time.
- R12. `check` must re-render and byte-compare the expected file set without writing; current state exits 0, content drift or render/sanitizer failure exits 1, usage or configuration errors exit 2, and I/O or transaction failures exit 3.
- R13. Only a valid previous Rustdoc receipt may authorize deletion of stale managed outputs; unknown files must never be deleted.

#### Migration and governance

- R14. The repository must migrate its Rustdoc examples to committed fragments consumed with `#[doc = include_str!(...)]` or `#![doc = include_str!(...)]`, and package tests must prove the fragments survive publication and offline docs.rs-shaped use.
- R15. Retain the `merman-rustdoc` package, tests, native renderer profile, and one-step attribute API as an explicitly separate integration; `merman-cli` must not depend on it, execute it, or inherit its native renderer feature recipe.
- R16. Supersede only the Rustdoc-specific single-path assumption of ADR-0076 while retaining its capability vocabulary and artifact-profile ownership for both independently distributed integrations.
- R17. The distributed CLI feature recipe, capabilities report, generated completions, man pages, README, and release validation must include the Rustdoc command and remain fresh.
- R18. A dedicated WASM spike must measure a full Rustdoc guest against pre-registered closure, package-size, parity, latency, memory, sandbox, and offline-package gates; the spike must not become a product backend in this plan and all experimental code must be removed after evidence is recorded.
- R19. Documentation must require each generated fragment to be included at most once on one rendered Rustdoc page because repeated inclusion would duplicate deterministic SVG DOM IDs.
- R20. `rustdoc build` and `rustdoc check` must never modify declared Markdown, Mermaid, configuration, or include inputs. Build may mutate only the fixed managed output root; source history and successful generated-output rollback remain version-control responsibilities.
- R21. The root, CLI, and `merman-rustdoc` documentation must cross-link both supported paths with runnable Cargo/Rust/CI examples and a decision table covering dependency closure, one-step ergonomics, generated-file ownership, docs.rs behavior, failure timing, and rollback.

### Success Criteria

- SC1. A clean consumer fixture builds and documents with generated fragments while `cargo tree --locked --edges normal,build` contains none of the packages prohibited by R1.
- SC2. Repeated `rustdoc build` runs are byte-identical and the second run leaves output modification times unchanged.
- SC3. Every stale, missing, extra-managed, and tampered fixture makes `rustdoc check` exit 1 without filesystem mutation.
- SC4. A packaged fixture documents successfully from unpacked, read-only source with Cargo offline, no CLI on `PATH`, and no network.
- SC5. The exact CLI distribution recipe passes 1, 10, and 100 diagram Rustdoc workloads without unbounded memory growth and within existing resource-policy limits.
- SC6. The CLI consumer path changes the attributable Rustdoc closure from the measured native macro baseline of 214 normal packages and 222 normal-plus-build packages to zero; the retained macro closure remains separately measured and is not represented as fixed by this plan.
- SC7. The WASM evidence records a reproducible verdict against pre-registered gates of host closure at or below 30 packages, final `.crate` projection below 8 MiB, full layout/math/theme parity, warm render no slower than 2x the native oracle, and bounded failure behavior; passing the gates is not required for this refactor, and any missed gate is recorded without retaining backend code.

### Key Flows

- F1. **Build:** discover and parse config -> validate all paths and ownership -> acquire all sources/includes -> scan diagrams -> render and validate all variants -> construct deterministic fragments and receipt -> acquire publication lock -> revalidate source generations -> publish changed files with crash-recoverable transaction semantics -> report counts.
- F2. **Check:** perform the same pure acquisition and render pipeline as F1 -> compare expected bytes and managed file set -> return 0 when current or 1 when stale -> perform no writes, lock recovery, or timestamp changes.
- F3. **Cargo/docs.rs:** package committed config, sources, and generated fragments -> expand native `include_str!` once per fragment per page -> render Rustdoc HTML -> execute no Merman-specific code.
- F4. **Adoption:** externalize selected macro-owned docs -> generate fragments -> replace those attributes with native includes -> remove the macro dependency only from that consumer -> run package and offline docs tests; other consumers may retain the one-step macro path.
- F5. **WASM evidence:** build an isolated full-capability guest and minimal host harness -> measure registered gates -> record reproducible results -> remove harness and artifacts from the product tree.

### Acceptance Examples

- AE1. Covers R2-R8. Given `architecture.md` with two Mermaid fences, `rustdoc build` writes `docs/generated/merman-rustdoc/architecture.md` with two inline light/dark diagrams while preserving surrounding Markdown bytes.
- AE2. Covers R6, R9. Given `include_mmd!("diagrams/model.mmd")`, the include resolves relative to the config root; `../outside.mmd`, an absolute path, a symlink escape, or an HTTP URL fails before publication.
- AE3. Covers R11-R13. Given a valid prior receipt with fragments A and B, then config removes B, `build` transactionally updates the receipt and removes only managed B; after success or recovery the bundle is entirely old or entirely new, and an unrelated file beside B remains untouched.
- AE4. Covers R12. Given a modified generated SVG with unchanged source hashes, `check` re-renders, detects the byte mismatch, exits 1, and leaves the modified file unchanged.
- AE5. Covers R14. Given an unpacked crate with read-only source and no Merman binary, `cargo doc --offline --no-deps` succeeds because Rust only reads packaged fragments.
- AE6. Covers R15-R16. Given final workspace metadata and feature contracts, `merman-cli` and its Rustdoc feature have no dependency edge to `merman-rustdoc`, while the existing macro package and native artifact profile remain independently testable.
- AE7. Covers R20-R21. Given the same diagram, documentation shows both the attribute-macro and checked-generation forms; running CLI build changes only the managed output root, and the source remains byte-identical.

### Scope Boundaries

In scope:

- A first-class CLI Rustdoc generator and checker.
- Portable receipt and managed transactional publication.
- External Markdown/raw Mermaid input, local includes, inline dual-theme SVG, native Rust includes, migration, packaging, and release integration.
- Coexistence with the existing proc macro without a Cargo, runtime, fallback, or release-recipe coupling to the CLI path.
- A temporary, measured WASM feasibility experiment for a later macro optimization, with mandatory cleanup.

Out of scope:

- Browser-side Mermaid.js, CDN loading, Rustdoc HTML post-processing, `build.rs` generation, proc-macro subprocess discovery, or automatic backend fallback.
- Parsing arbitrary Rust source to discover doc comments or rewriting Rust source automatically.
- A persistent render cache, daemon, public Rust library API for Rustdoc generation, or a published WASM Rustdoc backend.
- Rewriting the retained proc macro to WASM or changing its existing attribute and native feature contract in this plan.

### Dependencies and Sources

- `docs/research/merman-cli-rustdoc-architecture-2026-08-14.md` owns the CLI-first architecture and transaction analysis.
- `docs/research/rustdoc-mermaid-ecosystem-2026-08-14.md` owns ecosystem, dependency-closure, docs.rs, and WASM evidence.
- `crates/merman-cli/src/markdown.rs`, `render/execute.rs`, `batch.rs`, and `transaction.rs` provide the current scanner, in-memory renderer boundary, batch orchestration, and publication machinery.
- `crates/merman-rustdoc/src/html.rs`, `svg.rs`, and `render.rs` provide behavior to align through shared fixtures while remaining an independent integration.
- `docs/adr/0076-capability-driven-feature-and-package-surfaces.md` contains the Rustdoc policy that R16 supersedes.
- [The Rust Reference `doc` attribute](https://doc.rust-lang.org/reference/attributes.html#the-doc-attribute) defines native `include_str!` consumption.
- [docs.rs build documentation](https://docs.rs/about/builds) defines the offline and mostly read-only hosted build boundary.
- [Cargo unstable features](https://doc.rust-lang.org/cargo/reference/unstable.html) confirms binary artifact dependencies are not a stable default delivery mechanism.

## Planning Contract

### Key Technical Decisions

- KTD1. Implement Rustdoc generation as a private deep `rustdoc` module in `merman-cli`, not as a new public library crate and not as another `BatchDialect`. This reuses real internals while keeping Rustdoc's inline-fragment and portable-receipt contract local. Governs R2-R13.
- KTD2. Add a positive `rustdoc` CLI Cargo feature that implies `markdown`, `svg`, both layout engines, math, TOML parsing, hashing, and XML validation; include it in the default, capability descriptor, `capabilities --json`, artifact profile, and cargo-dist recipes. The feature owns a real command surface under ADR-0076 rules. Governs R2, R5, R17.
- KTD3. Use `merman-rustdoc.toml` schema 1 with `[[fragments]] id`, `source`, and optional `source_display = "hide" | "details"`. Output paths are fixed at `docs/generated/merman-rustdoc/<id>.md`; configuration cannot redirect writes outside the managed root. Governs R3, R9, R13.
- KTD4. Keep CLI rendering policy tool-owned: deterministic environment, `readable` semantics, Rustdoc light/dark theme pair, complete layout/math capability, strict validation, and failure-as-error. Source-level Mermaid configuration remains available, but the macro policy matrix does not move into TOML. Governs R4-R6, R10, R15.
- KTD5. Extend the Markdown scanner with a replacement-neutral span API and recognize `include_mmd!` only as a complete directive line outside code fences. Keep batch image rewriting and Rustdoc inline rewriting as separate adapters over those spans. Governs R6, R9.
- KTD6. Add an internal in-memory SVG result path to the existing prepared graphical renderer. Do not add a renderer trait while only the Merman adapter exists. Invocation-local deduplication keys on source plus effective render profile. Governs R5, R7, R8, R10.
- KTD7. Rebase every SVG DOM ID and local reference with a prefix derived from fragment logical path, source digest, and same-source occurrence. Validate the rebased XML again before embedding. Governs R7-R9.
- KTD8. Define a portable canonical JSON receipt distinct from the batch `GenerationManifest`. It records schema, generator/Merman/Mermaid versions, capability digest, normalized logical paths, source/include hashes, output hashes, and the exact managed file set using slash-separated UTF-8 relative paths. Governs R8, R11-R13.
- KTD9. Deepen the existing transaction module only at publication primitives shared by batch and Rustdoc. Rustdoc owns receipt semantics and managed-set reconciliation; the transaction module owns locks, generation checks, staging, journal recovery, atomic replacement, and deterministic commit order. Governs R11-R13.
- KTD10. Implement `check` by constructing the same expected bundle as `build` and comparing bytes. Do not trust receipt hashes as freshness evidence and do not acquire a mutating recovery path. Governs R8, R12.
- KTD11. Publish receipt last inside the same transaction, preserve unchanged files, and authorize stale deletion only after parsing and validating the previous receipt against the fixed managed root. A missing receipt is stale state, while a malformed, unsupported, or transaction-inconsistent receipt is an operational error and grants no deletion authority. Governs R11-R13.
- KTD12. Keep `merman-rustdoc` and `merman-cli rustdoc` as explicit peer products. Neither may depend on, spawn, discover, or automatically fall back to the other. Preserve the current native macro implementation in this plan; a later WASM rewrite must remove the native backend rather than add another consumer-selectable fallback. (session-settled: user-updated - chosen to expose both ergonomic and zero-dependency workflows.) Governs R1, R14-R16, R21.
- KTD13. Run the WASM spike in a temporary or non-product harness and record commands, revisions, raw data, and the gate verdict in the research report. Regardless of verdict, remove harness code and generated artifacts from the final product diff; productizing a lightweight macro backend requires a separate follow-up plan. Governs R18.
- KTD14. Add a focused ADR that supersedes ADR-0076 lines that couple Rustdoc to `complete-svg`; do not rewrite the historical ADR or weaken artifact-profile rules for other packages. Governs R16, R17.
- KTD15. Treat rendered SVG as untrusted active content. Reject document types, `script`, `iframe`, `object`, `embed`, event attributes, external resource-bearing references, CSS imports, and non-local CSS URLs; sanitize safe HTML-label subtrees and validate the complete XML again after ID rebasing. Governs R4, R9.
- KTD16. Resolve the config root canonically, approve every read and publication target through existing path and generation guards, and use same-file checks for aliases. `check` must return an operational error when an unfinished transaction journal exists instead of recovering or mutating it. Governs R9, R11-R13.
- KTD17. Render CLI Rustdoc diagrams sequentially in logical source order for the first release, even when `parallel-markdown` is compiled. Invocation-local deduplication may reuse completed bytes, but concurrency requires separate measured admission because each diagram produces two inline SVG variants. Governs R8, R10.
- KTD18. Treat declared inputs as immutable source-of-truth files. The pure bundle builder reads them, check compares without mutation, and build publishes only below the fixed managed root. A successful prior generated state is recovered through Git, while the crash journal restores only interrupted transactions. Governs R11-R13, R20.

### High-Level Technical Design

```text
merman-rustdoc.toml + docs/rustdoc-src/*
                    |
                    v
       merman-cli rustdoc build/check
       +-----------------------------+
       | config + path preflight     |
       | Markdown/include scanner    |
       | deterministic Merman render |
       | SVG rebase + validation     |
       | fragment + receipt builder  |
       | transactional publisher     |
       +-----------------------------+
                    |
                    v
docs/generated/merman-rustdoc/*.md + receipt.json
                    |
                    v
       #[doc = include_str!(...)]
                    |
                    v
           cargo doc / docs.rs
```

The pure bundle builder owns all fallible acquisition and rendering before publication. `build` and `check` differ only after they receive an expected bundle. This boundary makes stale checking exact and lets tests exercise config-to-bytes behavior without a subprocess.

### Sequencing

1. Characterize behavior and add the new command/config model without touching the old macro.
2. Build the pure fragment renderer and portable receipt.
3. Integrate transactional `build` and non-mutating `check`.
4. Migrate representative repository examples and add package/docs.rs-shaped tests while retaining macro coverage.
5. Document and govern both independent Rustdoc paths.
6. Run and clean up the WASM evidence task for a later macro optimization.
7. Regenerate distribution assets and run the complete quality tail.

### System-Wide Impact

- **Cargo and packaging:** The CLI gains a complete renderer-owned command while `merman-rustdoc` remains independently packaged. U6 verifies that their feature recipes and dependency edges do not merge.
- **CLI contract:** A nested command changes help, completions, man pages, capabilities JSON, exact feature recipes, and release archives. U1 establishes the callable contract and U8 refreshes every generated projection.
- **Documentation lifecycle:** Rustdoc source becomes a checked source/generated pair. CI must fail on drift before compiling docs, while docs.rs remains a pure consumer that cannot repair drift.
- **Filesystem lifecycle:** Build adds a second production caller to the transaction engine with a different ownership manifest. KTD9 keeps portable Rustdoc receipt rules out of the native crash journal and batch namespace.
- **Failure propagation:** Source, render, sanitizer, and resource failures occur before publication. Lock, generation, and commit failures enter the existing recovery protocol. Check detects both classes but never recovers state.
- **Security boundary:** Local authors control Markdown prose, while renderer SVG is treated as active untrusted output before raw HTML embedding. KTD15 owns the sanitizer boundary and KTD16 owns path race resistance.

### Assumptions

- A package commits generated fragments to version control and includes them in its Cargo package. This is an unvalidated workflow assumption accepted by proceeding without a separate scoping confirmation.
- Fragment IDs can be portable ASCII identifiers and can map to fixed managed filenames. This is an unvalidated interface assumption; implementation tests must reject names that are not portable across supported filesystems.
- One config root maps to one Cargo package. Workspace users can keep one config per package and invoke the command with `--config`. This is an unvalidated scale assumption.
- Generated Markdown may contain raw inline HTML because Rustdoc already accepts the current proc macro's HTML output. Package and Rustdoc end-to-end tests must verify this on the pinned Rust toolchain.

### Risks and Mitigations

- **Generated drift:** R12 requires full re-render comparison, and CI documentation must put `rustdoc check` before `cargo doc`.
- **Cross-platform receipt drift:** KTD8 excludes native path encoding, absolute paths, timestamps, random values, and host-specific separators.
- **Unsafe inline SVG:** KTD4 and KTD7 require validation both before and after ID rewriting.
- **CSS and HTML-label injection:** KTD15 covers style element text, namespaced attributes, and safe `foreignObject` descendants rather than validating attributes alone.
- **Source races during publication:** KTD9 reuses generation evidence and revalidates acquired inputs after locking.
- **Transaction over-generalization:** U4 may extract only primitives needed by two production callers; Rustdoc receipt semantics remain outside `transaction/format.rs`.
- **Large generated diffs:** KTD6 performs invocation-local deduplication, KTD7 limits ID churn, and KTD11 skips unchanged writes. Persistent caching remains out of scope.
- **Dual-path drift:** U6 cross-links both packages and keeps shared behavioral fixtures explicit without creating a Cargo dependency between the integrations.
- **WASM experiment scope expansion:** KTD13 makes cleanup part of completion and forbids backend admission in this plan.

## Implementation Units

### U1. Freeze the Rustdoc CLI contract and characterize migrated behavior

**Goal:** Establish the new command, config, and behavior test surface before moving implementation.

**Requirements:** R2-R10, R12.

**Dependencies:** None.

**Files:** `crates/merman-cli/Cargo.toml`, `crates/merman-cli/src/cli.rs`, `crates/merman-cli/src/invocation.rs`, `crates/merman-cli/src/app.rs`, `crates/merman-cli/src/output.rs`, `crates/merman-cli/src/commands.rs`, `crates/merman-cli/src/capabilities.rs`, `crates/merman-cli/src/main.rs`, `crates/merman-cli/src/error.rs`, `crates/merman-cli/tests/cli_contract.rs`, `crates/merman-cli/tests/profile_contract.rs`, `crates/merman-cli/tests/rustdoc_cli.rs`, `crates/merman-cli/tests/fixtures/rustdoc/**`.

**Approach:** Add the feature-gated command hierarchy and normalized invocation types through the existing parse, working-directory, preflight, acquisition-anchor, and command dispatch layers. Introduce schema-1 parsing and portable fragment-ID validation behind `crates/merman-cli/src/rustdoc/config.rs`. Bump the CLI contract version and port only behavior-bearing fixtures from `merman-rustdoc`; do not copy proc-macro AST tests.

**Test scenarios:** Default and explicit config path; build/check/quiet help; unsupported schema and unknown fields; empty fragment list or source; duplicate and non-portable IDs including case/Unicode aliases; missing source; unsupported extension; absolute, parent, and symlink escapes; source/output overlap; invalid UTF-8; stable error locations.

**Verification:** `cargo nextest run -p merman-cli --features rustdoc --test rustdoc_cli`; CLI help snapshots or contract assertions pass with and without `rustdoc`.

### U2. Build deterministic Rustdoc fragments in memory

**Goal:** Convert validated sources into complete, safe, byte-stable generated Markdown without filesystem mutation.

**Requirements:** R4-R10.

**Dependencies:** U1.

**Files:** `crates/merman-cli/src/rustdoc.rs`, `crates/merman-cli/src/rustdoc/document.rs`, `crates/merman-cli/src/rustdoc/html.rs`, `crates/merman-cli/src/markdown.rs`, `crates/merman-cli/src/render.rs`, `crates/merman-cli/src/render/execute.rs`, `crates/merman-cli/src/resources.rs`, `crates/merman-cli/tests/rustdoc_cli.rs`, `crates/merman-cli/tests/fixtures/rustdoc/**`.

**Approach:** Add a replacement-neutral Markdown chart API and local include scanner. Expose an internal method that consumes `ExecutedArtifact` as SVG bytes. Render light and dark variants with deterministic settings, rebase IDs, validate XML and resource references, wrap them with scoped Rustdoc CSS, and splice fragments by source spans. Preserve non-chart bytes and line endings.

**Test scenarios:** Markdown with zero, one, repeated, and multiple fences; tilde/backtick fences; nested/indented fences; unclosed Mermaid fences rejected instead of extending to EOF; CRLF; raw `.mmd`; standalone local includes; directive-like text inside fences and prose; light/dark and fixed source config; Cytoscape, ELK, and math fixtures; duplicate source occurrence IDs; unrelated-fence insertion without global churn; complete rebasing of `href`, `xlink:href`, `aria-labelledby`, `aria-describedby`, SMIL timing, CSS selectors, and `url(#...)` references; document type, script, event, embedding element, namespaced href, style text, CSS import, and CSS URL rejection; safe HTML labels; resource limits; render diagnostics with source line and column.

**Verification:** Focused unit tests plus `cargo nextest run -p merman-cli --features rustdoc --test markdown_scanner --test rustdoc_cli`; golden files are byte-stable across two runs.

### U3. Define portable receipt and exact check semantics

**Goal:** Make generated state auditable and stale detection independent from host paths or optimistic hashes.

**Requirements:** R8, R11-R13.

**Dependencies:** U2.

**Files:** `crates/merman-cli/src/rustdoc/receipt.rs`, `crates/merman-cli/src/rustdoc.rs`, `crates/merman-cli/src/error.rs`, `crates/merman-cli/tests/rustdoc_cli.rs`.

**Approach:** Serialize canonical receipt JSON with sorted records and newline termination. Build an `ExpectedRustdocBundle` containing every path and byte vector. Compare the complete expected bundle to disk for `check`, including exact managed-set reconciliation and output hashes. Keep transaction journal formats private and separate.

**Test scenarios:** Missing receipt exits 1; unsupported, malformed, oversized, or transaction-inconsistent receipt exits 3; invalid configuration exits 2; source, include, config, output, capability, or version change; tampered output with matching source; missing output; extra previously managed output; ignored unknown neighboring file; Windows-style or non-normalized path in receipt; unfinished journal; read-only check; zero filesystem writes and unchanged mtimes.

**Verification:** `cargo nextest run -p merman-cli --features rustdoc --test rustdoc_cli check`; filesystem snapshot assertions prove check is non-mutating and returns exactly 0, 1, 2, or 3 per R12.

### U4. Publish Rustdoc bundles transactionally

**Goal:** Transactionally update changed fragments, stale managed outputs, and receipt with crash-recoverable all-or-recover semantics.

**Requirements:** R11-R13.

**Dependencies:** U3.

**Files:** `crates/merman-cli/src/rustdoc/publish.rs`, `crates/merman-cli/src/transaction.rs`, `crates/merman-cli/src/transaction/format.rs`, `crates/merman-cli/src/transaction/tests.rs`, `crates/merman-cli/src/output.rs`, `crates/merman-cli/src/commands.rs`, `crates/merman-cli/tests/rustdoc_cli.rs`.

**Approach:** Add generic staged write/delete targets only where batch and Rustdoc both need them. Stage the full expected bundle, preserve identical targets, validate prior receipt ownership before stale deletion, recheck acquired source generations after the lock, and commit receipt last. Keep crash journal encoding native because it is ephemeral; keep committed receipt encoding portable.

**Test scenarios:** First build; no-op rebuild and mtime preservation; one changed fragment; stale removal; malformed prior receipt; unknown files; concurrent source change; concurrent build; lock contention; injected failure before and during commit; interrupted transaction recovery; target swap and symlink race; case/Unicode collision; receipt-last ordering.

**Verification:** `cargo nextest run -p merman-cli --features rustdoc transaction rustdoc`; existing batch transaction tests remain unchanged and green.

### U5. Add a zero-dependency Rustdoc consumer and prove package use

**Goal:** Make a representative repository surface consume generated fragments while preserving the proc macro as a separately tested alternative.

**Requirements:** R1, R14, R19.

**Dependencies:** U4.

**Files:** `crates/merman/merman-rustdoc.toml`, `crates/merman/docs/rustdoc-src/**`, `crates/merman/docs/generated/merman-rustdoc/**`, `crates/merman/src/lib.rs`, `crates/merman/Cargo.toml`, `crates/merman-cli/tests/rustdoc_package.rs`, `.github/workflows/ci.yml`.

**Approach:** Externalize representative item/crate documentation. Generate and commit fragments. Replace the selected consumer's macro attributes and docs-only dependency with native `include_str!`, while leaving dedicated `merman-rustdoc` examples and tests intact. Add a fixture crate whose package contents are unpacked, made read-only, and documented offline without the CLI in `PATH`. Snapshot declared sources before build and prove only the managed output root changes.

**Test scenarios:** Crate-level and item-level docs; package include list; missing generated artifact; offline read-only source; `cargo doc --no-deps`; consumer dependency tree absence; generated raw HTML visible in Rustdoc; CI check-before-doc ordering.

**Verification:** `merman-cli rustdoc check --config crates/merman/merman-rustdoc.toml`; `cargo package -p merman --list --allow-dirty`; packaged fixture `cargo doc --offline --no-deps`; exact `cargo tree --locked --edges normal,build` absence assertions.

### U6. Govern and document the independent Rustdoc paths

**Goal:** Make the zero-dependency CLI workflow and one-step proc-macro workflow explicit, independently packaged choices with honest trade-offs.

**Requirements:** R15-R17, R20-R21.

**Dependencies:** U5.

**Files:** root `README.md`, `crates/merman-cli/README.md`, `crates/merman-rustdoc/README.md`, `capabilities/artifact-profiles-v1.json`, `capabilities/feature-surface-v1.json`, `capabilities/generated/feature-surface-v1.md`, `crates/merman-cli/src/generated/capability_surface.rs`, `crates/xtask/src/cmd/artifact_profiles.rs`, `crates/xtask/src/cmd/feature_matrix.rs`, `crates/merman-cli/tests/cli_contract.rs`, `crates/merman-cli/tests/profile_contract.rs`, `.github/workflows/ci.yml`, `docs/FEATURES.md`, `docs/release/PACKAGE_SURFACES.md`, `docs/adr/0076-capability-driven-feature-and-package-surfaces.md`, new `docs/adr/0082-rustdoc-dual-path-integrations.md`, `CHANGELOG.md`, and relevant release docs.

**Approach:** Keep the existing macro package, `rustdoc-static-svg` recipe, tests, and CI lane. Add a focused ADR that supersedes ADR-0076's single-path assumption and establishes independent ownership. Document both complete workflows and cross-link them from the root, CLI, and macro READMEs. Include equivalent diagram examples, a trade-off table, source immutability/rollback semantics, CI freshness guidance, and a non-breaking adoption mapping from macro attributes to external Markdown and native includes.

**Test scenarios:** Workspace metadata retains both packages; `merman-cli` has no dependency on `merman-rustdoc`; artifact-profile and feature-matrix validators accept the new CLI feature while preserving the macro recipe; each README links the other path and shows runnable examples; published CLI assets expose the command; changelog describes the additive workflow and measured trade-offs without claiming the native macro closure was removed.

**Verification:** `cargo tree -p merman-cli --all-features --edges normal,build` contains no `merman-rustdoc`; `cargo run -p xtask -- verify-feature-matrix --strict`; `cargo run -p xtask -- verify-artifact-profiles`; focused macro tests plus workspace nextest and doc checks.

### U7. Run the bounded WASM Rustdoc feasibility spike and remove it

**Goal:** Produce reproducible evidence for a later lightweight `merman-rustdoc` rewrite without adding an unproven backend now.

**Requirements:** R18.

**Dependencies:** U2 for parity corpus; independent of U3-U6 for code.

**Files:** temporary worktree or temporary directory outside tracked product paths; `docs/research/rustdoc-mermaid-ecosystem-2026-08-14.md`; existing WASM measurement scripts only when they already fit the task.

**Approach:** Build a dedicated full-capability `wasm32-unknown-unknown` guest and minimal `wasmi` host harness. Reuse transport concepts, not the Typst policy artifact. Register gates before measurement, run native/WASM differential fixtures and 1/10/100 workloads, test forbidden imports/trap/fuel/memory/output limits and offline package shape, record raw commands and results, then remove all harness code and binary artifacts.

**Test scenarios:** Flowchart, sequence, architecture/Cytoscape, ELK, math, light/dark, malformed source, ABI mismatch, forbidden import, trap, fuel exhaustion, memory/output limit, repeated clean artifact hash, projected `.crate` contents.

**Verification:** The research report contains a pass/fail table for every SC7 gate and reproduction commands. `git status` and repository search prove no spike crate, guest, embedded artifact, fallback feature, or generated binary remains.

### U8. Regenerate distribution assets and complete the quality tail

**Goal:** Ship one coherent CLI-first surface with complete documentation and no experimental residue.

**Requirements:** R1-R21.

**Dependencies:** U1-U7.

**Files:** `crates/merman-cli/assets/completions/**`, `crates/merman-cli/assets/man/**`, generated capability projections, release evidence files required by existing commands, all files changed by prior units.

**Approach:** Regenerate command-owned assets with `scripts/generate_cli_assets.py` and verify them with `scripts/verify_cli_assets.py`. Format Rust, run focused and broad tests, inspect package contents and dependency closures, run security and deterministic-output fixtures, then perform correctness, maintainability, testing, project-standards, and simplicity review passes. Apply high-confidence findings and rerun affected gates.

**Test scenarios:** Minimal CLI build without `rustdoc`; exact distribution CLI with `rustdoc`; shell completions and man page presence; broken pipe and quiet behavior; package/install smoke; Linux-reference closure verifier; no generated drift; no dead code or obsolete docs.

**Verification:** The complete Verification Contract passes from a clean-enough worktree, `git diff --check` is clean, and review findings have no unresolved blocker.

## Verification Contract

Run focused commands after their owning unit and avoid concurrent Cargo builds on the shared machine. Reuse the worktree `target` directory unless a package/offline test requires isolation.

### Formatting and static checks

```text
cargo fmt --all -- --check
cargo check -p merman-cli --no-default-features --features rustdoc
cargo check -p merman-cli --all-features
cargo clippy -p merman-cli --all-features --all-targets -- -D warnings
git diff --check
```

### Focused behavior

```text
cargo nextest run -p merman-cli --features rustdoc --test rustdoc_cli
cargo nextest run -p merman-cli --features rustdoc --test rustdoc_package
cargo nextest run -p merman-cli --features rustdoc transaction
cargo nextest run -p merman-cli --features rustdoc --test markdown_scanner
```

### Existing CLI and workspace regression

```text
cargo nextest run -p merman-cli --all-features
cargo nextest run --workspace --all-features
cargo test --doc --workspace --all-features
```

If the full workspace all-features command exceeds available memory, run the repository's established feature-matrix shards sequentially and record the substitution. Do not silently omit the broad regression gate.

### Architecture, generated assets, and release checks

```text
cargo run -p xtask -- verify-feature-matrix --strict
cargo run -p xtask -- verify-artifact-profiles
cargo run -p xtask -- verify --strict
cargo package -p merman-cli --allow-dirty
cargo package -p merman --list --allow-dirty
```

Use the repository's canonical asset-generation command discovered during U8, then verify a second invocation produces no diff.

### Consumer closure and hosted-doc shape

```text
cargo tree --locked --manifest-path <fixture>/Cargo.toml --edges normal,build
cargo doc --locked --offline --no-deps --manifest-path <unpacked-fixture>/Cargo.toml
```

The tree output must satisfy R1. The unpacked fixture must be read-only, have no CLI in `PATH`, and use no network. Record the pre-refactor 214/222 default package counts and the final zero-attributable result in the research report or release evidence.

### Determinism, safety, and performance

- Run `rustdoc build` twice and compare every byte plus second-run mtimes.
- Run `rustdoc check` against stale, tampered, missing, extra-managed, and unsafe fixtures; snapshot the tree before and after each check.
- Measure 1, 10, and 100 diagrams with the same machine, revision, Cargo cache state, command, resource profile, and concurrency. Record wall time and peak RSS.
- Run the U7 WASM matrix before deleting its harness. Preserve only the report and textual measurements.

### Review tail

- Correctness review: expected-bundle comparison, stale ownership, source immutability, source races, ID rewriting, and exit codes.
- Security review: path containment, symlink races, receipt trust, raw HTML, SVG URLs, and resource amplification.
- Maintainability review: private module depth, no fake renderer/filesystem traits, no duplicated batch dialect, no transaction semantics leak, and no CLI/macro dependency or fallback coupling.
- Testing review: assertions cover failures and mutation absence rather than only successful output snapshots.
- Project-standards review: capability descriptors, ADR supersession, generated assets, package contents, release commands, and dual-path documentation agree.
- Simplicity review: remove compatibility shims, unused policy knobs, experiment code, and premature persistent caching.

## Definition of Done

### Global

- [ ] R1-R21 and SC1-SC7 are satisfied with recorded evidence.
- [ ] `merman-cli rustdoc build/check` and `merman-rustdoc` are active, independent paths with no implicit invocation or fallback.
- [ ] CLI-path Rustdoc consumers use committed fragments through native `include_str!` and add zero attributable Cargo packages.
- [ ] Build/check publication is deterministic, bounded, strict, transactional, and covered for unhappy paths.
- [ ] Declared source/config/include files remain byte-identical across build and check; only the managed output root is mutable.
- [ ] The retained proc-macro crate, native renderer profile, tests, and CI lane remain independently green and are not dependencies of the CLI path.
- [ ] ADR, README, changelog, man pages, completions, capability projections, and package contents describe the same final surface.
- [ ] The WASM experiment has a reproducible report and no retained product code or binary artifact.
- [ ] All Verification Contract gates pass or an explicitly documented platform substitution provides equivalent evidence.
- [ ] No unresolved blocker remains from correctness, security, maintainability, testing, standards, or simplicity review.
- [ ] The final diff contains no abandoned attempt, dead compatibility layer, unrelated user file, generated drift, or trailing whitespace.

### Per Unit

- [ ] U1 command/config contracts and failure diagnostics are executable.
- [ ] U2 produces safe byte-stable fragments for the full representative corpus.
- [ ] U3 detects every defined stale state without mutation.
- [ ] U4 preserves transaction recovery, ownership, and unchanged mtimes.
- [ ] U5 proves native package and offline Rustdoc consumption without mutating declared sources.
- [ ] U6 proves independent package/governance edges and documents both workflows and trade-offs.
- [ ] U7 records every gate and removes every experiment artifact.
- [ ] U8 regenerates distribution surfaces and completes the full quality tail.

## Appendix

### Optional CLI Adoption Mapping

The macro API remains supported. Authors who choose the zero-dependency CLI path can translate it as follows:

| Macro-path surface | CLI-path equivalent |
| --- | --- |
| `#[cfg_attr(doc, merman_rustdoc::merman)]` | `#[doc = include_str!("../docs/generated/merman-rustdoc/<id>.md")]` |
| Mermaid fences inside Rust doc comments | Mermaid fences in declared external Markdown |
| `include_mmd!` inside doc comments | Local `include_mmd!` inside external Markdown, resolved from config root |
| `scope = "tree"` | One explicit generated fragment per documented item or crate |
| `source = "details"` | Fragment `source_display = "details"` or an authored `<details>` block |
| `pipeline` | Fixed tool-owned `readable` policy |
| `theme` | Fixed Rustdoc light/dark output; source-level Mermaid config remains available |
| `sanitize = "off"` | CLI path always uses strict validation |
| `fail = "keep-source"` | CLI generation fails and CI reports the source location |
| `complete-svg`, `svg`, `layout-*`, `math` on `merman-rustdoc` | CLI binary owns complete render capabilities outside the consumer Cargo graph |

### Receipt Ownership Rules

The fixed managed root contains fragment Markdown files and one receipt. A receipt can name only normalized direct managed paths produced from validated fragment IDs. The generator never follows a receipt path outside that root. A missing, malformed, unsupported, or internally inconsistent receipt grants no deletion authority. Build may replace known requested targets after normal publication approval, but it may delete stale targets only from a valid prior receipt. Check reports differences and never repairs them. Neither command writes declared source, include, or configuration paths. Interrupted builds recover through the transaction journal; successful historical output rollback uses Git because the source-of-truth inputs remain intact.
