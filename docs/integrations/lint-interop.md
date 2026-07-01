# Lint Interop

Merman and Mermaid lint tools can be integrated, layered, or run independently.

## Boundaries

Merman owns:

- parser-backed Mermaid syntax and compatibility diagnostics;
- Merman-governed authoring recommendations;
- source descriptors for `.mmd`, Markdown, and MDX;
- document-relative spans, related locations, and fix edits;
- rule catalog metadata for Merman rules.

External lint tools should continue to own:

- repository file discovery and ignore policy;
- CI formatting and exit-code policy;
- Mermaid.js fallback when they need Mermaid runtime authority;
- broad style policy that is not source-backed Merman governance;
- integration with markdownlint, remark, textlint, or project-specific rule packs.

## Optional Web Analysis Integration

Use `@mermanjs/web` when a JavaScript linter wants Merman evidence:

```ts
import { analyzeDocument } from "@mermanjs/web";

const analysis = analyzeDocument(markdownSource, {
  lint: { profile: "recommended" }
}, "file:///workspace/README.md");
```

`analyzeDocument()` uses the URI extension to choose standalone Mermaid, Markdown, or MDX source modeling. Markdown and MDX diagnostics are remapped to host-document coordinates, including fix edits.

## Recommended Adapter Shape

1. Keep the external linter's file discovery and ignore rules.
2. Call `analyzeDocument()` only for candidate Mermaid files or documents with Mermaid fences.
3. Convert Merman diagnostics into the host linter's diagnostic format.
4. Preserve Merman rule ids under their `merman.*` namespace.
5. Layer external rules separately instead of remapping them into Merman rule ids.
6. Keep Mermaid.js fallback if your tool promises Mermaid runtime parity.

## VS Code Coexistence

When an external linter owns Problems output, users can keep Merman language features and suppress duplicate Merman diagnostics:

```json
{
  "merman.diagnostics.enabled": false
}
```

This does not disable completion, hover, symbols, references, rename, semantic tokens, preview, export, or source actions.
