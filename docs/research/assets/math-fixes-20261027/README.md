# Actual native math exports

Open `comparison.html` to inspect the unmodified CLI PNGs. `comparison.png` is a
Chromium screenshot of that HTML. It shows the same input, white background and
drawing scale. No SVG paint, formula geometry or PNG pixels were edited.

- `release-0.9.2`: checksum-verified published macOS arm64 CLI, tag `8e303a43b`.
- `main-before`: PR head `0239c063bba12dee4c1b9555a96cad5ac6262b64`.
- `main-after`: source commit `3f0c6bf9f`, Mermaid 12.2.0.

`provenance.json` records exact argument lists and export SHA-256 hashes. Both
formats use `formula.mmd`, `--math-renderer ratex`, `--font-measurement native`
and `--background white`. PNG uses `--format png --output -`; SVG additionally
uses `--svg-pipeline resvg-safe`. SVG exports allow structural inspection; the
figure displays the direct Mermaid-to-PNG output, not a substitute raster path.

Maintenance is verified independently and keeps its existing Mermaid 12.1.0
contract. No package, release asset or tag is replaced by this patch.

The branch manifest now selects twelve current SVG exports from six inputs:
Flowchart, styled fractions, Sequence, Swimlane, CJK smash and emoji, each in
parity and native-safe modes. The original PNG overview remains bound to its
listed source commit; it is not regenerated as a substitute for these new exports.
`manifest.json` selects this branch's exports; `browser-results.json` records
Chromium 156.0.8078.4. Re-run the browser controls from the repository root:

```text
npm ci --prefix playground/tests
node docs/research/assets/math-fixes-20261027/check-browser.mjs
```

This inspects complete committed exports, checks that each formula contributes
visible pixels, requires each known fraction bar to contribute pixels independently,
and compares actual parity/native formula positions within one
CSS pixel of raster-edge quantization. It separately compares reassembled
isolated/contextual important paint screenshots byte for byte with a nonzero-ink
reference. That limited paint control is not a placement proof. To regenerate an export, render its
named `.mmd` using `--math-renderer ratex --format svg --svg-pipeline` and the
pipeline in the manifest. The separate Rust CLI tests rebuild and invoke the
actual current executable, including direct Mermaid-to-PNG.

`browser-mutations.json` records negative controls from an in-memory SVG-read
interceptor: remove native formula instances, hide real foreignObjects, or add a
20px translation around native instances. The fraction controls also delete or
hide only the Sequence fraction bar; both mutations must fail even while the
numerator and denominator remain visible. No tracked SVG or result file was
changed during those negative runs (`--check-only`). `fallback-bounds.json`
records main's post-repair containment observations, including the unresolved
host-text residual; maintenance retains its own historical before/after record.
`main-paint-identity.json` binds the unchanged full corpus to its prior
exact-byte root paint report.
