# System Font Query Feasibility

Date: 2026-10-10. Research only; production defaults, manifests and workspace
lockfile are unchanged. This is not performance admission or cross-platform
verification. The exploratory source, independent lockfile, ledger, harness and
raw samples are retained under ignored
`target/bench/experiments/system-font-query-2026-10-10/`.

## Question And Current Cost

Can system font lookup avoid reading every installed font before native
measurement? The existing Merman catalogue reads and hashes all candidates to
establish metadata and content identity before selecting actual snapshots.
The recorded Windows host admitted 541 files / 779,458,188 bytes. Its full CLI
Flowchart median was 1468.03 ms after the bounded-reader optimization. That
whole-render result is not an equivalent-operation baseline for this probe.

## Isolated Windows Probe

The independent Rust workspace uses font-kit 0.14.3, with default features off
and `source` on. Production Cargo files were not changed. The release executable
performs seven family queries and ten best-match requests, then reads/hashes
selected handles and checks A, V, U+4E2D and U+1F600 coverage with ttf-parser.
It does not shape, render or perform automatic character fallback.

Host: Windows 11 build 26100, Intel i7-11700, Rust 1.95.0,
x86_64-pc-windows-msvc. Three warmups precede 30 fresh processes with warm OS
caches. The fixed within-process case order warms later queries. This is one
diagnostic run, not balanced same-operation A/B or an optimization guarantee.

| Observation | Median, ms |
| --- | ---: |
| Entire diagnostic process including all cases and JSON capture | 117.98 |
| Entire Rust probe body | 80.92 |
| SystemSource initialization | 2.77 |
| Default cascade best match, after family queries | 3.51 |
| Arial bold best match | 4.07 |
| Fira Code best match | 2.98 |
| Missing family best match | 0.018 |
| Microsoft YaHei selected-file read / SHA-256 | 7.10 / 10.82 |
| Segoe UI Emoji selected-file read / SHA-256 | 4.99 / 6.83 |

Process p95 was 146.64 ms. All ten case identities (path, collection index,
content hash or missing result) were stable across 30 processes. The default
stack selected TREBUC.TTF; bold selected ARIALBD.TTF. YaHei supplied the Chinese
test glyph, and Segoe UI Emoji supplied the emoji test glyph. These explicit
candidates are coverage controls, not a general fallback implementation.

Even the diagnostic's ten requests are much narrower than reading the full
installed catalogue. However, its result cannot predict complete CLI time, cold
physical I/O, other hosts, or production memory/cancellation behavior.

Reproduction while this local exploratory workspace is retained:

```powershell
cargo build --release --manifest-path target/bench/experiments/system-font-query-2026-10-10/probe/Cargo.toml
pwsh -NoProfile -File target/bench/experiments/system-font-query-2026-10-10/measure.ps1
```

Do not cite ignored raw data as versioned release evidence. A later admitted
candidate needs a versioned implementation/harness and equivalent public-render
confirmation, including face identities and output controls.

## Source Findings

font-kit is useful for testing system lookup, but is not yet a production choice:

- `Source::select_best_match` calls `select_descriptions_in_family`, which loads
  each candidate with `Font::from_handle`. Not calling Handle::load explicitly
  does not imply metadata-only matching.
- Windows family queries return file handles and collection indices through
  DirectWrite; some source operations contain unwraps.
- The macOS family-to-handle path reads complete family files with slurp_file to
  recover collection indices. The probe's 64 MiB selected-read bound cannot
  constrain these internal reads or allocations.
- The Source interface does not supply a general character-level fallback API.
  Guessing a CJK/emoji family from this host would not be a correct replacement.
- Windows introduces dwrote/winapi and shared geometry dependencies. Other
  platforms have their own native library requirements. This is not simply a
  tiny path-index crate with identical behavior everywhere.

fontique was source-audited separately at Parley commit
`46fcad316d4ff05dc0d5647fda2d15def7be896e`, not built or timed:

- Windows enumerates family names, lazily resolves family data and uses
  DirectWrite MapCharacters for script-sample/locale fallback candidates.
- The audited macOS backend enumerates CoreText file paths and supplements them
  with directory scans, then uses a shared scan pipeline. A universal
  metadata-only/zero-scan claim would therefore be false.
- Script/locale fallback candidates still need actual glyph coverage checks;
  a sample character is not proof of coverage for every character in the script.

Primary source references:

- font-kit 0.14.3 `src/source.rs`, `src/sources/directwrite.rs`,
  `src/sources/core_text.rs`, `src/sources/fontconfig.rs` in servo/font-kit.
- The pinned Parley `fontique/src/backend/{dwrite,coretext,fontconfig}.rs` and
  `fontique/src/collection/{mod,query}.rs` in linebender/parley.

## Published Fontique Probe

