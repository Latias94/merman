# Continuous Integration

Merman separates pull-request feedback, scheduled maintenance, and release evidence. A check belongs
in the earliest layer that can justify its cost, but it should not be repeated in every later job
unless that job produces an independently releasable artifact.

## Pull Requests

Pull requests answer whether a change is safe to review and merge:

- formatting, repository hygiene, and generated-source freshness;
- full workspace tests on Linux, plus blocking SVG parity when its explicit inputs change;
- workspace compilation plus an explicit host-sensitive macOS/Windows inventory, including the
  Windows ELK small-stack regression;
- representative Cargo feature leaves, default surfaces, owner APIs, and feature-unification
  regressions;
- Web package build, size budgets, package smoke, and Playground browser behavior when their inputs
  change;
- binding or package smoke in the workflow that owns that user surface.

The central `CI` workflow is the pull-request and merge-queue orchestrator. Its planner compares the
trusted base and head commits with a NUL-delimited Git name-status diff, selects owner jobs, and
records the reasons in one validated JSON document. The detailed plan remains at the producer
boundary; only the fixed-size owner selector map crosses into downstream jobs. Unknown paths,
workflow or classifier changes, malformed diffs, and missing Git objects select every owner. A
valid empty diff is the only case that runs no owner job.

Every selected owner completes in that same workflow run. The final `pr-gate` check rejects failed,
cancelled, skipped, missing, or malformed selected results; unselected owners appear as deliberate
skips. It aggregates the bounded selector map and explicit job results rather than transporting the
full changed-path plan through a runner environment. This stable check is the repository-side
branch-protection target, but it is not a standalone trust root: a pull request can propose changes
to repository workflows or the planner itself. The checked-in `CODEOWNERS` file therefore assigns
those paths to the maintainer. A `main` ruleset must require pull requests, code-owner approval, and
`pr-gate` before the check is enforcement rather than reporting. Repository rules and
release-environment protection remain external maintainer configuration and must not be described
as enabled until a read-only GitHub query confirms them.

Performance evidence produced from pull-request code remains read-only: it may upload diagnostic
artifacts and append a job summary, but it does not receive repository write permissions or feed a
separate comment-writing job.

Reusable owner workflows do not start independent pull-request runs. The central orchestrator calls
them after classification, so one owner executes once and its result is available directly to
`pr-gate`. Main pushes still select every owner. A weekly and manually dispatchable core safety-net
run executes the full workspace on Linux, macOS, and Windows; routine pull requests keep the full
Linux suite while host runners compile the workspace and run focused filesystem, process, FFI, and
ELK stack-safety contracts. Only pull-request and merge-queue runs emit the required `pr-gate`
status name; push, schedule, and manual lifecycles use event-specific gate names so their results
cannot satisfy the pull-request check by identity collision.

The planner records SVG parity as a selector inside the same owner plan rather than as a second
owner. Pull requests select it for the SVG renderer and its shared parser/layout crates, the SVG
comparator and root-viewport oracle, active renderer fixtures, and pinned upstream authorities.
ASCII-only implementation changes still run the `core` workspace gate but do not run the full SVG
corpus or install Chromium. Main pushes, scheduled runs, manual runs, and fail-broad planner results
always select SVG parity as a safety net.

When selected, the Linux parity lane performs one Mermaid source parse and one local SVG render per
fixture. Within
the multi-policy DOM comparator, each upstream/local SVG is normalized and XML-parsed once,
descendant signatures are cached by the normalization profile that actually affects them, and the
blocking `structure`, `parity`, and `parity-root` policies are evaluated. `parity` and `parity-root`
share the same descendant signature; the root contract is evaluated separately from the same parsed
document. Specialist diagnostics may independently inspect XML, but they do not repeat the Mermaid
parse or local SVG render. Failures retain their mode attribution, and the `parity-root` evaluation
still emits the root-delta report at the existing
`target/compare/<diagram>_report_parity_root.md` path. The lane
then mounts the real `target/compare/<diagram>/*.svg` files from that comparison in Chromium and
checks that painted content stays inside each root viewport. It reuses that job's Node environment
and generated SVGs, installs only the locked
`playground/tests` dependencies and Chromium, and preserves the outer SVG's own width, height, and
max-width while mounting it in a fixed-width host. A local paint failure triggers the same paint
audit for its upstream SVG. This browser gate runs only after the same job's blocking DOM,
semantic-label, and exact browser-text receipt checks, so fixture identity and the admitted SVG
structure are already bound before paint evidence is compared.

