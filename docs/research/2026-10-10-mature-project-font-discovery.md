# Mature-project font discovery and reuse boundaries

Date: 2026-10-10. Status: source research, not production admission or a change
to Merman's defaults. No Cargo builds or production dependency edits were made
for this source-audit task. The companion [system-query feasibility report](2026-10-10-system-font-query-feasibility.md)
records isolated font-kit/Fontique probes performed separately, including their
measurement limitations.

## Revisions and scope

The locally cloned repositories were inspected at these exact revisions:

| Project | Revision | Local checkout |
| --- | --- | --- |
| Typst | `d7dfb34ae79689ed78857ab38a3f6a24a1747770` | `repo-ref/typst` |
| resvg/usvg | `77b12f4951a7b75d759c42e8c9497cff7c839c3c` | `repo-ref/resvg` |
| Parley/Fontique main | `46fcad316d4ff05dc0d5647fda2d15def7be896e` | `repo-ref/parley` |
| Published Fontique 0.12.0 | `6e1143dda817360dd3a2a64f9133c6e5dfabd881` from `.cargo_vcs_info.json` | `target/bench/experiments/system-font-query-2026-10-10/fontique-probe/vendor-inspect/fontique-0.12.0` |

The published Fontique source is a separate evidence set. Its `backend/dwrite.rs`,
`backend/coretext.rs`, `backend/fontconfig.rs`, `collection/mod.rs`, `font.rs`,
and `source_cache.rs` were compared with the Parley checkout: their contents
match after CRLF normalization. This is a check of those six files, not a claim
that the complete revisions or public APIs are identical. Published-source
links below use the release's own revision.

The fontdb implementation detail audit uses published **fontdb 0.23.0**, matching
Typst's dependency range. Its package records revision
`62cfd96671eba4debe73eeecf541b1a3a051d223`; local source is
`C:/Users/Administrator/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fontdb-0.23.0/src/lib.rs`.
resvg main currently specifies fontdb **0.24**; do not treat the 0.23 internal
audit as proof of all 0.24 internals. See [T3], [R3], [D1].

## Findings in brief

| Project | Reused crate | Discovery policy | Where laziness begins | Verified cache scope |
| --- | --- | --- | --- | --- |
| Typst CLI | fontdb, ttf-parser | Default system discovery; additional directories; optional embedded fonts | Entire font-store creation is lazy; after discovery, selected `Font` construction is lazy | `LazyLock` and per-slot `OnceLock`, retained in the current world |
| resvg CLI | fontdb; usvg for resolution and shaping | No font initialization for SVGs without SVG `text` nodes; otherwise system by default plus explicit files/directories | Whole initialization skipped for text-free SVG; selected shaping happens later | Database and resolver/layout caches are in-process |
| Parley | Fontique, read-fonts; platform APIs inside Fontique | System collection by default, platform-dependent enumeration; optional registered data/paths | Windows family records are lazy, but obtaining a family already maps candidate font files | Collection metadata and `SourceCache` are in-process; optional sharing across clones |

Sources: Typst [T1]-[T4]; resvg [R1]-[R3]; Parley/Fontique [P1]-[P7].

**The important distinction:** discovery, metadata parsing, font-data ownership,
and shaping are different stages. Lazy shaping/font objects do not prove that
discovery avoids every font file. Mapping a file is not equivalent to eagerly
reading or hashing its complete contents, but neither is it zero I/O. See
Typst's metadata construction [T2], fontdb's mmap/file paths [D1], and
Fontique's `FontInfo::from_source` [P3].

## Typst: reuse fontdb, then build its own font book

The checkout's current CLI no longer calls the historical `FontSearcher` API.
`SystemWorld` owns `LazyLock<FontStore>` whose initializer invokes
`discover_fonts`. `World::book` and `World::font` access it. Watch mode explicitly
forces discovery before its displayed compile timer for paged output. Thus an
internal compile-time measurement may exclude discovery; a process benchmark
must not. Sources and local paths: [T1], [T4].

