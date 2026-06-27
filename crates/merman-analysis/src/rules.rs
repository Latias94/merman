use crate::{
    AnalysisDiagnostic, AnalysisStatus, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticSeverity, DiagnosticSpan, SourceMap,
    source_directives::{directive_keyword_spans, init_directive_config_key_spans},
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
pub const BLOCK_WIDTH_RULE_ID: &str = "merman.block.width_exceeds_columns";
pub const FLOWCHART_EXPLICIT_DIRECTION_RULE_ID: &str =
    "merman.authoring.flowchart.explicit_direction";
pub const FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID: &str =
    "merman.semantic.flowchart.unknown_style_target";
pub const GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID: &str = "merman.git_graph.duplicate_commit_id";
pub const SEMANTIC_WARNING_RULE_ID: &str = "merman.semantic.warning";

const DEPRECATED_FLOWCHART_HTML_LABELS_CONFIG_PATHS: [&[&str]; 2] = [
    &["flowchart", "htmlLabels"],
    &["config", "flowchart", "htmlLabels"],
];
const DEPRECATED_EXTERNAL_DIAGRAM_LOADING_CONFIG_PATHS: [&[&str]; 2] =
    [&["lazyLoadedDiagrams"], &["loadExternalDiagramsAtStartup"]];

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
            configurable: descriptor.category != DiagnosticCategory::Internal,
            fixable: descriptor.fixable,
        }
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
    fixable: true,
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
        "crates/merman-core/src/editor.rs",
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
    evidence: &[
        "docs/adr/0070-diagnostics-first-analysis-contract.md",
        "crates/merman-analysis/src/analyzer.rs",
    ],
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
    evidence: &[
        "docs/adr/0072-lint-rule-governance.md",
        "crates/merman-analysis/src/rules.rs",
    ],
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
        "crates/merman-core/src/diagrams/block.rs",
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
        "crates/merman-core/src/diagrams/flowchart.rs",
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
        "crates/merman-core/src/diagrams/flowchart/build.rs",
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
        "crates/merman-core/src/diagrams/git_graph.rs",
    ],
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    default_profile: AnalysisRuleProfile::Core,
    origin: RuleOrigin::MermaidCompatibility,
    fixable: false,
};
const SEMANTIC_WARNING_RULE: RuleDescriptor = RuleDescriptor {
    id: SEMANTIC_WARNING_RULE_ID,
    description: "Project registered diagram-family semantic warnings that do not yet have a family-specific rule.",
    evidence: &[
        "crates/merman-core/src/diagram/mod.rs",
        "docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md",
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
    BLOCK_WIDTH_RULE,
    FLOWCHART_EXPLICIT_DIRECTION_RULE,
    FLOWCHART_UNKNOWN_STYLE_TARGET_RULE,
    GIT_GRAPH_DUPLICATE_COMMIT_RULE,
    SEMANTIC_WARNING_RULE,
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

pub fn rule_catalog_json_bytes() -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&rule_catalog())
}

pub fn configurable_rule_catalog_json_bytes() -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&configurable_rule_catalog())
}

pub fn configurable_rule_descriptors() -> impl Iterator<Item = RuleDescriptor> {
    RULE_DESCRIPTORS
        .iter()
        .copied()
        .filter(|descriptor| descriptor.category != DiagnosticCategory::Internal)
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
    ));
    diagnostics.extend(deprecated_external_diagram_loading_diagnostics(
        source,
        source_map,
        rule_config,
    ));
    diagnostics
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
        SEMANTIC_WARNING_RULE_ID => Some(SEMANTIC_WARNING_RULE),
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
) -> Vec<AnalysisDiagnostic> {
    let fix = crate::source_config_rewrite::flowchart_html_labels_to_root_fix(source, source_map);
    init_directive_config_key_diagnostics(
        source,
        source_map,
        rule_config,
        DEPRECATED_FLOWCHART_HTML_LABELS_RULE,
        &DEPRECATED_FLOWCHART_HTML_LABELS_CONFIG_PATHS,
        "`flowchart.htmlLabels` is deprecated; use root-level `htmlLabels` instead",
        "Mermaid keeps `flowchart.htmlLabels` as a compatibility fallback, but root-level `htmlLabels` takes precedence.",
    )
    .into_iter()
    .map(|mut diagnostic| {
        if let Some(fix) = fix.clone() {
            diagnostic = diagnostic.with_fix(fix);
        }
        diagnostic
    })
    .collect()
}

