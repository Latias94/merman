# v0.10.0-alpha.1 readiness review

Reviewed main source: `e9436275490eccc4b4a8cbb5dafde93d40090b19`, after PR #266. This is a development-readiness review and preparation patch, not a release declaration. The normal development version, release tags, registries, environments and public assets remain unchanged.

## Recommendation

Proceed with preparation of a scoped **v0.10.0-alpha.1** after merging the corrections identified here. The review found no runtime, theme-architecture or ABI defect requiring another broad refactor before an honestly scoped alpha. It did find incorrect migration instructions, incomplete capability disclosures and a Python publication-recovery defect; their corrections belong before the final source freeze.

The current development version remains `0.9.3`. This tree is not yet ready to tag as `v0.10.0-alpha.1`: the actual release still needs its version projection, dated package documentation, selected/deferred channel record and complete immutable `scope=release` preflight. External setup can delay individual channels without making the core alpha unsuitable.

## Evidence and scope

- Four independent reviews covered source/ABI compatibility, themes and host interaction, publishing/recovery, and the release documentation delta. They inspected the final main tree and published contracts rather than treating intermediate refactors as separate release features.
- Published maintenance `v0.9.3` is not an ancestor of main. Its merge base is `v0.9.0`. The documentation review compared the final maintenance and main trees, then classified main's integration history; it did not present maintenance fixes as newly introduced alpha features.
- Main's reviewed tree, `44045c9161598262cd67bef9d7ba3e87615e03b4`, equals the tree of `6361727a7d3c78911bb2910eda8f9fd9f03e7a43`. That commit's [PR CI](https://github.com/Latias94/merman/actions/runs/41310647755) completed with 68 successful jobs, four intentional skips and a successful `pr-gate`. Tree equality explains the applicability of that code evidence; it does not turn it into version-prepared release evidence.
- The [binding candidate](https://github.com/Latias94/merman/actions/runs/41308685750) qualified five Python wheel targets on minimum/current Python, seven native Node targets plus Node-WASM, five installed Web packages, an external Android Maven/JNI consumer on a 16 KB environment, and Flutter's complete native build and relocated macOS application. Its old Apple cleanup failure is superseded by the [successful Apple owner](https://github.com/Latias94/merman/actions/runs/41310644154), including local SwiftPM, iOS compile/link, Simulator, HTTPS resolution, checksum rejection and certificate revocation.
- In a separate disposable linked worktree, the existing version owner successfully projected `0.10.0-alpha.1`; all **136** version observations matched. Coupled Cargo dependencies use `=0.10.0-alpha.1`, npm selects `alpha`, and Python uses `0.10.0a1`. The normal development branch was not version-bumped.
- The projected candidate compiled through the existing prerelease checker from a fresh, lockfile-free Cargo consumer. The previous-facade lane was correctly skipped because `0.9.3` is outside the new `0.10` compatibility line. This is a dependency-resolution/compilation result, not proof that an arbitrary old application compiles unchanged.
- All 227 selected release/version/workflow/planner contracts, 19 Web discovery/descriptor tests and the TypeScript contract check passed. The changed workflow passed actionlint and high-severity zizmor. Python upload recovery additionally executes the real workflow step across 13 controlled local GitHub/download scenarios, and independent security review found no blocking issue; it does not perform a live upload. This review does not replace final-platform release consumers or establish a universal performance-regression result.

## Corrections from the review

| Finding | User or operator consequence | Correction |
| --- | --- | --- |
| A published upgrade guide contained unshipped theme/options/Web contracts | A maintenance upgrader could select APIs absent from the installed release | Restore the published guide's release boundary and link development migrations separately |
| The current Rust guide retained six removed render wrappers and `.semantic` fields | Following the guide produced unresolved types or fields | Use canonical family models and the complete `PresentationDiagram` handoff; retain actual presentation carriers |
| The Web preset example joined metadata IDs to syntax IDs | Git Graph and Quadrant Chart lost their family guidance despite successful detection | Join capability `diagram_type` to available detection `syntaxId` |
| Feature guides and recipe descriptions contradicted math-capable Web/native products | Users could choose the wrong artifact or unnecessarily build from source | Separate source-default math opt-in, ordinary prebuilt products and historical maintenance recipes |
| Release projections omitted geometry-budget and constrained-formula migrations | Binding users could retain rejected resource overrides or assume constrained formulas still execute | Name `max_geometry_json_bytes` and the policy change in the root and affected package changelogs |
| Python's full wheel upload could not resume a partially accepted upload | Retrying the original run collided with existing names and blocked its PyPI job | Compare all existing bytes before writes, upload missing original-run wheels, then anonymously compare every public wheel |
| npm bootstrap prose mixed a build-only subset with later publication/rebuild | New Node members could strand a partially published version or conflict with rebuilt bytes | Keep one held publishing run's exact group, or manually publish/reconcile the complete build-only group; preserve observation-only recovery |

The release skill and its prerelease/recovery references now carry the owner sequence, final-artifact boundaries and new bootstrap rules. Detailed parameters remain in the workflows and release guide.

## Product boundary suitable for this alpha

The main Rust API changes belong on the new 0.10 line: shared model/markup ownership, semantic versus presentation processing, prepared-source reuse, explicit fixes and compiled themes have migration guidance. Options JSON is schema 3 and the Web transport is API 8. Native C ABI 3, UniFFI API 7, Android JNI API 2 and Node transport 1 retain separate compatibility checks.

An unchanged ABI number does not make wrappers from different releases interchangeable. Complete maintained UniFFI wrappers reject missing required symbols; the other maintained bindings validate their schema/catalog boundary. Raw C ABI discovery negotiates layout and deliberately does not reject every compatible old C executable; applications interpreting options or metadata must also inspect the catalog and expected schema.

The theme feature is suitable as an experimental alpha with these declared limits:

- Eleven presets are discoverable and editable; family treatment distinguishes dedicated design, base adaptation and unreviewed families. Built-in qualification cells remain empty. Recipe availability does not certify every diagram/output combination.
- Brutalist hard shadows and Cyberpunk node effects require supported Classic Flowchart shapes. Selecting a recipe does not silently rewrite the diagram's look or layout.
- Ghibli has a dedicated Pie design. The maintained gallery demonstrates actual native exports, rather than promising all Modern Mermaid designs or pixel identity.
- Fonts resolve through the host. Successful theme application and useful best-effort output do not imply portable typography or qualified PNG/PDF output. Keep actual diagnostics and strict-policy behavior visible.
- Source features, prebuilt capabilities, encoders, font services and resource policies remain separate choices. In particular, a math-capable product still rejects formulas under constrained policy.

These limits should remain visible in the alpha announcement and installation guidance. Completing every family design, eliminating browser-owned font differences or restarting the paused performance-optimization program is not a prerequisite for this stated scope.

## Channel setup observed during review

Read-only observations on 2026-11-01 establish configuration/public visibility only, not future authenticated publication success:

| Channel | Observed state | Remaining condition |
| --- | --- | --- |
| Existing npm Web/Node roots | Public `latest` is 0.9.2 and `alpha` is 0.8.0-alpha.7 | Publish the chosen exact alpha group under `alpha`; verify every member and its provenance |
| New Node Linux ARM64 GNU/musl names | Anonymous package metadata returns 404 for both names | Confirm namespace permission and complete same-artifact first-publication bootstrap; configure and later verify their trusted publishers |
| npm environment hold | The environment has no required-reviewer rule | Configure an actual hold before choosing that bootstrap procedure, or use the documented complete-group manual procedure; do not assume dispatch will wait |
| PyPI and pub.dev | Public latest is 0.9.2; matching GitHub environments exist | Verify the selected release's publisher tuple at publication; environment existence is not a token-exchange test |
| Android signing | No `maven-central` environment is present | Establish an owned namespace and signing inputs before selecting formal Android asset publication; `publish=true` requires the signed bundle even if Central promotion is deferred |
| Apple public SwiftPM repository | Complete local/remote handoff is implemented; this review establishes no public delivery repository | Select and roll out the exact thin tree if remote repository installation is in scope; local SwiftPM asset delivery can proceed independently |
| Flutter native hosting | The owner requires a separate `flutter-v<version>` non-draft GitHub Release | Prepare that release for the fixed tag, upload/verify every target asset, then publish the frozen facade |
| Android Central | The owner prepares the standard signed bundle and does not submit it | Record Portal submission/promotion and public coordinate resolution separately, or defer this channel |

Public package observations came from the npm registry, PyPI and pub.dev APIs; environment configuration came from read-only GitHub APIs. No secret values, successful future authentication, namespace ownership or public installation were inferred.

## Remaining release preparation

1. Select the channels to ship and record each other channel as deferred with its observed public version. A core alpha does not require pretending that every ecosystem rollout is complete.
2. Project the selected version in a clean linked release worktree. Keep independent grammar, Roughr, Typst wrapper and editor versions on their own axes.
3. Consolidate and date the root and affected package release notes; verify install examples, wrapper/native pairing, generated CLI assets, legal projections and advisory paths for the final candidate. Preserve the historical maintenance sections.
4. Freeze the exact source commit/tree and run full `scope=release` preflight for `0.10.0-alpha.1`. A prior `scope=bindings` run or this disposable projection does not admit the release.
5. Resolve the selected channels' setup, then use their existing publication owners after separate release authorization. Keep the primary workspace and Flutter tags, Apple delivery commit and native producer identities distinct.
6. Verify public registry/download results per channel and update the release record and announcement. Retain original run artifacts for recovery; matching source alone does not guarantee identical rebuilt native archives.

The recommendation is to complete this bounded preparation and expose the alpha to early adopters, rather than introduce another architecture rewrite without a demonstrated defect. Formal release admission and public rollout remain pending until the steps above are performed.
