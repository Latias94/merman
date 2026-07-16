# Modern Flowchart Comparison

These fixtures and images compare `origin/main` with the `merman-modern` flowchart work. Both
revisions render the same Mermaid source and JSON configuration with the vendored text measurer.

Build `merman-cli` in each revision, then render an image with:

```sh
target/debug/merman-cli \
  -i docs/assets/modern-flowchart/02-orthogonal-routing.mmd \
  -c docs/assets/modern-flowchart/merman-modern.json \
  -o docs/assets/modern-flowchart/02-orthogonal-routing-after.png
```
