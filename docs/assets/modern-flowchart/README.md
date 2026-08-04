# Modern Flowchart Comparison

These fixtures and images exercise the `merman-modern` presentation profile. The JSON file contains only official Mermaid configuration; the profile flag selects Merman-owned presentation behavior independently.

Build `merman-cli`, then render an image with:

```sh
target/debug/merman-cli render \
  docs/assets/modern-flowchart/02-orthogonal-routing.mmd \
  --presentation-profile merman-modern \
  -c docs/assets/modern-flowchart/merman-modern.json \
  --format png \
  -o docs/assets/modern-flowchart/02-orthogonal-routing-after.png
```
