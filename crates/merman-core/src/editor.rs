use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

pub use crate::generated::editor_rename_policy::EditorRenamePolicy;
use crate::{
    ParseControl, ParseControlResult,
    error::{Error, ParseDiagnostic, ParseDiagnosticSpanKind, ParseErrorSourceSpan},
    family::DiagramFamilyId,
};

/// Byte span attached to an editor-visible semantic fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

pub(crate) fn line_content_end(source: &str, end: usize) -> usize {
    let end = end.min(source.len());
    end.checked_sub(1)
        .filter(|index| source.as_bytes().get(*index) == Some(&b'\r'))
        .unwrap_or(end)
}

pub(crate) fn has_ascii_separator(source: &str, start: usize, end: usize) -> bool {
    source.get(start..end).is_some_and(|slice| {
        !slice.is_empty() && slice.as_bytes().iter().all(u8::is_ascii_whitespace)
    })
}

pub(crate) fn trailing_ascii_whitespace_slot(
    source: &str,
    start: usize,
    end: usize,
) -> Option<SourceSpan> {
    let end = line_content_end(source, end);
    let slice = source.get(start..end)?;
    (!slice.is_empty() && slice.as_bytes().last().is_some_and(u8::is_ascii_whitespace))
        .then_some(SourceSpan::new(end, end))
}

pub(crate) fn source_value_span(source: &str, span: SourceSpan, value: &str) -> Option<SourceSpan> {
    let slice = source.get(span.start..span.end)?;
    let relative_start = slice.find(value)?;
    Some(SourceSpan::new(
        span.start + relative_start,
        span.start + relative_start + value.len(),
    ))
}

/// Grammar-domain lexical classification emitted by preprocessing or one diagram family.
///
/// These names describe Mermaid syntax, not editor colors or transport token names. The editor
/// token descriptor owns the later projection into LSP/Monaco token types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorLexemeKind {
    Keyword,
    Comment,
    Operator,
    Delimiter,
    Identifier,
    Number,
    Date,
    Duration,
    Boolean,
    String,
    Style,
    Color,
    Literal,
    Frontmatter,
    Directive,
}

/// Grammar meaning attached to a lexical fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorLexemeModifier {
    Declaration,
    Definition,
    Reference,
    Readonly,
    Documentation,
    DefaultLibrary,
}

/// Allocation-free set of grammar-level lexeme modifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct EditorLexemeModifiers(u8);

impl EditorLexemeModifiers {
    pub const NONE: Self = Self(0);
    const VALID_BITS: u8 = (1 << 6) - 1;

    pub const fn from_modifier(modifier: EditorLexemeModifier) -> Self {
        Self(modifier.bit())
    }

    pub fn union(self, other: Self) -> Result<Self, EditorLexemeFailure> {
        let duplicate = self.0 & other.0;
        if duplicate != 0 {
            return Err(EditorLexemeFailure::DuplicateModifiers { bits: duplicate });
        }
        Self::try_from_bits(self.0 | other.0)
    }

    fn try_from_bits(bits: u8) -> Result<Self, EditorLexemeFailure> {
        let unknown = bits & !Self::VALID_BITS;
        if unknown != 0 {
            return Err(EditorLexemeFailure::UnknownModifierBits { bits: unknown });
        }
        Ok(Self(bits))
    }

    pub fn contains(self, modifier: EditorLexemeModifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = EditorLexemeModifier> {
        [
            EditorLexemeModifier::Declaration,
            EditorLexemeModifier::Definition,
            EditorLexemeModifier::Reference,
            EditorLexemeModifier::Readonly,
            EditorLexemeModifier::Documentation,
            EditorLexemeModifier::DefaultLibrary,
        ]
        .into_iter()
        .filter(move |modifier| self.contains(*modifier))
    }
}

impl<'de> Deserialize<'de> for EditorLexemeModifiers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bits = u8::deserialize(deserializer)?;
        Self::try_from_bits(bits).map_err(D::Error::custom)
    }
}

