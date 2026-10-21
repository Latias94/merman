use std::collections::BTreeSet;

#[cfg(any(feature = "diagram-packet", feature = "diagram-radar"))]
use crate::error::Error;
pub use crate::generated::editor_rename_policy::EditorRenamePolicy;
use crate::{OperationControl, OperationControlResult};

pub use crate::source_span::SourceSpan;

/// Public shape names supported by the compiled Flowchart grammar.
///
/// Editor consumers use the core feature union, including when another dependency enables the
/// shared Flowchart/Swimlane grammar independently of the editor's local feature selection.
/// Returns an empty iterator when that grammar is not compiled.
pub fn flowchart_shape_names() -> impl Iterator<Item = &'static str> {
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    {
        crate::diagrams::flowchart::flowchart_public_shape_names()
    }
    #[cfg(not(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )))]
    {
        std::iter::empty()
    }
}

/// Protocol-independent symbol classification for editor-facing consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorSemanticKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

/// Typed family semantics consumed by editor projections.
///
/// Parser families own this classification. Downstream editor layers must not recover it from a
/// diagram type name or other display string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorFamilySemantics {
    outline_kind: EditorSemanticKind,
}

impl Default for EditorFamilySemantics {
    fn default() -> Self {
        Self {
            outline_kind: EditorSemanticKind::Variable,
        }
    }
}

impl EditorFamilySemantics {
    pub(crate) const fn new(outline_kind: EditorSemanticKind) -> Self {
        Self { outline_kind }
    }

    pub const fn outline_kind(self) -> EditorSemanticKind {
        self.outline_kind
    }
}

/// How downstream editor indexes should project a parser-produced symbol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorSemanticRole {
    /// Addressable diagram entity: appears in completion, navigation, and outline surfaces.
    #[default]
    Entity,
    /// A parser-declared CSS/class definition. It participates in class completion and outline
    /// projection, but is not a diagram-node entity or a reference/rename target.
    ClassDefinition,
    /// A source occurrence that resolves to an addressable entity. References participate in
    /// navigation and rename, but never become completion or outline declarations themselves.
    Reference,
    /// Structural symbol that belongs in outline/hover, but is not a graph-node completion item.
    Outline,
    /// Span-rich parser payload for lint or future semantic consumers; not projected into LSP
    /// outline/completion/navigation by the migration index.
    Payload,
}

impl EditorRenamePolicy {
    pub fn is_renameable(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn accepts(self, candidate: &str) -> bool {
        match self {
            Self::None => false,
            Self::Identifier => {
                !candidate.is_empty()
                    && candidate
                        .chars()
                        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
            }
            Self::QualifiedIdentifier => {
                !candidate.is_empty() && candidate.split('.').all(is_ascii_identifier)
            }
            Self::EventModelingId => is_ascii_identifier(candidate),
            Self::EventModelingFrameId => {
                (1..=3).contains(&candidate.len())
                    && candidate.bytes().all(|byte| byte.is_ascii_digit())
            }
            #[cfg(not(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            )))]
            Self::FlowchartNodeId => false,
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::FlowchartNodeId => crate::diagrams::flowchart::is_valid_editor_node_id(candidate),
            #[cfg(not(feature = "diagram-git-graph"))]
            Self::GitGraphReference => false,
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraphReference => {
                crate::diagrams::git_graph::is_valid_editor_reference(candidate)
            }
            #[cfg(not(feature = "diagram-architecture"))]
            Self::ArchitectureIdentifier => false,
            #[cfg(feature = "diagram-architecture")]
            Self::ArchitectureIdentifier => {
                crate::diagrams::architecture::is_valid_editor_identifier(candidate)
            }
            #[cfg(not(feature = "diagram-railroad"))]
            Self::RailroadIrRule => false,
            #[cfg(feature = "diagram-railroad")]
            Self::RailroadIrRule => {
                crate::diagrams::railroad::is_valid_editor_ir_rule_identifier(candidate)
            }
            #[cfg(not(feature = "diagram-railroad"))]
            Self::RailroadEbnfRule => false,
            #[cfg(feature = "diagram-railroad")]
            Self::RailroadEbnfRule => {
                crate::diagrams::railroad::is_valid_editor_ebnf_rule_identifier(candidate)
            }
            #[cfg(not(feature = "diagram-railroad"))]
            Self::RailroadPegRule => false,
            #[cfg(feature = "diagram-railroad")]
            Self::RailroadPegRule => {
                crate::diagrams::railroad::is_valid_editor_peg_rule_identifier(candidate)
            }
            #[cfg(not(feature = "diagram-railroad"))]
            Self::RailroadAbnfRule => false,
            #[cfg(feature = "diagram-railroad")]
            Self::RailroadAbnfRule => {
                crate::diagrams::railroad::is_valid_editor_abnf_rule_identifier(candidate)
            }
            #[cfg(not(feature = "diagram-agentflow"))]
            Self::AgentflowNodeId => false,
            #[cfg(feature = "diagram-agentflow")]
            Self::AgentflowNodeId => crate::diagrams::agentflow::is_valid_editor_node_id(candidate),
            #[cfg(not(feature = "diagram-usecase"))]
            Self::UsecaseIdentifier => false,
            #[cfg(feature = "diagram-usecase")]
            Self::UsecaseIdentifier => {
                crate::diagrams::usecase::is_valid_editor_identifier(candidate)
            }
        }
    }
}

fn is_ascii_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

impl EditorSemanticRole {
    pub fn contributes_completion(self) -> bool {
        matches!(self, Self::Entity | Self::ClassDefinition)
    }

