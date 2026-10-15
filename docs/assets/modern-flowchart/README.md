# Modern Flowchart Comparison

These fixtures preserve the visual and routing evidence contributed through the former
`merman-modern` experiment. Alpha.4 removed that mixed presentation profile: the JSON file now
contains the explicit official Mermaid theme, look, and Flowchart renderer configuration. Typed
visual mechanisms from the contribution are tracked separately by the diagram-theme migration.

Build `merman-cli`, then render an image with:

```sh
target/debug/merman-cli render \
  docs/assets/modern-flowchart/02-orthogonal-routing.mmd \
  -c docs/assets/modern-flowchart/merman-modern.json \
  --format png \
  -o docs/assets/modern-flowchart/02-orthogonal-routing-after.png
```