The first transparent capture records all paint. A second capture suppresses SVG text,
font-laid-out HTML label text and its text-owned background/border, and RoughJS drawing paths.
Overflow removed by that second capture remains a browser-owned diagnostic; ordinary shapes,
markers, images, and non-label `foreignObject` paint remain structural. For structural overflow,
the oracle compares the maximum outward painted depth independently on each root edge against the
same upstream fixture. A new overflow edge or a deeper local edge blocks. Along-edge pixel count and
position remain diagnostic rather than becoming a fuzzy matcher: the preceding DOM and semantic
gates already bind the compared artifacts, while anti-aliasing and text-led layout can move an
otherwise identical clipped stroke along an edge. Depth is measured from the root boundary rather
than from the thickness of the painted fragment, and corner paint is attributed to every crossed
edge.

Indeterminate evidence fails closed unless upstream has the same structured reason set.
Capture-boundary evidence and marker paint whose capture reach cannot be bounded always block. A
pure capture-limit result may be inherited only with the same guard, a no-larger local capture
envelope, and no deeper geometry extent on any edge; filter and image-decode cases must also have no
deeper structural edge. The capture budget is expressed as both a maximum dimension and a maximum
pixel area, so tall or wide moderate-area diagrams can be measured without allowing unbounded
screenshot memory.

One reviewed out-of-domain XYChart extrapolation is admitted through
`fixtures/_verification/root-viewport-residuals.json`. This is not a numeric tolerance: the receipt
binds the exact local and upstream SVG SHA-256 values plus a closed reason. A changed or unused
receipt blocks. Every other local-only, new, or worse structural result remains blocking.
The JSON report at `target/root-viewport-diagnostic.json` is uploaded as a diagnostic artifact even
when the oracle fails; upstream browser measurements in that report remain diagnostic rather than
an acceptance policy. The oracle expands its transparent screenshot capture from browser geometry
only to ensure coverage; acceptance still comes from painted alpha pixels outside the root.

Editor-language descriptors are shared inputs to the browser editor and VS Code extension. Changes
under `contracts/editor-language/` therefore select both owners. Other shared authorities and
unknown paths fail broad instead of guessing a narrow consumer set.

Rust crates, fixtures, and repository scripts use an explicit path-prefix owner table. Ordinary
renderer and fixture changes select the Linux workspace owner plus hygiene; binding, package, and
platform crates add only their owning smoke workflows. Top-level Cargo authorities, capability and
ABI schemas, workflow/classifier code, legal policy, unclassified crates, and unknown paths still
select every owner. The table is intentionally static and reviewable rather than a partial Cargo or
Rust dependency analyzer.

The independently versioned Tree-sitter language distribution has its own `grammar` owner. Changes
under `distribution/tree-sitter-mermaid/` select that owner and `hygiene`; npm manifests and
lockfiles also select `npm` and `security`, while Cargo manifests and provenance also select
`security`. Package license and third-party notice changes also select `security`. Changes to the
composed contract under `contracts/tree-sitter/` select `grammar` and
`hygiene`. Workspace manifests, shared fixtures, and pinned upstream sources
remain shared authorities and therefore select every owner. The grammar workflow verifies the
35-family catalog projection, Rust package tests, production dependency isolation, legal inventory,
and Cargo/npm package assembly. A planned family is metadata only: it cannot advertise a support
tier or query evidence until the corresponding executable gates exist.

The pull-request feature matrix validates the complete declared feature graph but compiles a curated
set of representative products and transports. It deliberately does not compile every bounded
pairwise combination and artifact recipe.

## GitHub Pages and Playground

The Pages workflow owns the deployable Playground and browser integration evidence. It builds the
Web package group once, then uses those exact package artifacts for size budgets, package smoke,
Playground preparation, and browser tests. Generated-source freshness remains owned by the central
CI workflow for pull requests. A main-branch or manual Pages deployment repeats only the deployable
browser projections because deployment is an independently mutable side effect.

Pull requests build and test the site but do not deploy it. Main-branch runs upload the same tested
`playground/dist` directory to GitHub Pages. Chromium exercises the full browser suite; Firefox and
WebKit retain focused smoke coverage because browser-specific loading and worker failures are a
user-visible contract rather than a duplicate source-level test.

## Scheduled Maintenance