    pub fn contributes_references(self) -> bool {
        matches!(self, Self::Entity | Self::Reference)
    }

    pub fn contributes_outline(self) -> bool {
        matches!(self, Self::Entity | Self::ClassDefinition | Self::Outline)
    }

    pub const fn is_class_definition(self) -> bool {
        matches!(self, Self::ClassDefinition)
    }
}

/// A parser-produced symbol occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EditorSemanticSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSemanticKind,
    pub role: EditorSemanticRole,
    pub rename_policy: EditorRenamePolicy,
    pub span: SourceSpan,
    pub selection: SourceSpan,
}

impl EditorSemanticSymbol {
    pub fn new(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Entity,
            span,
            selection,
        )
    }

    pub fn outline(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Outline,
            span,
            selection,
        )
    }

    pub fn class_definition(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::ClassDefinition,
            span,
            selection,
        )
    }

    pub fn reference(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Reference,
            span,
            selection,
        )
    }

    pub fn payload(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Payload,
            span,
            selection,
        )
    }

    pub fn with_role(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        role: EditorSemanticRole,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        let rename_policy = if matches!(
            role,
            EditorSemanticRole::Entity | EditorSemanticRole::Reference
        ) {
            EditorRenamePolicy::Identifier
        } else {
            EditorRenamePolicy::None
        };
        Self {
            name: name.into(),
            detail,
            kind,
            role,
            rename_policy,
            span,
            selection,
        }
    }

    pub fn with_rename_policy(mut self, rename_policy: EditorRenamePolicy) -> Self {
        self.rename_policy = rename_policy;
        self
    }
}

/// Parser-backed diagnostic emitted while producing editor-visible semantic facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSemanticDiagnostic {
    pub message: String,
    pub span: Option<SourceSpan>,
    pub kind: EditorSemanticDiagnosticKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSemanticDiagnosticKind {
    ParserRecovery,
    ParserWarning,
}

impl EditorSemanticDiagnostic {
    pub fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
            kind: EditorSemanticDiagnosticKind::ParserWarning,
        }
    }

    pub fn parser_recovery(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
            kind: EditorSemanticDiagnosticKind::ParserRecovery,
        }
    }
}

/// Parser-known syntax category that is expected at a source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorExpectedSyntaxKind {
    Directive,
    Frontmatter,
    IdList,
    NodeIdentifier,
    ClassName,
    FlowchartOperator,
    ShapeValue,
    ShapeTrigger,
    FlowchartDirectionValue,
    CardinalDirectionValue,
    BlockDirectionValue,
    StyleValue,
    InteractionAction,
    Payload,
}

/// Parser-produced cursor context hint for completion and other editor features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorExpectedSyntax {
    pub kind: EditorExpectedSyntaxKind,
    pub span: SourceSpan,
}

impl EditorExpectedSyntax {
    pub fn new(kind: EditorExpectedSyntaxKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

/// Whether editor-facing facts came from a complete family parse or a recoverable partial parse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorSemanticCompleteness {
    #[default]
    Complete,
    Recovered,
}

/// Parser-produced facts used by lint, completion, and LSP without exposing a public AST.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct EditorSemanticFacts {
    pub completeness: EditorSemanticCompleteness,
    pub family_semantics: EditorFamilySemantics,
    pub symbols: Vec<EditorSemanticSymbol>,
    pub directive_prefixes: Vec<String>,
    pub diagnostics: Vec<EditorSemanticDiagnostic>,
    pub expected_syntax: Vec<EditorExpectedSyntax>,
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "Projection work is observed by the family demand tests."
)]
pub(crate) mod test_work {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct Counts {
        pub(crate) collections: usize,
        pub(crate) symbols: usize,
        pub(crate) directives: usize,
        pub(crate) diagnostics: usize,
        pub(crate) expected_syntax: usize,
    }

