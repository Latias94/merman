# Agent Instructions

## Mermaid Parity Strategy

For Mermaid parity work, prefer source-backed semantic and structural convergence over forced
pixel-perfect matching. Do not introduce brittle hacks, broad magic-number tuning, or model
distortions only to make a fixture match.

Prioritize:

- parser, model, layout, and render semantics from the pinned Mermaid source;
- stable SVG DOM structure and config/theme behavior;
- family-local evidence before main-matrix admission.

Treat browser-dependent behavior as a bounded residual unless there is a robust source-backed fix:

- text measurement;
- `getBBox()` floats;
- `foreignObject` and HTML labels;
- font rendering;
- D3 wrapper noise;
- RoughJS and hand-drawn output.

Comparator normalization must be narrow and non-semantic. Accepted residuals should be documented
rather than hidden.
