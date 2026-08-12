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
      "diagramSupport": {
        "families": [
          {
            "diagramType": "flowchart-v2",
            "semanticParser": true,
            "renderParser": true
          }
        ]
      },
      "requests": {
        "ruleCatalog": "merman/ruleCatalog",
        "configSchema": "merman/configSchema"
      }
    }
  }
}
```

Clients should feature-detect these fields instead of hard-coding extension availability.

Each `families` entry reports the canonical `diagramType` plus separate semantic-parser and
render-parser availability. Every parser-capable Merman build reports the same complete pinned
Mermaid language catalog; layout and output backends remain separate product capabilities. The
family list describes that language catalog, not files currently open in the workspace.

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
        "docs/adr/0072-lint-rule-governance.md"
      ],
      "default_severity": "hint",
      "category": "semantic",
      "tags": [],
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
      "tags": [],
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
      "tags": ["deprecated"],
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
      "tags": ["deprecated"],
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
metadata. `tags` is optional additive schema-1 metadata; clients must treat a missing field as an
empty list. Current deprecation metadata is emitted explicitly as `"deprecated"` rather than
inferred from rule IDs or descriptions.

## `merman/configSchema`

Request params: none.

Response. The JSON below is abbreviated; implementations return the complete
`configurable_rule_ids` list from `merman/ruleCatalog` entries where `configurable == true`.

```json
{
  "version": 2,
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
  "constraints": {
    "version": 1,
    "settings": [
      {
        "path": "fixed_today",
        "change_scope": "snapshot_affecting",
        "runtime_constraints": [
          { "kind": "canonical_civil_date" },
          {
            "kind": "representable_local_midnight",
            "offset_setting_path": "fixed_local_offset_minutes"
          }
        ],
        "normalization": {
          "kind": "string",
          "pattern": "^(?:...)-\\d{2}-\\d{2}$"
        }
      },
      {
        "path": "fixed_local_offset_minutes",
        "change_scope": "snapshot_affecting",
        "runtime_constraints": [],
        "normalization": {
          "kind": "integer",
          "minimum": -1439,
          "maximum": 1439
        }
      },
      {
        "path": "site_config",
        "change_scope": "snapshot_affecting",
        "runtime_constraints": [],
        "normalization": { "kind": "object" }
      },
      {
        "path": "resources.limits.max_source_bytes",
        "change_scope": "snapshot_affecting",
        "runtime_constraints": [],
        "normalization": {
          "kind": "integer",
          "minimum": 1,
          "maximum": 4294967295
        }
      },
      {
        "path": "resources.limits.max_document_diagrams",
        "change_scope": "snapshot_affecting",
        "runtime_constraints": [],
        "normalization": {
          "kind": "integer",
          "minimum": 0,
          "maximum": 4294967295
        }
      },
      {
        "path": "lint.profile",
        "change_scope": "diagnostics_only",
        "runtime_constraints": [],
        "normalization": {
          "kind": "string",
          "values": "profiles"
        }
      },
      {
        "path": "lint.enable_rules",
        "change_scope": "diagnostics_only",
        "runtime_constraints": [],
        "normalization": { "kind": "rule_id_list" }
      },
      {
        "path": "lint.disable_rules",
        "change_scope": "diagnostics_only",
        "runtime_constraints": [],
        "normalization": { "kind": "rule_id_list" }
      },
      {
        "path": "lint.rule_severities",
        "change_scope": "diagnostics_only",
        "runtime_constraints": [],
        "normalization": {
          "kind": "rule_severity_overrides",
          "fields": [
            {
              "name": "rule_id",
              "required": true,
              "normalization": {
                "kind": "string",
                "values": "configurable_rule_ids"
              }
            },
            {
              "name": "severity",
              "required": true,
              "normalization": {
                "kind": "string",
                "values": "severities"
              }
            }
          ]
        }
      }
    ]
  },
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
                "type": ["string", "null"],
                "enum": [null, "core", "recommended", "strict"]
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
    },
    "oneOf": [
      { "description": "Direct analysis options" },
      { "description": "Exactly one merman wrapper" },
      { "description": "Exactly one analysis wrapper" }
    ]
  }
}
```

The three root shapes are mutually exclusive. Unknown root and `lint` fields are
forward-compatible, while `resources` and its limit IDs are strict. The schema is projected by
`merman-analysis`; the LSP adapter adds only host defaults. Calendar validity and the
fixed-date/fixed-offset representable-instant check remain named runtime constraints because a
standard JSON Schema pattern is not a second civil-date parser.

Editor clients should treat `schema` as an opaque standards-based inspection surface. Static
settings manifests and runtime normalization consume the versioned `constraints` DTO instead of
depending on JSON Schema implementation details such as `$defs`, `$ref`, or `allOf` layout. The
abbreviated pattern above stands in for the complete analysis-owned value returned by the server.
Each setting carries its invalidation scope, runtime constraints, and typed normalization metadata;
object-list normalizers also carry their owner-defined field names, requiredness, and catalog
references. A bundled settings manifest may expose the baseline catalogs as open `examples`, but it
must not use them as a closed `enum`: the connected server's negotiated catalogs remain authoritative
and may add profiles, severities, or configurable rules.

The schema describes the same analysis options accepted by `initialize.initializationOptions` and
`workspace/didChangeConfiguration`: `lint`, `resources.limits.max_source_bytes`,
`resources.limits.max_document_diagrams`, `site_config`, `fixed_today`, and
`fixed_local_offset_minutes`. The document-diagram limit counts Mermaid fences in Markdown and MDX
documents; clients that omit it use the server default of 256. The schema is intentionally
permissive with `additionalProperties` so alpha clients are not broken by future options. Clients
should use it for settings completion, settings validation hints, and profile/rule pickers, then
use `merman/ruleCatalog` for the richer rule explanations and evidence metadata.

`fixed_today` uses the canonical signed-32-bit `CivilDate` spelling. Years `0000` through `9999`
use `YYYY-MM-DD`; later years use `+YEAR-MM-DD`, and negative years use `-YEAR-MM-DD`. Signed years
do not admit unnecessary leading zeroes.

## Standard LSP Pairing

- Clients that do not negotiate pull diagnostics receive standard
  `textDocument/publishDiagnostics`; pull clients request `textDocument/diagnostic` instead and do
  not also receive pushed analysis diagnostics.
- Rule ids appear on Merman diagnostics and code actions through the shared analysis payload.
- Quickfixes use standard `textDocument/codeAction` and only exist when the current server
  analysis snapshot carries explicit `DiagnosticFix` metadata; diagnostic data contains identity
  and version validation only.
- Runtime analysis configuration should flow through initialization options or
  `workspace/didChangeConfiguration`. A diagnostic-affecting change republishes open-document
  diagnostics for push clients, or sends `workspace/diagnostic/refresh` for pull clients only when
  they advertise diagnostic refresh support. A snapshot-affecting change independently sends
  `workspace/semanticTokens/refresh` when semantic tokens and refresh support are negotiated;
  diagnostic-only lint changes do not invalidate or refresh semantic tokens.

Analysis always retains family parse failures in its closed snapshot. The removed
`parse.suppress_errors` analysis setting is not an ignored compatibility field; clients must remove
it. Lenient parsing remains available only on parse, render, and ASCII operation options.
