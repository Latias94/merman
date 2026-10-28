# Fuzzing

Merman uses `cargo-fuzz` and libFuzzer for panic and sanitizer coverage across the parser, the
headless renderer, and the `resvg-safe` SVG pipeline.

The fuzz project is intentionally an independent Cargo workspace under `fuzz/`. Keep its
`Cargo.lock` committed and separate from the root workspace so nightly-only fuzz dependencies do
not change the stable public crate lockfile.

## Toolchain

Use the same versions as CI unless a local investigation needs a newer nightly:

```sh
rustup toolchain install nightly-2026-07-01 --component rust-src
cargo install cargo-fuzz --version 0.13.2 --locked
```

The repository root stays on stable Rust. Invoke `cargo-fuzz` with the nightly toolchain explicitly.

## Targets

| Target | Surface | Seed corpus | Dictionary |
| --- | --- | --- | --- |
| `parse_mermaid` | Semantic JSON, typed render models, editor snapshots, and lenient recovery through detected and known-type entrypoints | `fuzz/seeds/mermaid` | `fuzz/dictionaries/mermaid.dict` |
| `render_mermaid` | Strict parse, layout, SVG render, and `resvg-safe` output | `fuzz/seeds/mermaid` | `fuzz/dictionaries/mermaid.dict` |
| `svg_pipeline` | Raw XML SVG through `SvgPipeline::resvg_safe()` | `fuzz/seeds/svg` | `fuzz/dictionaries/svg.dict` |
| `ffi_api` | ABI 3 discovery, generic collect operations, result ownership, engine/request option paths, reusable engine calls, and host text-measure callbacks | `fuzz/seeds/ffi` | `fuzz/dictionaries/mermaid.dict` |
| `tree_sitter_mermaid_parse` | Arbitrary-byte Tree-sitter fresh parsing, repeat determinism, and bounded CST spans | `distribution/tree-sitter-mermaid/fuzz/corpus/all-families` | `fuzz/dictionaries/mermaid.dict` |
| `tree_sitter_mermaid_edits` | Bounded byte edits, tree geometry, and restored CST equivalence after inverse edits | `fuzz/seeds/tree-sitter-edits` | `fuzz/dictionaries/mermaid.dict` |
| `tree_sitter_mermaid_scanner` | External scanner state canonicalization plus arbitrary valid-symbol masks and row scans | `fuzz/seeds/tree-sitter-scanner` | `fuzz/dictionaries/mermaid.dict` |

`ffi_api` keeps the text seeds above readable, but random inputs use a small binary frame so
options, document URI, and source bytes can evolve independently:

```text
selector options_len options_bytes [uri_len uri_bytes] source_bytes
```

The optional URI field is present when the selector's high bit is set. Otherwise the harness uses a
fixed default URI and treats the remaining bytes as Mermaid source.

## Local Smoke

`parse_mermaid` compares the semantic-JSON and typed-render parse paths using the same fixed
runtime/configuration, in both strict and paired lenient modes. It checks result categories,
diagram identity, title, authored/effective configuration and the complete lossless managed
semantic projection. Detected family identities also exercise semantic/typed known-type parity
independently in each mode, retaining the original family when suppression produces `error`.
Each input additionally exercises one catalog-selected known-type entrypoint, so malformed headers
and empty sources reach family parsers independently of type detection.
Editor snapshots must retain parser-backed facts with source-bounded, UTF-8-aligned spans. Their
preprocessing uses the same `RecoverLine` policy as lenient models: parsed snapshots must match
their complete metadata/model, and retained family failures must match suppressed error metadata
and messages. Without an incomplete-directive recovery receipt, snapshots also match strict
parsing. Recovery never bypasses these snapshot checks; parser panics always fail. A semantic
failure can still carry recovered symbols, diagnostics, and completion context. The fuzz package's
default `editor-facts` feature forwards `merman/editor-facts` and enables these shared snapshot
assertions; `--no-default-features` omits them.
Strict and lenient results are not universally equal: strict preprocessing removes an unfinished
directive through EOF, while `RecoverLine` retains later statements. Explicit policy regressions
cover both a recovered family error and two accepted models with different semantics, alongside
typed-path acceptance/projection negative controls.
Acceptance disagreement fails even when one side returns no diagram.
Catalog-derived family/alias checks and extension, recovery and cancellation boundaries live in
`crates/merman-core/tests/parse_projection_contract.rs` and related core parse tests; custom
registry contracts are not inferred from built-in model kinds.

