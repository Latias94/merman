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

The eight additional SVGs and four named Mermaid inputs cover Flowchart,
styled fractions, Sequence and Swimlane in parity and native-safe modes.
`manifest.json` selects this branch's exports; `browser-results.json` records
Chromium 156.0.8078.4. Re-run the browser controls from the repository root:

```text
npm ci --prefix playground/tests
node docs/research/assets/math-fixes-20261027/check-browser.mjs
```

This inspects the committed exports, checks primitive paint, captures actual
browser output and compares isolated/contextual important paint screenshots
byte for byte with a nonzero-ink reference. To regenerate an export, render its
named `.mmd` using `--math-renderer ratex --format svg --svg-pipeline` and the
pipeline in the manifest. The separate Rust CLI tests rebuild and invoke the
actual current executable, including direct Mermaid-to-PNG.
