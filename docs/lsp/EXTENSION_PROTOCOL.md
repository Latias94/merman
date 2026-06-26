---
type: Skill Contract
status: active
---

# Merman LSP Extension Protocol

Merman LSP stays editor-agnostic. It does not ship VS Code, JetBrains, Neovim, or Web UI in this
layer. Plugin authors can build UI by combining standard LSP features with the custom requests
advertised under `ServerCapabilities.experimental.merman`.

## Discovery

During `initialize`, the server advertises:

```json
{
  "experimental": {
    "merman": {
      "schemaVersion": 1,
      "requests": {
        "ruleCatalog": "merman/ruleCatalog"
      }
    }
  }
}
```

Clients should feature-detect these fields instead of hard-coding extension availability.

## `merman/ruleCatalog`

Request params: none.

Response:

```json
{
  "version": 1,
  "rules": [
    {
      "id": "merman.authoring.flowchart.explicit_direction",
      "description": "Recommend explicit flowchart header directions and offer an insertion quickfix.",
      "evidence": [
        "repo-ref/mermaid/packages/mermaid/src/docs/syntax/flowchart.md",
        "crates/merman-core/src/diagrams/flowchart.rs",
        "docs/adr/0072-lint-rule-governance.md"
      ],
      "default_severity": "hint",
      "category": "semantic",
      "default_enabled": false,
      "default_profile": "recommended",
      "origin": "merman_authoring",
      "configurable": true,
      "fixable": true
    }
  ]
}
```

Rules use the same metadata vocabulary as CLI and binding catalog surfaces. Plugin authors should
filter `configurable == true` for settings UI, use `origin` and `evidence` when explaining rule
authority, and use `fixable` only as a hint that diagnostics from the rule may carry quickfix
metadata.

## Standard LSP Pairing

- Diagnostics use standard `textDocument/publishDiagnostics`.
- Rule ids appear on Merman diagnostics and code actions through the shared analysis payload.
- Quickfixes use standard `textDocument/codeAction` and only exist when diagnostics carry explicit
  `DiagnosticFix` metadata.
- Runtime lint configuration should flow through initialization options or
  `workspace/didChangeConfiguration`; the server then republishes diagnostics and refreshes semantic
  tokens when the client advertises refresh support.
