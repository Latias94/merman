# Adding A Release Surface

Use this checklist when Merman gains a new public package, registry channel, generated artifact, or
installation path. A surface is public when a user can reasonably depend on it by name, import path,
package name, binary artifact, extension id, or documented install command.

## Decide The Surface Shape

Record the user-facing reason first:

- Who installs it?
- What can they do with it?
- What dependency weight does it carry compared with existing surfaces?
- Is it a package, a GitHub Release artifact, a manual registry flow, or a checked blocker?

Prefer extending an existing package with a documented subpath or feature when that actually reduces
user dependency weight. Add a new package only when it gives users a meaningfully smaller install,
separate host contract, separate registry policy, or clearer compatibility boundary.

## Update The Contract

Edit `docs/release/SURFACES.json` and add:

- a stable `id`;
- the exact `entry_point` users will type;
- `dependency_weight` and `capabilities`;
- every package manifest that owns the surface;
- every channel and its `declared_state`;
- docs paths and release gates.

Use these declared states:

- `published`: registry or install channel is intended to publish for this release kind.
- `artifact-only`: CI builds or uploads an artifact, but no registry package is published.
- `credential-blocked`: the registry path exists in design but lacks credentials or signing setup.
- `registry-blocked`: the package manager contract needs more design before publication.
- `manual-registry`: publication happens outside this repo through a manual registry PR or review.
- `not-built`: the surface is documented as not produced by current automation.
- `not-applicable`: the channel cannot apply to the selected release kind.

Run:

```bash
python scripts/release-status.py --view public
python scripts/verify-release-surfaces.py
```

For a release candidate, also run:

```bash
VERSION="<version>"
python scripts/release-status.py --version "$VERSION" --view maintainer
```

Add `--probe` only when you want best-effort network checks against npm, pub.dev, PyPI, crates.io,
or GitHub Release after publication.

## Update User Docs

Every new surface needs a package-choice explanation, not just a maintainer workflow note:

- `README.md` for first-contact users.
- `docs/FEATURES.md` when a feature flag or preset changes dependency weight.
- `docs/release/PACKAGE_SURFACES.md` for release readiness and package/subpath matrices.
- The package README closest to the entry point.

Avoid adding a new "analysis" or "full" alias just because the implementation has an internal
feature with that name. Public names should describe what a user can install or import.

## Update CI

The release surface verifier should fail until the new surface is represented from both sides:

- the contract declares it;
- the manifest or workflow exists;
- docs explain the user-facing choice;
- generated artifact source contracts are covered by the relevant build or prepack gate.

When adding a registry publish job, also add workflow-security tests before enabling credentials.
Marketplace and registry jobs must verify the artifact they publish, not just build something with a
matching file name.
