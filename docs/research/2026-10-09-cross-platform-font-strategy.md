# Cross-Platform Font Strategy

Date: 2026-10-09. Scope: native CLI measurement and explicit font sources. This is architectural research, not a browser-parity or cross-platform byte/pixel identity promise. **Verified** means inspected source; **Inference** means a design conclusion.

## First Principles

**Inference:** determinism and font accuracy are independent. A font-agnostic estimator can be stable without measuring a named font. A font-backed measurer can be stable when its full inputs are controlled. For closed-font runs, control font-byte hashes, collection face indices, duplicate-family precedence, ordered fallback, engine versions, size, weight/style/variation axes, script/direction/locale, spacing, and line breaking. Strict mode should reject missing fonts/glyphs or unsupported requests rather than silently produce heuristic geometry. Even then, use measured cross-target tolerances; do not promise unconditional byte/pixel identity.

**Verified:** Merman's built-in provider estimates character classes, Unicode display width, spacing, line height, and wrapping. Valid host results are authoritative, with per-request deterministic fallback; it does not claim named-font, shaping, baseline, or browser-bbox accuracy. Source: `docs/adr/0086-deterministic-text-measurement-without-vendored-font-tables.md:34`.

Useful policies are distinctions, not three new providers:

| Policy | Contract |
| --- | --- |
| Font-agnostic fallback | Portable estimate, not actual font geometry |
| Native system fonts | Match the native painter in this environment |
| Closed explicit fonts | Supplied bytes and controlled engine/config; reproducible selection |

## Platform Boundary

**Verified:** fontdb 0.23.0 scans predefined directories for system fonts, rather than using OS font APIs. It stores/matches faces but does not implement fallback. It accepts font files, directories, and binary data, including collection faces. Sources: fontdb `src/lib.rs:1`, `:195`, `:258`, `:304`, `:400`; https://docs.rs/fontdb/0.23.0/fontdb/struct.Database.html.

**Verified:** usvg provides a configurable `FontResolver`; default family selection uses fontdb queries and missing-character fallback searches the database. This is not native DirectWrite/CoreText fallback equivalence. Source: usvg `src/text/mod.rs:60`, `:85`, `:143`.

**Inference:** CLI needs one portable measurement implementation, not Windows/macOS/Linux measurement engines. Dependencies can own discovery-specific code. Add application platform code only for a demonstrated discovery gap or deliberate native-fallback feature.

## Stack Comparison

**Existing usvg/resvg (recommended first):** Merman already uses usvg with system-font/text support; export shares a system database and custom selection, but raster and PDF have separate resolver policies. Sources: `crates/merman-export/Cargo.toml:74`; `crates/merman-export/src/lib.rs:2737`, `:2764`, `:2781`, `:2793`. usvg lays out SVG text with rustybuzz and records shaped advances, font-derived ascent, positioning, and geometry; rustybuzz is shaping, not font discovery or whole multiline layout. Sources: usvg `src/text/layout.rs:400`, `:1406`, `:1480`; https://github.com/harfbuzz/rustybuzz/blob/v0.20.1/README.md. **Inference:** share this font context, resolvers, and layout/tree results with measurement and export. Direct rustybuzz measurement alone is not automatically equivalent to usvg SVG spacing/fallback/placement.

**COSMIC Text:** upstream describes HarfRust shaping, swash rendering, custom bidi/multiline layout, and custom fallback using browser fallback lists; it uses fontdb and supports Linux/macOS/Windows. Source: https://github.com/pop-os/cosmic-text/blob/main/README.md. **Inference:** good for an owned text widget, but measuring with it while usvg paints creates two stacks and does not provide Mermaid HTML/CSS parity.

**Parley/Fontique:** Parley 0.9.0 describes rich-text layout backed by HarfRust. Fontique selects DirectWrite, CoreText, fontconfig, or Android backends and provides script/language fallback; unsupported targets get a dummy backend. Sources: local `parley-0.9.0/README.md`, `fontique-0.9.0/src/fallback.rs:4`, `src/backend/mod.rs:6`; https://github.com/linebender/parley. **Inference:** appropriate if Merman later owns rich-text/native fallback; not needed merely to load custom files, and native fallback is not identical cross-platform selection.

**Typst:** current upstream separates embedded, system, and directory font providers; `FontStore` preserves order and lazily loads faces with `OnceLock`; system/directory discovery uses fontdb. Source: https://github.com/typst/typst/blob/main/crates/typst-kit/src/fonts.rs. **Inference:** borrow explicit sources, ordering, and lazy retention, not the document engine.

**Graphviz:** official FAQ describes fontconfig selection, Pango metrics determining node sizes, raster painting with installed fonts, and warns ordinary SVG can render elsewhere with different fonts. Source: https://graphviz.org/faq/font/ (modified 2026-06-23). **Inference:** borrow coherent measurement/painting ownership, not its native dependency set; SVG font portability is a separate artifact issue.

## Recommendation

Retain deterministic estimation as core fallback. First add an opt-in CLI font context using existing usvg. It should own supplied/discovered faces, generic-family defaults, family/missing-glyph resolvers, measurement cache, and provenance; measurement and native export consume the same context. Do not change defaults before evidence and compatibility review.

Explicit files/directories and disabling system discovery configure font sources; Mermaid `fontFamily` merely requests a family. Closed-font mode needs strict diagnostics and must not silently inherit ambient fonts or heuristic widths. Validate fixed licensed fonts across Windows/macOS/Linux: selected faces, advance versus ink bounds, mixed scripts, whitespace, multiline wrapping, missing glyphs, and painted containment. Keep estimator tests separate. Existing host-protocol and HTML-conversion gaps remain; see `docs/research/2026-10-09-native-font-measurement-feasibility.md`.

## Source Notes

Local crate paths are relative to `C:/Users/Administrator/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Installed versions are inspected sources, not latest-version claims. Upstream pages were fetched on 2026-10-09. No production code or cross-platform test was changed/executed for this note.