impl EditorLexemeModifier {
    const fn bit(self) -> u8 {
        match self {
            Self::Declaration => 1 << 0,
            Self::Definition => 1 << 1,
            Self::Reference => 1 << 2,
            Self::Readonly => 1 << 3,
            Self::Documentation => 1 << 4,
            Self::DefaultLibrary => 1 << 5,
        }
    }
}

/// The parser-owned stage that produced a lexical fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorLexemeProducerKind {
    GlobalPreprocess,
    FamilyLexer,
    FamilyParser,
    FamilyRecovery,
}

/// Provenance for a lexical fact. Family-owned facts always name their logical family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EditorLexemeProducer {
    kind: EditorLexemeProducerKind,
    family: Option<DiagramFamilyId>,
}

impl EditorLexemeProducer {
    const fn global_preprocess() -> Self {
        Self {
            kind: EditorLexemeProducerKind::GlobalPreprocess,
            family: None,
        }
    }

    const fn unsealed_family(kind: EditorLexemeProducerKind) -> Self {
        Self { kind, family: None }
    }

    pub fn kind(self) -> EditorLexemeProducerKind {
        self.kind
    }

    pub fn family(self) -> Option<DiagramFamilyId> {
        self.family
    }

    fn seal_family(&mut self, family: DiagramFamilyId) -> Result<(), EditorLexemeFailure> {
        if self.kind == EditorLexemeProducerKind::GlobalPreprocess || self.family.is_some() {
            return Err(EditorLexemeFailure::InvalidProvenance);
        }
        self.family = Some(family);
        Ok(())
    }

    fn mark_recovered(&mut self) {
        if self.family.is_none()
            && matches!(
                self.kind,
                EditorLexemeProducerKind::FamilyLexer | EditorLexemeProducerKind::FamilyParser
            )
        {
            self.kind = EditorLexemeProducerKind::FamilyRecovery;
        }
    }
}

/// One parser/preprocessor-owned lexical fact in caller-source byte coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EditorLexeme {
    kind: EditorLexemeKind,
    modifiers: EditorLexemeModifiers,
    span: SourceSpan,
    producer: EditorLexemeProducer,
}

impl EditorLexeme {
    pub(crate) fn global(kind: EditorLexemeKind, span: SourceSpan) -> Self {
        Self {
            kind,
            modifiers: EditorLexemeModifiers::NONE,
            span,
            producer: EditorLexemeProducer::global_preprocess(),
        }
    }

    fn unsealed_family(
        kind: EditorLexemeKind,
        modifiers: EditorLexemeModifiers,
        span: SourceSpan,
        producer: EditorLexemeProducerKind,
    ) -> Self {
        Self {
            kind,
            modifiers,
            span,
            producer: EditorLexemeProducer::unsealed_family(producer),
        }
    }

    pub fn kind(&self) -> EditorLexemeKind {
        self.kind
    }

    pub fn modifiers(&self) -> EditorLexemeModifiers {
        self.modifiers
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn producer(&self) -> EditorLexemeProducer {
        self.producer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, thiserror::Error)]
pub enum EditorLexemeFailure {
    #[error("family lexer emitted an invalid editor lexeme span {span:?}")]
    InvalidSpan { span: SourceSpan },
    #[error("family lexer emitted overlapping editor lexemes {left:?} and {right:?}")]
    Overlap { left: SourceSpan, right: SourceSpan },
    #[error("editor lexeme provenance was already sealed or used the global owner")]
    InvalidProvenance,
    #[error("editor lexeme modifiers contain unknown bits {bits:#010b}")]
    UnknownModifierBits { bits: u8 },
    #[error("editor lexeme modifiers contain duplicate bits {bits:#010b}")]
    DuplicateModifiers { bits: u8 },
}

pub(crate) struct EditorLexemeBatch(Vec<EditorLexeme>);
pub(crate) type EditorLexemeBatchResult = Result<EditorLexemeBatch, EditorLexemeFailure>;

/// Validated sink used by a family lexer/parser while it consumes its real token stream.
pub(crate) struct EditorLexemeJournal<'source> {
    source: &'source str,
    producer: EditorLexemeProducerKind,
    lexemes: Vec<EditorLexeme>,
    error: Option<EditorLexemeFailure>,
}