A separate independent workspace now builds published **fontique 0.12.0**,
with default features disabled and `system` enabled. This is distinct from the
Parley main-branch source audit above. The workspace lockfile and production
dependencies remain unchanged.

On the same Windows host, three warmups and 30 fresh processes with warm OS
caches produced these diagnostic observations:

| Observation | Median, ms |
| --- | ---: |
| Entire process, including all queries, selected reads/hashes and JSON capture | 87.12 |
| Entire Rust probe body | 46.31 |
| Collection initialization | 1.83 |
| Trebuchet MS family metadata query | 1.07 |
| Arial family metadata query | 1.80 |
| Fira Code family metadata query | 1.12 |
| Microsoft YaHei family metadata query | 0.71 |
| Segoe UI Emoji family metadata query | 0.67 |

Process p95 was 96.97 ms. Family queries precede style/fallback queries in a
fixed order. Normal selections and the missing-family result were stable
across all 30 processes (path/index/SHA-256 or absence); this is not a complete
render benchmark or a balanced comparison against font-kit, whose probe runs
a different set of operations. Executable SHA-256:
`be5420fcef1c49d9b615a0ca8d617ac59b23f8336b9fbee376eb02588d8c1799`.

The published crate's Windows backend enumerates native family names on
initialization and resolves requested families lazily. However,
`FontInfo::from_source(SourceKind::Path)` maps the entire candidate file with
`memmap2::Mmap::map` while constructing family metadata. Mapping is not the same
as reading/hashing the entire file, but it is also not an application-controlled
metadata-only path lookup. Our explicit 64 MiB inspection limit does not bound
this internal mapping or its page faults.

The probe also exercises script/locale fallback (Latin and Han) and generic
sans-serif/emoji candidates. These APIs return candidates, not proof of every
input character's coverage. Merman would still need bounded byte admission,
glyph validation and consistent measurement/painting snapshots. The existing
usvg shaping engine is not replaced by this discovery experiment.

Reproduction and ignored raw evidence:

```powershell
cargo build --release --manifest-path target/bench/experiments/system-font-query-2026-10-10/fontique-probe/Cargo.toml
pwsh -NoProfile -File target/bench/experiments/system-font-query-2026-10-10/measure.ps1 -Executable fontique-probe/target/release/merman-fontique-query-probe.exe -Output fontique-samples.json
```

Published source: fontique 0.12.0 `src/font.rs`, `src/backend/dwrite.rs`
and `src/collection/mod.rs`. The extracted crate is
retained in the probe's `vendor-inspect/fontique-0.12.0/` directory. Its native
service calls are not covered by Merman cancellation/resource guarantees.

## Recommended Boundary And Next Decision

Follow-up source review found **no public Fontique 0.12.0 hook** that provides
system candidate path/index before internal metadata mapping, or injects a
controlled loader for that mapping. `CollectionOptions` controls sharing/system
access; `SourceCacheOptions` controls sharing, not acquisition policy. Calling
`FontInfo::source()` happens after metadata access. Using `register_fonts` with
bounded memory data does not discover the system candidate paths.

Consequently this published API is **not admitted as a drop-in bounded system
adapter**. A narrow upstream extension should separate candidate discovery from
metadata parsing, or provide a fallible loader policy covering metadata scans,
family parsing and source-cache access. Cancellation/budget errors must not be
converted to a cached missing-font success. Checkpoints can guard platform
calls but cannot promise preemptive interruption of native OS services.

The [isolated bridge plan](../plans/2026-10-10-system-font-query-candidate.md)
tests selected-family paths against existing public rendering without relaxing
production acquisition policy. Its experimental feature lives outside the
published workspace until the resource boundary is resolved.

Keep the measurement engine unchanged. A system lookup adapter should only
provide candidate path/index metadata. Application-controlled bounded reads,
SHA-256, explicit-source precedence and shared immutable snapshots must remain
the authority for actual measurement/painting bytes. Cache keys and provenance
must distinguish requested family, actual face, style and fallback policy.

System-native fallback can select differently from the current fontdb catalogue
order. This is a product-policy change unless equivalence is demonstrated, not a
pure same-output optimization. Missing names, duplicates, localized family names,
collections, variable fonts and mixed scripts require dedicated controls.
OS calls cannot automatically inherit Merman per-block cancellation/allocation
limits; do not claim strict enforcement inside the platform service.

Do not introduce required user configuration or persistent caching yet. First
prototype a bounded integration with explicit default/style/coverage controls
and actual public SVG/PNG/JPEG/PDF operations. Evaluate fontique's fallback and
source-cache ownership before choosing a dependency. Measure Linux and macOS
initialization separately. Only then choose runtime defaults or a cache design.
If a safe, affordable adapter is not available, opt-in system measurement remains
valid; compiling the feature need not imply automatically activating discovery.
