use crate::{
    AnalysisDiagnostic, AnalysisStatus, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticSeverity, DiagnosticSpan, SourceMap,
    source_directives::{
        directive_keyword_spans, frontmatter_config_key_spans, init_directive_config_key_spans,
    },
};
use merman_core::{
    BLOCK_WIDTH_WARNING_RULE_ID, DiagramWarningFact, FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
    FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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
pub const MALFORMED_FRONT_MATTER_RULE_ID: &str = "merman.config.malformed_front_matter";
pub const INVALID_DIRECTIVE_JSON_RULE_ID: &str = "merman.config.invalid_directive_json";
pub const INVALID_FRONT_MATTER_YAML_RULE_ID: &str = "merman.config.invalid_front_matter_yaml";
pub const PANIC_RULE_ID: &str = "merman.internal.panic";
pub const INTERNAL_RULE_REGISTRY_GAP_RULE_ID: &str = "merman.internal.rule_registry_gap";
pub const FLOWCHART_FACTS_PROJECTION_RULE_ID: &str = "merman.internal.flowchart_facts_projection";
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

const PREFER_INIT_DIRECTIVE_RULE: RuleDescriptor = RuleDescriptor {
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

const PREFER_FRONTMATTER_CONFIG_RULE: RuleDescriptor = RuleDescriptor {
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

const NO_DIAGRAM_RULE: RuleDescriptor = RuleDescriptor {
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

const DIAGRAM_PARSE_RULE: RuleDescriptor = RuleDescriptor {
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

const UNSUPPORTED_DIAGRAM_RULE: RuleDescriptor = RuleDescriptor {
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

const RECOVERED_EDITOR_FACTS_RULE: RuleDescriptor = RuleDescriptor {
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

const MALFORMED_FRONT_MATTER_RULE: RuleDescriptor = RuleDescriptor {
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

const INVALID_DIRECTIVE_JSON_RULE: RuleDescriptor = RuleDescriptor {
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

const INVALID_FRONT_MATTER_YAML_RULE: RuleDescriptor = RuleDescriptor {
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

const PANIC_RULE: RuleDescriptor = RuleDescriptor {
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

const INTERNAL_RULE_REGISTRY_GAP_RULE: RuleDescriptor = RuleDescriptor {
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

const FLOWCHART_FACTS_PROJECTION_RULE: RuleDescriptor = RuleDescriptor {
    id: FLOWCHART_FACTS_PROJECTION_RULE_ID,
    description: "Report an internal failure while projecting flowchart parser model facts.",
    evidence: &["docs/adr/0070-diagnostics-first-analysis-contract.md"],
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
    MALFORMED_FRONT_MATTER_RULE,
    INVALID_DIRECTIVE_JSON_RULE,
    INVALID_FRONT_MATTER_YAML_RULE,
    PANIC_RULE,
    INTERNAL_RULE_REGISTRY_GAP_RULE,
    FLOWCHART_FACTS_PROJECTION_RULE,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRuleConfig {
    #[serde(default)]
    profile: AnalysisRuleProfile,
    #[serde(default)]
    enabled_rules: BTreeSet<String>,
    #[serde(default)]
    disabled_rules: BTreeSet<String>,
    #[serde(default)]
    severity_overrides: BTreeMap<String, DiagnosticSeverity>,
}

impl AnalysisRuleConfig {
    pub fn with_profile(mut self, profile: AnalysisRuleProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn profile(&self) -> AnalysisRuleProfile {
        self.profile
    }

    pub fn with_rule_enabled(mut self, rule_id: impl Into<String>) -> Self {
        self.enable_rule(rule_id);
        self
    }

    pub fn with_rule_disabled(mut self, rule_id: impl Into<String>) -> Self {
        self.disable_rule(rule_id);
        self
    }

    pub fn with_rule_severity(
        mut self,
        rule_id: impl Into<String>,
        severity: DiagnosticSeverity,
    ) -> Self {
        self.set_rule_severity(rule_id, severity);
        self
    }

    pub fn set_profile(&mut self, profile: AnalysisRuleProfile) {
        self.profile = profile;
    }

    pub fn enable_rule(&mut self, rule_id: impl Into<String>) {
        self.enabled_rules.insert(rule_id.into());
    }

    pub fn disable_rule(&mut self, rule_id: impl Into<String>) {
        self.disabled_rules.insert(rule_id.into());
    }

    pub fn set_rule_severity(&mut self, rule_id: impl Into<String>, severity: DiagnosticSeverity) {
        self.severity_overrides.insert(rule_id.into(), severity);
    }

    pub fn is_rule_enabled(&self, descriptor: RuleDescriptor) -> bool {
        if self.disabled_rules.contains(descriptor.id) {
            return false;
        }
        if self.enabled_rules.contains(descriptor.id) {
            return true;
        }
        descriptor.default_enabled || self.profile.includes(descriptor.default_profile)
    }

    pub fn severity_for(&self, descriptor: RuleDescriptor) -> DiagnosticSeverity {
        self.severity_overrides
            .get(descriptor.id)
            .copied()
            .unwrap_or(descriptor.default_severity)
    }
}

pub(crate) fn source_lint_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let mut diagnostics = init_directive_alias_diagnostics(source, source_map, rule_config);
    diagnostics.extend(prefer_frontmatter_config_diagnostics(
        source,
        source_map,
        rule_config,
    ));
    diagnostics.extend(deprecated_flowchart_html_labels_diagnostics(
        source,
        source_map,
        rule_config,
        &DEPRECATED_FLOWCHART_HTML_LABELS_INIT_CONFIG_PATHS,
        &DEPRECATED_FLOWCHART_HTML_LABELS_FRONTMATTER_CONFIG_PATHS,
    ));
    diagnostics.extend(deprecated_external_diagram_loading_diagnostics(
        source,
        source_map,
        rule_config,
    ));
    diagnostics
}

pub(crate) fn parsed_source_lint_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    diagram_type: &str,
) -> Vec<AnalysisDiagnostic> {
    if merman_core::diagram_type_family_kind(diagram_type) != Some("flowchart") {
        return Vec::new();
    }

    deprecated_flowchart_html_labels_diagnostics(
        source,
        source_map,
        rule_config,
        &DEPRECATED_FLOWCHART_HTML_LABELS_FLOWCHART_INIT_WRAPPER_PATHS,
        &[],
    )
}

pub(crate) fn semantic_warning_diagnostics(
    diagram_type: &str,
    model: &Value,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let span = source_map.whole_source_span().ok();
    let Some(warning_facts) = model
        .get("warningFacts")
        .and_then(|value| serde_json::from_value::<Vec<DiagramWarningFact>>(value.clone()).ok())
    else {
        return Vec::new();
    };

    semantic_warning_fact_diagnostics(diagram_type, warning_facts, span, source_map, rule_config)
}

fn semantic_warning_fact_diagnostics(
    diagram_type: &str,
    warning_facts: Vec<DiagramWarningFact>,
    fallback_span: Option<DiagnosticSpan>,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let mut diagnostics = Vec::with_capacity(warning_facts.len());

    for fact in warning_facts {
        match warning_fact_rule_descriptor(&fact.rule_id) {
            Some(descriptor) if rule_config.is_rule_enabled(descriptor) => {
                diagnostics.push(warning_for_fact(
                    diagram_type,
                    fact,
                    fallback_span.clone(),
                    source_map,
                    descriptor,
                    rule_config,
                ))
            }
            Some(_) => {}
            None => diagnostics.push(
                internal_rule_registry_gap_diagnostic(
                    format!(
                        "unknown warning fact rule id `{}`: {}",
                        fact.rule_id, fact.message
                    ),
                    fallback_span.clone(),
                )
                .with_diagram_type(diagram_type),
            ),
        }
    }

    diagnostics
}

fn warning_for_fact(
    diagram_type: &str,
    fact: DiagramWarningFact,
    fallback_span: Option<DiagnosticSpan>,
    source_map: &SourceMap,
    descriptor: RuleDescriptor,
    rule_config: &AnalysisRuleConfig,
) -> AnalysisDiagnostic {
    let span = warning_fact_span(&fact, source_map, fallback_span);
    let fix = warning_fact_fix(&fact, descriptor, source_map);
    let mut diagnostic = AnalysisDiagnostic::new(
        descriptor.id,
        rule_config.severity_for(descriptor),
        descriptor.category,
        fact.message,
    )
    .with_diagram_type(diagram_type);

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span);
    }

    if let Some(fix) = fix {
        diagnostic = diagnostic.with_fix(fix);
    }

    diagnostic
}

fn warning_fact_span(
    fact: &DiagramWarningFact,
    source_map: &SourceMap,
    fallback_span: Option<DiagnosticSpan>,
) -> Option<DiagnosticSpan> {
    fact.span
        .and_then(|span| source_map.span(span.start, span.end).ok())
        .or(fallback_span)
}

fn warning_fact_fix(
    fact: &DiagramWarningFact,
    descriptor: RuleDescriptor,
    source_map: &SourceMap,
) -> Option<DiagnosticFix> {
    let fix_span = fact.fix_span.or(fact.span)?;
    let fix_span = source_map.span(fix_span.start, fix_span.end).ok()?;
    match descriptor.id {
        FLOWCHART_EXPLICIT_DIRECTION_RULE_ID => Some(
            DiagnosticFix::new(
                "Insert `TB` into the flowchart header",
                vec![DiagnosticFixEdit::new(fix_span, " TB")],
            )
            .preferred(),
        ),
        _ => None,
    }
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

fn init_directive_alias_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    if !rule_config.is_rule_enabled(PREFER_INIT_DIRECTIVE_RULE) {
        return Vec::new();
    }
    if rule_config.is_rule_enabled(PREFER_FRONTMATTER_CONFIG_RULE) {
        return Vec::new();
    }
    let severity = rule_config.severity_for(PREFER_INIT_DIRECTIVE_RULE);

    directive_keyword_spans(source)
        .into_iter()
        .filter_map(|keyword| {
            (source.get(keyword.start..keyword.end) == Some("initialize"))
                .then_some(keyword)
        })
        .filter_map(|keyword| {
            let span = source_map.span(keyword.start, keyword.end).ok()?;
            Some(
                AnalysisDiagnostic::new(
                    PREFER_INIT_DIRECTIVE_RULE.id,
                    severity,
                    PREFER_INIT_DIRECTIVE_RULE.category,
                    "prefer `init` directive keyword over the `initialize` alias",
                )
                .with_span(span.clone())
                .with_help("`initialize` is accepted as an alias; `init` is the canonical Mermaid directive keyword.")
                .with_fix(
                    DiagnosticFix::new(
                        "Replace `initialize` with `init`",
                        vec![DiagnosticFixEdit::new(span, "init")],
                    )
                    .preferred(),
                ),
            )
        })
        .collect()
}

fn prefer_frontmatter_config_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    if !rule_config.is_rule_enabled(PREFER_FRONTMATTER_CONFIG_RULE) {
        return Vec::new();
    }
    let severity = rule_config.severity_for(PREFER_FRONTMATTER_CONFIG_RULE);
    let fix = crate::source_config_rewrite::init_directives_to_frontmatter_fix(source, source_map);

    directive_keyword_spans(source)
        .into_iter()
        .filter_map(|keyword| {
            matches!(source.get(keyword.start..keyword.end), Some("init" | "initialize"))
                .then_some(keyword)
        })
        .filter_map(|keyword| {
            let span = source_map.span(keyword.start, keyword.end).ok()?;
            let mut diagnostic = AnalysisDiagnostic::new(
                PREFER_FRONTMATTER_CONFIG_RULE.id,
                severity,
                PREFER_FRONTMATTER_CONFIG_RULE.category,
                "prefer frontmatter `config` over Mermaid init directives",
            )
            .with_span(span)
            .with_help(
                "Mermaid deprecated directives from v10.5.0; diagram authors should move configuration into the diagram frontmatter `config` block.",
            );
            if let Some(fix) = fix.clone() {
                diagnostic = diagnostic.with_fix(fix);
            }
            Some(diagnostic)
        })
        .collect()
}

fn deprecated_flowchart_html_labels_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    init_matching_paths: &[&[&str]],
    frontmatter_matching_paths: &[&[&str]],
) -> Vec<AnalysisDiagnostic> {
    config_key_diagnostics(
        source,
        source_map,
        rule_config,
        DEPRECATED_FLOWCHART_HTML_LABELS_RULE,
        init_matching_paths,
        frontmatter_matching_paths,
        "`flowchart.htmlLabels` is deprecated; use root-level `htmlLabels` instead",
        "Mermaid keeps `flowchart.htmlLabels` as a compatibility fallback, but root-level `htmlLabels` takes precedence.",
    )
}

fn deprecated_external_diagram_loading_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    config_key_diagnostics(
        source,
        source_map,
        rule_config,
        DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE,
        &DEPRECATED_EXTERNAL_DIAGRAM_LOADING_CONFIG_PATHS,
        &DEPRECATED_EXTERNAL_DIAGRAM_LOADING_FRONTMATTER_CONFIG_PATHS,
        "deprecated external diagram loading config; use `registerExternalDiagrams` instead",
        "Mermaid warns that `lazyLoadedDiagrams` and `loadExternalDiagramsAtStartup` are deprecated in favor of the `registerExternalDiagrams` API.",
    )
}

fn config_key_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    descriptor: RuleDescriptor,
    init_matching_paths: &[&[&str]],
    frontmatter_matching_paths: &[&[&str]],
    message: &'static str,
    help: &'static str,
) -> Vec<AnalysisDiagnostic> {
    if !rule_config.is_rule_enabled(descriptor) {
        return Vec::new();
    }
    let severity = rule_config.severity_for(descriptor);

    let mut spans = init_directive_config_key_spans(source, init_matching_paths);
    spans.extend(frontmatter_config_key_spans(
        source,
        frontmatter_matching_paths,
    ));

    spans
        .into_iter()
        .filter_map(|span| {
            let span = source_map.span(span.start, span.end).ok()?;
            Some(
                AnalysisDiagnostic::new(descriptor.id, severity, descriptor.category, message)
                    .with_span(span)
                    .with_help(help),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