impl<'source> EditorLexemeJournal<'source> {
    pub(crate) fn family_lexer(source: &'source str) -> Self {
        Self::family_stage(source, EditorLexemeProducerKind::FamilyLexer)
    }

    pub(crate) fn family_parser(source: &'source str) -> Self {
        Self::family_stage(source, EditorLexemeProducerKind::FamilyParser)
    }

    pub(crate) fn family_recovery(source: &'source str) -> Self {
        Self::family_stage(source, EditorLexemeProducerKind::FamilyRecovery)
    }

    fn family_stage(source: &'source str, producer: EditorLexemeProducerKind) -> Self {
        Self {
            source,
            producer,
            lexemes: Vec::new(),
            error: None,
        }
    }

    pub(crate) fn push(
        &mut self,
        kind: EditorLexemeKind,
        modifiers: EditorLexemeModifiers,
        span: SourceSpan,
    ) {
        if self.error.is_some() {
            return;
        }
        if span.start >= span.end
            || span.end > self.source.len()
            || !self.source.is_char_boundary(span.start)
            || !self.source.is_char_boundary(span.end)
        {
            self.error = Some(EditorLexemeFailure::InvalidSpan { span });
            return;
        }
        self.lexemes.push(EditorLexeme::unsealed_family(
            kind,
            modifiers,
            span,
            self.producer,
        ));
    }

    pub(crate) fn finish(self) -> EditorLexemeBatchResult {
        let control = ParseControl::new();
        self.finish_controlled(&control)
            .expect("a private parse control cannot be cancelled")
    }

