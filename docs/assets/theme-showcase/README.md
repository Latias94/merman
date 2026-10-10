# Theme comparison

These images are native Merman renders of the same Flowchart, Sequence, and XY Chart
sources with four theme selections: no compiled preset, Brutalist, Spotless, and Cyberpunk.
They are output examples, not screenshots of Modern Mermaid or a claim of pixel-identical
reproduction.

The sources are copied from the public Cyberpunk fixtures. Every render uses the same
explicit configuration: SVG text labels, classic Flowchart and Sequence looks, and Dagre
Flowchart layout. This keeps the presentation configuration fixed while the compiled recipe
changes. Typography and theme strokes may still affect measured geometry.

The catalog is deliberately scoped. A preset can be available in an artifact while only some
families have a dedicated design; `dedicated`, `base_only`, and missing family entries are
different support claims. Effects are applied only when the selected SVG/native target and
resource policy support them. Host fonts, browser text layout, and native rasterization can
still change geometry or pixels, and recipes do not embed fonts.

Reproduce from the repository root with the current all-feature CLI:

```text
cargo build --locked -p merman-cli --all-features
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --format png --output docs/assets/theme-showcase/default.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset brutalist --format png --output docs/assets/theme-showcase/brutalist.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset spotless --format png --output docs/assets/theme-showcase/spotless.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset cyberpunk --format png --output docs/assets/theme-showcase/cyberpunk.png
target/debug/merman-cli render docs/assets/theme-showcase/sequence.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset cyberpunk --format png --output docs/assets/theme-showcase/sequence-cyberpunk.png
target/debug/merman-cli render docs/assets/theme-showcase/xychart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset cyberpunk --format png --output docs/assets/theme-showcase/xychart-cyberpunk.png
```

Use `--format svg --svg-pipeline parity` and an `.svg` output path to reproduce the
corresponding SVGs. PNGs use the native exporter; fonts are resolved on the rendering
host. The PNG files make the PR preview independent of GitHub's SVG image restrictions.
