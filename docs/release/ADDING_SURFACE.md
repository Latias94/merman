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

## Record Ownership

Put each fact beside its natural owner instead of adding it to a central release database:

- define the stable package, binary, import path, or artifact name in its manifest or descriptor;
- add its exact artifact profile when the shipped closure differs from the source default;
- make the publishing workflow build, package, verify, and upload that owner’s artifact;
- add an owner-specific smoke or install test that proves the published boundary;
- document the delivery route in the closest package README and in `PACKAGE_SURFACES.md`.

Use direct registry or GitHub Release queries after publication. A planned, manual, or credential-gated
delivery path belongs in its owner documentation and workflow issue, not in a second status model.

## Update User Docs

Every new surface needs a package-choice explanation, not just a maintainer workflow note:

- `README.md` for first-contact users.
- `docs/FEATURES.md` when a feature flag, artifact profile, or dependency boundary changes.
- `docs/release/PACKAGE_SURFACES.md` for release readiness and package/subpath matrices.
- The package README closest to the entry point.

Avoid adding a new "analysis" or "full" alias just because the implementation has an internal
feature with that name. Public names should describe what a user can install or import.

## Update CI

The new surface must be represented from both sides:

- its manifest, descriptor, or artifact profile declares the boundary;
- the owning workflow exists and has security tests before credentials are enabled;
- docs explain the user-facing choice;
- generated artifact source contracts are covered by the relevant build, prepack, or installation gate.

When adding a registry publish job, also add workflow-security tests before enabling credentials.
Marketplace and registry jobs must verify the artifact they publish, not just build something with a
matching file name.
