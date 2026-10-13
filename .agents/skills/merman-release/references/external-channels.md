# External package and deployment channels

Read this branch before changing an external package repository, submitting the Typst wrapper, or
deploying Pages. Registry credentials belong to
[Required Credentials](../../../../docs/release/RELEASING.md#required-credentials); the public
update procedure belongs to [External Package-Manager Updates](../../../../docs/release/RELEASING.md#external-package-manager-updates).
CLI artifact and installation checks belong to [Releasing CLI](../../../../docs/releasing/CLI.md).

## Observe the existing owner

Follow the linked public update procedure for bot updates, unaccepted initial-package PRs, current
CLA/review state, and platform-build failures. Read the current upstream contribution rules before
writing. Use an isolated checkout, preserve another contributor's history, and prepare the exact
patch and PR description before any required personal review.

When the maintainer reserves review replies for themselves, prepare the code, verification and
suggested response evidence, then leave comment publication and thread resolution to them.

Reconcile the owning feedstock/bucket/formula/manifests with the actual registry/catalog version.
For Nixpkgs, verify release source/NAR hash, Cargo.lock, evaluation/build and installation checks.
For Homebrew, inspect formula/bottles/API and tag-owned installed CLI checks. Repository health
gates can be stricter than the bot's basic package test; inspect support assets and capabilities.

## Typst Universe

Keep wrapper version/date, bundled runtime source, plugin ABI and minimum/current compiler support
distinct. Reuse the frozen selected WASM and inspect the final package's README, changelog, legal
material and exclude list. A documentation-only refresh can retain that runtime after the owner
verifies unchanged non-documentation files and records the final package bytes.

Run the existing package builder, interpreter and full package smoke at minimum and current
supported Typst versions. Submit the complete version directory once through Universe's PR process.
Describe user-visible rendering compatibility and line breaking with the actual upstream PR/issue
evidence; name the supported SVG/CSS behavior rather than claiming general CSS support.

Done: the submitted package and final commit match the retained receipt; upstream acceptance and
registry availability are reported independently from passing PR checks.

## Pages

Use an explicitly requested reviewed ref and bind the deployment to its approved 40-character
commit. A matching-path push to `main` or `master` also triggers deployment; inspect the workflow's
current triggers before pushing and confirm deployment belongs to the existing authorization.
Follow `.github/workflows/pages-deploy.yml` and identify its run by commit/event. Verify
both build and deploy, a successful `github-pages` deployment status, and a reachable deployment
URL for that same commit.

Done: every external channel is either available with owner evidence or submitted with a concrete
outstanding review, CI, or propagation condition.
