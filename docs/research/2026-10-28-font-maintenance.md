# Font dependency maintenance and documentation lock repair

Reviewed on 2026-10-28 against published crate sources, registry release metadata,
the RustSec advisory database, and pinned upstream commits. This is a migration
decision, not a completed font backend migration or a performance result.

## Decision

Repair the independent issue-comparison lock now. Migrate the application-owned
font parsing surface and the SVG backend together in a later implementation
slice, then retire the PDF and math dependency paths through their actual owners.
Replacing only Merman's direct dependencies cannot remove either advisory from
the complete current dependency graph.

Do not expand advisory ignores, change audit policy, replace historical captures,
or adopt upstream Git heads solely to make an audit green. No production
workspace manifest, workspace lock, or export implementation changes belong to
this repair.

## CI failure and bounded repair

The supplied main run `40662958348`, job `122735777454`, checked revision
`0ce21f1cc66435a6650a4dfae2dca39a9028f50f`. Its blocking finding was
`pulldown-cmark 0.13.4` in `docs/assets/issues-228-232/Cargo.lock`.
[RUSTSEC-2026-0414][rs-markdown] describes exponential event growth with
`ENABLE_WIKILINKS`, including `Options::all()`, and specifies a fix in `>=0.13.5`.
The workspace already required and locked `0.13.5`; this separate `[workspace]`
documentation crate retained an older resolution.

`security-owner` calls `.github/workflows/security-audit.yml`. Its tracked-lock
matrix already discovers the comparison lock and runs `cargo audit --file`
against it. `.github/CODEOWNERS` assigns the CI trust roots to the maintainer.
Existing discovery and RustSec-ledger contracts cover this boundary; a new
version-specific test or workflow change would duplicate the audit owner.

Both the main and 0.9 maintenance comparison locks were updated using:

```text
cargo update --manifest-path docs/assets/issues-228-232/Cargo.toml -p pulldown-cmark --precise 0.13.5
cargo update --manifest-path docs/assets/issues-228-232/Cargo.toml -p json5 --precise 1.3.4 --offline
```

The second command aligns the required `json5` refresh with the current workspace
lock instead of retaining a newer incidental resolution. Cargo also reconciled
stale local package entries and dependencies with each checkout's current path
manifests. Main updates local package versions from `0.9.1` to `0.9.2` and the
current YAML/canonicalizer requirements. Maintenance additionally replaces its
stale `lol_html`/`selectors` closure with the current `html5ever` closure. These
are lock reconciliation changes, not manifest edits made by this task. Retaining
the old dependency edges would misrepresent the source used by `--locked`.

The comparison manifests, captured SVG/PNG/HTML/JSON files, workspace manifests,
workspace locks, and production source were not edited. No Rust compilation or
capture regeneration was performed.

`cargo-audit 0.22.2` verified the repaired locks with zero vulnerabilities and the
same two unmaintained warnings. The database fetched during this review had
1,381 advisories at commit `40f5be6fb12ed93fc591bf4ab23b7d793b3f2239`.
Main resolves 180 packages; maintenance resolves 179. The existing Python audit
and exception tests passed all 12 cases; main's audit discovery and exception
ledger checks passed. This is local evidence for the audit repair, not a claim
that the full remote CI run has been repeated.

## Advisory meaning and maintained replacements

| Package | Primary advisory | Classification and recommended replacement |
| --- | --- | --- |
| `rustybuzz 0.20.1` | [RUSTSEC-2026-0206][rs-shaping], dated 2026-07-11 | `informational = "unmaintained"`, no patched release; recommends HarfBuzz's `harfrust`. |
| `ttf-parser 0.25.1` | [RUSTSEC-2026-0192][rs-parser], dated 2026-06-28 | `informational = "unmaintained"`, no patched release; recommends Fontations' `skrifa`. |

