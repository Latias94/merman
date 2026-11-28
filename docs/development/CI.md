# Continuous Integration

Merman separates pull-request feedback, scheduled maintenance, and release evidence. A check belongs
in the earliest layer that can justify its cost, but it should not be repeated in every later job
unless that job produces an independently releasable artifact.

Ordinary Linux CI uses the explicit `ubuntu-26.04` and `ubuntu-26.04-arm` labels, which
[became generally available in September 2026](https://github.com/actions/runner-images/issues/14747).
Pinning the LTS version keeps runner upgrades intentional. Native artifact builders retain their
declared compatibility images, including Ubuntu 22.04 for GNU CLI/LSP archives and VS Code runtimes.
Release and performance workflows keep their existing qualified environments; their migration is
separate from ordinary CI.
Actionlint's native label configuration admits the two official 26.04 labels until its built-in
runner list includes them.

## Rust Toolchains

`rust-toolchain.toml` pins development and ordinary CI/release builds to Rust **1.100.0**, with
`rustfmt` and `clippy`. Maintained workflow toolchain references track this pin; SHA-pinned
`dtolnay/rust-toolchain` steps use the commit for that exact release.

The minimum supported Rust version (MSRV) remains **1.95**. Keep explicit MSRV/compiler-floor
recipes on their declared versions. Toolchain upgrades preserve dated compatibility receipts and
performance baselines; fuzzing retains its separately pinned nightly.

Cargo 1.100 changes the internal build-directory layout. Tools that discover test or benchmark
executables must read `compiler-artifact` JSON messages instead of reconstructing paths under
`deps`, `build`, or `.fingerprint`. Final binaries and libraries in the target directory retain
their ordinary output paths. Keep the existing shared-target freeze and provenance checks.

Rust 1.100 uses Unicode 18.0 in the standard library. Parser and text-layout regressions remain
part of the toolchain upgrade checks; a compiler upgrade does not authorize refreshing SVG or
layout baselines without reviewing the semantic difference. New standard-library APIs must remain
available at the declared MSRV before entering library code.

The operation-context thread-local key already has a const initializer. Clippy 1.100 reports
`missing_const_for_thread_local` from the standard library's TLS fallback on Android. Keep the
allowance on that declaration until an admitted compiler update handles the expansion correctly.

The default xtask build supports contract checks such as `verify-feature-matrix`,
`verify-native-abi`, and `verify-capability-surface` without the SVG renderer. Use
`--features render-parity` for rendered fixture commands and the full binding and resource
projections. Those projections validate capabilities compiled into an actual artifact contract.
Use `--features wasm-host` for WASM inspection and Typst package tasks without selecting the
native SVG renderer.

`cargo run --locked -p xtask -- verify --strict` remains the complete local gate. The default
binary delegates `verify` to the render-parity build, which compiles all four vendored
renderer crates. Workspace tests follow
their manifests, and `merman-export` integration tests cover Krilla's PDF behavior. The Clippy
gate applies `-D warnings` to workspace-owned targets. See the separate
[independent crate release gates](../release/RELEASING.md#independent-crate-publication-and-recovery).

## Runtime and job dependencies

The core, native Node, and SVG/browser owners publish separate same-run results to `pr-gate`:

- `build-test` retains the Linux workspace suite, generated checks with their pinned
  DOMPurify/Node inputs, and the three-host font/platform contracts. Its macOS host-font
  run includes Core Text matching, snapshot identity and selection reuse, plus host-font
  shaping comparisons for right-to-left and Indic text.
  Linux exercises automatic CLI selection with both provider defaults, an empty Fontconfig
  configuration, and an unloadable Fontconfig library. The GNU archive and container jobs
  exercise raw SVG conversion on the archived CLI, including the empty and missing Fontconfig cases.
- `feature-contracts` owns Linux representative feature builds, isolated consumers, compile-fail
  examples, Rustdoc subsets/math, feature unification, runtime adapters, FFI and ASCII contracts;
- The exact CLI process matrix exercises Grid and Layered in separate defaults-off profiles. It
  verifies reported capabilities, diagram and site-config selection, SVG, and layout JSON;
- `theme-contracts` verifies embedded Theme v2 preset colors and runs Rust theme, render-operation,
  and prepared-source tests.
- `svg-parity` owns the complete DOM comparison and Chromium containment oracle;
- `node-contracts` runs Node-WASM and N-API as parallel matrix members on PRs and main;
- Pages builds the standard Web package group and the Playground's Shoal WASM in one job.
  The optimized size tool builds independently. Size reporting and Playground qualification
  use the uploaded artifacts and run in parallel.
- A shared native runtime workflow builds each non-Linux complete CLI once when both CLI
  and VS Code are selected. The same platform job checks the analysis profile, builds LSP and
  the complete CLI, then packages and tests VSIX. CLI-only runs retain their three-host profile
  checks; VS Code-only runs retain all four packages without the additional analysis build;
- GNU archives, containers and setup Action checks retain their independent paths. Arch has
  its own source-build owner. VS Code package builds run alongside the editor contract;
- Central CI passes its immutable source SHA directly into the runtime workflow. Standalone
  dispatch still validates its protected branch or release tag and resolves it to a SHA before
  consumers build it. Both selected owners require the shared workflow's successful result.

Ordinary macOS/Windows runs compile the workspace and execute the existing host-sensitive
regression inventory. Weekly and manual core safety nets additionally run full workspace tests
on those hosts. Linux still runs the full suite on every selected event. The refactor preserves
blocking assertions; it changes their placement and dependencies. Runner queue time and cold
builds remain part of elapsed feedback time, so measured PR results are required before claiming
a runtime target has been met.

The feature and theme jobs use the `core` selector and contribute required results to `pr-gate`.
Their Cargo recipes select features for each check. Rust caches retain registry data with
`cache-targets: false`; these jobs do not transfer compiled targets or reuse results from a
different source revision.

On macOS safety-net runs, the full workspace suite completes before
`cargo clean --workspace --profile dev` reclaims its workspace artifacts. The subsequent
font and feature recipes rebuild workspace packages while reusing third-party dependency
artifacts. This bounds accumulation across the two phases on the hosted runner's limited
disk without removing any feature recipe or test. Ordinary host runs keep their compiled
workspace artifacts. The macOS job logs target size and available disk before and after
reclamation, and reports disk usage on both success and failure. These measurements distinguish
workspace accumulation from dependency or runner-image growth.

## Apple Binding Checks

The platform owner separates compiler compatibility from Apple SDK integration:

- `apple-swift-5-9-smoke` type-checks both checked-in Swift files and their unchanged C
  declarations with the official Swift **5.9.2** Linux compiler, pinned by container digest.
  A temporary module map omits the published map's Darwin imports. This exercises the real
  pre-Swift-6 compiler branches; a new compiler's `-swift-version 5` mode is not a substitute.
- `apple-uniffi-smoke` builds all five native architectures on `macos-15`, regenerates the
  binding, checks its freshness, and runs the macOS smoke. It packages a complete local
  SwiftPM archive, builds a separate consumer against its extracted contents for macOS and
  iOS device/simulator, and executes the macOS and iOS Simulator consumers. The explicit
  iOS 14 target is a build check; execution uses an installed simulator runtime, not proof
  of the minimum supported OS. The same owner, release preflight and Apple candidates compare
  matched original/cleaned XCFramework compression and equivalent macOS consumer output.

Apple package archives and qualification work directories live under `RUNNER_TEMP`, outside
restored Cargo target caches. Packaging retains its refusal to overwrite existing outputs;
workflow artifact paths and simulator-cleanup receipts use the same temporary root.

GitHub's [macOS 14 retirement](https://github.com/actions/runner-images/issues/13518) removed
the hosted Xcode 15.2 lane. The Linux compiler check does **not** validate Apple Foundation,
the published Darwin module map, or Xcode 15.2's iOS 17.2 SDK with Swift 5.9. Before claiming
fresh evidence for that older Apple stack, use an official Xcode 15.2 installation on its
supported macOS host, build `scripts/build-apple-xcframework.sh --ios`, and type-check both
generated Swift files against the matching device and simulator SDKs and XCFramework headers.
Retain the compiler/SDK versions and results in release evidence. Neither Linux plus a current
Apple SDK nor a deployment-target flag establishes that older-stack result. The migration
does not raise the package's declared Swift or OS requirements.

## Flutter Binding Checks

Flutter preflight and publication call the same read-only `build-flutter.yml` candidate owner.
It freezes the thin package and target assets, checks the source binding, and runs the external
relocated macOS app consumer before retaining artifacts. A facade repair may explicitly reuse
an original native-asset run; the same candidate checks then exercise the repaired facade against
those unchanged libraries. Upload and OIDC permissions remain in `release-flutter.yml` only.

## Windows Checkout Paths

Repository-relative tracked paths are limited to 160 UTF-16 code units by
`python scripts/check_path_lengths.py` in the hygiene job. This leaves 99 units for the checkout
prefix, including its final separator, under Windows' traditional 260-unit limit including NUL.
Cargo Git dependencies check out the repository beneath their cache path, including documentation
that Cargo packages may exclude. Keep filenames short and put detailed titles in the document.

The gate reads the Git index, so stage intended renames before checking locally. It does not scan build caches or
untracked user files.

Windows also limits a process command line to 32,767 UTF-16 code units. A workspace-wide
`cargo fmt --all --check` can exceed that limit when Cargo groups hundreds of target paths into
one `rustfmt` invocation, even when every individual path is short. On Windows, `xtask verify`
checks each workspace package separately using Cargo metadata, preserving all targets and stopping
on the first formatting failure. For a focused manual check, use `cargo fmt --package <name> --check`.

An already-published tag retains its original paths. For consumers of such a tag on Windows,
use a short Cargo cache directory and enable `core.longpaths` for Git commands when appropriate.
Changing a dependency's repository config cannot repair a checkout that fails before that config
can be read. Long-path support also depends on the application and Windows configuration; a
short cache prefix avoids relying on every tool's long-path support.

## Pull Requests

Pull requests answer whether a change is safe to review and merge:

- formatting, repository hygiene, and generated-source freshness;
- full workspace tests on Linux, plus blocking SVG parity when its explicit inputs change;
- workspace compilation plus an explicit focused macOS/Windows inventory, including filesystem,
  process, FFI, CLI and ELK small-stack contracts;
- representative Cargo feature leaves, default surfaces, owner APIs, and feature-unification
  regressions;
- Web package build, size reports, package smoke, and Playground browser behavior when their inputs
  change;
- binding or package smoke in the workflow that owns that user surface.

The central `CI` workflow is the pull-request and merge-queue orchestrator. Its planner compares the
trusted base and head commits with a NUL-delimited Git name-status diff, selects owner jobs, and
records the reasons in one validated JSON document. The detailed plan remains at the producer
boundary; only fixed-size selectors cross into downstream jobs. Unknown paths,
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
run executes the full workspace on Linux, macOS, and Windows; routine pull requests and main pushes keep the full
Linux suite while host runners compile the workspace and run focused filesystem, process, FFI, and
ELK stack-safety contracts. The historical KTD23 retirement authorization and its dedicated
commands were removed with their witness. Current checks cover native preset colors, theme
rendering, and font boundaries. Only pull-request and merge-queue
runs emit the required `pr-gate` status name;
push, schedule, and manual lifecycles use event-specific gate names so their results cannot satisfy
the pull-request check by identity collision.

For an explicit full-branch verification, dispatch `CI` with `all_owners=true`:

```text
gh workflow run ci.yml --ref <branch> -f all_owners=true
```

This selects every existing owner, including platform bindings, Android instrumentation and Apple
consumers, without requiring a mergeable PR or changing the branch's source baseline. The default
manual and weekly runs retain the core/SVG safety net. Performance contracts in this run do not
replace a separately registered timing confirmation.

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

Reviewed residuals are recorded in `fixtures/_verification/root-viewport-residuals.json`.
The out-of-domain XYChart extrapolation binds both SVG SHA-256 values and a versioned fingerprint
of the live Chromium decision facts: root dimensions, paint status, capture policy, indeterminate
reasons, and maximum structural overflow depth on each edge. That decision fingerprint deliberately
excludes raw screenshot pixels and browser patch identity.

The Flowchart title 029 display-font residual additionally binds an exact paired audit fingerprint:
both roots, geometry, painted and structural overflow, capture state, structural pixel hashes, and
the browser environment. That admission requires `active-filter` to be the only indeterminate
reason on both sides, with no paint reaching the capture boundary. Missing roots, capture limits,
unbounded markers, and image failures cannot use this admission.

The 2026-10-12 dependency refresh revalidated title 029 against [main run 38203131903](https://github.com/Latias94/merman/actions/runs/38203131903/job/114661820476) and [PR run 38221177606](https://github.com/Latias94/merman/actions/runs/38221177606/job/114718117719). Both SVG hashes and every paired geometry, capture, violation, and structural-pixel field were identical. Only Playwright/Chromium changed from 1.62.1/151.0.7922.34 to 1.64.0/156.0.8078.4; the exact audit fingerprint was regenerated for that browser environment.

Neither receipt is a numeric tolerance or a claim that clipping was repaired. The original
containment classification and full paint evidence remain in the report. A changed, malformed,
or unused receipt and every other local-only, new, or worse structural result remain blocking.
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
`security`. Package license and third-party notice changes also select `security`. Grammar sources,
generated native sources, Rust bindings, the Cargo manifest, and grammar fuzz corpora also select
`fuzz`. The language WASM and portable highlight query select `web` because the
Playground stages those assets. Workspace manifests, shared fixtures, and pinned upstream sources
remain shared authorities and therefore select every owner. The grammar workflow owns native and
WASM generation freshness, the standard corpus, the strict-valid conformance and 37-family query
tests, scanner and incremental behavior, native binding builds, and Cargo/npm package assembly.
Shared dependency and legal gates retain their existing owners. The package-local provenance,
corpus, fixtures, and queries define the syntax contract.

The Windows grammar job prepares Node.js development files separately from compilation with the
lockfile-installed `node-gyp install --ensure`. Its job-local SDK cache under
`${{ runner.temp }}/tree-sitter-node-gyp` is shared with `build:node` through
`npm_package_config_node_gyp_devdir`. Preparation allows at most three attempts, waits 5 and 10
seconds between failures, and has a 10-minute step timeout. node-gyp retains its upstream download
checksum validation, rollback, and per-request retries. The native build runs once and uses
`NODE_GYP_FORCE_PYTHON` with setup-python's `python-path` output; compile and link errors remain
blocking. The generation checks are unchanged. This does not add a cache shared across workflow runs.

This boundary addresses the interrupted Node 24.21.0 development-file download in
[main run 40662958348, Windows job 122730428757](https://github.com/Latias94/merman/actions/runs/40662958348/job/122730428757).
The failure reached HTTP 200 for the headers, checksums, and `node.lib`, then ended with
`TypeError: terminated` during installation before C++ compilation. See the official
[node-gyp options](https://github.com/nodejs/node-gyp#command-options),
[environment configuration](https://github.com/nodejs/node-gyp#environment-variables), and
[Python selection](https://github.com/nodejs/node-gyp#configuring-python-dependency).
Local workflow contracts and actionlint validate the setup/build boundary. Remote
`grammar-owner / generation-windows` evidence must additionally show successful development-file
preparation, the workflow-selected Python version, and the original native build succeeding without another download.

The pull-request feature matrix validates the complete declared feature graph but compiles a curated
set of representative products and transports. It deliberately does not compile every bounded
pairwise combination and artifact recipe.
Renderer library and family-admission tests also run with all diagram families selected and either
no graph engine or only Shoal. The feature-unification checks compile the Core facade without
`editor-facts` while its parser crates enable it. They also exercise Rustdoc with graph engines
compiled by a sibling package and verify that the macro requires its own layout features.

## GitHub Pages and Playground

The Pages workflow owns the deployable Playground and browser integration evidence. It builds the
standard Web package group for package smoke, size reports, and the installed browser consumer.
The same producer builds an additional full WASM with `layout-shoal`. The Playground job assembles
that variant after qualifying the standard packages, verifies its provenance, and tests the
resulting site. Generated-source freshness remains owned by the central CI workflow for pull
requests. A main-branch or manual Pages deployment repeats the deployable browser projections.

Web and Playground WASM sizes are reported without byte ceilings. Package provenance, capability
identity, package smoke, and browser tests remain required. Typst publication owns its separate
artifact size budgets.
Playground engine and bootstrap bundles also have no build-time size caps; byte-length and digest
identity, CSP, export ownership and runtime transport resource limits remain enforced.

Pull requests build and test the site but do not deploy it. Main-branch runs upload the same tested
`playground/dist` directory to GitHub Pages. Chromium exercises the full browser suite; Firefox and
WebKit retain focused smoke coverage because browser-specific loading and worker failures are a
user-visible contract rather than a duplicate source-level test.

Mobile interaction coverage also checks shared links that initially show Preview: neither Code nor
Config mounts an editor before the workspace becomes visible. Opening Editor or widening the page
activates it; later workspace switches preserve its model and undo history. Desktop startup checks
keep the editor usable while preview WASM is blocked and retain staged language-worker activation.
Editor synchronization coverage preserves external replacement, empty documents, Unicode edits,
late mounting and undo/redo. Preview controls must invalidate old artifacts when any current
render input changes, while repeated debounce scheduling leaves an unchanged preview alone.
Theme workshop browser tests exercise preset selection, theme document editing, contrast
diagnostics, and preview updates.

JavaScript toolchains are pinned in their owning workflows. Linux Node native packages use
separate GNU and musl container baselines; update them only with native compatibility
qualification. A newer host Node.js or npm version does not itself change those runtime baselines.

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

Performance contract jobs use the pull request's merge revision so the checked-out Cargo features
match the workflow's build recipes even when the contributor branch predates a feature change.
Measurement jobs retain the actual base and head revisions, with lane descriptors loaded from the
head checkout. Manual contract runs honor the explicit head repository and revision inputs.

`python3 tools/bench/bench.py ci select` validates the `performance_workflow` registry in
`tools/bench/corpus.json` and writes the measurement matrix for GitHub Actions. The registry owns
lane recipes, pull-request labels, scheduled lanes, and manual dispatch groups. Each descriptor uses
the same base/head runner, receipt, artifact, summary, and outcome consumer.

Central CI runs `python3 -m unittest discover -s tools/bench` and
`node --test tools/bench/collectors/node_runner.test.mjs` when the classifier selects the performance
owner. The standalone Performance workflow also runs these tests on scheduled and manual runs. Its
standalone pull-request contracts require the performance selector or a `perf` or `perf-frontmatter`
label. For those pull requests, it verifies the compiled ASCII benchmark list. It also verifies the
broader native list and memory smoke contract on `perf` pull requests and on scheduled and manual
runs. Actionlint owns workflow syntax and expressions, and zizmor owns workflow security checks.

The weekly schedule runs regression and frontmatter self-comparisons. Manual `reference` and
`full` dispatches run the locked Node reference collector when the selected revision contains the
collector and its consumer lockfile. ASCII timing requires a manual dispatch with a compatible
benchmark-only base backport. Pull-request measurements use read-only repository permissions and
publish their artifacts and job summaries in the same run.

## Release Preflight

Release preflight runs against an immutable source revision and owns exhaustive or artifact-exact
evidence:

- the strict Cargo feature and artifact-recipe matrix;
- target-scoped dependency closures and legal reports;
- exact package contents, checksums, symbols, provenance, and installation smoke;
- full SDK and registry package assembly;
- release version and documentation projections.

The `cli-and-lsp-archives` job owns final CLI/LSP archive evidence on native x86_64 and ARM64
runners for both macOS and GNU Linux. It evaluates the full pinned cargo-dist plan with the
requested version without creating a tag, checks the selected target's runner/host and planned
assets, then builds and executes both final archives with adjacent-checksum verification.
Both macOS targets run the mixed-script PNG check. Both GNU runners check the glibc floor.
The GNU ARM64 runner also runs system resource probes.

Each target uploads its plan, build manifest, archives, and checksums as `preflight-cli-lsp-<target>`.
Keep the run and source SHA with any target-admission decision. This job replaces the former
x86_64-only archive build inside `versions-and-packages`. The tagged release workflow owns the
global installer, bundle, and attestation gates.

The Web size gate measures the final wasm-bindgen binaries copied into the npm packages. It must not
build and measure a second Cargo-only approximation of the same package.

The tagged crates workflow reuses the strict feature matrix and Web build/smoke only from a successful full preflight with an explicit immutable source input and the exact same workflow commit. The read-only verifier binds repository, source commit/tree/version, run/attempt, and the downloaded artifact's ID and digest. Missing or stale evidence runs the original checks; fresh registry resolution, static gates, Typst, and publication barriers remain mandatory. This is a narrow source-check optimization, not a general release authorization cache.

Ubuntu jobs use their runner's package sources and ordinary `apt-get update` / `apt-get install`
commands. Browser installers use their standard dependency installation. Merman does not rewrite
mirrors or install custom APT configuration. Dependency-installation steps retain their time limits,
and strict index updates stop on download failures. Containers use the pinned base image's sources.

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

The locked `cargo deny` graph explicitly includes `xtask/render-parity`, preserving its native
render and math coverage when default contract tooling does not link those features. Artifact
closure checks still resolve each target's own features separately.

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

Review ordinary Markdown for accuracy and working links. CI contracts validate the underlying
manifests, commands, and artifacts; ADR titles and numbering, fuzzing command examples, and
Homebrew documentation snippets are not machine inputs.

For local workflow checks, make ShellCheck available on `PATH` or pass its executable with
`actionlint -shellcheck <path>`. Without ShellCheck, actionlint skips the shell-script diagnostics
that the Linux CI runner enforces.

Workflow syntax and expression semantics are checked with actionlint 1.7.12. High-severity workflow
security findings are checked with zizmor 1.32.0. CI verifies the downloaded actionlint archive and
the selected zizmor wheel by SHA-256 before execution, and prints both tool versions. Repository
tests cover only Merman-specific boundaries: the same-run fail-closed gate, read-only PR owner
closure, trusted deployment separation, maintained action release tags, checkout credential
isolation, and npm provenance policy. GitHub Actions use current stable release tags, while actions
whose ref is part of their public interface retain readable tool or toolchain refs. The zizmor
configuration accepts version refs without disabling its other workflow-security audits, and weekly
Dependabot updates maintain the selected action versions. Release workflows also accept explicit Action release versions; moving branch names and floating major-only tags are not used for their Action dependencies. This policy concerns Action implementations, not release source commits, artifact checksums, or provenance receipts, which remain immutable.

Weekly Dependabot version updates use one group for GitHub Actions and one for the npm package
under `distribution/tree-sitter-mermaid/`. Each ecosystem permits one open version-update PR.
The npm group keeps related tooling such as `tree-sitter-cli` and `web-tree-sitter` in the same
review. Other npm workspaces are outside this configuration.

## Native theme checks

The preset verifier checks contrast, font stack portability, and perceptual differences across the
embedded appearances. The Rust contract tests compare each resolved paint and data color with the
same reference values.

```text
python3 scripts/verify_v2_presets.py --check-colors crates/merman-theme-contract/tests/fixtures/preset-colors-v2.json
cargo nextest run --locked -p merman-theme-contract --features presets --test theme_v2 --test theme_v2_decoding
```

The SVG parity job compares Mermaid-compatible output against the pinned upstream fixtures.
Family and export tests cover the emitted SVG and raster output.

## Playground build identity

The Playground keeps the WASM package version separate from its source build identity. Pages builds
show `Main build` and the checkout's short commit ID; pull-request builds show `Preview build`.
Tag builds are labeled as such without claiming that the tag has been published. The source link
opens the full commit in the workflow repository. CI fails if the checkout does not match
`GITHUB_SHA`, including detached pull-request merge checkouts.

Local builds show `Local build`, with a modified marker for uncommitted files. Source archives
without Git metadata omit the commit. Each production bundle includes `build-info.json` with the
same source identity shown in the footer and Merman comparison pane. Package versions used by the
runtime and benchmark protocols retain their existing meaning.

## Mobile device acceptance

Playwright mobile emulation covers automated interactions but cannot prove software-keyboard,
dynamic browser-chrome, safe-area, or native touch behavior. Before making a broad mobile-support
claim, check current iOS Safari and Android Chrome on real devices:

- Show the software keyboard with the caret near the editor's last visible line. Keep the caret,
  active line, dialog close controls, and primary actions reachable without horizontal page scroll.
- Exercise Examples, Benchmark, Share, and Export in portrait and landscape. Check safe areas,
  the home indicator, keyboard visibility, downloads, and focus returning to each dialog launcher.
- Rotate and collapse or expand browser chrome while editing and comparing. Keep toolbar and
  footer actions visible, both Compare panes reachable, and every canvas nonzero and fitted.
  Preserve the workspace, frozen render, export recipe, and Bounds preference.
- Exercise pinch and page zoom, text selection, canvas pan, Zoom, and Fit. Switching Infinite
  Canvas/ViewBox Frame must preserve the rendered artifact and camera behavior in both panes.
- Restore workspace and issue links. Workspace links restore render settings; issue links also
  restore the selected page, editor tab, preview/presentation mode, and Bounds preference. Neither
  serializes host dimensions or camera coordinates.

Record device, OS, browser version, orientation, and unresolved behavior in the PR or release
evidence. Real-device results remain separate from the automated browser lanes.