    pub(crate) fn finish_controlled(
        mut self,
        control: &ParseControl,
    ) -> ParseControlResult<EditorLexemeBatchResult> {
        control.checkpoint()?;
        if let Some(error) = self.error {
            return Ok(Err(error));
        }
        self.lexemes.sort_by(|left, right| {
            (left.span.start, left.span.end, left.kind).cmp(&(
                right.span.start,
                right.span.end,
                right.kind,
            ))
        });
        control.checkpoint()?;
        self.lexemes.dedup();
        control.checkpoint()?;
        for (index, pair) in self.lexemes.windows(2).enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if pair[0].span.end > pair[1].span.start {
                return Ok(Err(EditorLexemeFailure::Overlap {
                    left: pair[0].span,
                    right: pair[1].span,
                }));
            }
        }
        Ok(Ok(EditorLexemeBatch(self.lexemes)))
    }
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
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
            Self::FlowchartNodeId => crate::diagrams::flowchart::is_valid_editor_node_id(candidate),
            Self::GitGraphReference => {
                crate::diagrams::git_graph::is_valid_editor_reference(candidate)
            }
            Self::ArchitectureIdentifier => {
                crate::diagrams::architecture::is_valid_editor_identifier(candidate)
            }
            Self::RailroadIrRule => {
                crate::diagrams::railroad::is_valid_editor_ir_rule_identifier(candidate)
            }
            Self::RailroadEbnfRule => {
                crate::diagrams::railroad::is_valid_editor_ebnf_rule_identifier(candidate)
            }
            Self::RailroadPegRule => {
                crate::diagrams::railroad::is_valid_editor_peg_rule_identifier(candidate)
            }
            Self::RailroadAbnfRule => {
                crate::diagrams::railroad::is_valid_editor_abnf_rule_identifier(candidate)
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
        matches!(self, Self::Entity)
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
        let rename_policy = if role == EditorSemanticRole::Entity {
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
    lexemes: Vec<EditorLexeme>,
    lexeme_failure: Option<EditorLexemeFailure>,
    pub directive_prefixes: Vec<String>,
    pub diagnostics: Vec<EditorSemanticDiagnostic>,
    pub expected_syntax: Vec<EditorExpectedSyntax>,
}

impl EditorSemanticFacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_symbol(&mut self, symbol: EditorSemanticSymbol) {
        self.symbols.push(symbol);
    }

    pub fn lexemes(&self) -> &[EditorLexeme] {
        &self.lexemes
    }

    pub fn lexeme_failure(&self) -> Option<EditorLexemeFailure> {
        self.lexeme_failure
    }

    pub(crate) fn replace_family_lexemes(&mut self, batch: EditorLexemeBatchResult) {
        if self.lexeme_failure.is_some() {
            self.lexemes.clear();
            return;
        }
        match batch {
            Ok(EditorLexemeBatch(lexemes)) => {
                self.lexemes = lexemes;
                if self.completeness == EditorSemanticCompleteness::Recovered {
                    for lexeme in &mut self.lexemes {
                        lexeme.producer.mark_recovered();
                    }
                }
            }
            Err(error) => {
                self.lexemes.clear();
                self.lexeme_failure = Some(error);
            }
        }
    }

    pub(crate) fn remap_lexemes_controlled(
        &mut self,
        mut remap: impl FnMut(SourceSpan) -> Option<SourceSpan>,
        control: &ParseControl,
    ) -> ParseControlResult<usize> {
        let original_count = self.lexemes.len();
        let mut remapped = Vec::with_capacity(original_count);
        for (index, mut lexeme) in std::mem::take(&mut self.lexemes).into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(span) = remap(lexeme.span) {
                lexeme.span = span;
                remapped.push(lexeme);
            }
        }
        self.lexemes = remapped;
        control.checkpoint()?;
        Ok(original_count - self.lexemes.len())
    }

    #[cfg(test)]
    pub(crate) fn finalize_lexemes(
        &mut self,
        family: DiagramFamilyId,
        global_lexemes: &[EditorLexeme],
    ) {
        let control = ParseControl::new();
        self.finalize_lexemes_controlled(family, global_lexemes, &control)
            .expect("a private parse control cannot be cancelled");
    }

    pub(crate) fn finalize_lexemes_controlled(
        &mut self,
        family: DiagramFamilyId,
        global_lexemes: &[EditorLexeme],
        control: &ParseControl,
    ) -> ParseControlResult<()> {
        control.checkpoint()?;
        if self.lexeme_failure.is_some() {
            self.lexemes.clear();
            return Ok(());
        }
        let result = self.try_finalize_lexemes_controlled(family, global_lexemes, control)?;
        if let Err(error) = result {
            self.lexemes.clear();
            self.lexeme_failure = Some(error);
        }
        control.checkpoint()
    }

    fn try_finalize_lexemes_controlled(
        &mut self,
        family: DiagramFamilyId,
        global_lexemes: &[EditorLexeme],
        control: &ParseControl,
    ) -> ParseControlResult<Result<(), EditorLexemeFailure>> {
        for (index, lexeme) in self.lexemes.iter_mut().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Err(error) = lexeme.producer.seal_family(family) {
                return Ok(Err(error));
            }
        }
        control.checkpoint()?;
        self.lexemes.extend_from_slice(global_lexemes);
        control.checkpoint()?;
        self.lexemes.sort_by(|left, right| {
            (left.span.start, left.span.end, left.kind, left.modifiers).cmp(&(
                right.span.start,
                right.span.end,
                right.kind,
                right.modifiers,
            ))
        });
        control.checkpoint()?;
        self.lexemes.dedup();
        control.checkpoint()?;
        for (index, pair) in self.lexemes.windows(2).enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if pair[0].span.end > pair[1].span.start {
                return Ok(Err(EditorLexemeFailure::Overlap {
                    left: pair[0].span,
                    right: pair[1].span,
                }));
            }
        }
        Ok(Ok(()))
    }

    pub fn mark_recovered(&mut self) {
        self.completeness = EditorSemanticCompleteness::Recovered;
        for lexeme in &mut self.lexemes {
            lexeme.producer.mark_recovered();
        }
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
        self.diagnostics
            .push(EditorSemanticDiagnostic::parser_recovery(message, span));
    }

    pub fn push_diagnostic(&mut self, message: impl Into<String>, span: Option<SourceSpan>) {
        self.diagnostics
            .push(EditorSemanticDiagnostic::new(message, span));
    }

    pub fn push_directive_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if !self.directive_prefixes.contains(&prefix) {
            self.directive_prefixes.push(prefix);
        }
    }

    pub fn push_expected_syntax(&mut self, expected: EditorExpectedSyntax) {
        self.expected_syntax.push(expected);
    }

    pub(crate) fn finalize_expected_syntax_controlled(
        &mut self,
        control: &ParseControl,
    ) -> ParseControlResult<()> {
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

pub(crate) fn editor_keyword_value_span(
    source: &str,
    statement_start: usize,
    statement_end: usize,
    keyword: &str,
) -> Option<SourceSpan> {
    let raw = source.get(statement_start..statement_end)?;
    let trimmed = raw.trim_start();
    let leading = raw.len().saturating_sub(trimmed.len());
    let keyword_source = trimmed.get(..keyword.len())?;
    if !keyword_source.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let after_keyword = trimmed.get(keyword.len()..)?;
    if after_keyword
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace())
    {
        return None;
    }
    let whitespace = after_keyword
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let value_start = statement_start + leading + keyword.len() + whitespace;
    let value_len = after_keyword[whitespace..]
        .chars()
        .take_while(|ch| !ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    Some(SourceSpan::new(value_start, value_start + value_len))
}

pub(crate) fn editor_recovery_fallback_span(source: &str) -> SourceSpan {
    let mut line_start = 0;
    for segment in source.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let start = line_start + line.find(trimmed).unwrap_or_default();
            return SourceSpan::new(start, start + trimmed.len());
        }
        line_start += segment.len();
    }
    SourceSpan::new(source.len(), source.len())
}

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

