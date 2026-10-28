# Text geometry comparison

Open `comparison.html` to view the same actual SVGs used in `comparison.png`.
The figure illustrates main / Mermaid 12.2.0. Maintenance behavior is verified
separately; the image is not a Mermaid 12.1 browser qualification.

The old CLI was built from clean commit `cba3033d8`. The repaired CLI was rebuilt
from `0c739e3b6ca43793c9fe7c2de23550bb88300f1f` and its SVGs match the figure byte for byte.
The corresponding maintenance source is `ad724572711bdc48a38f2ec6300637692243eb56`.
The repaired CLI uses the
same feature recipe, inputs, configuration and Arial regular/bold/italic font
files. The SVGs contain no embedded proprietary font files. No SVG geometry or
text was edited for the figure.

Build each revision with the existing CLI:

```text
cargo build --locked -p merman-cli --no-default-features --features diagram-architecture,diagram-xychart,layout-cytoscape,svg,native-fonts
```

Render each `.mmd` with its adjacent `.json` configuration using `render -c`,
`--font-file` for the three Arial files, and `-o` for the corresponding SVG.
The source fonts are supplied by macOS and are not redistributed here.
`provenance.json` records the source revision and artifact SHA-256 values.

The screenshot uses Chromium with a 1440px viewport and device scale factor 1,
after fonts and images have loaded. The HTML shows XYChart at 1.4x and
Architecture at 2.5x on both sides. Dashed image borders expose the exported
viewport; the screenshot adds no crop to simulate the old bugs.
