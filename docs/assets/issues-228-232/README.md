# Host-theme issue comparisons (maintenance)

Download and open [index.html](index.html) locally for the self-contained comparison; PNGs are embedded. Per-issue screenshots are captured from that HTML in Microsoft Edge at a 1600 px viewport. Original SVG/PNG bytes, exact options and inputs are retained under `before/`, `after/` and `inputs/`; `comparison.json` records their SHA-256 hashes.

- Before: `f6202f702dbde0a75049b1a62b3124feae709b3d`.
- After runtime source: `741a2e231d8962fbe3b621caeb0f4d5bd6dee4d2`. Documentation-only commits after this capture do not change it.
- Diagram input: the issue reproductions in `inputs/`, plus a Timeline source-scale control.
- Host preset: `editor-dark`; SVG pipeline: `resvg-safe`; requested root background: `#1e1f22`.
- Main uses `theme`; maintenance uses the released `presentation.theme` API.

## Reproduce the captures

From the repository root, use the matching checkout's comparison manifest and the fixed font fixtures:

```text
cargo run --locked --offline --manifest-path docs/assets/issues-228-232/Cargo.toml -- main docs/assets/issues-228-232/inputs target/issue-228-232-capture crates/merman-export/tests/fonts
```

Use `maintenance` in place of `main` in the maintenance checkout. `capture.rs` calls the public binding SVG/PNG API for diagrams and the public native font context for font probes. Dependencies use relative paths to this checkout, so the same manifest works on either branch. The font fixtures, licenses and derivation notes live in `crates/merman-export/tests/fonts/` and `crates/merman-render/tests/fixtures/fonts/`.

The font panels intentionally constrain the font database. Generic fallback loads the Symbol fixture before the complete Cousine text fixture; glyph fallback loads Noto Sans (a partial subset), Symbol, then Cousine. The selected painting faces are recorded in each font JSON. This reproduces both resolver failures with controlled fixtures on Windows; it does not claim a Linux Fontconfig integration run.

Gantt comparisons also retain the native usvg task-label fill receipt, since the public PNG API and its fallback SVG processing can use different text-paint paths. PNG pixels are not recolored or retouched. The HTML places transparent PNGs on the same dark host canvas on both sides; this does not alter the legacy root-background contract. Text measurement and installed-font painting can still differ, including existing clipping. These images demonstrate palette, layering and font-selection behavior rather than pixel equivalence with Mermaid.js.
