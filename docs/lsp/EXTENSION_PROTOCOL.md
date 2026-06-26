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
        "ruleCatalog": "merman/ruleCatalog",
        "configSchema": "merman/configSchema"
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
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/syntax/flowchart.md",
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
    },
    {
      "id": "merman.authoring.config.prefer_frontmatter_config",
      "description": "Prefer diagram frontmatter `config` over Mermaid init directives.",
      "evidence": [
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md"
      ],
      "default_severity": "hint",
      "category": "config",
      "default_enabled": false,
      "default_profile": "recommended",
      "origin": "merman_authoring",
      "configurable": true,
      "fixable": true
    },
    {
      "id": "merman.compatibility.config.deprecated_flowchart_html_labels",
      "description": "Report deprecated `flowchart.htmlLabels` config and recommend the root-level `htmlLabels` option.",
      "evidence": [
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.type.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md"
      ],
      "default_severity": "warning",
      "category": "config",
      "default_enabled": true,
      "default_profile": "core",
      "origin": "mermaid_compatibility",
      "configurable": true,
      "fixable": false
    },
    {
      "id": "merman.compatibility.config.deprecated_external_diagram_loading",
      "description": "Report deprecated external diagram loading config and recommend `registerExternalDiagrams`.",
      "evidence": [
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.spec.ts"
      ],
      "default_severity": "warning",
      "category": "config",
      "default_enabled": true,
      "default_profile": "core",
      "origin": "mermaid_compatibility",
      "configurable": true,
      "fixable": false
    }
  ]
}
```

Rules use the same metadata vocabulary as CLI and binding catalog surfaces. Plugin authors should
filter `configurable == true` for settings UI, use `origin` and `evidence` when explaining rule
authority, and use `fixable` only as a hint that diagnostics from the rule may carry quickfix
metadata.

## `merman/configSchema`

Request params: none.

Response:

```json
{
  "version": 1,
  "rule_catalog_method": "merman/ruleCatalog",
  "accepted_roots": ["direct", "merman", "analysis"],
  "profiles": ["core", "recommended", "strict"],
  "severities": ["error", "warning", "info", "hint"],
  "configurable_rule_ids": [
    "merman.authoring.config.prefer_init_directive",
    "merman.authoring.config.prefer_frontmatter_config",
    "merman.authoring.flowchart.explicit_direction",
    "merman.compatibility.config.deprecated_flowchart_html_labels",
    "merman.compatibility.config.deprecated_external_diagram_loading"
  ],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Merman analysis options",
    "$defs": {
      "analysisOptions": {
        "type": "object",
        "properties": {
          "lint": {
            "type": "object",
            "properties": {
              "profile": {
                "type": "string",
                "enum": ["core", "recommended", "strict"]
              },
              "enable_rules": {
                "type": "array",
                "items": { "$ref": "#/$defs/ruleId" }
              },
              "disable_rules": {
                "type": "array",
                "items": { "$ref": "#/$defs/ruleId" }
              },
              "rule_severities": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["rule_id", "severity"]
                }
              }
            }
          }
        }
      }
    }
  }
}
```

The schema describes the same analysis options accepted by `initialize.initializationOptions` and
`workspace/didChangeConfiguration`: `lint`, `parse.suppress_errors`, `resources.max_source_bytes`,
`site_config`, `fixed_today`, and `fixed_local_offset_minutes`. It is intentionally permissive with
`additionalProperties` so alpha clients are not broken by future options. Clients should use it for
settings completion, settings validation hints, and profile/rule pickers, then use
`merman/ruleCatalog` for the richer rule explanations and evidence metadata.

## Standard LSP Pairing

- Diagnostics use standard `textDocument/publishDiagnostics`.
- Rule ids appear on Merman diagnostics and code actions through the shared analysis payload.
- Quickfixes use standard `textDocument/codeAction` and only exist when diagnostics carry explicit
  `DiagnosticFix` metadata.
- Runtime lint configuration should flow through initialization options or
  `workspace/didChangeConfiguration`; the server then republishes diagnostics and refreshes semantic
  tokens when the client advertises refresh support.
