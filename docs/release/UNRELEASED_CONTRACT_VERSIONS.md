# Unreleased contract version audit

Checked on 2026-09-15 at candidate source `646ac9228`. This is a development decision, not a
release or a C7a freeze record. The release baseline is the latest artifact for each surface,
not the largest number found on the development branch.

The 2026-10-13 theme hash-domain follow-up below supersedes the earlier decision to retain
unpublished theme hash-domain counters. It compares the new domains with frozen main
`f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.

The maintainer selected `v0.8.0-alpha.7` as the next workspace release on 2026-09-15.
New theme schemas, recipes, catalogs and qualification formats remain version 1.
The package release number does not change those independent protocol identities.

## Published baseline

GitHub Releases and crates.io identify `v0.8.0-alpha.6`, published on 2026-09-02, as the
latest workspace prerelease. PyPI also contains `merman 0.8.0a6`. The immutable workspace
source has UniFFI API 6, Web/WASM API 5, C ABI 3, binding Options 2, ASCII output schema 2,
CLI contract 5, and Mermaid theme artifact schema 1.

Typst is independent. The official `typst/packages` registry contains versions 0.1.0,
0.2.0 and 0.3.0. The actual 0.3.0 WASM returns `2` from `abi_version()` when loaded by
Typst. Its SHA-256 is `6d32095f7be7156a2aaa3a544618c38e05c79790add6c0013dc4d63e421147ae`.
The branch's claim that published Typst 0.3.0 used ABI 3 was incorrect.

## Public version decisions

| Axis / owner | Published baseline | Unreleased decision |
| --- | --- | --- |
| UniFFI operation request record, `MermanOperationRequestV4` | V4 | Keep V4; this type name already occurs in the published API 6 source. |
| UniFFI binding API, `merman-uniffi` | 6 | Consolidate 9 to 7. ASCII selection fields and theme errors ship together. Regenerate Swift/Python with the native library. |
| Typst plugin ABI, `wasm-profiles.json` | 2 | Consolidate 4 to 3. Theme authoring and catalog exports are one unreleased change. |
| Web/WASM transport API | 5 | Advance once to 6 for the source theme branch's replacement of `presentationCatalog` with `themeCatalog` and its theme-authoring exports. |
| Native C ABI | 3 | Keep 3; it was already published. |
| Node transport / runtime catalog / binding result / operation metadata | 1 | Keep 1; no development-counter increase. |
| Binding Options | 2 | Keep candidate 3 for the incompatible compiled-theme grammar. |
| ASCII output | 2 | Keep candidate 3 for requested/effective layout and Compact-attempt metadata. |
| CLI contract | 5 | Keep candidate 6. |
| Mermaid theme artifact, `merman-core` / `xtask theme-snapshot` | 1 | Keep candidate 2. This existing artifact is distinct from the new compiled-theme format. |
| Theme definition / expansion / complete spec / materialized wire / authoring error | Absent | Keep first-release 1. |
| Theme support query and descriptor | Absent | One subject-based v1. Remove the earlier flat target/facet shape and its decoder; no V2 alias. |
| Renderer support claim revision | Absent | Consolidate 90 to 1. Git source identity distinguishes development builds. |
| Binding theme catalog | Absent | Consolidate 3 to 1, including Web, Flutter, Python and Node consumers. |
| Preset catalog / preset recipe revision / public qualified cells | Absent | Keep schemas at 1; consolidate recipe revisions 4/5 to 1 for all ten entries. Empty cells remain empty unless qualified for the actual artifact. |
| Preset qualification receipt and candidate record | Absent | Consolidate 3 to 1 and name the host profile `native-flowchart-state-sequence-system-fonts-v1`. Old build records require fresh execution and cannot be relabeled. |
| Theme execution evidence / text-layout request and projection contracts | Absent | Keep 1. |
| Artifact/resource descriptors and independently versioned package releases | Separate owners | Keep their published compatibility boundaries and existing version decisions; do not reset them merely because themes changed. |

Unknown support schemas remain rejected. Unknown subject identifiers remain bounded discovery
input and produce Unverified; version consolidation does not broaden admission. Explicit
negative tests reject both the unpublished flat query and a subject query labeled schema 2.

## Internal evidence identities retained

The following numbers do not count product releases. Replacing them all with 1 would identify
different historical observations or encodings as the same evidence:

- KTD17 authorization manifest revision 89 and KTD23 retirement manifest revision 9 identify
  migration batches. Retirement rows retain their original commit, batch and digest relations.
- C6 acceptance schema 4 consumes a frozen schema-3 predecessor and verifies the digest chain.
  The `c6-v3.json` and `c6-v4.json` filenames are historical ledger identities. Their 18-cell
  result must not be reissued under a new label without executing and reviewing that ledger.
- The reference fixture catalog, expectation and theme-input schemas at 2 belong to internal
  reference evidence. Their versioned parsing and recorded source inputs are not authoring
  APIs or public package versions.
- Archived recipe `v4`, raster binding `v3`, cutover receipt `v2`, and historical projection
  `v2` domains remain part of their original source and proof identities. Preserve those
  recorded digests. The live first-release theme domains follow the policy below; changing
  their domains requires fresh compiled fingerprints and execution evidence.
- Plan sections C5/C6/C7a/C7b, ADR/KTD identifiers, dated verification records, upstream Mermaid
  versions and externally defined font/container versions are identifiers, not release counters.

Future public version changes follow the release guide: compare with the last published
contract, consolidate one unreleased compatibility change once, and leave a new contract at 1.
Source SHA, artifact digest and existing replay checks identify development iterations. Do not
add another version registry or migration framework for this policy.

The 2026-10-11 theme-branch follow-up supersedes the original decision to keep Web API 5.
The exported WASM surface now removes the published `presentationCatalog` function. Reusing
epoch 5 would let an old wrapper initialize the new module and fail only when calling that
function. Epoch 6 is shared by the Rust producer and generated Web contract; initialization
rejects mixed epochs. This changes no theme schema or independently versioned native transport.

## Preset revision follow-up

A follow-up at `cad8b2051` found one implementation mismatch with this policy: the preset
entry factory still assigned recipe revision 4 to retained compatibility recipes and 5 to
native recipes, and its metadata test expected those values. That follow-up set all ten
unpublished recipes to revision 1 and removed development revision suffixes from fingerprint
constant names. At that source, hash values, the canonical hash domain, recipe contents and
qualification scope stayed unchanged. The 2026-10-13 follow-up below supersedes the decision
to retain the live canonical domain. Existing archive records keep their original source identity.

The follow-up checked theme contract and renderer version owners, binding projections,
qualification metadata, current theme plans and coverage documentation. Authoring, expansion,
complete spec, materialization, support, catalog, claim, text-layout and qualification versions
are already 1. Version 2 and higher in rejection fixtures remain intentional invalid input;
KTD/C6/reference evidence and archived hashes retain the identities listed above. The pre-existing
Mermaid theme artifact schema 2 is the successor of published schema 1, not an additional
version of the new compiled-theme format.

Validation for this follow-up: all 13 renderer preset tests passed, including the all-entry
metadata check, exact compiled fingerprints and complete-spec round trips. `cargo fmt --all
-- --check` and `git diff --check` passed. No platform artifacts were rebuilt for this change.

## Theme hash-domain follow-up

On 2026-10-13, the maintainer consolidated the remaining unpublished theme hash domains to
version 1. Frozen main `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f` contains none of these
namespaces; their higher suffixes identified development iterations rather than published
contracts. The live domains are now:

- `merman-theme-recipe-v1` for canonical compiled theme recipes.
- `merman-target-evidence-experimental-v1` for target-admission evidence.
- `merman-rendered-document-experimental-v1` for rendered-document evidence.

The canonical encoding and all resource, operation, capability, text, math, and terminal-proof
bindings remain intact. The ten preset catalog fingerprints are regenerated from the real
compiler and checked by `preset_catalog_fingerprints_match_the_exact_compiled_recipes`.
Document and target evidence likewise derive their digests from the current execution.
Old qualification and verification records keep their original source and digest identities;
they cannot be relabeled as evidence for these domains.

The theme wire contracts remain closed version 1 formats. The authoring registry recognizes
only `(authoring_schema_version = 1, expansion_version = 1, spec_schema_version = 1)`;
unknown tuples and schemas fail closed. There are no decoders, aliases, or upgrade adapters
for unpublished older theme versions. Version-rejection tests remain required input validation.

This decision does not reset the existing Mermaid runtime or audit artifacts, native text
measurement domain, Options grammar, or published transport and ABI namespaces. Their
compatibility histories are independent from the new theme contracts.

## Local validation

The consolidated working tree passed these scoped checks:

- Support wire and renderer discovery: 58 tests; shared binding theme paths: 64 tests.
  The added old-shape/schema rejection check passed in the five-test query subset.
- UniFFI, Typst and C ABI theme contracts: 14 tests. WASM's host-side theme operation test
  passed; the separately compiled C consumers passed both integration tests.
- Release preset qualification and Block/Class retirement: 10 tests, no skips. This
  reexecutes the exact three native recipes; it does not promote the public catalog.
- Web authoring/catalog: 10 tests and the TypeScript contract/type checks. Node API
  contracts: 40 tests. Qualification and release-bundle Python contracts: 30 tests.
- Rebuilt `apple-uniffi-native` macOS ARM64 library, generated Python projection, real
  Python/Swift render smokes, 29 Python contracts, and Dart ABI/catalog contracts passed.
  Python exercised all 22 support vectors and all three budgeted authoring operations
  through both one-shot and reusable consumers.
- Typst generated ABI constants, Rust formatting and `git diff --check` passed.

The version review also found that the merge had restored removed `presentation` and raw-CSS
examples in the Options guide. Restore the pre-merge compiled-theme sections while preserving
main's newer ASCII Auto and SVG-pipeline guidance; the guide must describe the actual Options 3
grammar, not just put a new number on an old example.

These results are local validation, not installed-package coverage for every platform. The
current candidate now also has installed Node N-API and Node WASM witnesses plus macOS ARM64
CLI/LSP archive replay; their source identities and commands are recorded in the transport
verification record. Linux/Windows hosts, Apple Swift 5.9, Android devices and the complete
release archive/profile matrix remain part of C7a's candidate gate. Existing historical artifact
records retain their original source, profile and numbering and are not evidence for this tree.
