# Shared HTML5 Fragments: Local Verification

Date: 2026-10-15. Platform: Windows 11, PowerShell 7, Rust 1.95.0.
Base: `b9844db7b747111690bf6369d4536885a85e35a7`.
Plan: [Shared HTML5 Fragments](../../../plans/2026-10-15-1547-refactor-html5-fragments-plan.md).

## Behavior and Source Evidence

The original nine-node shared Flowchart renders without editing the missing `</b>`.
The final CLI produced a 63,134-byte SVG; a separate XML parse found all nine node groups.
The minimal public regressions compare missing and misnested formatting with their browser-recovered structure.

DOMPurify reference checkout: `b9b9d80f7e401771c2ccaef5f45def7eec8f27d7` (3.4.16).
Its `dist/purify.cjs.js` SHA-256 is `1144c3ba99465d58ff93ab2419ebd99b7d7e12525b1ca4f45308771acf34cfec`, matching the selected npm distribution used for the local jsdom oracle.
Source-backed cases include inert document/body selection, templates, leading whitespace, noscript, namespace transitions, target hooks, and XML attribute whitespace.

## Rust and Artifact Gates

- Core: `cargo nextest run --locked -p merman-core --lib --cargo-quiet --no-fail-fast`: 479 passed.
- Renderer: `all-diagrams,math,layout-cytoscape,layout-elk`, library plus Flowchart/Mindmap/Architecture public SVG suites: 3,543 passed, three intentional skips. The initial run exposed a static-inline SVG-anchor namespace failure. After its fix, both the focused regressions and the complete suite passed; the complete suite was verified again before PR submission.
- Shared HTML controls: seven selected tests passed, including cancellation specifically triggered at the second SVG parse epoch, work rejection, output limits, and repeated parse accounting.
- DOMPurify generator: six selected tests passed, including malformed-source refusal without overwriting generated output.
- Optimized 64 KiB stack probe: 2,048 nested spans completed success, parse-budget rejection, and teardown, using the final optimized core library. The checked-in lifecycle canary uses 512 KiB for the dependency's large unoptimized frames and 64 KiB when optimized.
- `cargo clippy --locked -p merman-core -p merman-render -p merman-cli -p xtask --all-targets --all-features -- -D warnings`: passed.
- `cargo fmt --all -- --check`, `git diff --check`, and indexed path-length validation: passed.
- WASM: `cargo check --locked -p merman-wasm --target wasm32-unknown-unknown --no-default-features --features all-diagrams,svg,layout-cytoscape,layout-elk,math`: passed.
- `xtask verify-dompurify-defaults`: passed against the selected checkout.
- `xtask verify-feature-matrix`: passed isolated consumers and all 45 build cases; 36 artifact recipes validated structurally.
- Representative artifact dependency closures: 22 profiles passed, including Rust, Web, and Typst.
- `cargo deny check advisories bans licenses sources`: all four passed. The unused selectors license exception was removed with its dependency.
- Legal owner chain: all 13 Rust reports fresh, all 424 release projections fresh, third-party contract valid, and 18 Python legal tests passed.
- Performance tooling contracts: 16 + 51 + 29 + 121 Python tests passed.

Before PR submission, the recovered 58-path change set matched its review snapshot byte for byte. Core (479 passed), the complete renderer selection (3,543 passed, three skips), formatting, diff hygiene, indexed path lengths, and the legal owner chain were rechecked successfully. The earlier full SVG structure, feature, WASM, Clippy, and memory results below apply to that same source state; those gates were not repeated during recovery.

## Full SVG Structure Gate

Executed exactly:

