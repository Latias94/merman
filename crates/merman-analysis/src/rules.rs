use crate::{
    AnalysisDiagnostic, AnalysisStatus, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticSeverity, DiagnosticSpan, SourceMap,
    diagnostic_projection::{
        DiagnosticCandidate, append_diagnostic_candidates_cancellable,
        rule_candidate_without_default_span,
    },
};
use merman_core::{
    BLOCK_WIDTH_WARNING_RULE_ID, DiagramWarningFact, FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
    FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
    MermaidConfig,
    preprocess::{SourceConfigEvidence, SourceConfigOrigin},
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

pub const PREFER_INIT_DIRECTIVE_RULE_ID: &str = "merman.authoring.config.prefer_init_directive";
pub const PREFER_FRONTMATTER_CONFIG_RULE_ID: &str =
    "merman.authoring.config.prefer_frontmatter_config";
pub const DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID: &str =
    "merman.compatibility.config.deprecated_flowchart_html_labels";
pub const DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID: &str =
    "merman.compatibility.config.deprecated_external_diagram_loading";
pub const NO_DIAGRAM_RULE_ID: &str = "merman.parse.no_diagram";
pub const DIAGRAM_PARSE_RULE_ID: &str = "merman.parse.diagram_parse";
pub const UNSUPPORTED_DIAGRAM_RULE_ID: &str = "merman.compatibility.unsupported_diagram";
pub const RECOVERED_EDITOR_FACTS_RULE_ID: &str = "merman.parse.recovered_editor_facts";
pub const RESOURCE_LIMIT_RULE_ID: &str = "merman.resource.source_bytes_exceeded";
pub const DOCUMENT_DIAGRAM_LIMIT_RULE_ID: &str = "merman.resource.document_diagrams_exceeded";
pub const MALFORMED_FRONT_MATTER_RULE_ID: &str = "merman.config.malformed_front_matter";
pub const INVALID_DIRECTIVE_JSON_RULE_ID: &str = "merman.config.invalid_directive_json";
pub const INVALID_FRONT_MATTER_YAML_RULE_ID: &str = "merman.config.invalid_front_matter_yaml";
pub const INVALID_THEME_COLOR_RULE_ID: &str = "merman.config.invalid_theme_color";
pub const PANIC_RULE_ID: &str = "merman.internal.panic";
pub const PARSER_CONTRACT_VIOLATION_RULE_ID: &str = "merman.internal.parser_contract_violation";
pub const INTERNAL_RULE_REGISTRY_GAP_RULE_ID: &str = "merman.internal.rule_registry_gap";
pub const BLOCK_WIDTH_RULE_ID: &str = "merman.block.width_exceeds_columns";
pub const FLOWCHART_EXPLICIT_DIRECTION_RULE_ID: &str =
    "merman.authoring.flowchart.explicit_direction";
pub const FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID: &str =
    "merman.semantic.flowchart.unknown_style_target";
pub const GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID: &str = "merman.git_graph.duplicate_commit_id";
pub const RULE_CATALOG_RESPONSE_VERSION: u32 = 1;

const DEPRECATED_FLOWCHART_HTML_LABELS_INIT_CONFIG_PATHS: [&[&str]; 1] =
    [&["flowchart", "htmlLabels"]];
const DEPRECATED_FLOWCHART_HTML_LABELS_FLOWCHART_INIT_WRAPPER_PATHS: [&[&str]; 2] = [
    &["config", "htmlLabels"],
    &["config", "flowchart", "htmlLabels"],
];
const DEPRECATED_FLOWCHART_HTML_LABELS_FRONTMATTER_CONFIG_PATHS: [&[&str]; 2] = [
    &["flowchart", "htmlLabels"],
    &["config", "flowchart", "htmlLabels"],
];
const DEPRECATED_EXTERNAL_DIAGRAM_LOADING_CONFIG_PATHS: [&[&str]; 2] =
    [&["lazyLoadedDiagrams"], &["loadExternalDiagramsAtStartup"]];
const DEPRECATED_EXTERNAL_DIAGRAM_LOADING_FRONTMATTER_CONFIG_PATHS: [&[&str]; 2] = [
    &["config", "lazyLoadedDiagrams"],
    &["config", "loadExternalDiagramsAtStartup"],
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisRuleProfile {
    #[default]
    Core,
    Recommended,
    Strict,
}

impl AnalysisRuleProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Recommended => "recommended",
            Self::Strict => "strict",
        }
    }

    const fn includes(self, minimum: Self) -> bool {
        self as u8 >= minimum as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    MermaidSyntax,
    MermaidCompatibility,
    MermanAuthoring,
    MermanResourcePolicy,
    MermanInternal,
}

