# merman-cli

CLI wrapper for `merman` (headless Mermaid) to parse/layout/render diagrams without a browser.

Baseline: Mermaid `@11.12.2` (upstream Mermaid is treated as the spec).

## Usage

```bash
# Detect diagram type
merman-cli detect path/to/diagram.mmd

# Parse → semantic JSON
merman-cli parse path/to/diagram.mmd --pretty

# Layout → layout JSON
merman-cli layout path/to/diagram.mmd --pretty

# Render SVG
merman-cli render path/to/diagram.mmd --out out.svg

# Render raster formats
merman-cli render --format png --out out.png path/to/diagram.mmd
merman-cli render --format jpg --out out.jpg path/to/diagram.mmd
merman-cli render --format pdf --out out.pdf path/to/diagram.mmd
```

Raster output is best-effort: pure-Rust rasterizers do not fully support SVG `<foreignObject>` HTML labels.
See `docs/rendering/RASTER_OUTPUT.md` in the repository.

