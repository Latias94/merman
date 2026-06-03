# HPD-050 Architecture Service Phase Join

Date: 2026-06-04

## Summary

Joined the new local `debug-architecture-delta` service contribution table with the existing
browser `debug-architecture-fcose-probe` final node bounds for the three active direct
Architecture group-width tails.

No code changed in this slice. The goal was to decompose the current `+5px`, `+5px`, and `+3px`
emitted group-width tails into child-content and final-expansion phases.

## Phase Join

For default-padding rows, local emitted group width is:

```text
local content union width + local group expansion width
```

The local group expansion is `85px` (`padding + 2.5px` per side), while the browser final
`node.boundingBox()` expansion over `childrenBoundingBoxIncludeLabels` is `83px`.

| fixture / group | browser children labels width | local content width | content dw | expansion dw | emitted group dw |
|---|---:|---:|---:|---:|---:|
| `batch5` / `pipeline` | `379.926` | `382.926` | `+3.000` | `+2.000` | `+5.000` |
| `html_titles` / `ui` | `316.926` | `319.926` | `+3.000` | `+2.000` | `+5.000` |
| `unicode` / `i` | `306.822` | `307.822` | `+1.000` | `+2.000` | `+3.000` |

The height side explains why a pure group-padding reduction is still rejected:

| fixture / group | browser children labels height | local content height | content dh | expansion dh | emitted group dh |
|---|---:|---:|---:|---:|---:|
| `batch5` / `pipeline` | `299.926` | `297.926` | `-2.000` | `+2.000` | `0.000` |
| `html_titles` / `ui` | `299.926` | `297.926` | `-2.000` | `+2.000` | `0.000` |
| `unicode` / `i` | `300.593` | `298.593` | `-2.000` | `+2.000` | `0.000` |

## Evidence Inputs

- Local emitted and service contribution reports:
  `target\compare\architecture-delta-service-contribution-hpd050`
- Browser/Cytoscape final node summaries:
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Representative local child contribution rows now visible in Markdown:

- `batch5` / `pipeline` / `storage`: local union `225x97`; browser final service bbox `223x100`.
- `html_titles` / `ui` / `web`: local union `129x97`; browser final service bbox `129x100`.
- `unicode` / `i` / `metrics`: local union `125x97`; browser final service bbox `123x100`.

## Conclusion

The remaining direct group-width tails are now split:

- `+2px` comes from local final group expansion being wider than browser final expansion.
- The remaining `+3px`, `+3px`, and `+1px` come from local child content union width.
- The same `+2px` expansion cancels a `-2px` local content-height gap, so changing group padding
  alone would reintroduce height residuals instead of solving the phase model.

The next useful seam is individual service label/content union width versus browser final service
`node.boundingBox()`, including service position drift and label width rounding. Do not try another
global group-padding or final rect emission change from these rows.