`discover_fonts` adds system fonts unless `--ignore-system-fonts`, embedded fonts
unless disabled, and recursively scans each `--font-path` directory. The CLI
also supports `TYPST_FONT_PATHS`, `TYPST_IGNORE_SYSTEM_FONTS`, and a separate
embedded-font policy. This audited entry point advertises directories, not a
dedicated single-font-file flag. Source/local path: [T1].

`typst-kit::fonts::system` creates a fresh fontdb database and calls
`load_system_fonts`; Windows/macOS also add Adobe font locations. `with_db`
walks **every discovered face**, obtains its bytes using `with_face_data`, and
calls Typst's `FontInfo::new`. That constructor uses ttf-parser and enumerates
Unicode cmap coverage. This is a broad metadata/coverage pass, not merely asking
the OS for the currently requested CSS family. Sources/local paths: [T2], [T5].

With the enabled fontdb `memmap` feature, fontdb 0.23 maps a font file to parse
metadata, retaining a path source rather than necessarily retaining the mapping.
The later Typst metadata pass accesses file data again. Full file byte hashing
is not part of these audited discovery functions. `FontPath::load` later uses
`fs::read`, and `FontSlot` caches the resulting optional `Font` in `OnceLock`.
The slot cache avoids repeated selected-font construction in one world; it does
not erase the initial all-face discovery pass. Sources/local paths: [D1], [T2], [T3].

For Linux, fontdb 0.23's `fontconfig` feature uses **fontconfig-parser** to read
configuration and scan configured directories. It is not the same implementation
as Fontique's calls into the native Fontconfig library and must not be credited
with native Fontconfig's cached catalogue automatically. Source/local path: [D1].

No application-owned persistent font-index read/write path was found in these
audited Typst CLI/kit discovery functions. `FontStore` slots survive `SystemWorld`
reset because reset only resets files/time; this is in-process reuse, not a font
index carried into a fresh CLI process. Package caches and watch export caches
are not evidence for a font catalogue cache. Sources/local paths: [T1], [T2], [T4].

## resvg: avoid discovery for text-free input, offer explicit font sources

The actual resvg CLI first parses XML and checks for an SVG-namespace `text`
element. Only then does it invoke `load_fonts`. That function calls
`fontdb.load_system_fonts()` unless `--skip-system-fonts`, loads `--font-file`
inputs, scans `--font-dir` inputs, and sets configurable generic-family names.
Consequently explicit files do **not** automatically disable the system scan;
the skip flag is separate. Source/local path: [R1].

usvg's default font selector queries the supplied fontdb using CSS family/style/
weight/stretch and appends its serif fallback. Its default missing-character
resolver iterates the database, skips already-used faces, and checks character
coverage. This is not an OS-native per-character fallback service. The code also
checks style attributes; it does not require all three attributes to be equal
because its rejection condition combines mismatches with `&&`. Source/local
path: [R2].

`FontResolver` is a useful integration boundary: its selection callbacks receive
a mutable `Arc<fontdb::Database>` and can load fonts on demand. The library's
`Options` defaults to an empty database; global system scanning is CLI policy,
not an inherent requirement of the usvg library. Sources/local paths: [R2], [R4].

This checkout's usvg main uses **harfrust** and **skrifa**, with fontdb 0.24.
Merman's root `Cargo.toml` currently specifies resvg/usvg **0.47** and rustybuzz
**0.20.1**. This source audit of upstream main is not a claim that Merman's pinned
versions have the exact same code or dependency graph.
The architectural boundary is reusable, but upgrading Merman's shaping engine
is a separate decision, not necessary just to improve font discovery. The
shaping path accesses `fontdb.with_face_data` and turns positioned glyphs into
outlines for rendering. Sources/local paths: [R2], [R3].

No application-owned persistent font-index read/write was found in the audited
resvg CLI loading path or usvg resolver. `Arc<Database>` and layout caches are
in-memory state. This audit does not infer that OS filesystem caches are cold,
nor claim a repeated fresh CLI process gets no OS caching. Sources/local paths:
[R1], [R2], [R4].

## Parley/Fontique: platform discovery plus separate source ownership