`tree_sitter_mermaid_edits` checks byte/point consistency, parent containment, and sibling ordering
for every node after each incremental edit, including every error subtree. Edited intermediates
can have different CSTs from fresh parses: tolerant recovery nodes can represent malformed input
even when `has_error()` is false. Cross-tree node ranges are not an invariant either; a reused
recovery token can retain leading extras after an edit makes the document valid. Each edit records
its inverse. After undoing all edits, the restored source must equal the original bytes, and the
restored tree must have consistent geometry. If the initial tree has no errors, its restored CST
must also match the original fresh tree in node kinds, fields, flags, and structure. This follows
the intermediate-geometry/restored-output strategy in the fixed Tree-sitter 0.27.1 fuzz source
(`crates/cli/src/fuzz.rs` and `crates/cli/src/fuzz/corpus_test.rs`); it does not replicate upstream's
scope-based `changed_ranges` checker. Focused grammar/editor regressions continue to assert source
ranges, and the separate parse target covers fresh-parse determinism.

Stable regressions for the shared SVG oracle and incremental-edit implementation can run without
building the instrumented libFuzzer executables:

```sh
cargo nextest run --locked --manifest-path fuzz/Cargo.toml --lib
```

Generated layout assertions in `flowchart_layout_test` and `swimlane_layout_test` use fixed seeds
and their explicitly supported Dagre/Swimlane graph subsets. They exclude self/ancestor edges from
generic separation assertions; they are not universal geometric invariants for every backend.

Run a fast smoke before changing fuzz harnesses:

```sh
cargo +nightly-2026-07-01 check --manifest-path fuzz/Cargo.toml --locked
mkdir -p fuzz/corpus/parse_mermaid fuzz/corpus/render_mermaid fuzz/corpus/svg_pipeline fuzz/corpus/ffi_api fuzz/corpus/tree_sitter_mermaid_parse fuzz/corpus/tree_sitter_mermaid_edits fuzz/corpus/tree_sitter_mermaid_scanner
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address parse_mermaid fuzz/corpus/parse_mermaid fuzz/seeds/mermaid -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address render_mermaid fuzz/corpus/render_mermaid fuzz/seeds/mermaid -- -runs=64 -timeout=10 -max_len=32768 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address svg_pipeline fuzz/corpus/svg_pipeline fuzz/seeds/svg -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/svg.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address ffi_api fuzz/corpus/ffi_api fuzz/seeds/ffi -- -runs=64 -timeout=10 -max_len=16384 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address tree_sitter_mermaid_parse fuzz/corpus/tree_sitter_mermaid_parse distribution/tree-sitter-mermaid/fuzz/corpus/all-families -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address tree_sitter_mermaid_edits fuzz/corpus/tree_sitter_mermaid_edits fuzz/seeds/tree-sitter-edits -- -runs=64 -timeout=10 -max_len=262144 -dict=fuzz/dictionaries/mermaid.dict
cargo +nightly-2026-07-01 fuzz run --fuzz-dir fuzz --sanitizer address tree_sitter_mermaid_scanner fuzz/corpus/tree_sitter_mermaid_scanner fuzz/seeds/tree-sitter-scanner -- -runs=64 -timeout=10 -max_len=16384 -dict=fuzz/dictionaries/mermaid.dict
```

On macOS, local `cargo-fuzz` installations may default to the wrong host target if the binary was
installed under Rosetta. In that case, reinstall `cargo-fuzz` natively or add the explicit target
triple for the local host. The CI authority is Linux x86_64 with ASan.

## CI Campaigns

Fuzz CI separates deterministic merge evidence from randomized discovery:

- The central pull-request workflow and every `main` push build every target with ASan, then invoke
  each harness on the committed seed corpus, minimized corpus, and crash regressions as fixed input
  files. No mutation loop runs in this lane.
- The weekly scheduled run gives every target a 15-minute discovery budget. A randomized campaign
  must continue even when the repository has no new commits, so this run supplements rather than
  replaces the deterministic gate.

The shared Mermaid seeds include lexer-progress regressions for bare Sequence comments, malformed
actor configs, unterminated Class bodies, unexpected Class braces, and missing State IDs. The EOF
cases intentionally omit the final newline; the CRLF comment seed is marked binary in
`.gitattributes` to preserve its authored line endings. Both parser and renderer targets replay
these inputs under the existing 10-second libFuzzer timeout before randomized discovery.

`workflow_dispatch` can select one target or the complete target set with `smoke`, `extended`, or
`long` randomized budgets. Pull-request jobs receive only read access to repository contents and do
not consume release credentials, including for contributions from forks.

