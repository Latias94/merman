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

## Rule Policy Boundary

Merman lint configuration accepts only Merman-governed rule ids from the analysis rule catalog.
External rule ids such as `require-direction`, `duplicate-ids`, `no-empty-labels`, or
`no-orphan-nodes` remain external linter policy. Passing those ids through `lint.enable_rules`,
`lint.disable_rules`, or `lint.rule_severities` is invalid by design.

When an external tool wants to combine rule sets, keep two namespaces:

| External policy | Merman boundary |
| --- | --- |
| `require-direction` | Closest Merman rule is `merman.authoring.flowchart.explicit_direction`, but it is an opt-in Merman authoring hint, not the same rule id or authority claim. |
| `duplicate-ids` | Keep the external rule id for project-wide style policy. Merman only emits the specific parser/semantic diagnostics it owns, such as `merman.git_graph.duplicate_commit_id`. |
| `no-empty-labels`, `no-orphan-nodes`, project style packs | Keep these in the external linter. Do not mirror them into Merman config unless Merman later adds source-backed rules with new `merman.*` ids. |

If an external rule wants to reuse Merman parser evidence, keep the mapping in the adapter: run
`analyzeDocument()` with a Merman profile or explicit Merman rule ids, read Merman spans and related
locations, emit the external tool's own rule id from the external rule implementation, and attach
the original `merman.*` id as secondary metadata only when it helps debugging.

## VS Code Coexistence

When an external linter owns Problems output, users can keep Merman language features and suppress duplicate Merman diagnostics:

```json
{
  "merman.diagnostics.enabled": false
}
```

This does not disable completion, hover, symbols, references, rename, semantic tokens, preview, export, or source actions.
