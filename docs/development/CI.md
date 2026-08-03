# Continuous Integration

Merman separates pull-request feedback, scheduled maintenance, and release evidence. A check belongs
in the earliest layer that can justify its cost, but it should not be repeated in every later job
unless that job produces an independently releasable artifact.

## Pull Requests

Pull requests answer whether a change is safe to review and merge:

- formatting, repository hygiene, and generated-source freshness;
- broad workspace tests on supported hosts, with Linux owning the repository-wide parity and
  architecture contracts;
- representative Cargo feature leaves, default surfaces, owner APIs, and feature-unification
  regressions;
- Web package build, size budgets, package smoke, and Playground browser behavior when their inputs
  change;
- binding or package smoke in the workflow that owns that user surface.

The pull-request feature matrix validates the complete declared feature graph but compiles a curated
set of representative products and transports. It deliberately does not compile every bounded
pairwise combination and artifact recipe.

## GitHub Pages and Playground

The Pages workflow owns the deployable Playground and browser integration evidence. It builds the
Web package group once, then uses those exact package artifacts for size budgets, package smoke,
Playground preparation, and browser tests. Generated-source freshness remains owned by the central
CI workflow and is not repeated in Pages.

Pull requests build and test the site but do not deploy it. Main-branch runs upload the same tested
`playground/dist` directory to GitHub Pages. Chromium exercises the full browser suite; Firefox and
WebKit retain focused smoke coverage because browser-specific loading and worker failures are a
user-visible contract rather than a duplicate source-level test.

## Scheduled Maintenance

The repository has focused weekly schedules for fuzzing, security, performance, and Homebrew
compatibility. There is no umbrella daily `nightly` workflow. Scheduled checks answer questions
that need time, repeated observation, or external-state refresh rather than immediate merge
feedback.

`cargo-fuzz` uses a pinned Rust nightly toolchain because sanitizer-backed fuzz instrumentation
requires it. The workspace and release artifacts continue to use the pinned stable Rust toolchain.

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
