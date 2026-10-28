# Rendering audit exports

`comparison.html` shows direct CLI PNG outputs for three maintenance before/after
controls. The corresponding SVGs remain available for structural inspection.
No SVG or PNG is edited to construct the comparison.

`before-provenance.json` binds the previously qualified maintenance binary and
its actual math exports. That binary does not include Block; no Block before
image is claimed. New full-feature exports and their source/binary hashes are
recorded separately after rebuilding. All commands read the adjacent `.mmd`
inputs, use `--math-renderer ratex --font-measurement native --background white`,
and select `resvg-safe` for SVG. PNG is direct Mermaid-to-PNG output.

The root audit document records the independent reviews, semantic regressions,
and validation boundaries. These artifacts do not make a performance claim.