    thread_local! {
        static COUNTS: Cell<Counts> = const { Cell::new(Counts {
            collections: 0,
            symbols: 0,
            directives: 0,
            diagnostics: 0,
            expected_syntax: 0,
        }) };
    }

    pub(crate) fn reset() {
        COUNTS.set(Counts::default());
    }

    pub(crate) fn read() -> Counts {
        COUNTS.get()
    }

    pub(crate) fn record_collection() {
        update(|work| work.collections += 1);
    }

    pub(super) fn update(record: impl FnOnce(&mut Counts)) {
        let mut work = COUNTS.get();
        record(&mut work);
        COUNTS.set(work);
    }
}

impl EditorSemanticFacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_symbol(&mut self, symbol: EditorSemanticSymbol) {
        #[cfg(test)]
        test_work::update(|work| work.symbols += 1);
        self.symbols.push(symbol);
    }

    pub fn mark_recovered(&mut self) {
        self.completeness = EditorSemanticCompleteness::Recovered;
    }

    pub fn mark_recovered_with_diagnostic(
        &mut self,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        self.mark_recovered();
        self.push_diagnostic(message, span);
    }

    pub fn mark_recovered_from_parse_error(
        &mut self,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        self.mark_recovered();
        #[cfg(test)]
        test_work::update(|work| work.diagnostics += 1);
        self.diagnostics
            .push(EditorSemanticDiagnostic::parser_recovery(message, span));
    }

    pub fn push_diagnostic(&mut self, message: impl Into<String>, span: Option<SourceSpan>) {
        #[cfg(test)]
        test_work::update(|work| work.diagnostics += 1);
        self.diagnostics
            .push(EditorSemanticDiagnostic::new(message, span));
    }

    pub fn push_directive_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if !self.directive_prefixes.contains(&prefix) {
            #[cfg(test)]
            test_work::update(|work| work.directives += 1);
            self.directive_prefixes.push(prefix);
        }
    }

    pub fn push_expected_syntax(&mut self, expected: EditorExpectedSyntax) {
        #[cfg(test)]
        test_work::update(|work| work.expected_syntax += 1);
        self.expected_syntax.push(expected);
    }

    pub(crate) fn finalize_expected_syntax_controlled(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut seen = BTreeSet::new();
        let mut unique = Vec::with_capacity(self.expected_syntax.len());
        for (index, expected) in std::mem::take(&mut self.expected_syntax)
            .into_iter()
            .enumerate()
        {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if seen.insert((expected.kind, expected.span.start, expected.span.end)) {
                unique.push(expected);
            }
        }
        self.expected_syntax = unique;
        control.checkpoint()
    }
}

#[cfg(any(feature = "diagram-packet", feature = "diagram-radar"))]
pub(crate) fn ensure_editor_recovery_from_error(
    mut facts: EditorSemanticFacts,
    error: &Error,
    fallback_span: SourceSpan,
) -> EditorSemanticFacts {
    let (message, span) = match error {
        Error::DiagramParse { diagnostic, .. } => (
            diagnostic.message().to_string(),
            diagnostic.span().unwrap_or(fallback_span),
        ),
        other => (other.to_string(), fallback_span),
    };
    let already_reported = facts.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
            && diagnostic.message == message
            && diagnostic.span == Some(span)
    });
    if already_reported {
        facts.mark_recovered();
        return facts;
    }

    facts.mark_recovered_from_parse_error(message, Some(span));
    facts
}

#[cfg(test)]
mod tests {
    use super::{EditorRenamePolicy, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol};

