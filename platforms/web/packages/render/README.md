# @mermanjs/web-render

This is the browser-only complete SVG rendering package. Its artifact contains `svg`, both
supported layout engines, and RaTeX math, but does not contain analysis, editor, or ASCII APIs.

Install it when an application needs complete Mermaid SVG rendering without the analysis, editor,
or ASCII workflows from `@mermanjs/web`:

```ts
import { initMerman, renderSvg } from "@mermanjs/web-render";

await initMerman();
const svg = renderSvg("flowchart TD\\n  A --> B");
```

The package participates in the same lockstep release, provenance, legal-material, declaration,
and lifecycle checks as the rest of the public browser package group. Its size is measured against
`@mermanjs/web`, but its product boundary is complete SVG capability rather than a 15% slim-workflow
threshold.