```powershell
cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

Exit 0 across 37 families: 3,756 selected, 3,729 rendered, 27 intentional skips.
Existing exact browser-text-layout and parser-diagnostic residual receipts remained applicable.
No comparator normalization or tracked upstream SVG was changed.
Reports are generated beneath `target/compare/*_report_structure.md`.
This gate checks structure; it does not assert browser pixel identity or root viewport parity.

## Runtime Memory Smoke

The native-memory owner cold build exceeded its 900-second prebuild timeout.
The same exact probe recipe was resumed in `target/html5-native-memory` with its completed dependency cache:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_PROFILE_BENCH_DEBUG = '0'
cargo bench --locked -p merman --no-default-features --features all-diagrams,svg --bench native_memory --target-dir target/html5-native-memory --no-run --message-format=json-render-diagnostics
python tools/bench/run_native_memory.py --smoke --executable <compiler-artifact executable> --json-out target/bench/html5-native-memory-smoke.json
python tools/bench/test_native_memory_driver_contracts.py --verify-smoke-report target/bench/html5-native-memory-smoke.json
```

Both runtime commands passed. The report records `protocol_smoke_pass`, zero contract errors, and an operation/zero pair at each registered boundary (1x and 100x).
This is infrastructure smoke with `candidate_admission=false`; it does not claim a complete six-scale regression/admission experiment.

## Matched CLI Diagnostic

Both binaries use the same Windows debug profile and `--no-default-features --features diagram-flowchart,svg`.
The baseline executable was preserved before editing at the base commit above.
One warm-up per variant precedes five measured processes per workload, with alternating variant order.
The rich/plain workloads each contain 200 nodes and a 199-edge chain; labels are respectively `<b>Item n</b><br><small>Domain &amp; service</small>` and `Item n`.
The closed workload is the reported graph with an explicit formatting closure, so the incumbent can render it.

| Workload | Baseline median ms | Replacement median ms | Change |
| --- | ---: | ---: | ---: |
| closed | 117.55 | 115.30 | -1.92% |
| bench-plain | 215.98 | 222.09 | +2.83% |
| bench-rich | 877.95 | 863.22 | -1.68% |

These are diagnostic process timings, including startup and output I/O, rather than optimized release benchmark evidence.
The observed small differences do not establish a speedup or a release latency regression.
The debug executable changed from 30,731,264 to 30,804,480 bytes (+73,216 bytes, +0.24%).
The recovered-tree representation and prepared XHTML retention are accepted correctness costs; release package-size policies remain owned by their platform build lanes.

Raw timing samples and executable/source hashes are retained in the companion JSON.

## Review and Delivery

Nine independent code-review lenses completed with no remaining actionable findings after fixes.
The review used Codex subagents only. No code was sent to an external model.
A later three-agent deep-review attempt ended with platform policy errors and produced no reports; it is not counted as completed coverage.
The delivery branch is `refactor/html5-fragments`. Git and the PR record track its submission; no release was performed. Complete cross-platform and browser parity checks remain owned by the selected PR workflows.


## PR #203 Default-Workspace Follow-up (2026-10-16)

The Linux default-workspace run exposed two label-measurement regressions and five stale
serialization assertions/projections. HTML paragraphs now collapse ASCII whitespace while
preserving non-breaking spaces. Ordinary Markdown paragraphs retain the browser's 1/64px width
lattice, while unsupported raw blocks and min-content overflow retain the host measurer's
precision. Paragraph gaps and image-only lines are derived from the final sanitized XHTML.

Complete plain paragraphs still use the host's complete-label measurement request. This matters
for profiles that supply a captured browser label box: replacing that request with per-word
measurements changes terminal geometry even when the deterministic text profile passes. The
controlled browser-terminal test passed against the submitted implementation, caught that
follow-up regression, and passed again after restoring the complete-label request with the
whitespace and Markdown width rules above.

Six focused regressions cover whitespace/NBSP, Markdown quantization, raw blocks, min-content
overflow, entity-only root text, and entities beside an image. Canonical HTML assertions now
expect the selected sanitizer's serialization, including `<br>` and escaped angle text.

Snapshot audit: 156 model files (544 string leaves) and 24 layout files (27 string leaves) changed.
No numeric value, non-string value, array order, or object/array structure changed. All 571 old/new
string pairs produced equivalent DOMs in the selected DOMPurify oracle's `text/html` body parsing
context, with attribute order normalized. Template-fragment parsing was not used as an equivalent
context. No layout coordinate was refreshed to hide a regression.

Follow-up validation:

- Focused label and controlled browser-terminal tests: 32 passed.
- Default-workspace feature unification, core/renderer libraries, and the affected snapshot,
  Flowchart, Usecase, Mindmap, Architecture, and Swimlane integration owners: 5,569 passed,
  three intentional skips. Cargo build parallelism was limited to two jobs after an earlier
  unconstrained local build exhausted Windows commit memory.
- The exact full SVG structure gate above was repeated successfully after the follow-up: 37 families, 3,756 selected, 3,729 rendered, and 27 intentional skips. Existing exact diagnostic residual receipts remained applicable.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