Parley's `FontContext` directly contains a Fontique `Collection` and `SourceCache`,
and is intended as an application/thread resource. Fontique's default feature is
`system`; `CollectionOptions.system_fonts` defaults to true and can disable
system discovery. Registered byte blobs and explicit path loading are separate
interfaces. Sources/local paths: [P1], [P2].

Published Fontique 0.12.0 exposes the following platform differences:

- **Windows:** DirectWrite enumerates family names initially. Family metadata
  is cached lazily. However, `family()` visits the fonts in the requested family,
  and `FontInfo::from_dwrite` invokes `FontInfo::from_source`, which opens and
  mmaps each candidate file to parse cmap, attributes, and axes. Metadata lookup
  is not path-only/no-file-I/O. It avoids Merman's current full-catalogue byte
  hashing, but Merman's post-selection read limit would not bound this earlier
  crate-owned mapping stage. Sources/local paths: [P3], [P4].
- **macOS:** CoreText yields paths, extra Library/Fonts discovery includes hidden
  system UI fonts, and `ScannedCollection::from_paths` scans those files. The
  common scanner opens/mmaps files. Do not promise that using this crate removes
  the initial catalogue scan on every platform. Sources/local paths: [P5], [P7].
- **Linux/FreeBSD:** the backend uses native Fontconfig: configuration/font-set
  initialization, family-name enumeration, generic substitution/sorting, and
  later requested-family resolution. Family metadata still calls
  `FontInfo::from_source`. Native cache creation/staleness behavior belongs to
  Fontconfig; it is not a Fontique-owned persistent font-index file. This source
  audit does not establish warm/cold performance. Source/local path: [P6].

Fallback families are keyed by script/locale. Windows and macOS ask native APIs
using a representative script sample. Such a candidate is not proof that the
actual text, variation selectors, or emoji sequence is supported. Fontique's
`Query` additionally has its own ordering/loading policy: `matches_with` loads
blobs through `SourceCache`, and `set_fallbacks` has a documented source-level
Han fallback workaround. A Merman adapter using only family metadata must still
validate actual glyph coverage and preserve its own fallback contract.
Sources/local paths: [P2], [P4], [P5], [P8].

`SourceCache` is a `HashMap` of blobs/failures, with optional `Arc<Mutex<...>>`
sharing and generation pruning. Its file loader opens and mmaps a path; there
is no serialized persistent cache in this implementation. Calling
`FontInfo::load`, `SourceCache::get`, or query callbacks would transfer selected
byte ownership to that mapping strategy. Merman can instead obtain candidates
and explicitly read bounded, immutable snapshots for measurement and painting,
but this does not constrain earlier metadata mappings. Source/local path: [P3].

## Implications for Merman

These are design conclusions from the audited code, not measured production
results or an implementation commitment:

1. Reuse a discovery/matching crate rather than write three operating-system
   adapters. Fontique is the strongest next candidate when script/locale
   fallback is required; font-kit remains a simpler targeted-family alternative
   covered in the companion feasibility report. Neither is a drop-in browser.
2. Keep discovery, selected byte snapshots, shaping, and painting distinct.
   Retain Merman's cancellation, size budgets, and measurement/paint byte identity.
   Review crate-owned mapping before promising the same discovery resource bounds.
3. Do not switch from fontdb to Fontique solely because its font-data cache is
   lazy: macOS still scans, and Windows family lookup still parses files. The
   useful hypothesis is avoiding full-catalogue byte reads/hashes, not eliminating
   all system enumeration or all file access.
4. Explicit font files and closed font directories remain valuable for CI,
   containers, privacy, and reproducible output. Mature CLI designs have these
   controls; resvg also demonstrates that an explicit source and system policy
   should be separately specified rather than silently conflated.
5. No audited project supplies an application-level cross-process font catalogue
   cache that Merman can simply copy. First test platform-backed discovery plus
   bounded selected-byte loading. Consider a persistent cache only if measured
   startup still justifies invalidation/versioning/security complexity.
6. Benchmarks must include fresh-process discovery and separate system/explicit,
   Latin/CJK/emoji and fallback cases. Typst's watch timer and retained Parley
   contexts are not evidence that a new Merman process has free font discovery.

## Pinned primary sources