pub(crate) fn lalrpop_recovery_span<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    fallback_offset: usize,
) -> SourceSpan {
    match error {
        lalrpop_util::ParseError::InvalidToken { location } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, .. }
        | lalrpop_util::ParseError::ExtraToken { token } => SourceSpan::new(token.0, token.2),
        lalrpop_util::ParseError::User { .. } => SourceSpan::new(fallback_offset, fallback_offset),
    }
}

pub(crate) fn lalrpop_parse_diagnostic<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    fallback_offset: usize,
) -> ParseDiagnostic
where
    T: std::fmt::Debug,
    E: std::fmt::Display + ParseErrorSourceSpan,
{
    let message = format_lalrpop_parse_error(error);
    match error {
        lalrpop_util::ParseError::InvalidToken { location }
        | lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
            ParseDiagnostic::new(message).with_span(
                SourceSpan::new(*location, *location),
                ParseDiagnosticSpanKind::InsertionPoint,
            )
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, .. }
        | lalrpop_util::ParseError::ExtraToken { token } => ParseDiagnostic::new(message)
            .with_span(
                SourceSpan::new(token.0, token.2),
                ParseDiagnosticSpanKind::Exact,
            ),
        lalrpop_util::ParseError::User { error } => {
            if let Some(span) = error.source_span() {
                ParseDiagnostic::new(message).with_span(span, ParseDiagnosticSpanKind::Exact)
            } else {
                ParseDiagnostic::new(message).with_span(
                    SourceSpan::new(fallback_offset, fallback_offset),
                    ParseDiagnosticSpanKind::Fallback,
                )
            }
        }
    }
}

pub(crate) fn format_lalrpop_parse_error<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
) -> String
where
    T: std::fmt::Debug,
    E: std::fmt::Display,
{
    match error {
        lalrpop_util::ParseError::InvalidToken { .. } => "unexpected token".to_string(),
        lalrpop_util::ParseError::UnrecognizedEof { expected, .. } => {
            let expected = format_expected_tokens(expected);
            if expected.is_empty() {
                "unexpected end of input".to_string()
            } else {
                format!("unexpected end of input; expected {expected}")
            }
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, expected } => {
            let expected = format_expected_tokens(expected);
            let found = format_found_token(&token.1);
            if expected.is_empty() {
                format!("unexpected {found}")
            } else {
                format!("unexpected {found}; expected {expected}")
            }
        }
        lalrpop_util::ParseError::ExtraToken { token } => {
            format!("unexpected extra {}", format_found_token(&token.1))
        }
        lalrpop_util::ParseError::User { error } => error.to_string(),
    }
}

