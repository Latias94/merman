# Theme comparison

These images are native Merman renders of the same Flowchart source with four theme
selections: no compiled preset, Brutalist, Spotless, and Cyberpunk. They are output
examples, not screenshots of Modern Mermaid or a claim of pixel-identical reproduction.

The source is copied from the public Cyberpunk fixture. Every render uses the same
explicit configuration: SVG text labels, classic Flowchart look, and Dagre layout.
This keeps the presentation configuration fixed while the compiled recipe changes.
Typography and theme strokes may still affect the measured geometry.

Reproduce from the repository root with the current all-feature CLI:

```text
cargo build --locked -p merman-cli --all-features
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --format png --output docs/assets/theme-showcase/default.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset brutalist --format png --output docs/assets/theme-showcase/brutalist.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset spotless --format png --output docs/assets/theme-showcase/spotless.png
target/debug/merman-cli render docs/assets/theme-showcase/flowchart.mmd --config-file docs/assets/theme-showcase/render-config.json --theme-preset cyberpunk --format png --output docs/assets/theme-showcase/cyberpunk.png
```

Use `--format svg --svg-pipeline parity` and an `.svg` output path to reproduce the
corresponding SVGs. PNGs use the native exporter; fonts are resolved on the rendering
host. The PNG files make the PR preview independent of GitHub's SVG image restrictions.