Each reference names its local path and the corresponding immutable upstream
source. These references support the specific implementation observations above,
not claims about a future release or untested platform latency.

- [T1] `repo-ref/typst/crates/typst-cli/src/fonts.rs` and `args.rs`: [discovery](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-cli/src/fonts.rs), [flags](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-cli/src/args.rs).
- [T2] `repo-ref/typst/crates/typst-kit/src/fonts.rs`: [font store, system/scan, with_db, selected load](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-kit/src/fonts.rs).
- [T3] `repo-ref/typst/Cargo.toml` and `crates/typst-kit/Cargo.toml`: [fontdb version](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/Cargo.toml), [scan features](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-kit/Cargo.toml).
- [T4] `repo-ref/typst/crates/typst-cli/src/world.rs` and `watch.rs`: [lazy discovery/reset](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-cli/src/world.rs), [watch timing](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-cli/src/watch.rs).
- [T5] `repo-ref/typst/crates/typst-library/src/text/font/info.rs`: [cmap coverage construction](https://github.com/typst/typst/blob/d7dfb34ae79689ed78857ab38a3f6a24a1747770/crates/typst-library/src/text/font/info.rs).
- [D1] Local fontdb 0.23 registry path recorded above, `src/lib.rs` and `Cargo.toml`: [fontdb file access/system/config discovery](https://github.com/RazrFalcon/fontdb/blob/62cfd96671eba4debe73eeecf541b1a3a051d223/src/lib.rs), [dependencies](https://github.com/RazrFalcon/fontdb/blob/62cfd96671eba4debe73eeecf541b1a3a051d223/Cargo.toml).
- [R1] `repo-ref/resvg/crates/resvg/src/main.rs`: [text gate and CLI load_fonts](https://github.com/linebender/resvg/blob/77b12f4951a7b75d759c42e8c9497cff7c839c3c/crates/resvg/src/main.rs).
- [R2] `repo-ref/resvg/crates/usvg/src/text/mod.rs` and `layout.rs`: [resolver/fallback](https://github.com/linebender/resvg/blob/77b12f4951a7b75d759c42e8c9497cff7c839c3c/crates/usvg/src/text/mod.rs), [shaping](https://github.com/linebender/resvg/blob/77b12f4951a7b75d759c42e8c9497cff7c839c3c/crates/usvg/src/text/layout.rs).
- [R3] `repo-ref/resvg/crates/usvg/Cargo.toml`: [dependency/feature boundaries](https://github.com/linebender/resvg/blob/77b12f4951a7b75d759c42e8c9497cff7c839c3c/crates/usvg/Cargo.toml).
- [R4] `repo-ref/resvg/crates/usvg/src/parser/options.rs`: [empty default database and resolver](https://github.com/linebender/resvg/blob/77b12f4951a7b75d759c42e8c9497cff7c839c3c/crates/usvg/src/parser/options.rs).
- [P1] `repo-ref/parley/parley/src/font.rs`: [FontContext](https://github.com/linebender/parley/blob/46fcad316d4ff05dc0d5647fda2d15def7be896e/parley/src/font.rs).
- [P2] Published Fontique directory recorded above, `src/collection/mod.rs` and `Cargo.toml`: [Collection policy/registration](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/collection/mod.rs), [system feature](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/Cargo.toml).
- [P3] Published Fontique, `src/font.rs` and `src/source_cache.rs`: [metadata/load](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/font.rs), [cache and mapping](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/source_cache.rs).
- [P4] Published Fontique, `src/backend/dwrite.rs`: [Windows enumeration/family/fallback](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/backend/dwrite.rs).
- [P5] Published Fontique, `src/backend/coretext.rs`: [Apple discovery/fallback](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/backend/coretext.rs).
- [P6] Published Fontique, `src/backend/fontconfig.rs`: [Fontconfig initialization/family](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/backend/fontconfig.rs).
- [P7] Published Fontique, `src/scan.rs`: [explicit/platform file scanner](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/scan.rs).
- [P8] Published Fontique, `src/collection/query.rs` and `src/fallback.rs`: [query/load ordering](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/collection/query.rs), [fallback key model](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/fontique/src/fallback.rs).