fn deprecated_external_diagram_loading_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    init_directive_config_key_diagnostics(
        source,
        source_map,
        rule_config,
        DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE,
        &DEPRECATED_EXTERNAL_DIAGRAM_LOADING_CONFIG_PATHS,
        "deprecated external diagram loading config; use `registerExternalDiagrams` instead",
        "Mermaid warns that `lazyLoadedDiagrams` and `loadExternalDiagramsAtStartup` are deprecated in favor of the `registerExternalDiagrams` API.",
    )
}

fn init_directive_config_key_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
    descriptor: RuleDescriptor,
    matching_paths: &[&[&str]],
    message: &'static str,
    help: &'static str,
) -> Vec<AnalysisDiagnostic> {
    if !rule_config.is_rule_enabled(descriptor) {
        return Vec::new();
    }
    let severity = rule_config.severity_for(descriptor);

    init_directive_config_key_spans(source, matching_paths)
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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_lint_prefers_init_directive_and_provides_fix() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);
        let config = config.with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.id, PREFER_INIT_DIRECTIVE_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
        let span = diagnostic.span.as_ref().expect("keyword span");
        assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
        assert_eq!(diagnostic.fixes.len(), 1);
        assert_eq!(
            diagnostic.fixes[0].title,
            "Replace `initialize` with `init`"
        );
        assert!(diagnostic.fixes[0].is_preferred);
        assert_eq!(diagnostic.fixes[0].edits.len(), 1);
        assert_eq!(diagnostic.fixes[0].edits[0].replacement, "init");
        assert_eq!(
            diagnostic.fixes[0].edits[0].span.byte_start,
            span.byte_start
        );
    }

    #[test]
    fn source_lint_prefers_frontmatter_config_over_init_directive() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.id, PREFER_FRONTMATTER_CONFIG_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        assert_eq!(diagnostic.fixes.len(), 1);
        assert_eq!(
            diagnostic.fixes[0].title,
            "Move init directive config into frontmatter"
        );
        assert!(diagnostic.fixes[0].is_preferred);
        assert_eq!(diagnostic.fixes[0].edits.len(), 1);
        assert!(
            diagnostic.fixes[0].edits[0]
                .replacement
                .starts_with("---\nconfig:\n")
        );
        assert!(
            diagnostic.fixes[0].edits[0]
                .replacement
                .contains("theme: dark\n")
        );
        let span = diagnostic.span.as_ref().expect("directive keyword span");
        assert_eq!(&source[span.byte_start..span.byte_end], "init");
    }

    #[test]
    fn source_lint_prefers_frontmatter_config_over_initialize_directive() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled(PREFER_INIT_DIRECTIVE_RULE_ID);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.id, PREFER_FRONTMATTER_CONFIG_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        assert_eq!(diagnostic.fixes.len(), 1);
        assert_eq!(
            diagnostic.fixes[0].title,
            "Move init directive config into frontmatter"
        );
        assert!(diagnostic.fixes[0].is_preferred);
        assert_eq!(diagnostic.fixes[0].edits.len(), 1);
        assert!(
            diagnostic.fixes[0].edits[0]
                .replacement
                .starts_with("---\nconfig:\n")
        );
        assert!(
            diagnostic.fixes[0].edits[0]
                .replacement
                .contains("theme: dark\n")
        );
        let span = diagnostic.span.as_ref().expect("directive keyword span");
        assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
    }

    #[test]
    fn source_lint_leaves_canonical_init_directive_alone() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

        assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
    }

    #[test]
    fn source_authoring_lints_are_not_enabled_by_core_profile() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default()).is_empty()
        );
    }

    #[test]
    fn rule_config_can_disable_source_lint_rules() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled(PREFER_INIT_DIRECTIVE_RULE_ID)
            .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

        assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
    }

    #[test]
    fn rule_config_can_enable_authoring_rules_without_recommended_profile() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default().with_rule_enabled(PREFER_INIT_DIRECTIVE_RULE_ID);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, PREFER_INIT_DIRECTIVE_RULE_ID);
    }

    #[test]
    fn rule_config_can_override_rule_severity() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .with_rule_severity(PREFER_INIT_DIRECTIVE_RULE_ID, DiagnosticSeverity::Warning);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn source_lint_reports_deprecated_flowchart_html_labels_directive() {
        let source = "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics =
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        assert_eq!(diagnostic.fixes.len(), 1);
        assert_eq!(
            diagnostic.fixes[0].title,
            "Move deprecated `flowchart.htmlLabels` to root `htmlLabels`"
        );
        assert!(diagnostic.fixes[0].is_preferred);
        assert_eq!(diagnostic.fixes[0].edits.len(), 2);
        assert!(
            diagnostic.fixes[0].edits[0]
                .replacement
                .contains("htmlLabels: false")
        );
        let span = diagnostic.span.as_ref().expect("htmlLabels span");
        assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
    }

    #[test]
    fn source_lint_reports_config_wrapped_flowchart_html_labels_directive() {
        let source = "%%{init: { \"config\": { \"flowchart\": { \"htmlLabels\": true } } }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics =
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
        assert_eq!(diagnostics[0].fixes.len(), 1);
        assert_eq!(
            diagnostics[0].fixes[0].title,
            "Move deprecated `flowchart.htmlLabels` to root `htmlLabels`"
        );
        let span = diagnostics[0].span.as_ref().expect("htmlLabels span");
        assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
    }

    #[test]
    fn source_lint_does_not_report_class_html_labels_without_deprecation_evidence() {
        let source =
            "%%{init: { \"class\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
        let source_map = SourceMap::new(source);

        let diagnostics =
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn source_lint_leaves_root_html_labels_alone() {
        let source = "%%{init: { \"htmlLabels\": false, \"flowchart\": { \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default()).is_empty()
        );
    }

    #[test]
    fn rule_config_can_disable_deprecated_flowchart_html_labels_rule() {
        let source =
            "%%{init: { \"flowchart\": { \"htmlLabels\": false } }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_rule_disabled(DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);

        assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
    }

    #[test]
    fn source_lint_reports_deprecated_external_diagram_loading_directive_config() {
        let source = "%%{init: { \"lazyLoadedDiagrams\": true, \"loadExternalDiagramsAtStartup\": false }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics =
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID
                && diagnostic.severity == DiagnosticSeverity::Warning
                && diagnostic.category == DiagnosticCategory::Config
                && diagnostic.fixes.is_empty()
        }));
        let spans: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                let span = diagnostic.span.as_ref().expect("deprecated key span");
                &source[span.byte_start..span.byte_end]
            })
            .collect();
        assert_eq!(
            spans,
            vec!["lazyLoadedDiagrams", "loadExternalDiagramsAtStartup"]
        );
    }

    #[test]
    fn rule_config_can_disable_deprecated_external_diagram_loading_rule() {
        let source = "%%{init: { \"lazyLoadedDiagrams\": true }}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_rule_disabled(DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID);

        assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
    }

    #[test]
    fn rule_config_can_disable_block_warning_rules() {
        let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default().with_rule_disabled(BLOCK_WIDTH_RULE_ID);

        let diagnostics = semantic_warning_diagnostics(
            "block",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                        "message": "Block A exceeds configured column width 1"
                    }
                ]
            }),
            &source_map,
            &config,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn semantic_warning_facts_use_rule_ids_when_present() {
        let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let source_map = SourceMap::new(source);

        let diagnostics = semantic_warning_diagnostics(
            "block",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                        "message": "Block A exceeds configured column width 1"
                    }
                ]
            }),
            &source_map,
            &AnalysisRuleConfig::default(),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn semantic_warning_facts_map_flowchart_missing_direction_rule_id() {
        let source = "flowchart\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);

        let diagnostics = semantic_warning_diagnostics(
            "flowchart-v2",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
                        "message": "flowchart headers should declare an explicit direction",
                        "span": { "start": 0, "end": 9 },
                        "fixSpan": { "start": 9, "end": 9 }
                    }
                ]
            }),
            &source_map,
            &config,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, FLOWCHART_EXPLICIT_DIRECTION_RULE_ID);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Hint);
        assert_eq!(diagnostics[0].category, DiagnosticCategory::Semantic);
        assert_eq!(diagnostics[0].diagram_type.as_deref(), Some("flowchart-v2"));
        assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_start, 0);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_end, 9);
        assert_eq!(diagnostics[0].fixes.len(), 1);
        assert_eq!(
            diagnostics[0].fixes[0].title,
            "Insert `TB` into the flowchart header"
        );
        assert!(diagnostics[0].fixes[0].is_preferred);
        assert_eq!(diagnostics[0].fixes[0].edits[0].replacement, " TB");
        assert_eq!(diagnostics[0].fixes[0].edits[0].span.byte_start, 9);
        assert_eq!(diagnostics[0].fixes[0].edits[0].span.byte_end, 9);
    }

    #[test]
    fn semantic_warning_facts_map_flowchart_unknown_style_target_rule_id() {
        let source = "flowchart TD\nstyle Q background:#fff\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics = semantic_warning_diagnostics(
            "flowchart-v2",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
                        "message": "Style applied to unknown node \"Q\". This may indicate a typo. The node will be created automatically.",
                        "span": { "start": 19, "end": 20 }
                    }
                ]
            }),
            &source_map,
            &AnalysisRuleConfig::default(),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].category, DiagnosticCategory::Semantic);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_start, 19);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_end, 20);
    }

    #[test]
    fn semantic_authoring_warning_facts_are_not_enabled_by_core_profile() {
        let source = "flowchart\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics = semantic_warning_diagnostics(
            "flowchart-v2",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
                        "message": "flowchart headers should declare an explicit direction",
                        "span": { "start": 0, "end": 9 },
                        "fixSpan": { "start": 9, "end": 9 }
                    }
                ]
            }),
            &source_map,
            &AnalysisRuleConfig::default(),
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn rule_config_can_override_block_warning_severity() {
        let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_rule_severity(BLOCK_WIDTH_RULE_ID, DiagnosticSeverity::Hint);

        let diagnostics = semantic_warning_diagnostics(
            "block",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                        "message": "Block A exceeds configured column width 1"
                    }
                ]
            }),
            &source_map,
            &config,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Hint);
        assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
    }

    #[test]
    fn rule_descriptors_expose_stable_rule_metadata() {
        let descriptors = rule_descriptors();

        assert_eq!(descriptors.len(), 19);
        assert_eq!(descriptors[0].id, PREFER_INIT_DIRECTIVE_RULE_ID);
        assert!(descriptors[0].description.contains("canonical `init`"));
        assert_eq!(descriptors[0].default_severity, DiagnosticSeverity::Hint);
        assert_eq!(descriptors[0].category, DiagnosticCategory::Config);
        assert!(!descriptors[0].default_enabled);
        assert_eq!(
            descriptors[0].default_profile,
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(descriptors[0].origin, RuleOrigin::MermanAuthoring);
        assert!(descriptors[0].fixable);
        let prefer_frontmatter = descriptors
            .iter()
            .find(|descriptor| descriptor.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .expect("prefer frontmatter config descriptor");
        assert!(
            prefer_frontmatter
                .description
                .contains("frontmatter `config`")
        );
        assert_eq!(prefer_frontmatter.origin, RuleOrigin::MermanAuthoring);
        assert_eq!(
            prefer_frontmatter.default_profile,
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            prefer_frontmatter.default_severity,
            DiagnosticSeverity::Hint
        );
        assert_eq!(prefer_frontmatter.category, DiagnosticCategory::Config);
        assert!(!prefer_frontmatter.default_enabled);
        assert!(prefer_frontmatter.fixable);
        assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
        assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md")
        );
        let deprecated_html_labels = descriptors
            .iter()
            .find(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
            .expect("deprecated htmlLabels descriptor");
        assert_eq!(
            deprecated_html_labels.origin,
            RuleOrigin::MermaidCompatibility
        );
        assert!(deprecated_html_labels.default_enabled);
        assert_eq!(
            deprecated_html_labels.default_profile,
            AnalysisRuleProfile::Core
        );
        assert_eq!(
            deprecated_html_labels.default_severity,
            DiagnosticSeverity::Warning
        );
        assert_eq!(deprecated_html_labels.category, DiagnosticCategory::Config);
        assert!(deprecated_html_labels.fixable);
        assert!(
            deprecated_html_labels
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == NO_DIAGRAM_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == DIAGRAM_PARSE_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == UNSUPPORTED_DIAGRAM_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == RECOVERED_EDITOR_FACTS_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == RESOURCE_LIMIT_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == MALFORMED_FRONT_MATTER_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == INVALID_DIRECTIVE_JSON_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == INVALID_FRONT_MATTER_YAML_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == PANIC_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == BLOCK_WIDTH_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID)
        );
        let deprecated_external_loading = descriptors
            .iter()
            .find(|descriptor| descriptor.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID)
            .expect("deprecated external diagram loading descriptor");
        assert_eq!(
            deprecated_external_loading.origin,
            RuleOrigin::MermaidCompatibility
        );
        assert!(deprecated_external_loading.default_enabled);
        assert_eq!(
            deprecated_external_loading.default_profile,
            AnalysisRuleProfile::Core
        );
        assert_eq!(
            deprecated_external_loading.default_severity,
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            deprecated_external_loading.category,
            DiagnosticCategory::Config
        );
        assert!(!deprecated_external_loading.fixable);
        assert!(
            deprecated_external_loading
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts")
        );
        assert!(
            descriptors
                .iter()
                .find(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
                .is_some_and(|descriptor| {
                    descriptor.fixable
                        && !descriptor.default_enabled
                        && descriptor.default_profile == AnalysisRuleProfile::Recommended
                        && descriptor.origin == RuleOrigin::MermanAuthoring
                })
        );
        let flowchart_unknown_style = descriptors
            .iter()
            .find(|descriptor| descriptor.id == FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID)
            .expect("flowchart unknown style target descriptor");
        assert_eq!(
            flowchart_unknown_style.default_severity,
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            flowchart_unknown_style.category,
            DiagnosticCategory::Semantic
        );
        assert!(flowchart_unknown_style.default_enabled);
        assert_eq!(
            flowchart_unknown_style.default_profile,
            AnalysisRuleProfile::Core
        );
        assert_eq!(
            flowchart_unknown_style.origin,
            RuleOrigin::MermaidCompatibility
        );
        assert!(!flowchart_unknown_style.fixable);
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == SEMANTIC_WARNING_RULE_ID)
        );
    }

    #[test]
    fn semantic_warning_facts_use_rule_ids_even_when_messages_differ() {
        let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let source_map = SourceMap::new(source);

        let diagnostics = semantic_warning_diagnostics(
            "block",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                        "message": "this message does not need to mention width"
                    }
                ]
            }),
            &source_map,
            &AnalysisRuleConfig::default(),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
    }

    #[test]
    fn semantic_warning_facts_surface_unknown_rule_ids_as_internal_errors() {
        let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let source_map = SourceMap::new(source);

        let diagnostics = semantic_warning_diagnostics(
            "block",
            &json!({
                "warningFacts": [
                    {
                        "ruleId": "merman.block.unregistered_warning",
                        "message": "Block A emitted a future warning"
                    }
                ]
            }),
            &source_map,
            &AnalysisRuleConfig::default(),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, INTERNAL_RULE_REGISTRY_GAP_RULE_ID);
        assert_eq!(diagnostics[0].category, DiagnosticCategory::Internal);
        assert_eq!(
            diagnostics[0].code,
            Some(AnalysisStatus::InternalError.code())
        );
    }

    #[test]
    fn configurable_rule_descriptors_exclude_internal_rules() {
        let descriptors: Vec<_> = configurable_rule_descriptors().collect();

        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.category != DiagnosticCategory::Internal)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == BLOCK_WIDTH_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == SEMANTIC_WARNING_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.id != PANIC_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.id != INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
        );
    }

    #[test]
    fn rule_catalog_serializes_public_rule_metadata() {
        let catalog = rule_catalog();
        let prefer_init = catalog
            .iter()
            .find(|entry| entry.id == PREFER_INIT_DIRECTIVE_RULE_ID)
            .expect("prefer init catalog entry");

        assert!(prefer_init.description.contains("canonical `init`"));
        assert_eq!(prefer_init.origin, RuleOrigin::MermanAuthoring);
        assert_eq!(
            prefer_init.default_profile,
            AnalysisRuleProfile::Recommended
        );
        assert!(prefer_init.configurable);
        assert!(prefer_init.fixable);
        assert!(
            prefer_init
                .evidence
                .contains(&"docs/adr/0072-lint-rule-governance.md")
        );
        let prefer_frontmatter = catalog
            .iter()
            .find(|entry| entry.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .expect("prefer frontmatter catalog entry");
        assert!(
            prefer_frontmatter
                .description
                .contains("frontmatter `config`")
        );
        assert_eq!(prefer_frontmatter.origin, RuleOrigin::MermanAuthoring);
        assert_eq!(
            prefer_frontmatter.default_profile,
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            prefer_frontmatter.default_severity,
            DiagnosticSeverity::Hint
        );
        assert_eq!(prefer_frontmatter.category, DiagnosticCategory::Config);
        assert!(prefer_frontmatter.configurable);
        assert!(prefer_frontmatter.fixable);
        assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
        assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md")
        );
        assert!(catalog.iter().all(|entry| !entry.evidence.is_empty()));
        let deprecated_html_labels = catalog
            .iter()
            .find(|entry| entry.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
            .expect("deprecated htmlLabels catalog entry");
        assert_eq!(
            deprecated_html_labels.origin,
            RuleOrigin::MermaidCompatibility
        );
        assert!(deprecated_html_labels.default_enabled);
        assert!(!deprecated_html_labels.fixable);

        let json: serde_json::Value =
            serde_json::from_slice(&rule_catalog_json_bytes().expect("catalog JSON"))
                .expect("catalog should serialize as JSON");
        let first = json.as_array().expect("catalog array").first().unwrap();
        assert_eq!(first["id"], PREFER_INIT_DIRECTIVE_RULE_ID);
        assert_eq!(first["origin"], "merman_authoring");
        assert_eq!(first["default_profile"], "recommended");
        assert_eq!(first["default_severity"], "hint");
        assert_eq!(first["category"], "config");
        assert_eq!(first["configurable"], true);
        assert_eq!(first["fixable"], true);
        assert!(
            first["evidence"]
                .as_array()
                .expect("evidence array")
                .iter()
                .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
        );
    }

    #[test]
    fn configurable_rule_catalog_excludes_internal_rules() {
        let catalog = configurable_rule_catalog();

        assert!(
            catalog
                .iter()
                .all(|entry| entry.category != DiagnosticCategory::Internal)
        );
        assert!(
            catalog
                .iter()
                .any(|entry| entry.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
        );
        assert!(
            catalog
                .iter()
                .any(|entry| entry.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
        );
        assert!(
            catalog
                .iter()
                .any(|entry| entry.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
        );
        assert!(
            catalog
                .iter()
                .all(|entry| entry.id != INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
        );
    }
}
