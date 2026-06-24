use crate::{
    AnalysisDiagnostic, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit, DiagnosticSeverity,
    DiagnosticSpan, SourceMap,
};
use merman_core::{
    BLOCK_WIDTH_WARNING_RULE_ID, DiagramWarningFact, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const PREFER_INIT_DIRECTIVE_RULE_ID: &str = "merman.config.prefer_init_directive";
pub const BLOCK_WIDTH_RULE_ID: &str = "merman.block.width_exceeds_columns";
pub const BLOCK_WARNING_RULE_ID: &str = "merman.block.warning";
pub const GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID: &str = "merman.git_graph.duplicate_commit_id";
pub const GIT_GRAPH_WARNING_RULE_ID: &str = "merman.git_graph.warning";
pub const SEMANTIC_WARNING_RULE_ID: &str = "merman.semantic.warning";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleDescriptor {
    pub id: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub default_enabled: bool,
    pub fixable: bool,
}

const PREFER_INIT_DIRECTIVE_RULE: RuleDescriptor = RuleDescriptor {
    id: PREFER_INIT_DIRECTIVE_RULE_ID,
    default_severity: DiagnosticSeverity::Hint,
    category: DiagnosticCategory::Config,
    default_enabled: true,
    fixable: true,
};

const BLOCK_WIDTH_RULE: RuleDescriptor = RuleDescriptor {
    id: BLOCK_WIDTH_RULE_ID,
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    fixable: false,
};
const BLOCK_WARNING_RULE: RuleDescriptor = RuleDescriptor {
    id: BLOCK_WARNING_RULE_ID,
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    fixable: false,
};
const GIT_GRAPH_DUPLICATE_COMMIT_RULE: RuleDescriptor = RuleDescriptor {
    id: GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID,
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    fixable: false,
};
const GIT_GRAPH_WARNING_RULE: RuleDescriptor = RuleDescriptor {
    id: GIT_GRAPH_WARNING_RULE_ID,
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    fixable: false,
};
const SEMANTIC_WARNING_RULE: RuleDescriptor = RuleDescriptor {
    id: SEMANTIC_WARNING_RULE_ID,
    default_severity: DiagnosticSeverity::Warning,
    category: DiagnosticCategory::Semantic,
    default_enabled: true,
    fixable: false,
};

const RULE_DESCRIPTORS: &[RuleDescriptor] = &[
    PREFER_INIT_DIRECTIVE_RULE,
    BLOCK_WIDTH_RULE,
    BLOCK_WARNING_RULE,
    GIT_GRAPH_DUPLICATE_COMMIT_RULE,
    GIT_GRAPH_WARNING_RULE,
    SEMANTIC_WARNING_RULE,
];

pub fn rule_descriptors() -> &'static [RuleDescriptor] {
    RULE_DESCRIPTORS
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRuleConfig {
    #[serde(default)]
    disabled_rules: BTreeSet<String>,
    #[serde(default)]
    severity_overrides: BTreeMap<String, DiagnosticSeverity>,
}

impl AnalysisRuleConfig {
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

    pub fn disable_rule(&mut self, rule_id: impl Into<String>) {
        self.disabled_rules.insert(rule_id.into());
    }

    pub fn set_rule_severity(&mut self, rule_id: impl Into<String>, severity: DiagnosticSeverity) {
        self.severity_overrides.insert(rule_id.into(), severity);
    }

    pub fn is_rule_enabled(&self, descriptor: RuleDescriptor) -> bool {
        descriptor.default_enabled && !self.disabled_rules.contains(descriptor.id)
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
    init_directive_alias_diagnostics(source, source_map, rule_config)
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

    semantic_warning_fact_diagnostics(diagram_type, warning_facts, span, rule_config)
}

fn semantic_warning_fact_diagnostics(
    diagram_type: &str,
    warning_facts: Vec<DiagramWarningFact>,
    span: Option<DiagnosticSpan>,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    warning_facts
        .into_iter()
        .filter_map(|fact| {
            let descriptor =
                warning_fact_rule_descriptor(diagram_type, &fact.rule_id, &fact.message);
            rule_config
                .is_rule_enabled(descriptor)
                .then_some((descriptor, fact))
        })
        .map(|(descriptor, fact)| {
            warning_for_fact(diagram_type, fact, span.clone(), descriptor, rule_config)
        })
        .collect()
}

fn warning_for_fact(
    diagram_type: &str,
    fact: DiagramWarningFact,
    span: Option<DiagnosticSpan>,
    descriptor: RuleDescriptor,
    rule_config: &AnalysisRuleConfig,
) -> AnalysisDiagnostic {
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

    diagnostic
}

fn semantic_warning_rule_descriptor(diagram_type: &str, message: &str) -> RuleDescriptor {
    match diagram_type {
        "block" | "block-beta" if is_block_width_warning(message) => BLOCK_WIDTH_RULE,
        "block" | "block-beta" => BLOCK_WARNING_RULE,
        "gitGraph" if is_git_graph_duplicate_commit_warning(message) => {
            GIT_GRAPH_DUPLICATE_COMMIT_RULE
        }
        "gitGraph" => GIT_GRAPH_WARNING_RULE,
        _ => SEMANTIC_WARNING_RULE,
    }
}

fn warning_fact_rule_descriptor(
    diagram_type: &str,
    rule_id: &str,
    message: &str,
) -> RuleDescriptor {
    match rule_id {
        BLOCK_WIDTH_WARNING_RULE_ID => BLOCK_WIDTH_RULE,
        GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID => GIT_GRAPH_DUPLICATE_COMMIT_RULE,
        _ => semantic_warning_rule_descriptor(diagram_type, message),
    }
}

fn is_block_width_warning(message: &str) -> bool {
    message.starts_with("Block ") && message.contains(" exceeds configured column width ")
}

fn is_git_graph_duplicate_commit_warning(message: &str) -> bool {
    message.starts_with("Commit ID ") && message.ends_with(" already exists")
}

fn init_directive_alias_diagnostics(
    source: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    if !rule_config.is_rule_enabled(PREFER_INIT_DIRECTIVE_RULE) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ByteSpan {
    start: usize,
    end: usize,
}

fn directive_keyword_spans(source: &str) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = source[cursor..].find("%%{") {
        let directive_start = cursor + relative_start;
        let body_start = directive_start + "%%{".len();
        let Some(relative_end) = source[body_start..].find("}%%") else {
            break;
        };
        let directive_end = body_start + relative_end;
        if let Some(span) = directive_keyword_span(source, body_start, directive_end) {
            spans.push(span);
        }
        cursor = directive_end + "}%%".len();
    }

    spans
}

fn directive_keyword_span(source: &str, body_start: usize, body_end: usize) -> Option<ByteSpan> {
    let body = source.get(body_start..body_end)?;
    let leading = body
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
        .unwrap_or(body.len());
    let keyword_start = body_start + leading;
    let tail = source.get(keyword_start..body_end)?;
    let keyword_len = tail
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_ascii_alphabetic() && ch != '_').then_some(idx))
        .unwrap_or(tail.len());
    if keyword_len == 0 {
        return None;
    }

    let keyword_end = keyword_start + keyword_len;
    let after_keyword = source.get(keyword_end..body_end)?.trim_start();
    if after_keyword.is_empty()
        || after_keyword
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, ':' | '{'))
    {
        Some(ByteSpan {
            start: keyword_start,
            end: keyword_end,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_lint_prefers_init_directive_and_provides_fix() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let diagnostics =
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

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
    fn source_lint_leaves_canonical_init_directive_alone() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(
            source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default()).is_empty()
        );
    }

    #[test]
    fn rule_config_can_disable_source_lint_rules() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config =
            AnalysisRuleConfig::default().with_rule_disabled(PREFER_INIT_DIRECTIVE_RULE_ID);

        assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
    }

    #[test]
    fn rule_config_can_override_rule_severity() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = AnalysisRuleConfig::default()
            .with_rule_severity(PREFER_INIT_DIRECTIVE_RULE_ID, DiagnosticSeverity::Warning);

        let diagnostics = source_lint_diagnostics(source, &source_map, &config);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
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

        assert_eq!(descriptors.len(), 6);
        assert_eq!(descriptors[0].id, PREFER_INIT_DIRECTIVE_RULE_ID);
        assert_eq!(descriptors[0].default_severity, DiagnosticSeverity::Hint);
        assert_eq!(descriptors[0].category, DiagnosticCategory::Config);
        assert!(descriptors[0].default_enabled);
        assert!(descriptors[0].fixable);
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == BLOCK_WIDTH_RULE_ID)
        );
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.id == GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID)
        );
    }

    #[test]
    fn directive_keyword_spans_ignore_unterminated_directives() {
        assert!(directive_keyword_spans("%%{ initialize: {\"theme\":\"dark\"}").is_empty());
    }
}