    #[test]
    #[cfg(all(
        feature = "diagram-flowchart",
        feature = "diagram-git-graph",
        feature = "diagram-architecture",
        feature = "diagram-railroad"
    ))]
    fn rename_policies_follow_family_identifier_grammars() {
        assert!(EditorRenamePolicy::FlowchartNodeId.accepts("foo.bar"));
        assert!(EditorRenamePolicy::FlowchartNodeId.accepts("foo-bar"));
        assert!(!EditorRenamePolicy::FlowchartNodeId.accepts("foo--bar"));
        assert!(!EditorRenamePolicy::FlowchartNodeId.accepts("foo.->bar"));
        assert!(!EditorRenamePolicy::FlowchartNodeId.accepts("end.foo"));
        assert!(!EditorRenamePolicy::FlowchartNodeId.accepts("graph.foo"));
        assert!(!EditorRenamePolicy::FlowchartNodeId.accepts("subgraph.foo"));

        assert!(EditorRenamePolicy::GitGraphReference.accepts("release/v1.2"));
        assert!(EditorRenamePolicy::GitGraphReference.accepts("release-"));
        assert!(EditorRenamePolicy::GitGraphReference.accepts("_private"));
        assert!(!EditorRenamePolicy::GitGraphReference.accepts("release/"));
        assert!(!EditorRenamePolicy::GitGraphReference.accepts("release branch"));

        assert!(EditorRenamePolicy::ArchitectureIdentifier.accepts("rowspan"));
        assert!(EditorRenamePolicy::ArchitectureIdentifier.accepts("service-2"));
        assert!(!EditorRenamePolicy::ArchitectureIdentifier.accepts("service-"));
        assert!(!EditorRenamePolicy::ArchitectureIdentifier.accepts("align"));

        assert!(EditorRenamePolicy::RailroadIrRule.accepts("rule_name"));
        assert!(!EditorRenamePolicy::RailroadIrRule.accepts("1rule"));
        assert!(!EditorRenamePolicy::RailroadIrRule.accepts("terminal"));
        assert!(EditorRenamePolicy::RailroadEbnfRule.accepts("terminal"));
        assert!(EditorRenamePolicy::RailroadPegRule.accepts("terminal"));
        assert!(EditorRenamePolicy::RailroadAbnfRule.accepts("rule-name"));
        assert!(!EditorRenamePolicy::RailroadAbnfRule.accepts("rule_name"));
    }

    #[test]
    #[cfg(feature = "diagram-agentflow")]
    fn agentflow_rename_policy_uses_its_own_keyword_boundaries() {
        for candidate in ["flow", "connector", "global", "flow-guide", "global注文"] {
            assert!(
                !EditorRenamePolicy::AgentflowNodeId.accepts(candidate),
                "{candidate}"
            );
        }
        for candidate in ["flow_user", "flowUser", "Connector", "friend-end"] {
            assert!(
                EditorRenamePolicy::AgentflowNodeId.accepts(candidate),
                "{candidate}"
            );
        }
    }

    #[test]
    #[cfg(feature = "diagram-usecase")]
    fn usecase_rename_policy_rejects_statement_only_keywords() {
        for candidate in [
            "package",
            "PACKAGE",
            "rectangle",
            "allowmixing",
            "newpage",
            "skinparam",
        ] {
            assert!(
                !EditorRenamePolicy::UsecaseIdentifier.accepts(candidate),
                "{candidate}"
            );
        }
        for candidate in ["package_user", "packageUser", "1User"] {
            assert!(
                EditorRenamePolicy::UsecaseIdentifier.accepts(candidate),
                "{candidate}"
            );
        }
    }

    #[test]
    fn generated_rename_policy_ids_match_serde_exactly() {
        assert_eq!(
            EditorRenamePolicy::ALL.map(EditorRenamePolicy::as_str),
            EditorRenamePolicy::IDS
        );
        assert_eq!(
            EditorRenamePolicy::default(),
            EditorRenamePolicy::Identifier
        );

        for (policy, id) in EditorRenamePolicy::ALL
            .into_iter()
            .zip(EditorRenamePolicy::IDS)
        {
            assert_eq!(serde_json::to_string(&policy).unwrap(), format!("\"{id}\""));
            assert_eq!(
                serde_json::from_str::<EditorRenamePolicy>(&format!("\"{id}\""))
                    .expect("generated rename policy id must deserialize"),
                policy
            );
        }
        assert!(serde_json::from_str::<EditorRenamePolicy>("\"unknown\"").is_err());
    }

    #[test]
    fn class_definition_role_is_typed_completion_without_reference_identity() {
        let role = EditorSemanticRole::ClassDefinition;
        assert!(role.contributes_completion());
        assert!(!role.contributes_references());
        assert!(role.contributes_outline());
        assert!(role.is_class_definition());

        let span = crate::SourceSpan::new(0, 3);
        let symbol = EditorSemanticSymbol::class_definition(
            "hot",
            Some("display text may change".to_string()),
            EditorSemanticKind::Class,
            span,
            span,
        );
        assert_eq!(symbol.role, role);
        assert_eq!(symbol.rename_policy, EditorRenamePolicy::None);
    }

    #[test]
    fn reference_role_is_navigation_only() {
        let role = EditorSemanticRole::Reference;
        assert!(!role.contributes_completion());
        assert!(role.contributes_references());
        assert!(!role.contributes_outline());
        assert!(!role.is_class_definition());

        let span = crate::SourceSpan::new(0, 3);
        let symbol = EditorSemanticSymbol::reference(
            "ref",
            Some("display text may change".to_string()),
            EditorSemanticKind::Class,
            span,
            span,
        );
        assert_eq!(symbol.role, role);
        assert_eq!(symbol.rename_policy, EditorRenamePolicy::Identifier);
    }
}