fn format_expected_tokens(expected: &[String]) -> String {
    expected
        .iter()
        .map(|token| humanize_expected_token(token))
        .collect::<Vec<_>>()
        .join(", ")
}

fn humanize_expected_token(token: &str) -> String {
    match token {
        "Id" => "node identifier".to_string(),
        "EdgeLabel" => "edge label".to_string(),
        "Direction" => "diagram direction".to_string(),
        "AlphaNumToken" => "identifier".to_string(),
        "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "StringLit" => "string literal".to_string(),
        other => humanize_token_name(other),
    }
}

fn format_found_token<T>(token: &T) -> String
where
    T: std::fmt::Debug,
{
    let debug = format!("{token:?}");
    let variant = debug
        .split_once('(')
        .map(|(name, _)| name)
        .unwrap_or(debug.as_str());

    match variant {
        "Sep" | "Newline" => "statement separator".to_string(),
        "StyleSep" => "style separator".to_string(),
        "Amp" => "`&`".to_string(),
        "Comma" => "`,`".to_string(),
        "Plus" => "`+`".to_string(),
        "Minus" => "`-`".to_string(),
        "Arrow" => "edge operator".to_string(),
        "SignalType" => "message operator".to_string(),
        "Id" | "Actor" | "StyledId" => "identifier".to_string(),
        "Direction" | "DirectionStmt" => "diagram direction".to_string(),
        "EdgeLabel" | "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "NodeLabel" | "StateDescr" | "CompositState" => "node label".to_string(),
        "StringLit" => "string literal".to_string(),
        "Num" => "number".to_string(),
        other => humanize_token_name(other),
    }
}