Any target failure fails the workflow and uploads both the generated crash artifacts and the full
libFuzzer log. The job summary distinguishes sanitizer findings from Rust or harness panics; a
libFuzzer `deadly signal` caused by a Rust assertion must not be reported as an ASan memory-safety
finding.

## Sanitizer Policy

CI uses AddressSanitizer because it catches the most relevant native memory faults with reasonable
signal-to-noise for this codebase and dependency graph. `cargo-fuzz` also supports leak, memory,
thread, and no-sanitizer modes, but those are investigation tools rather than required release gates.

All UTF-8 SVG inputs, including malformed XML, pass through the pipeline for panic and sanitizer
coverage. For well-formed input, the assertions mirror the documented `resvg-safe` structural
contract: output must stay XML-parseable, remove active elements, event-handler and unsafe URL
attributes, and `foreignObject`, while preserving safe local fragment references and safe raster
image data URIs. Successful output must also remain unchanged by another finalization pass.
Unclosed CSS strings, comments, URLs, and blocks are regression inputs for this terminal-output
contract; safe neighboring CSS must survive removal of malformed content. `resvg-safe` is a raster-compatibility pipeline, not a general sanitizer for
arbitrary host CSS; browser and webview consumers must follow `docs/security/RENDERING_SECURITY.md`.

When a crash is found, minimize it before promoting it into a regression test:

```sh
cargo +nightly-2026-07-01 fuzz tmin --fuzz-dir fuzz <target> fuzz/artifacts/<target>/<crash-file>
```

If the minimized input exposes a public API bug, add a focused stable test under the affected crate.
Keep the fuzz corpus for exploration, not as the only regression proof.

## Selected Mermaid Parser Differential

The independent `xtask parser-differential` command runs the selected real Mermaid runtime in one
serial Chromium realm. Its Flowchart and Sequence adapters observe actual parser/database records;
they do not simulate Jison callbacks or compare generated DOM IDs. The family lane accepts no
frontmatter or init directives and supplies identical explicit site settings. A separate public
`mermaid.parse` lane checks whole-source acceptance and returned configuration. Flowchart labels use
the selected package's actual entity decoders at Merman's public semantic observation stage.

Provision `repo-ref/mermaid` at the commit recorded in `tools/upstreams/REPOS.lock.json`, install the
locked `tools/mermaid-cli` dependencies, and install that Puppeteer's selected headless shell:

```sh
npm ci --prefix tools/mermaid-cli
node tools/mermaid-cli/node_modules/puppeteer/lib/puppeteer/node/cli.js browsers install chrome-headless-shell
cargo run --locked -p xtask -- parser-differential --cases 256
cargo run --locked -p xtask -- parser-differential --replay target/parser-differential/mismatch-<digest>.json
cargo run --locked -p xtask -- parser-differential --minimize target/parser-differential/mismatch-<digest>.json --max-attempts 128 --timeout-seconds 900
```

`CARGO_TARGET_DIR` also controls this command's evidence directory. Each failure retains exact UTF-8
source bytes, site settings, family/lane, corpus identity, seed and ordered mutation operations,
local/upstream observations, selected source/package/runtime identities, and harness/decoder digests.
Replay reads those retained bytes instead of rerunning a potentially changed generator. Explicit
minimization tries bounded line/chunk deletion and then Unicode-safe smaller fragments, accepting only
the same mismatch class. A missing/stale runtime, unexpected local engine error, invalid protocol,
process/browser failure or timeout fails tooling; none counts as an ordinary parse rejection.

The SVG parity job replays the fixed corpus before the 256 deterministic mutations per family in
the same serial batch.
The separate scheduled/manual discovery job admits at most 5,000 mutations per family and a 15-minute
command deadline. Mutated inputs are bounded to 4 KiB; these are campaign limits, not library limits.
The in-process libFuzzer jobs remain separate and never start a browser for each input. Promote a
reviewed minimized failure into its family regression tests and `fuzz/seeds/mermaid`; do not add
comparator exceptions or regenerate rendering baselines to conceal it.

This observes bounded parser/DB semantics, not complete grammar coverage, browser layout or SVG
parity. Nonsecure theme presentation metadata can deliberately differ because Merman's existing
source-value admission and historical metadata projection have distinct responsibilities; that
pre-existing theme scope is outside this campaign's fixed public-config cases. Its actual observed
mismatch can still be replayed and minimized without a comparator allowance. Source-secure keys in
public metadata follow the explicit initialized `secure` list or the selected upstream default list;
rendering retains Merman's separate hardened admission.
