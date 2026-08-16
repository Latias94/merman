# Continuous Integration

Merman separates pull-request feedback, scheduled maintenance, and release evidence. A check belongs
in the earliest layer that can justify its cost, but it should not be repeated in every later job
unless that job produces an independently releasable artifact.

## Pull Requests

Pull requests answer whether a change is safe to review and merge:

- formatting, repository hygiene, and generated-source freshness;
- full workspace tests and blocking parity on Linux;
- workspace compilation plus an explicit host-sensitive macOS/Windows inventory, including the
  Windows ELK small-stack regression;
- representative Cargo feature leaves, default surfaces, owner APIs, and feature-unification
  regressions;
- Web package build, size budgets, package smoke, and Playground browser behavior when their inputs
  change;
- binding or package smoke in the workflow that owns that user surface.

The central `CI` workflow is the pull-request and merge-queue orchestrator. Its planner compares the
trusted base and head commits with a NUL-delimited Git name-status diff, selects owner jobs, and
records the reasons in one JSON document. Unknown paths, workflow or classifier changes, malformed
diffs, and missing Git objects select every owner. A valid empty diff is the only case that runs no
owner job.

Every selected owner completes in that same workflow run. The final `pr-gate` check rejects failed,
cancelled, skipped, missing, or malformed selected results; unselected owners appear as deliberate
skips. This stable check is the repository-side branch-protection target, but it is not a standalone
trust root: a pull request can propose changes to repository workflows or the planner itself. The
checked-in `CODEOWNERS` file therefore assigns those paths to the maintainer. A `main` ruleset must
require pull requests, code-owner approval, and `pr-gate` before the check is enforcement rather
than reporting. Repository rules and release-environment protection remain external maintainer
configuration and must not be described as enabled until a read-only GitHub query confirms them.

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

The Linux parity lane performs one Mermaid source parse and one local SVG render per fixture. Within
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
audit for its upstream SVG. This browser gate runs only after the same job's blocking DOM parity
suite. It compares root-relative structural overflow pixels and accepts inherited evidence
only when the local footprint is no larger and every local pixel is present in the upstream
footprint or its immediate raster neighbor. That one-physical-pixel neighborhood handles binary
alpha coverage moving between adjacent pixels during anti-aliasing; it is not a CSS-space or
fixture tolerance. Root and geometry floats remain diagnostic. It repeats the alpha capture with
SVG text and RoughJS drawing paths suppressed. Overflow attributable only to browser-owned text
measurement or RoughJS output stays diagnostic,
while shapes, markers, images, and `foreignObject` paint remain in the blocking pass. An
indeterminate result is also diagnostic only when upstream has the same structured reason set and
no new structural overflow pixel. Browser geometry and required capture dimensions remain report
data, not a numeric acceptance policy. Paint that reaches the audit boundary always blocks because
its full extent is unknown.
Local-only, new, or worse structural evidence remains blocking. No
fixture-specific tolerance or inheritance list is used.
The JSON report at `target/root-viewport-diagnostic.json` is uploaded as a diagnostic artifact even
when the oracle fails; upstream browser measurements in that report remain diagnostic rather than
an acceptance policy. The oracle expands its transparent screenshot capture from browser geometry
only to ensure coverage; acceptance still comes from painted alpha pixels outside the root. A
capture that exceeds the global bound, or paint such as an active filter whose extent cannot be
proven, is indeterminate and fails closed instead of being treated as contained.

Editor-language descriptors are shared inputs to the browser editor and VS Code extension. Changes
under `contracts/editor-language/` therefore select both owners. Other shared authorities and
unknown paths fail broad instead of guessing a narrow consumer set.

Rust crates, fixtures, and repository scripts use an explicit path-prefix owner table. Ordinary
renderer and fixture changes select the Linux workspace owner plus hygiene; binding, package, and
platform crates add only their owning smoke workflows. Top-level Cargo authorities, capability and
ABI schemas, workflow/classifier code, legal policy, unclassified crates, and unknown paths still
select every owner. The table is intentionally static and reviewable rather than a partial Cargo or
Rust dependency analyzer.

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
matrix. Each descriptor uses the same base/head runner, receipt, artifact, summary, and outcome
consumer. Its standalone contracts run for pull requests only when the shared CI classifier selects
the performance owner or an explicit `perf`, `perf-ascii`, or `perf-frontmatter` label requests a
measurement. Pull requests remain read-only and write only to the job summary; schedules run both
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