fn humanize_token_name(token: &str) -> String {
    let token = token.strip_prefix("Kw").unwrap_or(token);
    let mut out = String::new();
    let mut previous_is_lowercase = false;

    for ch in token.chars() {
        if ch == '_' || ch == '-' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            previous_is_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_is_lowercase && !out.ends_with(' ') {
            out.push(' ');
        }

        if ch.is_ascii_digit() && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }

        out.push(ch.to_ascii_lowercase());
        previous_is_lowercase = ch.is_ascii_lowercase();
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        EditorLexeme, EditorLexemeFailure, EditorLexemeJournal, EditorLexemeKind,
        EditorLexemeModifier, EditorLexemeModifiers, EditorLexemeProducerKind, EditorRenamePolicy,
        EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
        lalrpop_parse_diagnostic,
    };
    use crate::{ParseControl, ParseDiagnosticSpanKind};

    #[test]
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
    fn lalrpop_parse_diagnostic_preserves_unrecognized_token_span() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::UnrecognizedToken {
            token: (3, "bad", 6),
            expected: vec!["ID".to_string()],
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 10);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 3);
        assert_eq!(span.end, 6);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        assert!(diagnostic.message().contains("\"bad\""));
    }

    #[test]
    fn lalrpop_parse_diagnostic_preserves_eof_insertion_point() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::UnrecognizedEof {
            location: 12,
            expected: vec!["]".to_string()],
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 99);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 12);
        assert_eq!(span.end, 12);
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
        assert!(diagnostic.message().contains("unexpected end of input"));
    }

    #[test]
    fn lalrpop_parse_diagnostic_marks_user_errors_as_fallback() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::User {
            error: "custom parse failure".to_string(),
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 8);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 8);
        assert_eq!(span.end, 8);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
        assert_eq!(diagnostic.message(), "custom parse failure");
    }

    #[test]
    fn lexeme_journal_rejects_overlap_in_release_builds_too() {
        let mut journal = EditorLexemeJournal::family_lexer("abcdef");
        journal.push(
            EditorLexemeKind::Keyword,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 4),
        );
        journal.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(3, 6),
        );

        assert!(matches!(
            journal.finish(),
            Err(EditorLexemeFailure::Overlap { .. })
        ));
    }

    #[test]
    fn lexeme_modifier_sets_reject_duplicates_and_unknown_bits() {
        let declaration = EditorLexemeModifiers::from_modifier(EditorLexemeModifier::Declaration);
        assert!(matches!(
            declaration.union(declaration),
            Err(EditorLexemeFailure::DuplicateModifiers { .. })
        ));
        assert!(serde_json::from_str::<EditorLexemeModifiers>("64").is_err());
    }

    #[test]
    fn failed_family_lexeme_batch_is_monotonic_through_finalization() {
        let mut invalid = EditorLexemeJournal::family_lexer("abc");
        invalid.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 4),
        );
        let mut facts = EditorSemanticFacts::new();
        facts.replace_family_lexemes(invalid.finish());
        let failure = facts.lexeme_failure().expect("invalid batch failure");

        let mut valid = EditorLexemeJournal::family_lexer("abc");
        valid.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 3),
        );
        facts.replace_family_lexemes(valid.finish());
        facts.mark_recovered();
        let family =
            crate::family::diagram_type_family_id("flowchart").expect("flowchart family identity");
        facts.finalize_lexemes(family, &[]);

        assert_eq!(facts.lexeme_failure(), Some(failure));
        assert!(facts.lexemes().is_empty());
    }

    #[test]
    fn family_lexeme_finalization_observes_cancellation_during_large_batches() {
        let source = "a ".repeat(256);
        let mut journal = EditorLexemeJournal::family_lexer(&source);
        for index in 0..256 {
            let start = index * 2;
            journal.push(
                EditorLexemeKind::Identifier,
                EditorLexemeModifiers::NONE,
                crate::SourceSpan::new(start, start + 1),
            );
        }
        let mut facts = EditorSemanticFacts::new();
        facts.replace_family_lexemes(journal.finish());
        let family =
            crate::family::diagram_type_family_id("flowchart").expect("flowchart family identity");
        let control = ParseControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            facts.finalize_lexemes_controlled(family, &[], &control),
            Err(crate::ParseCancelled)
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    fn recovery_promotes_the_complete_unsealed_family_batch_once() {
        let mut journal = EditorLexemeJournal::family_lexer("alpha beta");
        journal.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 5),
        );
        journal.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(6, 10),
        );
        let mut facts = EditorSemanticFacts::new();
        facts.replace_family_lexemes(journal.finish());

        facts.mark_recovered();
        facts.mark_recovered();

        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer.kind == EditorLexemeProducerKind::FamilyRecovery
                && lexeme.producer.family.is_none()
        }));
    }

    #[test]
    fn attaching_a_batch_to_recovered_facts_preserves_recovery_provenance() {
        let mut facts = EditorSemanticFacts::new();
        facts.mark_recovered();
        let mut journal = EditorLexemeJournal::family_parser("alpha");
        journal.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 5),
        );

        facts.replace_family_lexemes(journal.finish());

        assert_eq!(
            facts.lexemes()[0].producer.kind,
            EditorLexemeProducerKind::FamilyRecovery
        );
        assert_eq!(facts.lexemes()[0].producer.family, None);
    }

    #[test]
    fn recovery_after_family_sealing_does_not_rewrite_provenance() {
        let mut journal = EditorLexemeJournal::family_lexer("alpha");
        journal.push(
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::NONE,
            crate::SourceSpan::new(0, 5),
        );
        let mut facts = EditorSemanticFacts::new();
        facts.replace_family_lexemes(journal.finish());
        let family =
            crate::family::diagram_type_family_id("flowchart").expect("flowchart family identity");
        let global = EditorLexeme::global(EditorLexemeKind::Comment, crate::SourceSpan::new(6, 12));
        facts.finalize_lexemes(family, &[global]);

        facts.mark_recovered();

        assert_eq!(
            facts.lexemes()[0].producer.kind,
            EditorLexemeProducerKind::FamilyLexer
        );
        assert_eq!(facts.lexemes()[0].producer.family, Some(family));
        assert_eq!(
            facts.lexemes()[1].producer.kind,
            EditorLexemeProducerKind::GlobalPreprocess
        );
        assert_eq!(facts.lexemes()[1].producer.family, None);
    }
}