The [rustybuzz maintainer's response][rb-maintainer] directs consumers to
HarfRust. The [ttf-parser maintainer's response][ttf-maintainer] says the library
is unmaintained. These advisories are maintenance findings; they do not assign
the Markdown vulnerability to either font package or prove a specific
memory-safety defect in Merman.

The replacement layers have different jobs:

- **HarfRust** shapes Unicode text into positioned glyphs. Its official
  [README][harfrust-readme] describes its origin as a rustybuzz fork moved onto
  `read-fonts`. It does not replace platform discovery, CSS family ranking,
  fallback policy, or SVG text layout.
- **Skrifa** supplies font metadata, metrics, variation-aware outlines and other
  higher-level font operations. **read-fonts** provides the underlying OpenType
  tables and collection parsing. Use these to replace `Face`, cmap, name and OS/2
  reads; replacing font parsing with a shaper would add an unnecessary layer.
  See the [published Skrifa source][skrifa-source] and
  [read-fonts parsing contract][read-fonts-readme].
- Parsing is lazy and malformed fields may produce errors/defaults. The
  application must retain its own admitted-byte, table-count, collection-count,
  cancellation and resource limits; a maintained parser is not an admission
  policy.

## Published support, checked on 2026-10-28

Registry metadata was checked rather than inferring versions from branch names.
Source manifests below come from the published `.crate` archives, whose
`.cargo_vcs_info.json` identifies their source commits.

| Owner | Latest stable release checked | Actual support and remaining boundary |
| --- | --- | --- |
| [resvg/usvg][resvg-registry] | `0.49.0`, published 2026-10-24 | Usvg uses `harfrust ^0.12.0`, `skrifa ^0.44`, `fontdb ^0.24`; neither old font crate is a declared dependency. The published `0.48.1` already has this stack. [Source][usvg-published] |
| [fontdb][fontdb-registry] | `0.24.0`, published 2026-07-29 | Removes the external `ttf-parser` dependency but **inlines a minimal subset from ttf-parser 0.25.1** for discovery metadata. This clears a Cargo package edge; it is not a complete replacement of the parser implementation. [Source][fontdb-inline] |
| [krilla][krilla-registry] | `0.8.2`, published 2026-06-04 | Published `simple-text` is optional and depends on rustybuzz. Default features enable it. Krilla already uses Skrifa for other font work. [Source][krilla-published] |
| [krilla-svg][krilla-svg-registry] | `0.8.1`, published 2026-06-04 | Depends on usvg/resvg `0.47` and fontdb `0.23`; its krilla dependency enables default features as well as `raster-images`. Top-level `default-features = false` does not disable defaults enabled by this edge. [Source][krilla-svg-published] |
| [ratex-svg][ratex-registry] / ratex-unicode-font | `0.1.15`, published 2026-10-16 | `embed-fonts -> standalone` enables `ab_glyph` and font-loader/Unicode support. Unicode-font directly requires `ttf-parser 0.25` and fontdb `0.23`. [SVG source][ratex-svg-published], [Unicode source][ratex-unicode-published] |
| [harfrust][harfrust-registry] | `0.15.0`, published 2026-10-22 | Uses read-fonts `0.46`; its current public API differs from `0.12`, including `Buffer`/`ShaperFont`. It is not a rename-only substitution for rustybuzz or usvg's `0.12` API. |
| [skrifa][skrifa-registry] / [read-fonts][read-fonts-registry] | `0.49.0` / `0.46.0`, published 2026-10-21 | Current font/scaler APIs differ from the Skrifa `0.44` family used by usvg. Select a coherent family for the migration instead of independently bumping every font crate. |

The [resvg release notes][resvg-changelog] identify `0.48` as the migration from
rustybuzz/ttf-parser to HarfRust/Skrifa. They explicitly allow rendering changes
and mention a possible binary-size increase. `0.49` also changes text anchoring,
RTL positioning, nested baseline inheritance, clipping and filters. No local
speed or size improvement has been measured here.

Upstream branch state is ahead of release state:

- Krilla main at [`631e4645c9cdde1bd0bab652b9bf443d603d535a`][krilla-head]
  moves `simple-text` to HarfRust, but its root manifest still selects
  usvg/resvg `0.47` and fontdb `0.23`. A Git switch alone would not retire the old
  SVG font closure; it would also introduce unrelated Git dependency ownership.
- RaTeX main at [`97a8f87696540641438eecd94e96e5feaef3c9aa`][ratex-head]
  still selects `ab_glyph` in standalone SVG and `ttf-parser 0.25` plus fontdb
  `0.23` in Unicode-font. A branch update is not a font migration.

## Ownership in Merman

The following paths are established from manifests, current lock entries, and
source reads. A workspace lock includes dev and optional dependencies; it is not
an assertion that every binary or target activates every edge.

| Surface | Current owner and required work |
| --- | --- |
| Native source admission and metadata | `merman-export/src/fonts.rs`, `fonts/{registry,system,platform}.rs` use `rustybuzz::ttf_parser`; `font_selection.rs` directly uses `ttf_parser`. Replace collection/index validation, name aliases, cmap snapshots, symbolic classification and coverage together. |
| Native measurement and painting | Production rustybuzz references in export use its parser re-export; native measurement already consumes usvg text layout. Preserve that shared shaping/layout authority instead of creating a second direct HarfRust measurement engine. |
| SVG/raster transitive ownership | `export -> usvg 0.47 -> rustybuzz/ttf-parser`; `usvg -> fontdb 0.23 -> ttf-parser`. Upgrade usvg/resvg coherently with application parsing. The independent comparison crate also pins both to `=0.47.0`. |
| PDF ownership | `export -> krilla-svg -> usvg/resvg 0.47, fontdb 0.23`; `krilla-svg -> krilla default/simple-text -> rustybuzz`. Updating only Merman's usvg would keep a second old SVG backend and incompatible usvg tree types at the PDF boundary. |
| Math outline ownership | `render/math-native -> ratex-svg/embed-fonts -> standalone -> ab_glyph -> owned_ttf_parser -> ttf-parser`; font-loader also depends on ab_glyph and Unicode-font. |
| Math discovery and emoji ownership | `ratex-unicode-font -> ttf-parser` for collections and bitmap strikes; `ratex-unicode-font -> fontdb 0.23` and native `system-fonts 0.1.2 -> fontdb 0.23`. Migrating only glyph outlines leaves these paths. |
| Test/example ownership | Export's primitive shaping tests/example and the facade's rustybuzz dev dependency must migrate too; otherwise tracked workspace locks still retain rustybuzz after production migration. |

The issue-comparison crate enables native fonts and PNG, not PDF or native math.
Its repaired lock's remaining font paths belong to export/usvg/fontdb. A
successful future removal from this smaller graph would not prove the complete
CLI/PDF/math artifacts are clean.

## Recommended implementation slices

1. **Application parsing and raster backend.** Move usvg/resvg together to
   `0.49.0`, replace application-owned parser calls with read-fonts/Skrifa, and
   port shaping test oracles to HarfRust. Start with the compatible
   `harfrust 0.12` / `skrifa 0.44` / `read-fonts 0.41` family used by published
   usvg; both font manifests agree on read-fonts `0.41`. Remove the direct
   rustybuzz dependency when all its re-export and test users are gone. Review
   the comparison manifest pins as an independent consumer, without overwriting
   historical screenshots.
2. **PDF bridge.** Prefer a maintained krilla-svg release that accepts the same
   usvg/fontdb types. If release timing blocks the migration, a narrowly owned
   krilla-svg port/fork is feasible: it consumes already positioned glyphs via
   `draw_glyphs`, so disable krilla's unused `simple-text` defaults at that
   dependency edge and port tree/glyph/variation APIs to the new usvg version.
   Verify text extraction, PDF embedding, subsetting, fallback, filters and
   raster/vector equivalence before adoption. Changing the top-level krilla
   feature alone is insufficient.
3. **Math producer.** Coordinate changes across ratex-svg, ratex-font-loader and
   ratex-unicode-font. Replace ab_glyph's face/outline/cache types with Skrifa
   operations and use read-fonts/Skrifa for collections, cmap, font metrics and
   emoji bitmap strikes. Upgrade or remove the native `system-fonts` discovery
   edge as part of this owner. Preserve KaTeX advances, face indices, outline
   bounds, zero-advance glyphs, emoji placement and provenance. Turning off
   `embed-fonts` to avoid the dependencies would remove existing behavior.
4. **Retirement.** Re-resolve all tracked locks, inspect reverse dependency
   trees including dev dependencies and every supported artifact profile, then
   remove each exception only when its package is absent from every affected
   closure. Review fontdb's inlined metadata parser separately; absence of the
   Cargo package does not establish removal of that code lineage.

| Alternative | Benefit | Cost and decision |
| --- | --- | --- |
| Upgrade only direct application dependencies | Small first change | Leaves usvg/PDF/math paths; useful only as an explicitly partial parsing migration. |
| Staged coherent migration above | Clear ownership, published SVG base, retained output contracts | Requires PDF and ratex owner work; recommended. |
| Switch everything to upstream Git main | May acquire individual unreleased fixes | Current heads still retain old paths; adds unrelated source/dependency policy work. Not sufficient. |
| Replace discovery/layout with a new complete text stack | Could eventually remove fontdb's inlined parser | Much larger semantics and platform scope; no need demonstrated for this audit repair. |

This repair also coordinates the existing two reasons in `deny.toml` and
`docs/security/RUSTSEC_EXCEPTIONS.json` on both main and maintenance. They now
state that **pinned usvg 0.47 and published krilla-svg 0.8.1 still resolve
rustybuzz, while those paths plus ratex 0.1.15 still resolve ttf-parser**.
Published resvg/usvg 0.48+ provides a partial migration path; the remaining
application, PDF, math and test paths require their owners' migrations.
`scripts/verify_rustsec_exceptions.py` checks exact ID/reason equality between
the two policy documents. Only the reason strings change: advisory IDs,
dependency paths, affected profiles, owners, review dates, deadlines and exit
conditions remain unchanged. This wording correction does not renew or expand
an exception. The complete font migration remains design research outside the
current text repair.

## Verification handoff

After the maintainer's current Cargo work finishes, run these serially from each
checkout. The first group does not compile:

```text
cargo audit --file docs/assets/issues-228-232/Cargo.lock
python3 -m unittest scripts/test_audit_plan.py scripts/test_verify_rustsec_exceptions.py
python3 scripts/audit_plan.py
python3 scripts/verify_rustsec_exceptions.py
cargo metadata --locked --offline --format-version 1 --manifest-path docs/assets/issues-228-232/Cargo.toml > /tmp/merman-issue-comparison-metadata.json
```

An offline `cargo tree` probe during this task stopped because the incidental
`json5 1.3.6` download was absent; the lock was subsequently aligned to `1.3.4`.
No final metadata/tree success or Rust test execution is claimed. Cache misses
should be handled separately from compilation, with the configured proxy if
needed.

When disk and build capacity permit, an optional compilation check for this
lock-only repair is:

```text
cargo check --locked --manifest-path docs/assets/issues-228-232/Cargo.toml
```

Do not run the capture program merely to validate a dependency repair. A future
font migration should instead run the existing native font primitives, native
font/context, raster/PDF, Unicode math and artifact closure tests serially with
nextest. Inspect their current feature requirements at implementation time.

## Separate historical CI observations

Read-only inspection of Fuzz run `40479759415` (2026-10-27) and Performance run
`40294905358` (2026-10-26), both at
`8eba295103d406e320fad3bee448fe54d717cbf3`, found:

- Parser discovery job `122106813378`: oracle validation rejects the scheduled
  `--cases 5000` request with `Invalid batch size`; no parser mismatch is
  established by that failure.
- SVG fuzz job `122106851050`: `fuzz/src/lib.rs:102` asserts that resvg-safe
  output is terminally normalized. Malformed CSS with an unfinished escape
  changes trailing braces between passes and violates idempotence.
- Parser fuzz job `122106851258`: `parse_equivalence.rs:29` compares `gitGraph`
  against `error` for an input containing `branch develoInfinity%%{`.
- Performance jobs `121478598266` and `121478598440`: head benchmark compilation
  and benchmark enumeration complete, followed by `contract_failure (exit 2)`
  before any timing comparison is logged. The logs do not identify the exact
  contract field; that requires the comparison artifact. This evidence does
  not establish a measured performance regression.

No Fuzz or Performance implementation, thresholds or policy was changed here.

[rs-markdown]: https://github.com/RustSec/advisory-db/blob/40f5be6fb12ed93fc591bf4ab23b7d793b3f2239/crates/pulldown-cmark/RUSTSEC-2026-0414.md
[rs-shaping]: https://github.com/RustSec/advisory-db/blob/40f5be6fb12ed93fc591bf4ab23b7d793b3f2239/crates/rustybuzz/RUSTSEC-2026-0206.md
[rs-parser]: https://github.com/RustSec/advisory-db/blob/40f5be6fb12ed93fc591bf4ab23b7d793b3f2239/crates/ttf-parser/RUSTSEC-2026-0192.md
[rb-maintainer]: https://github.com/harfbuzz/rustybuzz/issues/166#issuecomment-4949166696
[ttf-maintainer]: https://github.com/harfbuzz/ttf-parser/issues/217#issuecomment-4770909313
[harfrust-readme]: https://github.com/harfbuzz/harfrust/blob/71d965130ea93905f81b8f03a03d7ecc794fbc47/README.md
[skrifa-source]: https://github.com/googlefonts/fontations/blob/47a301615fd95acf0da9fb7dbcd0ea7c7df36943/skrifa/src/lib.rs
[read-fonts-readme]: https://github.com/googlefonts/fontations/blob/47a301615fd95acf0da9fb7dbcd0ea7c7df36943/read-fonts/README.md
[resvg-registry]: https://crates.io/api/v1/crates/resvg
[fontdb-registry]: https://crates.io/api/v1/crates/fontdb
[krilla-registry]: https://crates.io/api/v1/crates/krilla
[krilla-svg-registry]: https://crates.io/api/v1/crates/krilla-svg
[ratex-registry]: https://crates.io/api/v1/crates/ratex-svg
[harfrust-registry]: https://crates.io/api/v1/crates/harfrust
[skrifa-registry]: https://crates.io/api/v1/crates/skrifa
[read-fonts-registry]: https://crates.io/api/v1/crates/read-fonts
[usvg-published]: https://github.com/linebender/resvg/blob/66ee0dc7d787d7e0b0ab3085b7849f78838cf2ba/crates/usvg/Cargo.toml
[fontdb-inline]: https://github.com/RazrFalcon/fontdb/blob/ecb707e6096b59146b750f66a32c125a43515192/src/ttf_parser/mod.rs
[krilla-published]: https://github.com/LaurenzV/krilla/blob/3ffdf0588cf98050aad6edba51ca70162e1fb5b5/crates/krilla/Cargo.toml
[krilla-svg-published]: https://github.com/LaurenzV/krilla/blob/3ffdf0588cf98050aad6edba51ca70162e1fb5b5/crates/krilla-svg/Cargo.toml
[ratex-svg-published]: https://github.com/erweixin/RaTeX/blob/7d9f9a39218914bead8ffdf39b79028a01d12c22/crates/ratex-svg/Cargo.toml
[ratex-unicode-published]: https://github.com/erweixin/RaTeX/blob/7d9f9a39218914bead8ffdf39b79028a01d12c22/crates/ratex-unicode-font/Cargo.toml
[resvg-changelog]: https://github.com/linebender/resvg/blob/v0.49.0/CHANGELOG.md
[krilla-head]: https://github.com/LaurenzV/krilla/blob/631e4645c9cdde1bd0bab652b9bf443d603d535a/Cargo.toml
[ratex-head]: https://github.com/erweixin/RaTeX/tree/97a8f87696540641438eecd94e96e5feaef3c9aa