The repository has focused weekly schedules for full host Rust tests, fuzzing, security,
performance, and Homebrew compatibility. There is no umbrella daily `nightly` workflow. Scheduled
checks answer questions that need time, repeated observation, or external-state refresh rather
than immediate merge feedback.

`cargo-fuzz` uses a pinned Rust nightly toolchain because sanitizer-backed fuzz instrumentation
requires it. The workspace and release artifacts continue to use the pinned stable Rust toolchain.
Pull requests build every harness and replay every committed seed, corpus entry, and crash
regression without mutation. Only scheduled and manually dispatched fuzz runs perform randomized
discovery.

The performance workflow selects regression and frontmatter descriptors into one measurement
matrix. `tools/bench/performance_lanes.json` owns the lane recipes, labels, scheduled set, and manual
selection groups; `tools/bench/performance_workflow.py` validates that registry and writes the
matrix consumed by GitHub Actions. Contract tests exercise the same structured interface instead
of parsing workflow YAML or shell text. Corpus, runner/recipe, statistics, report-consumer, and
workflow contracts live in focused test modules behind the legacy aggregate command. Workflow
syntax and expression checks remain owned by actionlint; security checks remain owned by zizmor.
Each descriptor uses the same base/head runner, receipt, artifact, summary, and outcome consumer.
Its standalone contracts run for pull requests only when the shared CI classifier selects the
performance owner or an explicit `perf` or `perf-frontmatter` label requests a measurement. ASCII
timing requires a manual dispatch with a compatible benchmark-only base backport. Pull requests
remain read-only and write only to the job summary; schedules run both
self-comparison descriptors plus the independent external-renderer reference lane.

## Release Preflight

Release preflight runs against an immutable source revision and owns exhaustive or artifact-exact
evidence:

- the strict Cargo feature and artifact-recipe matrix;
- target-scoped dependency closures and legal reports;
- exact package contents, checksums, symbols, provenance, and installation smoke;
- full SDK and registry package assembly;
- release version and documentation projections.

The Web size gate measures the final wasm-bindgen binaries copied into the npm packages. It must not
build and measure a second Cargo-only approximation of the same package.

## Evidence Ownership

| Claim | Owning evidence |
| --- | --- |
| Rust behavior | Workspace and focused owner tests |
| Cargo feature boundaries | Static feature graph, representative PR builds, strict release matrix |
| Browser package size | Final npm package WASM measurements |
| Cross-browser behavior | Playground browser tests |
| Native ABI compatibility | ABI descriptor, frozen consumer, symbol, and lifecycle tests |
| Tree-sitter family support | Language metadata, composed catalog contract, corpus, incremental, query, and conformance gates |
| Dependency policy | `cargo deny`, RustSec governance, and release closure reports |
| Published bytes | Release package, checksum, provenance, and install smoke |

## Digest Policy

Use a digest when it identifies an external immutable input or a released artifact: upstream
tarballs, attestation material, ABI fixtures, generated baselines, and published archives are good
examples.

Do not use an opaque digest as the review interface for ordinary local source files, workflow text,
or every dependency closure. Local manifests and lockfiles should be validated by their native
tools and semantic contracts. A dependency change should produce a readable package, version,
source, and feature diff rather than only a replacement hash.

## Adding a Check

Before adding a standing PR check, identify:

1. the user-visible failure it detects;
2. why an existing test or owner workflow cannot detect the same failure;
3. whether it validates source, a generated projection, or the final artifact;
4. the expected wall-clock and runner-minute cost;
5. whether release preflight or a focused weekly schedule is the more accurate owner.

Prefer native tools such as Cargo, nextest, actionlint, npm, and package installers. Do not grow
repository scripts into partial parsers for Rust, Cargo, GitHub Actions, or shell merely to prove a
workflow row or source line is safe.

Workflow syntax and expression semantics are checked with actionlint 1.7.12. High-severity workflow
security findings are checked with zizmor 1.29.0. CI verifies the downloaded actionlint archive and
the selected zizmor wheel by SHA-256 before execution, and prints both tool versions. Repository
tests cover only Merman-specific boundaries: the same-run fail-closed gate, read-only PR owner
closure, trusted deployment separation, maintained action release tags, checkout credential
isolation, and npm provenance policy. GitHub Actions use current stable release tags, while actions
whose ref is part of their public interface retain readable tool or toolchain refs. The zizmor
configuration accepts version refs without disabling its other workflow-security audits, and weekly
Dependabot updates maintain the selected action versions.
