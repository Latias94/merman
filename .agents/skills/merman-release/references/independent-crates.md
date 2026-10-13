# Independent implementation crates

Read this branch before releasing a separately versioned implementation crate such as
`roughr-merman`. Use its manifest, owning workflow and registry history as version/tool authority.

1. Apply Cargo SemVer to the public API. A `0.y.z` patch preserves source compatibility; public
   breaking changes advance `y`. Removed methods, signature/default-feature changes and public
   dependency types need compatibility adapters if they ship on the same line.
2. Keep the workspace path dependency's version aligned with the candidate and use a compatible
   Cargo requirement. An exact incident pin needs a documented removal condition.
3. Compare the candidate API against the owner-selected compatibility floor with
   `cargo semver-checks check-release -p <package> --color always` and the workflow's pinned tool.
   Preserve an intentional compatibility-restoration baseline from a closed historical incident
   rather than treating that old incident as a new candidate failure.
4. Identify published stable and prerelease dependents whose requirements admit the candidate.
   Compile the newest dependent per distinct dependency/API contract from crates.io in fresh
   projects without copied lockfiles, first with the candidate and then after publication with
   ordinary registry resolution. Workspace path builds bypass the resolution being tested.
5. Run the focused crate tests and legal/package checks. Verify registry visibility and fresh
   dependent resolution after the authorized upload.

For incident recovery, publish and verify the compatible replacement before any separately
authorized yank. Record which dependents resolve the replacement; existing lockfiles can continue
using a yanked predecessor.

Done: the API comparison is compatible, required published-dependent lanes compile, workspace
dependency projections match, and post-upload registry checks confirm the replacement.
