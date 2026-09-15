# Unreleased contract version audit

Checked on 2026-09-15, starting from `c306fa698`. This is a development decision, not a
release or a C7a freeze record. The release baseline is the latest artifact for each surface,
not the largest number found on the development branch.

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
| UniFFI binding API, `merman-uniffi` | 6 | Consolidate 9 to 7. ASCII selection fields and theme errors ship together. Regenerate Swift/Python with the native library. |
| Typst plugin ABI, `wasm-profiles.json` | 2 | Consolidate 4 to 3. Theme authoring and catalog exports are one unreleased change. |
| Web/WASM transport API | 5 | Keep 5; it was already published. |
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
| Preset catalog / preset recipe revision / public qualified cells | Absent | Keep 1. Empty cells remain empty unless qualified for the actual artifact. |
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
- Hash domain labels such as recipe `v4`, raster binding `v3`, cutover receipt `v2`, and
  historical projection `v2` separate byte encodings and bind existing independent evidence.
  They are not advertised format compatibility versions. Preserve the domains and digests;
  do not recompute proof expectations just to reduce a number.
- Plan sections C5/C6/C7a/C7b, ADR/KTD identifiers, dated verification records, upstream Mermaid
  versions and externally defined font/container versions are identifiers, not release counters.

Future public version changes follow the release guide: compare with the last published
contract, consolidate one unreleased compatibility change once, and leave a new contract at 1.
Source SHA, artifact digest and existing replay checks identify development iterations. Do not
add another version registry or migration framework for this policy.

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

These results are local validation, not installed-package coverage for every platform. Packed
Node, complete browser/WASM builds, final Typst package assembly, Android devices and the full
release archive/profile matrix remain part of C7a's candidate gate. Existing historical artifact
records retain their original source, profile and numbering and are not evidence for this tree.