impl RuleOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MermaidSyntax => "mermaid_syntax",
            Self::MermaidCompatibility => "mermaid_compatibility",
            Self::MermanAuthoring => "merman_authoring",
            Self::MermanResourcePolicy => "merman_resource_policy",
            Self::MermanInternal => "merman_internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleDescriptor {
    pub id: &'static str,
    pub description: &'static str,
    pub evidence: &'static [&'static str],
    pub default_severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub default_enabled: bool,
    pub default_profile: AnalysisRuleProfile,
    pub origin: RuleOrigin,
    pub fixable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleCatalogEntry {
    pub id: &'static str,
    pub description: &'static str,
    pub evidence: &'static [&'static str],
    pub default_severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub default_enabled: bool,
    pub default_profile: AnalysisRuleProfile,
    pub origin: RuleOrigin,
    pub configurable: bool,
    pub fixable: bool,
}

impl RuleCatalogEntry {
    fn from_descriptor(descriptor: RuleDescriptor) -> Self {
        Self {
            id: descriptor.id,
            description: descriptor.description,
            evidence: descriptor.evidence,
            default_severity: descriptor.default_severity,
            category: descriptor.category,
            default_enabled: descriptor.default_enabled,
            default_profile: descriptor.default_profile,
            origin: descriptor.origin,
            configurable: is_configurable_rule_descriptor(descriptor),
            fixable: descriptor.fixable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleCatalogResponse {
    pub version: u32,
    pub rules: Vec<RuleCatalogEntry>,
}

impl RuleCatalogResponse {
    pub fn from_rules(rules: Vec<RuleCatalogEntry>) -> Self {
        Self {
            version: RULE_CATALOG_RESPONSE_VERSION,
            rules,
        }
    }

    pub fn current() -> Self {
        Self::from_rules(rule_catalog())
    }

    pub fn configurable() -> Self {
        Self::from_rules(configurable_rule_catalog())
    }
}

pub(crate) const PREFER_INIT_DIRECTIVE_RULE: RuleDescriptor = RuleDescriptor {
    id: PREFER_INIT_DIRECTIVE_RULE_ID,
    description: "Prefer the canonical `init` directive keyword over the accepted `initialize` alias.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/utils.ts",
        "docs/adr/0072-lint-rule-governance.md",
    ],
    default_severity: DiagnosticSeverity::Hint,
    category: DiagnosticCategory::Config,
    default_enabled: false,
    default_profile: AnalysisRuleProfile::Recommended,
    origin: RuleOrigin::MermanAuthoring,
    fixable: true,
};

pub(crate) const PREFER_FRONTMATTER_CONFIG_RULE: RuleDescriptor = RuleDescriptor {
    id: PREFER_FRONTMATTER_CONFIG_RULE_ID,
    description: "Prefer diagram frontmatter `config` over Mermaid init directives.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md",
    ],
    default_severity: DiagnosticSeverity::Hint,
    category: DiagnosticCategory::Config,
    default_enabled: false,
    default_profile: AnalysisRuleProfile::Recommended,
    origin: RuleOrigin::MermanAuthoring,
    fixable: true,
};

const DEPRECATED_FLOWCHART_HTML_LABELS_RULE: RuleDescriptor = RuleDescriptor {
    id: DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID,
    description: "Report deprecated `flowchart.htmlLabels` config and recommend the root-level `htmlLabels` option.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.type.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};

const DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE: RuleDescriptor = RuleDescriptor {
    id: DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID,
    description: "Report deprecated external diagram loading config and recommend `registerExternalDiagrams`.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.spec.ts",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};

pub(crate) const NO_DIAGRAM_RULE: RuleDescriptor = RuleDescriptor {
    id: NO_DIAGRAM_RULE_ID,
    description: "Report input that does not contain a Mermaid diagram.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/detectType.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.spec.ts",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Parse,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

pub(crate) const DIAGRAM_PARSE_RULE: RuleDescriptor = RuleDescriptor {
    id: DIAGRAM_PARSE_RULE_ID,
    description: "Report Mermaid diagram syntax that the parser cannot accept.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.ts",
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Parse,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

pub(crate) const UNSUPPORTED_DIAGRAM_RULE: RuleDescriptor = RuleDescriptor {
    id: UNSUPPORTED_DIAGRAM_RULE_ID,
    description: "Report Mermaid diagram types that are recognized but unavailable in this build.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/detectType.ts",
        "docs/release/PACKAGE_SURFACES.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Compatibility,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};

pub(crate) const RECOVERED_EDITOR_FACTS_RULE: RuleDescriptor = RuleDescriptor {
    id: RECOVERED_EDITOR_FACTS_RULE_ID,
    description: "Report parser recovery diagnostics emitted while producing editor semantic facts.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/mermaid.ts",
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Parse,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

const RESOURCE_LIMIT_RULE: RuleDescriptor = RuleDescriptor {
    id: RESOURCE_LIMIT_RULE_ID,
    description: "Report Mermaid sources that exceed the configured analysis source byte budget.",
    evidence: &[
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
        "docs/bindings/OPTIONS_JSON.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Resource,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermanResourcePolicy,
    fixable: false,
};

const DOCUMENT_DIAGRAM_LIMIT_RULE: RuleDescriptor = RuleDescriptor {
    id: DOCUMENT_DIAGRAM_LIMIT_RULE_ID,
    description: "Report host documents that exceed the configured embedded Mermaid diagram budget.",
    evidence: &[
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
        "docs/plans/2026-07-29-002-refactor-analysis-lsp-generation-session-plan.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Resource,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermanResourcePolicy,
    fixable: false,
};

pub(crate) const MALFORMED_FRONT_MATTER_RULE: RuleDescriptor = RuleDescriptor {
    id: MALFORMED_FRONT_MATTER_RULE_ID,
    description: "Report malformed YAML front matter blocks before diagram parsing.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/frontmatter.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/frontmatter.spec.ts",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

pub(crate) const INVALID_DIRECTIVE_JSON_RULE: RuleDescriptor = RuleDescriptor {
    id: INVALID_DIRECTIVE_JSON_RULE_ID,
    description: "Report Mermaid directive blocks whose JSON payload cannot be parsed.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/regexes.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/utils.ts",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

pub(crate) const INVALID_FRONT_MATTER_YAML_RULE: RuleDescriptor = RuleDescriptor {
    id: INVALID_FRONT_MATTER_YAML_RULE_ID,
    description: "Report Mermaid front matter whose YAML payload cannot be parsed.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/frontmatter.ts",
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagram-api/frontmatter.spec.ts",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidSyntax,
    fixable: false,
};

pub(crate) const INVALID_THEME_COLOR_RULE: RuleDescriptor = RuleDescriptor {
    id: INVALID_THEME_COLOR_RULE_ID,
    description: "Report theme color values rejected by Mermaid's pinned Khroma calculations.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid/src/themes/theme-default.js",
        "docs/adr/0068-render-side-presentation-theme-view.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};

pub(crate) const PANIC_RULE: RuleDescriptor = RuleDescriptor {
    id: PANIC_RULE_ID,
    description: "Report an internal panic caught while analyzing Mermaid source.",
    evidence: &["docs/adr/0070-diagnostics-first-analysis-contract.md"],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Internal,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermanInternal,
    fixable: false,
};

pub(crate) const PARSER_CONTRACT_VIOLATION_RULE: RuleDescriptor = RuleDescriptor {
    id: PARSER_CONTRACT_VIOLATION_RULE_ID,
    description: "Report a custom parser that returned cancellation to a non-cancellable analysis facade.",
    evidence: &[
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
        "docs/adr/0073-family-owned-diagram-architecture.md",
    ],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Internal,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermanInternal,
    fixable: false,
};

pub(crate) const INTERNAL_RULE_REGISTRY_GAP_RULE: RuleDescriptor = RuleDescriptor {
    id: INTERNAL_RULE_REGISTRY_GAP_RULE_ID,
    description: "Report an internal rule registry gap while projecting diagnostics.",
    evidence: &["docs/adr/0072-lint-rule-governance.md"],
    default_severity: DiagnosticSeverity::Error,
    category: DiagnosticCategory::Internal,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermanInternal,
    fixable: false,
};

const BLOCK_WIDTH_RULE: RuleDescriptor = RuleDescriptor {
    id: BLOCK_WIDTH_RULE_ID,
    description: "Report block diagram entries that exceed the configured column width.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/syntax/block.md",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};
const FLOWCHART_EXPLICIT_DIRECTION_RULE: RuleDescriptor = RuleDescriptor {
    id: FLOWCHART_EXPLICIT_DIRECTION_RULE_ID,
    description: "Recommend explicit flowchart header directions and offer an insertion quickfix.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/syntax/flowchart.md",
        "docs/adr/0072-lint-rule-governance.md",
    ],
    default_severity: DiagnosticSeverity::Hint,
    category: DiagnosticCategory::Semantic,
    default_enabled: false,
    default_profile: AnalysisRuleProfile::Recommended,
    origin: RuleOrigin::MermanAuthoring,
    fixable: true,
};
const FLOWCHART_UNKNOWN_STYLE_TARGET_RULE: RuleDescriptor = RuleDescriptor {
    id: FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID,
    description: "Report flowchart `style` directives that would auto-create an unknown node target.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagrams/flowchart/flowDb.ts",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};
const GIT_GRAPH_DUPLICATE_COMMIT_RULE: RuleDescriptor = RuleDescriptor {
    id: GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID,
    description: "Report duplicate gitGraph commit ids.",
    evidence: &[
        "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/diagrams/git/gitGraphAst.ts",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};
const RULE_DESCRIPTORS: &[RuleDescriptor] = &[
    PREFER_INIT_DIRECTIVE_RULE,
    PREFER_FRONTMATTER_CONFIG_RULE,
    DEPRECATED_FLOWCHART_HTML_LABELS_RULE,
    DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE,
    NO_DIAGRAM_RULE,
    DIAGRAM_PARSE_RULE,
    UNSUPPORTED_DIAGRAM_RULE,
    RECOVERED_EDITOR_FACTS_RULE,
    RESOURCE_LIMIT_RULE,
    DOCUMENT_DIAGRAM_LIMIT_RULE,
    MALFORMED_FRONT_MATTER_RULE,
    INVALID_DIRECTIVE_JSON_RULE,
    INVALID_FRONT_MATTER_YAML_RULE,
    INVALID_THEME_COLOR_RULE,
    PANIC_RULE,
    PARSER_CONTRACT_VIOLATION_RULE,
    INTERNAL_RULE_REGISTRY_GAP_RULE,
    BLOCK_WIDTH_RULE,
    FLOWCHART_EXPLICIT_DIRECTION_RULE,
    FLOWCHART_UNKNOWN_STYLE_TARGET_RULE,
    GIT_GRAPH_DUPLICATE_COMMIT_RULE,
];

pub fn rule_descriptors() -> &'static [RuleDescriptor] {
    RULE_DESCRIPTORS
}

pub fn rule_catalog() -> Vec<RuleCatalogEntry> {
    RULE_DESCRIPTORS
        .iter()
        .copied()
        .map(RuleCatalogEntry::from_descriptor)
        .collect()
}

pub fn configurable_rule_catalog() -> Vec<RuleCatalogEntry> {
    configurable_rule_descriptors()
        .map(RuleCatalogEntry::from_descriptor)
        .collect()
}

pub fn rule_catalog_response() -> RuleCatalogResponse {
    RuleCatalogResponse::current()
}

pub fn configurable_rule_catalog_response() -> RuleCatalogResponse {
    RuleCatalogResponse::configurable()
}

pub fn rule_catalog_response_json_bytes() -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&rule_catalog_response())
}

pub fn configurable_rule_catalog_response_json_bytes() -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&configurable_rule_catalog_response())
}

pub fn configurable_rule_descriptors() -> impl Iterator<Item = RuleDescriptor> {
    RULE_DESCRIPTORS
        .iter()
        .copied()
        .filter(|descriptor| is_configurable_rule_descriptor(*descriptor))
}

pub fn configurable_rule_descriptor(rule_id: &str) -> Option<RuleDescriptor> {
    configurable_rule_descriptors().find(|descriptor| descriptor.id == rule_id)
}

pub fn rule_descriptor(rule_id: &str) -> Option<RuleDescriptor> {
    RULE_DESCRIPTORS
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == rule_id)
}

fn is_configurable_rule_descriptor(descriptor: RuleDescriptor) -> bool {
    !matches!(
        descriptor.category,
        DiagnosticCategory::Internal | DiagnosticCategory::Resource
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRuleConfigError {
    rule_id: String,
}

impl AnalysisRuleConfigError {
    fn not_configurable(rule_id: String) -> Self {
        Self { rule_id }
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }
}

impl Display for AnalysisRuleConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rule id `{}` must reference a configurable analysis rule id",
            self.rule_id
        )
    }
}

impl StdError for AnalysisRuleConfigError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AnalysisRuleConfig {
    profile: AnalysisRuleProfile,
    enabled_rules: BTreeSet<String>,
    disabled_rules: BTreeSet<String>,
    severity_overrides: BTreeMap<String, DiagnosticSeverity>,
}

#[derive(Deserialize)]
struct AnalysisRuleConfigSerde {
    #[serde(default)]
    profile: AnalysisRuleProfile,
    #[serde(default)]
    enabled_rules: BTreeSet<String>,
    #[serde(default)]
    disabled_rules: BTreeSet<String>,
    #[serde(default)]
    severity_overrides: BTreeMap<String, DiagnosticSeverity>,
}

impl<'de> Deserialize<'de> for AnalysisRuleConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let serialized = AnalysisRuleConfigSerde::deserialize(deserializer)?;
        let mut config = Self::default().with_profile(serialized.profile);
        for rule_id in serialized.enabled_rules {
            config
                .enable_rule(rule_id)
                .map_err(serde::de::Error::custom)?;
        }
        for rule_id in serialized.disabled_rules {
            config
                .disable_rule(rule_id)
                .map_err(serde::de::Error::custom)?;
        }
        for (rule_id, severity) in serialized.severity_overrides {
            config
                .set_rule_severity(rule_id, severity)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(config)
    }
}

impl AnalysisRuleConfig {
    pub fn with_profile(mut self, profile: AnalysisRuleProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn profile(&self) -> AnalysisRuleProfile {
        self.profile
    }

    pub fn with_rule_enabled(
        mut self,
        rule_id: impl Into<String>,
    ) -> Result<Self, AnalysisRuleConfigError> {
        self.enable_rule(rule_id)?;
        Ok(self)
    }

    pub fn with_rule_disabled(
        mut self,
        rule_id: impl Into<String>,
    ) -> Result<Self, AnalysisRuleConfigError> {
        self.disable_rule(rule_id)?;
        Ok(self)
    }

    pub fn with_rule_severity(
        mut self,
        rule_id: impl Into<String>,
        severity: DiagnosticSeverity,
    ) -> Result<Self, AnalysisRuleConfigError> {
        self.set_rule_severity(rule_id, severity)?;
        Ok(self)
    }

    pub fn set_profile(&mut self, profile: AnalysisRuleProfile) {
        self.profile = profile;
    }

    pub fn enable_rule(
        &mut self,
        rule_id: impl Into<String>,
    ) -> Result<(), AnalysisRuleConfigError> {
        let rule_id = configurable_rule_id(rule_id)?;
        self.enabled_rules.insert(rule_id);
        Ok(())
    }

    pub fn disable_rule(
        &mut self,
        rule_id: impl Into<String>,
    ) -> Result<(), AnalysisRuleConfigError> {
        let rule_id = configurable_rule_id(rule_id)?;
        self.disabled_rules.insert(rule_id);
        Ok(())
    }

    pub fn set_rule_severity(
        &mut self,
        rule_id: impl Into<String>,
        severity: DiagnosticSeverity,
    ) -> Result<(), AnalysisRuleConfigError> {
        let rule_id = configurable_rule_id(rule_id)?;
        self.severity_overrides.insert(rule_id, severity);
        Ok(())
    }

    pub fn is_rule_enabled(&self, descriptor: RuleDescriptor) -> bool {
        if !is_configurable_rule_descriptor(descriptor) {
            return descriptor.default_enabled;
        }
        if self.disabled_rules.contains(descriptor.id) {
            return false;
        }
        if self.enabled_rules.contains(descriptor.id) {
            return true;
        }
        descriptor.default_enabled || self.profile.includes(descriptor.default_profile)
    }

    pub fn severity_for(&self, descriptor: RuleDescriptor) -> DiagnosticSeverity {
        if !is_configurable_rule_descriptor(descriptor) {
            return descriptor.default_severity;
        }
        self.severity_overrides
            .get(descriptor.id)
            .copied()
            .unwrap_or(descriptor.default_severity)
    }
}

fn configurable_rule_id(rule_id: impl Into<String>) -> Result<String, AnalysisRuleConfigError> {
    let rule_id = rule_id.into();
    if configurable_rule_descriptor(&rule_id).is_some() {
        Ok(rule_id)
    } else {
        Err(AnalysisRuleConfigError::not_configurable(rule_id))
    }
}

const PREFER_INIT_SUPPRESSORS: &[RuleDescriptor] = &[PREFER_FRONTMATTER_CONFIG_RULE];

pub(crate) fn source_lint_candidates_cancellable(
    source: &str,
    source_map: &SourceMap,
    captured_config: Option<&MermaidConfig>,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let alias_candidates =
        init_directive_alias_candidates_cancellable(source_map, source_config, cancellation)?;
    let mut candidates = alias_candidates;
    cancellation.checkpoint()?;
    append_diagnostic_candidates_cancellable(
        &mut candidates,
        prefer_frontmatter_config_candidates_with_config_cancellable(
            source,
            source_map,
            captured_config,
            source_config,
            cancellation,
        )?,
        cancellation,
    )?;
    cancellation.checkpoint()?;
    append_diagnostic_candidates_cancellable(
        &mut candidates,
        deprecated_flowchart_html_labels_candidates(
            source_map,
            &DEPRECATED_FLOWCHART_HTML_LABELS_INIT_CONFIG_PATHS,
            &DEPRECATED_FLOWCHART_HTML_LABELS_FRONTMATTER_CONFIG_PATHS,
            source_config,
            cancellation,
        )?,
        cancellation,
    )?;
    cancellation.checkpoint()?;
    append_diagnostic_candidates_cancellable(
        &mut candidates,
        deprecated_external_diagram_loading_candidates(source_map, source_config, cancellation)?,
        cancellation,
    )?;
    cancellation.checkpoint()?;
    Ok(candidates)
}

pub(crate) fn parsed_source_lint_candidates_cancellable(
    source_map: &SourceMap,
    diagram_type: &str,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    if merman_core::diagram_type_family_kind(diagram_type) != Some("flowchart") {
        return Ok(Vec::new());
    }
    deprecated_flowchart_html_labels_candidates(
        source_map,
        &DEPRECATED_FLOWCHART_HTML_LABELS_FLOWCHART_INIT_WRAPPER_PATHS,
        &[],
        source_config,
        cancellation,
    )
}

#[cfg(test)]
pub(crate) fn source_lint_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = crate::AnalysisCancellationToken::new();
    let captured_config = merman_core::Engine::new()
        .parse_metadata_sync(source)
        .ok()
        .map(|metadata| metadata.config);
    let source_config = source_config_evidence_for_test(source);
    let candidates = source_lint_candidates_cancellable(
        source,
        source_map,
        captured_config.as_ref(),
        &source_config,
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled");
    crate::diagnostic_projection::project_diagnostic_candidates(
        &candidates,
        &crate::AnalysisDiagnosticPolicy {
            rule_config: rule_config.clone(),
        },
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

#[cfg(test)]
fn source_config_evidence_for_test(source: &str) -> SourceConfigEvidence {
    let control = merman_core::ParseControl::new();
    match merman_core::Engine::new()
        .capture_diagram_snapshot_controlled_sync(source, &control)
        .expect("a private parse control cannot be cancelled")
    {
        merman_core::DiagramSnapshotCapture::Snapshot(Some(snapshot)) => {
            snapshot.source_config().clone()
        }
        merman_core::DiagramSnapshotCapture::Snapshot(None) => SourceConfigEvidence::default(),
        merman_core::DiagramSnapshotCapture::Failed { source_config, .. } => source_config,
    }
}

#[cfg(test)]
pub(crate) fn parsed_source_lint_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    diagram_type: &str,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = crate::AnalysisCancellationToken::new();
    let source_config = source_config_evidence_for_test(source);
    let candidates = parsed_source_lint_candidates_cancellable(
        source_map,
        diagram_type,
        &source_config,
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled");
    crate::diagnostic_projection::project_diagnostic_candidates(
        &candidates,
        &crate::AnalysisDiagnosticPolicy {
            rule_config: rule_config.clone(),
        },
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

#[cfg(test)]
pub(crate) fn semantic_warning_diagnostics(
    diagram_type: &str,
    warning_facts: &[DiagramWarningFact],
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = crate::AnalysisCancellationToken::new();
    semantic_warning_diagnostics_cancellable(
        diagram_type,
        warning_facts,
        source_map,
        rule_config,
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

#[cfg(test)]
pub(crate) fn semantic_warning_diagnostics_cancellable(
    diagram_type: &str,
    warning_facts: &[DiagramWarningFact],
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<AnalysisDiagnostic>, crate::AnalysisCancelled> {
    let candidates = semantic_warning_candidates_cancellable(
        diagram_type,
        warning_facts,
        source_map,
        cancellation,
    )?;
    crate::diagnostic_projection::project_diagnostic_candidates(
        &candidates,
        &crate::AnalysisDiagnosticPolicy {
            rule_config: rule_config.clone(),
        },
        cancellation,
    )
}

pub(crate) fn semantic_warning_candidates_cancellable(
    diagram_type: &str,
    warning_facts: &[DiagramWarningFact],
    source_map: &SourceMap,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let fallback_span = source_map.whole_source_span_cancellable(cancellation)?.ok();
    semantic_warning_fact_candidates_cancellable(
        diagram_type,
        warning_facts,
        fallback_span,
        source_map,
        cancellation,
    )
}

fn semantic_warning_fact_candidates_cancellable(
    diagram_type: &str,
    warning_facts: &[DiagramWarningFact],
    fallback_span: Option<DiagnosticSpan>,
    source_map: &SourceMap,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    let mut candidates = Vec::with_capacity(warning_facts.len());

    for (fact_index, fact) in warning_facts.iter().enumerate() {
        if fact_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        match warning_fact_rule_descriptor(&fact.rule_id) {
            Some(descriptor) => candidates.push(warning_for_fact_candidate_cancellable(
                diagram_type,
                fact,
                fallback_span,
                source_map,
                descriptor,
                cancellation,
            )?),
            None => {
                let mut candidate = rule_candidate_without_default_span(
                    INTERNAL_RULE_REGISTRY_GAP_RULE,
                    AnalysisStatus::InternalError,
                    format!(
                        "unknown warning fact rule id `{}`: {}",
                        fact.rule_id, fact.message
                    ),
                )
                .with_diagram_type(diagram_type);
                if let Some(span) = fallback_span {
                    candidate = candidate.with_span(span);
                }
                candidates.push(candidate);
            }
        }
    }

    cancellation.checkpoint()?;
    Ok(candidates)
}

fn warning_for_fact_candidate_cancellable(
    diagram_type: &str,
    fact: &DiagramWarningFact,
    fallback_span: Option<DiagnosticSpan>,
    source_map: &SourceMap,
    descriptor: RuleDescriptor,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<DiagnosticCandidate, crate::AnalysisCancelled> {
    let span = warning_fact_span_cancellable(fact, source_map, fallback_span, cancellation)?;
    let fix = warning_fact_fix_cancellable(fact, descriptor, source_map, cancellation)?;
    let mut candidate =
        DiagnosticCandidate::new(descriptor, fact.message.clone()).with_diagram_type(diagram_type);

    if let Some(span) = span {
        candidate = candidate.with_span(span);
    }

    if let Some(fix) = fix {
        candidate = candidate.with_fix(fix);
    }

    Ok(candidate)
}

fn warning_fact_span_cancellable(
    fact: &DiagramWarningFact,
    source_map: &SourceMap,
    fallback_span: Option<DiagnosticSpan>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<DiagnosticSpan>, crate::AnalysisCancelled> {
    let span = match fact.span {
        Some(span) => source_map
            .span_cancellable(span.start, span.end, cancellation)?
            .ok(),
        None => None,
    };
    Ok(span.or(fallback_span))
}

fn warning_fact_fix_cancellable(
    fact: &DiagramWarningFact,
    descriptor: RuleDescriptor,
    source_map: &SourceMap,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<DiagnosticFix>, crate::AnalysisCancelled> {
    let Some(fix_span) = fact.fix_span.or(fact.span) else {
        return Ok(None);
    };
    let Some(fix_span) = source_map
        .span_cancellable(fix_span.start, fix_span.end, cancellation)?
        .ok()
    else {
        return Ok(None);
    };
    Ok(match descriptor.id {
        FLOWCHART_EXPLICIT_DIRECTION_RULE_ID => Some(
            DiagnosticFix::new(
                "Insert `TB` into the flowchart header",
                vec![DiagnosticFixEdit::new(fix_span, " TB")],
            )
            .preferred(),
        ),
        _ => None,
    })
}

fn warning_fact_rule_descriptor(rule_id: &str) -> Option<RuleDescriptor> {
    match rule_id {
        BLOCK_WIDTH_WARNING_RULE_ID => Some(BLOCK_WIDTH_RULE),
        FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID => Some(FLOWCHART_EXPLICIT_DIRECTION_RULE),
        FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID => Some(FLOWCHART_UNKNOWN_STYLE_TARGET_RULE),
        GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID => Some(GIT_GRAPH_DUPLICATE_COMMIT_RULE),
        _ => None,
    }
}

pub(crate) fn internal_rule_registry_gap_diagnostic(
    message: impl Into<String>,
    span: Option<DiagnosticSpan>,
) -> AnalysisDiagnostic {
    let mut diagnostic = AnalysisDiagnostic::error(
        INTERNAL_RULE_REGISTRY_GAP_RULE_ID,
        DiagnosticCategory::Internal,
        message,
    )
    .with_code(
        AnalysisStatus::InternalError.code(),
        AnalysisStatus::InternalError.code_name(),
    );

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span);
    }

    diagnostic
}

fn init_directive_alias_candidates_cancellable(
    source_map: &SourceMap,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    let mut candidates = Vec::new();
    for (index, directive) in source_config.directives().iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if !directive.complete() || directive.keyword() != "initialize" {
            continue;
        }
        let keyword = directive.keyword_span();
        let Ok(span) = source_map.span_cancellable(keyword.start, keyword.end, cancellation)?
        else {
            continue;
        };
        candidates.push(
            DiagnosticCandidate::new(
                PREFER_INIT_DIRECTIVE_RULE,
                "prefer `init` directive keyword over the `initialize` alias",
            )
            .with_span(span)
            .with_help("`initialize` is accepted as an alias; `init` is the canonical Mermaid directive keyword.")
            .with_fix(
                DiagnosticFix::new(
                    "Replace `initialize` with `init`",
                    vec![DiagnosticFixEdit::new(span, "init")],
                )
                .preferred(),
            )
            .with_suppressors(PREFER_INIT_SUPPRESSORS),
        );
    }
    cancellation.checkpoint()?;
    Ok(candidates)
}

fn prefer_frontmatter_config_candidates_with_config_cancellable(
    source: &str,
    source_map: &SourceMap,
    captured_config: Option<&MermaidConfig>,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    let fix = match captured_config {
        Some(config) => {
            crate::source_config_rewrite::init_directives_to_frontmatter_fix_cancellable(
                source,
                source_map,
                config,
                source_config,
                cancellation,
            )?
        }
        None => None,
    };

    let mut candidates = Vec::new();
    for (index, directive) in source_config.directives().iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if !directive.complete() || !matches!(directive.keyword(), "init" | "initialize") {
            continue;
        }
        let keyword = directive.keyword_span();
        let Ok(span) = source_map.span_cancellable(keyword.start, keyword.end, cancellation)?
        else {
            continue;
        };
        let mut candidate = DiagnosticCandidate::new(
                PREFER_FRONTMATTER_CONFIG_RULE,
                "prefer frontmatter `config` over Mermaid init directives",
            )
            .with_span(span)
            .with_help(
                "Mermaid deprecated directives from v10.5.0; diagram authors should move configuration into the diagram frontmatter `config` block.",
            );
        // Every matching diagnostic retains quick-fix discoverability. `DiagnosticFix`
        // clones share the immutable edit allocation, so an aggregate N-edit migration does
        // not become N independent retained edit arrays.
        if let Some(fix) = fix.clone() {
            candidate = candidate.with_fix(fix);
        }
        candidates.push(candidate);
    }
    cancellation.checkpoint()?;
    Ok(candidates)
}

#[cfg(test)]
fn prefer_frontmatter_config_diagnostics_with_config(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    captured_config: Option<&MermaidConfig>,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = crate::AnalysisCancellationToken::new();
    let source_config = source_config_evidence_for_test(source);
    let candidates = prefer_frontmatter_config_candidates_with_config_cancellable(
        source,
        source_map,
        captured_config,
        &source_config,
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled");
    crate::diagnostic_projection::project_diagnostic_candidates(
        &candidates,
        &crate::AnalysisDiagnosticPolicy {
            rule_config: rule_config.clone(),
        },
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

fn deprecated_flowchart_html_labels_candidates(
    source_map: &SourceMap,
    init_matching_paths: &[&[&str]],
    frontmatter_matching_paths: &[&[&str]],
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    config_key_diagnostics(
        source_map,
        ConfigKeyDiagnosticSpec {
            descriptor: DEPRECATED_FLOWCHART_HTML_LABELS_RULE,
            init_matching_paths,
            frontmatter_matching_paths,
            message: "`flowchart.htmlLabels` is deprecated; use root-level `htmlLabels` instead",
            help: "Mermaid keeps `flowchart.htmlLabels` as a compatibility fallback, but root-level `htmlLabels` takes precedence.",
        },
        source_config,
        cancellation,
    )
}

fn deprecated_external_diagram_loading_candidates(
    source_map: &SourceMap,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    config_key_diagnostics(
        source_map,
        ConfigKeyDiagnosticSpec {
            descriptor: DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE,
            init_matching_paths: &DEPRECATED_EXTERNAL_DIAGRAM_LOADING_CONFIG_PATHS,
            frontmatter_matching_paths:
                &DEPRECATED_EXTERNAL_DIAGRAM_LOADING_FRONTMATTER_CONFIG_PATHS,
            message: "deprecated external diagram loading config; use `registerExternalDiagrams` instead",
            help: "Mermaid warns that `lazyLoadedDiagrams` and `loadExternalDiagramsAtStartup` are deprecated in favor of the `registerExternalDiagrams` API.",
        },
        source_config,
        cancellation,
    )
}

struct ConfigKeyDiagnosticSpec<'a> {
    descriptor: RuleDescriptor,
    init_matching_paths: &'a [&'a [&'a str]],
    frontmatter_matching_paths: &'a [&'a [&'a str]],
    message: &'static str,
    help: &'static str,
}

fn config_key_diagnostics(
    source_map: &SourceMap,
    spec: ConfigKeyDiagnosticSpec<'_>,
    source_config: &SourceConfigEvidence,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;

    let matches_any_path = |key: &merman_core::preprocess::SourceConfigKeyEvidence,
                            paths: &[&[&str]]| {
        paths.iter().any(|path| key.matches_path(path))
    };
    let mut candidates = Vec::new();
    let mut append_candidate = |key: &merman_core::preprocess::SourceConfigKeyEvidence| {
        let span = key.span();
        let Ok(span) = source_map.span_cancellable(span.start, span.end, cancellation)? else {
            return Ok(());
        };
        candidates.push(
            DiagnosticCandidate::new(spec.descriptor, spec.message)
                .with_span(span)
                .with_help(spec.help),
        );
        Ok::<(), crate::AnalysisCancelled>(())
    };

    for (index, key) in source_config.keys().iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let SourceConfigOrigin::Directive { directive_index } = key.origin() else {
            continue;
        };
        let Some(directive) = source_config.directives().get(directive_index) else {
            continue;
        };
        if directive.complete()
            && matches!(directive.keyword(), "init" | "initialize")
            && matches_any_path(key, spec.init_matching_paths)
        {
            append_candidate(key)?;
        }
    }
    cancellation.checkpoint()?;

    for (index, key) in source_config.keys().iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if key.origin() == SourceConfigOrigin::Frontmatter
            && matches_any_path(key, spec.frontmatter_matching_paths)
        {
            append_candidate(key)?;
        }
    }
    cancellation.checkpoint()?;
    Ok(candidates)
}

#[cfg(test)]
mod tests;
