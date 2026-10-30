//! Source providers bound to the portable pinned Mermaid identity catalog.
//!
//! Model owns names, aliases, metadata/model-kind links, and selector facts. This module owns
//! detectors and their order/effects, editor semantics, and parser callbacks. Its capability
//! projection reports the source providers actually compiled into this crate.

use crate::detect::{BuiltinDetectorFn, DetectorConfigEffect};
#[cfg(feature = "compat-json")]
use crate::diagram::BuiltInDiagramSemanticParser;
#[cfg(feature = "compat-json")]
use crate::{DiagramWarningFact, ManagedSemanticJson};
#[cfg(feature = "editor-facts")]
use crate::{EditorFamilySemantics, EditorSemanticFacts, EditorSemanticKind};
use crate::{Error, OperationControl, OperationControlResult, Result};
#[cfg(feature = "presentation")]
use crate::{MermaidConfig, SourcePresentationMetadata};
#[cfg(test)]
use merman_model::family::diagram_type_metadata_id;
use merman_model::family::{
    DiagramFamilyDefinition, DiagramTypeId, FAMILY_CATALOG, FamilyVariantIdentity,
};
use merman_model::family::{DiagramFamilyId, DiagramHeaderFact, diagram_type_family_id};
use serde::Serialize;
#[cfg(feature = "compat-json")]
use serde_json::Value;
use std::sync::OnceLock;
#[cfg(feature = "presentation")]
use {
    crate::diagram::{BuiltInRenderSemanticParser, RenderSemanticParseOutput},
    merman_model::presentation::RenderSemanticModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
pub(crate) enum EditorFactsDemand {
    Omit,
    Recovery,
    Capture,
}

#[cfg(feature = "editor-facts")]
impl EditorFactsDemand {
    pub(crate) const fn needs_recovery(self) -> bool {
        !matches!(self, Self::Omit)
    }

    pub(crate) const fn captures_rich(self) -> bool {
        matches!(self, Self::Capture)
    }

    pub(crate) fn finish_facts(
        self,
        facts: Option<EditorSemanticFacts>,
    ) -> Option<EditorSemanticFacts> {
        match self {
            Self::Omit => None,
            Self::Recovery => {
                let mut facts = facts.unwrap_or_default();
                facts.retain_recovery_facts();
                Some(facts)
            }
            Self::Capture => facts,
        }
    }
}

pub(crate) struct SemanticConstruction<M> {
    pub(crate) model: Result<M>,
    #[cfg(feature = "editor-facts")]
    pub(crate) editor_facts: Option<EditorSemanticFacts>,
}

impl<M> SemanticConstruction<M> {
    pub(crate) fn map_model<N>(self, project: impl FnOnce(M) -> N) -> SemanticConstruction<N> {
        SemanticConstruction {
            model: self.model.map(project),
            #[cfg(feature = "editor-facts")]
            editor_facts: self.editor_facts,
        }
    }
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) type CombinedSemanticParser = fn(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<CombinedSemanticParse>;

#[cfg(feature = "compat-json")]
pub(crate) type WarningSemanticParser =
    fn(code: &str, meta: &SourcePresentationMetadata) -> Result<WarningSemanticParse>;

/// Built-in compatibility JSON paired with the typed warnings that produced its warning field.
#[cfg(feature = "compat-json")]
pub(crate) struct WarningSemanticParse {
    model: ManagedSemanticJson,
    warning_facts: Vec<DiagramWarningFact>,
}

#[allow(
    dead_code,
    reason = "Warning construction is used only by warning-producing diagram families."
)]
#[cfg(feature = "compat-json")]
impl WarningSemanticParse {
    pub(crate) fn new(model: Value, warning_facts: Vec<DiagramWarningFact>) -> Self {
        Self {
            model: model.into(),
            warning_facts,
        }
    }

    pub(crate) fn into_model(self) -> Value {
        self.model.into_unmanaged_value()
    }

    pub(crate) fn into_parts(self) -> (ManagedSemanticJson, Vec<DiagramWarningFact>) {
        (self.model, self.warning_facts)
    }
}

/// Closed result of one family semantic construction.
///
/// A successful construction hands typed warnings directly to the operation snapshot. A failed
/// construction still owns the parser-derived editor facts produced before the error. This
/// prevents callers from invoking a second recovery parser over the same source.
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) struct CombinedSemanticParse {
    model: Result<ManagedSemanticJson>,
    editor_facts: EditorSemanticFacts,
    warning_facts: Vec<DiagramWarningFact>,
}

/// Closed failure handoff produced after a family has retained its recovery journal.
#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) struct CombinedSemanticFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Box<EditorSemanticFacts>>,
    #[cfg(feature = "editor-facts")]
    recovery_parser: Option<&'static str>,
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
impl CombinedSemanticFailure {
    pub(crate) fn new(
        error: Error,
        #[cfg(feature = "editor-facts")] editor_facts: Option<EditorSemanticFacts>,
    ) -> Self {
        Self {
            error: Box::new(error),
            #[cfg(feature = "editor-facts")]
            editor_facts: editor_facts.map(Box::new),
            #[cfg(feature = "editor-facts")]
            recovery_parser: None,
        }
    }

    pub(crate) fn parser_recovery(
        #[cfg(feature = "editor-facts")] parser: &'static str,
        error: Error,
        #[cfg(feature = "editor-facts")] editor_facts: Option<EditorSemanticFacts>,
    ) -> Self {
        #[cfg(feature = "editor-facts")]
        let recovery_parser = editor_facts.as_ref().map(|_| parser);
        Self {
            error: Box::new(error),
            #[cfg(feature = "editor-facts")]
            editor_facts: editor_facts.map(Box::new),
            #[cfg(feature = "editor-facts")]
            recovery_parser,
        }
    }

    #[cfg(feature = "editor-facts")]
    pub(crate) fn into_parts(self) -> (Error, EditorSemanticFacts) {
        let mut editor_facts = self
            .editor_facts
            .expect("parser capture requests its failure journal");
        if let Some(parser) = self.recovery_parser {
            let (message, span) = match self.error.as_ref() {
                Error::DiagramParse { diagnostic, .. } => {
                    (diagnostic.message().to_string(), diagnostic.span())
                }
                error => (error.to_string(), None),
            };
            editor_facts.mark_recovered_from_parse_error(
                format!("{parser} parser recovered after parse error: {message}"),
                span,
            );
        }
        (*self.error, *editor_facts)
    }

    pub(crate) fn into_error(self) -> Error {
        *self.error
    }
}

#[cfg(all(test, feature = "editor-facts"))]
mod combined_semantic_failure_tests {
    use super::*;
    use crate::{EditorSemanticDiagnosticKind, ParseDiagnosticSpanKind, SourceSpan};

    #[test]
    fn parser_recovery_preserves_the_strict_error_and_exact_editor_diagnostic() {
        let span = SourceSpan::new(17, 29);
        let failure = CombinedSemanticFailure::parser_recovery(
            "quadrant chart",
            Error::diagram_parse_exact("quadrantChart", "expected point coordinates", span),
            Some(EditorSemanticFacts::new()),
        );

        let (error, facts) = failure.into_parts();
        let Error::DiagramParse {
            diagram_type,
            diagnostic,
        } = error
        else {
            panic!("expected diagram parse error");
        };
        assert_eq!(diagram_type, "quadrantChart");
        assert_eq!(diagnostic.message(), "expected point coordinates");
        assert_eq!(diagnostic.span(), Some(span));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(
            facts.diagnostics[0].message,
            "quadrant chart parser recovered after parse error: expected point coordinates"
        );
        assert_eq!(facts.diagnostics[0].span, Some(span));
        assert_eq!(
            facts.diagnostics[0].kind,
            EditorSemanticDiagnosticKind::ParserRecovery
        );
    }
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
impl CombinedSemanticParse {
    pub(crate) fn from_parts(
        model: Result<Value>,
        editor_facts: EditorSemanticFacts,
        warning_facts: Vec<DiagramWarningFact>,
    ) -> Self {
        Self {
            model: model.map(ManagedSemanticJson::from),
            editor_facts,
            warning_facts,
        }
    }

    #[allow(
        dead_code,
        reason = "Shared parser facilities have different consumers in each family selection."
    )]
    pub(crate) fn from_construction<S, F>(
        construction: std::result::Result<S, F>,
        success: impl FnOnce(S) -> (Result<Value>, EditorSemanticFacts),
        failure: impl FnOnce(F) -> (Error, EditorSemanticFacts),
    ) -> Self {
        match construction {
            Ok(source) => {
                let (model, editor_facts) = success(source);
                Self::from_parts(model, editor_facts, Vec::new())
            }
            Err(parse_failure) => {
                let (error, editor_facts) = failure(parse_failure);
                Self::from_parts(Err(error), editor_facts, Vec::new())
            }
        }
    }

    /// Builds a semantic parse while allowing the compatibility projection to observe the
    /// operation's cancellation channel. The uncontrolled constructor remains useful for the
    /// older family parsers; controlled families should use this variant so a cancellation that
    /// arrives after semantic construction is not converted into a successful parse result.
    #[allow(
        dead_code,
        reason = "Shared parser facilities have different consumers in each family selection."
    )]
    pub(crate) fn from_construction_controlled<S, F>(
        construction: std::result::Result<S, F>,
        control: &OperationControl,
        success: impl FnOnce(
            S,
        ) -> OperationControlResult<(
            Result<ManagedSemanticJson>,
            EditorSemanticFacts,
        )>,
        failure: impl FnOnce(F) -> (Error, EditorSemanticFacts),
    ) -> OperationControlResult<Self> {
        match construction {
            Ok(source) => {
                let (model, editor_facts) = success(source)?;
                Ok(Self {
                    model,
                    editor_facts,
                    warning_facts: Vec::new(),
                })
            }
            Err(parse_failure) => {
                let (error, editor_facts) = failure(parse_failure);
                control.checkpoint()?;
                Ok(Self {
                    model: Err(error),
                    editor_facts,
                    warning_facts: Vec::new(),
                })
            }
        }
    }

    #[allow(
        dead_code,
        reason = "Shared parser facilities have different consumers in each family selection."
    )]
    pub(crate) fn from_construction_with_warning_facts<S, F>(
        construction: std::result::Result<S, F>,
        success: impl FnOnce(S) -> (Result<Value>, EditorSemanticFacts, Vec<DiagramWarningFact>),
        failure: impl FnOnce(F) -> (Error, EditorSemanticFacts),
    ) -> Self {
        match construction {
            Ok(source) => {
                let (model, editor_facts, warning_facts) = success(source);
                Self::from_parts(model, editor_facts, warning_facts)
            }
            Err(parse_failure) => {
                let (error, editor_facts) = failure(parse_failure);
                Self::from_parts(Err(error), editor_facts, Vec::new())
            }
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Result<ManagedSemanticJson>,
        EditorSemanticFacts,
        Vec<DiagramWarningFact>,
    ) {
        (self.model, self.editor_facts, self.warning_facts)
    }
}

#[cfg(all(test, feature = "compat-json", feature = "editor-facts"))]
#[allow(
    dead_code,
    reason = "Test helpers serve different optional family suites."
)]
pub(crate) mod test_support {
    use super::{CombinedSemanticParse, CombinedSemanticParser};
    use crate::{
        EditorSemanticFacts, Error, OperationControl, OperationControlResult,
        SourcePresentationMetadata,
    };
    use serde_json::Value;

    pub(crate) fn into_result(
        parsed: OperationControlResult<CombinedSemanticParse>,
    ) -> std::result::Result<(Value, EditorSemanticFacts), Error> {
        let (model, editor_facts, _) = parsed
            .expect("a private parse control cannot be cancelled")
            .into_parts();
        model.map(|model| (model.into_unmanaged_value(), editor_facts))
    }

    pub(crate) fn editor_facts(
        parser: CombinedSemanticParser,
        code: &str,
        meta: &SourcePresentationMetadata,
    ) -> EditorSemanticFacts {
        parser(code, meta, &OperationControl::new())
            .expect("a private parse control cannot be cancelled")
            .into_parts()
            .1
    }

    pub(crate) fn editor_facts_from_frozen(
        parser: CombinedSemanticParser,
        code: &str,
        metadata: &merman_model::presentation::PresentationMetadata,
    ) -> EditorSemanticFacts {
        let source_metadata = SourcePresentationMetadata {
            diagram_type: metadata.diagram_type.clone(),
            config: metadata.config.clone(),
            effective_config:
                crate::presentation::SourcePresentationConfig::from_frozen_controlled(
                    metadata.effective_config.clone(),
                    &OperationControl::new(),
                )
                .unwrap(),
            title: metadata.title.clone(),
        };
        editor_facts(parser, code, &source_metadata)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DetectorFact {
    pub(crate) id: &'static str,
    pub(crate) detector: BuiltinDetectorFn,
    pub(crate) config_effect: DetectorConfigEffect,
}

#[derive(Clone, Copy)]
#[cfg(feature = "compat-json")]
pub(crate) struct SemanticParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: BuiltInDiagramSemanticParser,
}

#[derive(Clone, Copy)]
#[cfg(feature = "presentation")]
pub(crate) struct RenderParserFact {
    pub(crate) id: &'static str,
    pub(crate) metadata_id: Option<&'static str>,
    pub(crate) model_kind: &'static str,
    pub(crate) parser: BuiltInRenderSemanticParser,
}

#[derive(Clone, Copy)]
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) struct CombinedParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: CombinedSemanticParser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct DiagramFamilyCapability {
    /// Mermaid diagram type id used by the pinned detector and parser registries.
    pub diagram_type: &'static str,
    /// Catalog-owned logical family identity.
    pub family_id: DiagramFamilyId,
    /// Logical diagram family. This does not change when a family reuses another render model.
    pub logical_family_kind: &'static str,
    /// Public metadata identity, retained even when its implementation is unavailable.
    pub metadata_id: Option<&'static str>,
    /// Known typed render-model kind, independent of whether its parser is compiled.
    pub render_model_kind: Option<&'static str>,
    /// Whether this id participates in automatic detection.
    pub has_detector: bool,
    /// Whether this build includes a semantic parser for this diagram type.
    pub has_semantic_parser: bool,
    /// Whether this build includes parser-backed editor facts.
    pub has_editor_parser: bool,
    /// Whether JSON and editor facts share one combined semantic construction.
    pub has_combined_parser: bool,
    /// Whether this build includes a typed render-model parser for this diagram type.
    pub has_render_parser: bool,
    /// Whether this known id has an authoring header, without promising parser availability.
    pub has_header: bool,
    /// Mermaid configuration namespace associated with this id.
    pub config_namespace: Option<&'static str>,
}

impl DiagramFamilyCapability {
    /// Returns the concrete render-model family behind this parser/detector ID.
    ///
    /// Logical families may share a render model: Swimlane uses Flowchart's model, while
    /// metadata spellings such as `gitgraph` remain distinct from the detector's `gitGraph`.
    pub fn typed_render_family(&self) -> Option<&'static BuiltInTypedRenderFamily> {
        #[cfg(feature = "presentation")]
        {
            let parser = render_parser_facts()
                .iter()
                .find(|parser| parser.id == self.diagram_type)?;
            built_in_typed_render_families()
                .iter()
                .find(|family| family.render_model_kind == parser.model_kind)
        }
        #[cfg(not(feature = "presentation"))]
        {
            None
        }
    }
}

/// Canonical public identity for one compiled concrete built-in typed render family.
///
/// Parser aliases that share a render model contribute exactly one entry. Error and custom JSON
/// models are infrastructure variants rather than concrete Mermaid families and are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BuiltInTypedRenderFamily {
    pub diagram_type: &'static str,
    pub render_model_kind: &'static str,
}

struct FamilyCatalogProjection {
    detector_facts: Vec<DetectorFact>,
    #[cfg(feature = "compat-json")]
    semantic_parser_facts: Vec<SemanticParserFact>,
    #[cfg(feature = "presentation")]
    render_parser_facts: Vec<RenderParserFact>,
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    combined_parser_facts: Vec<CombinedParserFact>,
    supported_diagram_metadata_ids: Vec<&'static str>,
    diagram_header_facts: Vec<DiagramHeaderFact>,
    diagram_family_capabilities: Vec<DiagramFamilyCapability>,
    built_in_typed_render_families: Vec<BuiltInTypedRenderFamily>,
}

impl FamilyCatalogProjection {
    fn build<'a>(
        variants: impl IntoIterator<Item = (&'a DiagramFamilyDefinition, FamilyVariantDefinition)>,
    ) -> Self {
        let mut detector_facts = Vec::<(u16, DetectorFact)>::new();
        #[cfg(feature = "compat-json")]
        let mut semantic_parser_facts = Vec::<(u16, SemanticParserFact)>::new();
        #[cfg(feature = "presentation")]
        let mut render_parser_facts = Vec::<(u16, RenderParserFact)>::new();
        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
        let mut combined_parser_facts = Vec::<(u16, CombinedParserFact)>::new();
        let mut metadata_facts = Vec::<(u16, &'static str)>::new();
        let mut diagram_header_facts = Vec::<(u16, DiagramHeaderFact)>::new();
        let mut diagram_family_capabilities = Vec::<(u16, DiagramFamilyCapability)>::new();

        for (family, variant) in variants {
            let identity = &variant.identity;
            let implementation = &variant.implementation;
            if let Some(ordered) = variant.detector {
                detector_facts.push((
                    ordered.order,
                    DetectorFact {
                        id: identity.id,
                        detector: ordered.value,
                        config_effect: variant.detector_effect,
                    },
                ));
                if identity.key == DiagramTypeId::Error {
                    detector_facts.push((
                        1,
                        DetectorFact {
                            id: "---",
                            detector: crate::detect::detector_frontmatter_unparsed,
                            config_effect: DetectorConfigEffect::None,
                        },
                    ));
                }
            }
            #[cfg(feature = "compat-json")]
            if let Some(ordered) = implementation.semantic {
                semantic_parser_facts.push((
                    ordered.order,
                    SemanticParserFact {
                        id: identity.id,
                        parser: ordered.value,
                    },
                ));
            }
            #[cfg(feature = "presentation")]
            if let Some(ordered) = implementation.typed_render {
                render_parser_facts.push((
                    ordered.order,
                    RenderParserFact {
                        id: identity.id,
                        metadata_id: identity.metadata.map(|metadata| metadata.id),
                        model_kind: identity
                            .render_model_kind
                            .expect("typed render variants declare their model kind"),
                        parser: ordered.value,
                    },
                ));
            }
            #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
            if let Some(ordered) = implementation.combined {
                combined_parser_facts.push((
                    ordered.order,
                    CombinedParserFact {
                        id: identity.id,
                        parser: ordered.value,
                    },
                ));
            }
            if let Some((order, id)) = identity
                .metadata
                .and_then(|metadata| metadata.order.map(|order| (order, metadata.id)))
            {
                metadata_facts.push((order, id));
            }
            for header in identity.headers {
                diagram_header_facts.push((
                    header.order,
                    DiagramHeaderFact {
                        diagram_type: identity.id,
                        label: header.label,
                        detail: header.detail,
                    },
                ));
            }
            diagram_family_capabilities.push((
                identity.catalog_order,
                DiagramFamilyCapability {
                    diagram_type: identity.id,
                    family_id: family.id,
                    logical_family_kind: family.id.as_str(),
                    metadata_id: identity.metadata.map(|metadata| metadata.id),
                    render_model_kind: identity.render_model_kind,
                    has_detector: variant.detector.is_some(),
                    has_semantic_parser: implementation.has_semantic_parser(),
                    has_editor_parser: implementation.has_editor_parser(),
                    has_combined_parser: implementation.has_combined_parser(),
                    has_render_parser: implementation.has_render_parser(),
                    has_header: !identity.headers.is_empty(),
                    config_namespace: family.config.map(|config| config.namespace),
                },
            ));
        }

        let detector_facts = ordered_values(detector_facts);
        #[cfg(feature = "compat-json")]
        let semantic_parser_facts = ordered_values(semantic_parser_facts);
        #[cfg(feature = "presentation")]
        let render_parser_facts = ordered_values(render_parser_facts);
        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
        let combined_parser_facts = ordered_values(combined_parser_facts);
        let metadata_facts = ordered_values(metadata_facts);
        let diagram_header_facts = ordered_values(diagram_header_facts);
        let diagram_family_capabilities = ordered_values(diagram_family_capabilities);
        let supported_diagram_metadata_ids = metadata_facts
            .iter()
            .copied()
            .filter(|metadata_id| {
                diagram_family_capabilities
                    .iter()
                    .any(|fact| fact.metadata_id == Some(*metadata_id) && fact.has_semantic_parser)
            })
            .collect::<Vec<&'static str>>();
        #[cfg(feature = "presentation")]
        let mut built_in_typed_render_families = Vec::<BuiltInTypedRenderFamily>::new();
        #[cfg(not(feature = "presentation"))]
        let built_in_typed_render_families = Vec::new();
        #[cfg(feature = "presentation")]
        for metadata_id in metadata_facts {
            let Some(fact) = render_parser_facts
                .iter()
                .find(|fact| fact.metadata_id == Some(metadata_id))
            else {
                continue;
            };
            if !built_in_typed_render_families
                .iter()
                .any(|family| family.render_model_kind == fact.model_kind)
            {
                built_in_typed_render_families.push(BuiltInTypedRenderFamily {
                    diagram_type: metadata_id,
                    render_model_kind: fact.model_kind,
                });
            }
        }

        Self {
            detector_facts,
            #[cfg(feature = "compat-json")]
            semantic_parser_facts,
            #[cfg(feature = "presentation")]
            render_parser_facts,
            #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
            combined_parser_facts,
            supported_diagram_metadata_ids,
            diagram_header_facts,
            diagram_family_capabilities,
            built_in_typed_render_families,
        }
    }
}

fn ordered_values<T>(mut values: Vec<(u16, T)>) -> Vec<T> {
    values.sort_by_key(|(order, _)| *order);
    values.into_iter().map(|(_, value)| value).collect()
}

fn family_catalog_projection() -> &'static FamilyCatalogProjection {
    static CATALOG: OnceLock<FamilyCatalogProjection> = OnceLock::new();
    CATALOG.get_or_init(|| FamilyCatalogProjection::build(variants()))
}

pub(crate) fn detector_facts() -> &'static [DetectorFact] {
    family_catalog_projection().detector_facts.as_slice()
}

#[cfg(feature = "compat-json")]
pub(crate) fn semantic_parser_facts() -> &'static [SemanticParserFact] {
    family_catalog_projection().semantic_parser_facts.as_slice()
}

#[cfg(feature = "presentation")]
pub(crate) fn render_parser_facts() -> &'static [RenderParserFact] {
    family_catalog_projection().render_parser_facts.as_slice()
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn combined_parser_facts() -> &'static [CombinedParserFact] {
    family_catalog_projection().combined_parser_facts.as_slice()
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn combined_parser(diagram_type: &str) -> Option<CombinedSemanticParser> {
    combined_parser_facts()
        .iter()
        .find_map(|fact| (fact.id == diagram_type).then_some(fact.parser))
}

#[cfg(feature = "compat-json")]
pub(crate) fn warning_semantic_parser(diagram_type: &str) -> Option<WarningSemanticParser> {
    find_variant(diagram_type).and_then(|(_, variant)| variant.implementation.warning_semantic)
}

pub(crate) fn supported_diagram_metadata_ids() -> &'static [&'static str] {
    family_catalog_projection()
        .supported_diagram_metadata_ids
        .as_slice()
}

pub(crate) fn diagram_header_facts() -> &'static [DiagramHeaderFact] {
    family_catalog_projection().diagram_header_facts.as_slice()
}

pub(crate) fn diagram_family_capabilities() -> &'static [DiagramFamilyCapability] {
    family_catalog_projection()
        .diagram_family_capabilities
        .as_slice()
}

pub(crate) fn built_in_typed_render_families() -> &'static [BuiltInTypedRenderFamily] {
    family_catalog_projection()
        .built_in_typed_render_families
        .as_slice()
}

#[cfg(all(
    feature = "presentation",
    feature = "compat-json",
    feature = "editor-facts"
))]
pub(crate) fn is_builtin_diagram_type(diagram_type: &str) -> bool {
    find_variant(diagram_type).is_some()
}

/// Returns the presentation owner after configuration, without changing language identity.
///
/// Flowchart and Swimlane share paint consumers. This projection is used only for theme
/// contributions; parser admission and public logical identity always use the catalog.
#[cfg(feature = "presentation")]
pub(crate) fn operation_family_id(
    diagram_type: &str,
    effective_config: &MermaidConfig,
) -> Option<DiagramFamilyId> {
    let family = diagram_type_family_id(diagram_type)?;
    if family == DiagramFamilyId::FLOWCHART || family == DiagramFamilyId::SWIMLANE {
        return Some(
            if effective_config.get_str("layout") == Some(DiagramFamilyId::SWIMLANE.as_str()) {
                DiagramFamilyId::SWIMLANE
            } else {
                DiagramFamilyId::FLOWCHART
            },
        );
    }
    Some(family)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn diagram_type_editor_semantics(diagram_type: &str) -> Option<EditorFamilySemantics> {
    let family = diagram_type_family_id(diagram_type)?;
    Some(match family {
        DiagramFamilyId::FLOWCHART => FLOWCHART_EDITOR_SEMANTICS,
        DiagramFamilyId::SWIMLANE => SWIMLANE_EDITOR_SEMANTICS,
        DiagramFamilyId::MINDMAP => MINDMAP_EDITOR_SEMANTICS,
        DiagramFamilyId::SEQUENCE => SEQUENCE_EDITOR_SEMANTICS,
        DiagramFamilyId::C4 => CARDINAL_EDITOR_SEMANTICS,
        DiagramFamilyId::CLASS => CLASS_EDITOR_SEMANTICS,
        DiagramFamilyId::ER => ER_EDITOR_SEMANTICS,
        DiagramFamilyId::REQUIREMENT => CARDINAL_EDITOR_SEMANTICS,
        DiagramFamilyId::STATE => STATE_EDITOR_SEMANTICS,
        DiagramFamilyId::BLOCK => BLOCK_EDITOR_SEMANTICS,
        _ => GENERIC_EDITOR_SEMANTICS,
    })
}

#[cfg(feature = "presentation")]
mod render;
#[cfg(feature = "presentation")]
use render::*;
mod semantic;

#[derive(Clone, Copy)]
struct Ordered<T> {
    order: u16,
    value: T,
}

const fn ordered<T>(order: u16, value: T) -> Ordered<T> {
    Ordered { order, value }
}

/// One source binding paired with its shared portable identity.
#[derive(Clone, Copy)]
struct FamilyVariantDefinition {
    identity: FamilyVariantIdentity,
    detector: Option<Ordered<BuiltinDetectorFn>>,
    detector_effect: DetectorConfigEffect,
    implementation: FamilyImplementation,
}

/// Compiled entry points bound by closed keys from the shared identity catalog.
///
/// An absent callback makes only its executable projection unavailable; it never removes the
/// known identity or releases its built-in identity reservation.
#[derive(Clone, Copy, Default)]
struct FamilyImplementation {
    canonical: Option<CanonicalSemanticParser>,
    #[cfg(feature = "editor-facts")]
    canonical_editor_facts: bool,
    #[cfg(feature = "compat-json")]
    semantic: Option<Ordered<BuiltInDiagramSemanticParser>>,
    #[cfg(feature = "compat-json")]
    warning_semantic: Option<WarningSemanticParser>,
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    combined: Option<Ordered<CombinedSemanticParser>>,
    #[cfg(feature = "presentation")]
    typed_render: Option<Ordered<BuiltInRenderSemanticParser>>,
}

pub(crate) type CanonicalSemanticParser =
    fn(
        code: &str,
        metadata: &crate::SemanticMetadata,
        #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<SemanticConstruction<crate::SemanticDiagramModel>>;

impl FamilyImplementation {
    fn has_semantic_parser(&self) -> bool {
        if self.canonical.is_some() {
            return true;
        }
        #[cfg(feature = "compat-json")]
        {
            self.semantic.is_some()
        }
        #[cfg(not(feature = "compat-json"))]
        {
            false
        }
    }
    fn has_combined_parser(&self) -> bool {
        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
        {
            self.combined.is_some()
        }
        #[cfg(not(all(feature = "compat-json", feature = "editor-facts")))]
        {
            false
        }
    }
    fn has_editor_parser(&self) -> bool {
        #[cfg(feature = "editor-facts")]
        {
            (self.canonical.is_some() && self.canonical_editor_facts) || self.has_combined_parser()
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            false
        }
    }
    fn has_render_parser(&self) -> bool {
        #[cfg(feature = "presentation")]
        {
            self.typed_render.is_some()
        }
        #[cfg(not(feature = "presentation"))]
        {
            false
        }
    }
}

pub(crate) fn canonical_parser(diagram_type: &str) -> Option<CanonicalSemanticParser> {
    find_variant(diagram_type).and_then(|(_, variant)| variant.implementation.canonical)
}

#[cfg(feature = "editor-facts")]
const GENERIC_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Variable);
#[cfg(feature = "editor-facts")]
const FLOWCHART_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Module);
#[cfg(feature = "editor-facts")]
const SWIMLANE_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Variable);
#[cfg(feature = "editor-facts")]
const MINDMAP_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Namespace);
#[cfg(feature = "editor-facts")]
const SEQUENCE_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Event);
#[cfg(feature = "editor-facts")]
const CLASS_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Class);
#[cfg(feature = "editor-facts")]
const ER_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Struct);
#[cfg(feature = "editor-facts")]
const CARDINAL_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Variable);
#[cfg(feature = "editor-facts")]
const STATE_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Class);
#[cfg(feature = "editor-facts")]
const BLOCK_EDITOR_SEMANTICS: EditorFamilySemantics =
    EditorFamilySemantics::new(EditorSemanticKind::Object);

macro_rules! variant {
    (
        $(feature: $feature:literal,)?
        identity: $identity:expr,
        detector: $detector:expr,
        $(detector_effect: $detector_effect:expr,)?
        $(canonical: $canonical:expr,)?
        $(canonical_editor_facts: $canonical_editor_facts:expr,)?
        semantic: $semantic:expr,
        $(warning_semantic: $warning_semantic:expr,)?
        combined: $combined:expr,
        typed: $typed:expr $(,)?
    ) => {
        FamilyVariantDefinition {
            identity: $identity,
            detector: $detector,
            detector_effect: variant!(@detector_effect $($detector_effect)?),
            implementation: {
                $(#[cfg(feature = $feature)])?
                {
                    FamilyImplementation {
                        canonical: variant!(@canonical $($canonical)?),
                        #[cfg(feature = "editor-facts")]
                        canonical_editor_facts: variant!(@canonical_editor_facts $($canonical_editor_facts)?),
                        #[cfg(feature = "compat-json")]
                        semantic: $semantic,
                        #[cfg(feature = "compat-json")]
                        warning_semantic: variant!(@warning_semantic $($warning_semantic)?),
                        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                        combined: $combined,
                        #[cfg(feature = "presentation")]
                        typed_render: $typed,
                    }
                }
                $(#[cfg(not(feature = $feature))]
                {
                    FamilyImplementation {
                        canonical: None,
                        #[cfg(feature = "editor-facts")]
                        canonical_editor_facts: false,
                        #[cfg(feature = "compat-json")]
                        semantic: None,
                        #[cfg(feature = "compat-json")]
                        warning_semantic: None,
                        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                        combined: None,
                        #[cfg(feature = "presentation")]
                        typed_render: None,
                    }
                })?
            },
        }
    };
    (@detector_effect) => {
        DetectorConfigEffect::None
    };
    (@detector_effect $detector_effect:expr) => {
        $detector_effect
    };
    (@canonical) => { None };
    (@canonical $canonical:expr) => { Some($canonical) };
    (@canonical_editor_facts) => { true };
    (@canonical_editor_facts $canonical_editor_facts:expr) => { $canonical_editor_facts };
    (@warning_semantic) => {
        None
    };
    (@warning_semantic $warning_semantic:expr) => {
        Some($warning_semantic)
    };
}

fn variants() -> impl Iterator<Item = (&'static DiagramFamilyDefinition, FamilyVariantDefinition)> {
    merman_model::family::diagram_family_identities()
        .map(|(family, identity)| (family, bind_variant(identity)))
}

fn find_variant(
    diagram_type: &str,
) -> Option<(&'static DiagramFamilyDefinition, FamilyVariantDefinition)> {
    merman_model::family::find_variant(diagram_type)
        .map(|(family, identity)| (family, bind_variant(identity)))
}

#[derive(Clone, Copy)]
pub(crate) struct FrontmatterConfigAliasFact {
    pub(crate) source: &'static str,
    pub(crate) namespace: &'static str,
}

pub(crate) fn frontmatter_config_aliases() -> &'static [FrontmatterConfigAliasFact] {
    static FACTS: OnceLock<Vec<FrontmatterConfigAliasFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            let mut facts: Vec<_> = FAMILY_CATALOG
                .iter()
                .flat_map(|family| {
                    family.variants.iter().filter_map(move |variant| {
                        variant.frontmatter_alias_order.map(|order| {
                            let namespace = family
                                .config
                                .expect("config aliases require a family namespace")
                                .namespace;
                            (
                                order,
                                FrontmatterConfigAliasFact {
                                    source: variant.id,
                                    namespace,
                                },
                            )
                        })
                    })
                })
                .collect();
            facts.sort_by_key(|(order, _)| *order);
            facts.into_iter().map(|(_, fact)| fact).collect()
        })
        .as_slice()
}

pub(crate) fn frontmatter_config_namespaces() -> &'static [&'static str] {
    static NAMESPACES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMESPACES
        .get_or_init(|| {
            let mut namespaces: Vec<_> = FAMILY_CATALOG
                .iter()
                .filter_map(|family| {
                    family
                        .config
                        .map(|config| (config.frontmatter_order, config.namespace))
                })
                .collect();
            namespaces.sort_by_key(|(order, _)| *order);
            namespaces
                .into_iter()
                .map(|(_, namespace)| namespace)
                .collect()
        })
        .as_slice()
}

pub(crate) fn config_namespace_for_diagram_type(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).and_then(|(family, _)| family.config.map(|config| config.namespace))
}

fn bind_variant(identity: &FamilyVariantIdentity) -> FamilyVariantDefinition {
    match identity.key {
        DiagramTypeId::Error => variant! {
            identity: *identity,
            detector: Some(ordered(0, crate::detect::detector_error)),
            canonical: crate::diagrams::error_diagram::canonical_error,
            canonical_editor_facts: false,
            semantic: Some(ordered(0, crate::diagrams::error_diagram::parse_error)),
            combined: None,
            typed: Some(ordered(38, render_error)),
        },
        DiagramTypeId::FlowchartElk => variant! {
            feature: "diagram-flowchart",
            identity: *identity,
            detector: Some(ordered(2, crate::detect::detector_flowchart_elk)),
            detector_effect: DetectorConfigEffect::ElkLayout,
            canonical: semantic::canonical_flowchart,
            semantic: Some(ordered(3, crate::diagrams::flowchart::parse_flowchart)),
            warning_semantic: crate::diagrams::flowchart::parse_flowchart_with_warning_facts,
            combined: Some(ordered(2, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
            typed: Some(ordered(7, render_flowchart)),
        },
        DiagramTypeId::FlowchartV2 => variant! {
            feature: "diagram-flowchart",
            identity: *identity,
            detector: Some(ordered(17, crate::detect::detector_flowchart_v2)),
            canonical: semantic::canonical_flowchart,
            semantic: Some(ordered(1, crate::diagrams::flowchart::parse_flowchart)),
            warning_semantic: crate::diagrams::flowchart::parse_flowchart_with_warning_facts,
            combined: Some(ordered(0, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
            typed: Some(ordered(5, render_flowchart)),
        },
        DiagramTypeId::Flowchart => variant! {
            feature: "diagram-flowchart",
            identity: *identity,
            detector: None,
            canonical: semantic::canonical_flowchart,
            semantic: Some(ordered(2, crate::diagrams::flowchart::parse_flowchart)),
            warning_semantic: crate::diagrams::flowchart::parse_flowchart_with_warning_facts,
            combined: Some(ordered(1, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
            typed: Some(ordered(6, render_flowchart)),
        },
        DiagramTypeId::Swimlane => variant! {
            feature: "diagram-swimlane",
            identity: *identity,
            detector: Some(ordered(16, crate::detect::detector_swimlane)),
            canonical: semantic::canonical_flowchart,
            semantic: Some(ordered(9, crate::diagrams::flowchart::parse_flowchart)),
            warning_semantic: crate::diagrams::flowchart::parse_flowchart_with_warning_facts,
            combined: Some(ordered(3, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
            typed: Some(ordered(39, render_flowchart)),
        },
        DiagramTypeId::Mindmap => variant! {
            feature: "diagram-mindmap",
            identity: *identity,
            detector: Some(ordered(3, crate::detect::detector_mindmap)),
            canonical: semantic::canonical_mindmap,
            semantic: Some(ordered(22, crate::diagrams::mindmap::parse_mindmap)),
            combined: Some(ordered(5, crate::diagrams::mindmap::parse_mindmap_json_and_editor_facts)),
            typed: Some(ordered(0, render_mindmap)),
        },
        DiagramTypeId::Architecture => variant! {
            feature: "diagram-architecture",
            identity: *identity,
            detector: Some(ordered(4, crate::detect::detector_architecture)),
            canonical: semantic::canonical_architecture,
            semantic: Some(ordered(27, crate::diagrams::architecture::parse_architecture)),
            combined: Some(ordered(4, crate::diagrams::architecture::parse_architecture_json_and_editor_facts)),
            typed: Some(ordered(16, render_architecture)),
        },
        DiagramTypeId::Zenuml => variant! {
            feature: "diagram-zenuml",
            identity: *identity,
            detector: Some(ordered(5, crate::detect::detector_zenuml)),
            canonical: semantic::canonical_zenuml,
            semantic: Some(ordered(15, crate::diagrams::zenuml::parse_zenuml)),
            combined: Some(ordered(36, crate::diagrams::zenuml::parse_zenuml_json_and_editor_facts)),
            typed: Some(ordered(3, render_zenuml)),
        },
        DiagramTypeId::Sequence => variant! {
            feature: "diagram-sequence",
            identity: *identity,
            detector: Some(ordered(15, crate::detect::detector_sequence)),
            canonical: semantic::canonical_sequence,
            semantic: Some(ordered(8, crate::diagrams::sequence::parse_sequence)),
            combined: Some(ordered(19, crate::diagrams::sequence::parse_sequence_json_and_editor_facts)),
            typed: Some(ordered(4, render_sequence)),
        },
        DiagramTypeId::C4 => variant! {
            feature: "diagram-c4",
            identity: *identity,
            detector: Some(ordered(6, crate::detect::detector_c4)),
            canonical: semantic::canonical_c4,
            semantic: Some(ordered(6, crate::diagrams::c4::parse_c4)),
            combined: Some(ordered(28, crate::diagrams::c4::parse_c4_json_and_editor_facts)),
            typed: Some(ordered(10, render_c4)),
        },
        DiagramTypeId::Kanban => variant! {
            feature: "diagram-kanban",
            identity: *identity,
            detector: Some(ordered(7, crate::detect::detector_kanban)),
            canonical: semantic::canonical_kanban,
            semantic: Some(ordered(26, crate::diagrams::kanban::parse_kanban)),
            combined: Some(ordered(29, crate::diagrams::kanban::parse_kanban_json_and_editor_facts)),
            typed: Some(ordered(17, render_kanban)),
        },
        DiagramTypeId::ClassDiagram => variant! {
            feature: "diagram-class",
            identity: *identity,
            detector: Some(ordered(8, crate::detect::detector_class_v2)),
            canonical: semantic::canonical_class,
            semantic: Some(ordered(16, crate::diagrams::class::parse_class)),
            combined: Some(ordered(20, crate::diagrams::class::parse_class_json_and_editor_facts)),
            typed: Some(ordered(8, render_class)),
        },
        DiagramTypeId::Class => variant! {
            feature: "diagram-class",
            identity: *identity,
            detector: None,
            canonical: semantic::canonical_class,
            semantic: Some(ordered(17, crate::diagrams::class::parse_class)),
            combined: Some(ordered(21, crate::diagrams::class::parse_class_json_and_editor_facts)),
            typed: Some(ordered(9, render_class)),
        },
        DiagramTypeId::Er => variant! {
            feature: "diagram-er",
            identity: *identity,
            detector: Some(ordered(10, crate::detect::detector_er)),
            canonical: semantic::canonical_er,
            semantic: Some(ordered(18, crate::diagrams::er::parse_er)),
            combined: Some(ordered(17, crate::diagrams::er::parse_er_json_and_editor_facts)),
            typed: Some(ordered(29, render_er)),
        },
        DiagramTypeId::ErDiagram => variant! {
            feature: "diagram-er",
            identity: *identity,
            detector: None,
            canonical: semantic::canonical_er,
            semantic: Some(ordered(19, crate::diagrams::er::parse_er)),
            combined: Some(ordered(18, crate::diagrams::er::parse_er_json_and_editor_facts)),
            typed: Some(ordered(30, render_er)),
        },
        DiagramTypeId::Gantt => variant! {
            feature: "diagram-gantt",
            identity: *identity,
            detector: Some(ordered(11, crate::detect::detector_gantt)),
            canonical: semantic::canonical_gantt,
            semantic: Some(ordered(23, crate::diagrams::gantt::parse_gantt)),
            combined: Some(ordered(30, crate::diagrams::gantt::parse_gantt_json_and_editor_facts)),
            typed: Some(ordered(18, render_gantt)),
        },
        DiagramTypeId::Info => variant! {
            feature: "diagram-info",
            identity: *identity,
            detector: Some(ordered(12, crate::detect::detector_info)),
            canonical: semantic::canonical_info,
            semantic: Some(ordered(4, crate::diagrams::info::parse_info)),
            combined: Some(ordered(10, crate::diagrams::info::parse_info_json_and_editor_facts)),
            typed: Some(ordered(26, render_info)),
        },
        DiagramTypeId::Pie => variant! {
            feature: "diagram-pie",
            identity: *identity,
            detector: Some(ordered(13, crate::detect::detector_pie)),
            canonical: semantic::canonical_pie,
            semantic: Some(ordered(5, crate::diagrams::pie::parse_pie)),
            combined: Some(ordered(11, crate::diagrams::pie::parse_pie_json_and_editor_facts)),
            typed: Some(ordered(19, render_pie)),
        },
        DiagramTypeId::Requirement => variant! {
            feature: "diagram-requirement",
            identity: *identity,
            detector: Some(ordered(14, crate::detect::detector_requirement)),
            canonical: semantic::canonical_requirement,
            semantic: Some(ordered(7, crate::diagrams::requirement::parse_requirement)),
            combined: Some(ordered(31, crate::diagrams::requirement::parse_requirement_json_and_editor_facts)),
            typed: Some(ordered(23, render_requirement)),
        },
        DiagramTypeId::Timeline => variant! {
            feature: "diagram-timeline",
            identity: *identity,
            detector: Some(ordered(19, crate::detect::detector_timeline)),
            canonical: semantic::canonical_timeline,
            semantic: Some(ordered(24, crate::diagrams::timeline::parse_timeline)),
            combined: Some(ordered(32, crate::diagrams::timeline::parse_timeline_json_and_editor_facts)),
            typed: Some(ordered(21, render_timeline)),
        },
        DiagramTypeId::GitGraph => variant! {
            feature: "diagram-git-graph",
            identity: *identity,
            detector: Some(ordered(20, crate::detect::detector_git_graph)),
            canonical: semantic::canonical_git_graph,
            semantic: Some(ordered(29, crate::diagrams::git_graph::parse_git_graph)),
            warning_semantic: crate::diagrams::git_graph::parse_git_graph_with_warning_facts,
            combined: Some(ordered(15, crate::diagrams::git_graph::parse_git_graph_json_and_editor_facts)),
            typed: Some(ordered(33, render_git_graph)),
        },
        DiagramTypeId::StateDiagram => variant! {
            feature: "diagram-state",
            identity: *identity,
            detector: Some(ordered(21, crate::detect::detector_state_v2)),
            canonical: semantic::canonical_state,
            semantic: Some(ordered(20, crate::diagrams::state::parse_state)),
            combined: Some(ordered(26, crate::diagrams::state::parse_state_json_and_editor_facts)),
            typed: Some(ordered(1, render_state)),
        },
        DiagramTypeId::State => variant! {
            feature: "diagram-state",
            identity: *identity,
            detector: None,
            canonical: semantic::canonical_state,
            semantic: Some(ordered(21, crate::diagrams::state::parse_state)),
            combined: Some(ordered(27, crate::diagrams::state::parse_state_json_and_editor_facts)),
            typed: Some(ordered(2, render_state)),
        },
        DiagramTypeId::Journey => variant! {
            feature: "diagram-journey",
            identity: *identity,
            detector: Some(ordered(23, crate::detect::detector_journey)),
            canonical: semantic::canonical_journey,
            semantic: Some(ordered(25, crate::diagrams::journey::parse_journey)),
            combined: Some(ordered(33, crate::diagrams::journey::parse_journey_json_and_editor_facts)),
            typed: Some(ordered(22, render_journey)),
        },
        DiagramTypeId::QuadrantChart => variant! {
            feature: "diagram-quadrant-chart",
            identity: *identity,
            detector: Some(ordered(24, crate::detect::detector_quadrant)),
            canonical: semantic::canonical_quadrant_chart,
            semantic: Some(ordered(30, crate::diagrams::quadrant_chart::parse_quadrant_chart)),
            combined: Some(ordered(34, crate::diagrams::quadrant_chart::parse_quadrant_chart_json_and_editor_facts)),
            typed: Some(ordered(31, render_quadrant_chart)),
        },
        DiagramTypeId::Sankey => variant! {
            feature: "diagram-sankey",
            identity: *identity,
            detector: Some(ordered(25, crate::detect::detector_sankey)),
            canonical: semantic::canonical_sankey,
            semantic: Some(ordered(38, crate::diagrams::sankey::parse_sankey)),
            combined: Some(ordered(16, crate::diagrams::sankey::parse_sankey_json_and_editor_facts)),
            typed: Some(ordered(24, render_sankey)),
        },
        DiagramTypeId::Packet => variant! {
            feature: "diagram-packet",
            identity: *identity,
            detector: Some(ordered(26, crate::detect::detector_packet)),
            canonical: semantic::canonical_packet,
            semantic: Some(ordered(31, crate::diagrams::packet::parse_packet)),
            combined: Some(ordered(12, crate::diagrams::packet::parse_packet_json_and_editor_facts)),
            typed: Some(ordered(20, render_packet)),
        },
        DiagramTypeId::XyChart => variant! {
            feature: "diagram-xychart",
            identity: *identity,
            detector: Some(ordered(27, crate::detect::detector_xychart)),
            canonical: semantic::canonical_xychart,
            semantic: Some(ordered(39, crate::diagrams::xychart::parse_xychart)),
            combined: Some(ordered(38, crate::diagrams::xychart::parse_xychart_json_and_editor_facts)),
            typed: Some(ordered(32, render_xychart)),
        },
        DiagramTypeId::Block => variant! {
            feature: "diagram-block",
            identity: *identity,
            detector: Some(ordered(28, crate::detect::detector_block)),
            canonical: semantic::canonical_block,
            semantic: Some(ordered(28, crate::diagrams::block::parse_block)),
            warning_semantic: crate::diagrams::block::parse_block_with_warning_facts,
            combined: Some(ordered(37, crate::diagrams::block::parse_block_json_and_editor_facts)),
            typed: Some(ordered(28, render_block)),
        },
        DiagramTypeId::EventModeling => variant! {
            feature: "diagram-event-modeling",
            identity: *identity,
            detector: Some(ordered(29, crate::detect::detector_eventmodeling)),
            canonical: semantic::canonical_eventmodeling,
            semantic: Some(ordered(35, crate::diagrams::eventmodeling::parse_eventmodeling)),
            combined: Some(ordered(22, crate::diagrams::eventmodeling::parse_eventmodeling_json_and_editor_facts)),
            typed: Some(ordered(36, render_eventmodeling)),
        },
        DiagramTypeId::TreeView => variant! {
            feature: "diagram-tree-view",
            identity: *identity,
            detector: Some(ordered(30, crate::detect::detector_tree_view)),
            canonical: semantic::canonical_tree_view,
            semantic: Some(ordered(33, crate::diagrams::tree_view::parse_tree_view)),
            combined: Some(ordered(23, crate::diagrams::tree_view::parse_tree_view_json_and_editor_facts)),
            typed: Some(ordered(34, render_tree_view)),
        },
        DiagramTypeId::Radar => variant! {
            feature: "diagram-radar",
            identity: *identity,
            detector: Some(ordered(31, crate::detect::detector_radar)),
            canonical: semantic::canonical_radar,
            semantic: Some(ordered(32, crate::diagrams::radar::parse_radar)),
            combined: Some(ordered(14, crate::diagrams::radar::parse_radar_json_and_editor_facts)),
            typed: Some(ordered(25, render_radar)),
        },
        DiagramTypeId::Ishikawa => variant! {
            feature: "diagram-ishikawa",
            identity: *identity,
            detector: Some(ordered(32, crate::detect::detector_ishikawa)),
            canonical: semantic::canonical_ishikawa,
            semantic: Some(ordered(34, crate::diagrams::ishikawa::parse_ishikawa)),
            combined: Some(ordered(24, crate::diagrams::ishikawa::parse_ishikawa_json_and_editor_facts)),
            typed: Some(ordered(35, render_ishikawa)),
        },
        DiagramTypeId::Treemap => variant! {
            feature: "diagram-treemap",
            identity: *identity,
            detector: Some(ordered(33, crate::detect::detector_treemap)),
            canonical: semantic::canonical_treemap,
            semantic: Some(ordered(36, crate::diagrams::treemap::parse_treemap)),
            combined: Some(ordered(25, crate::diagrams::treemap::parse_treemap_json_and_editor_facts)),
            typed: Some(ordered(27, render_treemap)),
        },
        DiagramTypeId::Railroad => variant! {
            feature: "diagram-railroad",
            identity: *identity,
            detector: Some(ordered(34, crate::detect::detector_railroad)),
            canonical: semantic::canonical_railroad,
            semantic: Some(ordered(11, crate::diagrams::railroad::parse_railroad)),
            combined: Some(ordered(6, crate::diagrams::railroad::parse_railroad_json_and_editor_facts)),
            typed: Some(ordered(12, render_railroad)),
        },
        DiagramTypeId::RailroadEbnf => variant! {
            feature: "diagram-railroad",
            identity: *identity,
            detector: Some(ordered(35, crate::detect::detector_railroad_ebnf)),
            canonical: semantic::canonical_railroad_ebnf,
            semantic: Some(ordered(12, crate::diagrams::railroad::parse_railroad_ebnf)),
            combined: Some(ordered(7, crate::diagrams::railroad::parse_railroad_ebnf_json_and_editor_facts)),
            typed: Some(ordered(13, render_railroad_ebnf)),
        },
        DiagramTypeId::RailroadAbnf => variant! {
            feature: "diagram-railroad",
            identity: *identity,
            detector: Some(ordered(36, crate::detect::detector_railroad_abnf)),
            canonical: semantic::canonical_railroad_abnf,
            semantic: Some(ordered(13, crate::diagrams::railroad::parse_railroad_abnf)),
            combined: Some(ordered(8, crate::diagrams::railroad::parse_railroad_abnf_json_and_editor_facts)),
            typed: Some(ordered(14, render_railroad_abnf)),
        },
        DiagramTypeId::RailroadPeg => variant! {
            feature: "diagram-railroad",
            identity: *identity,
            detector: Some(ordered(37, crate::detect::detector_railroad_peg)),
            canonical: semantic::canonical_railroad_peg,
            semantic: Some(ordered(14, crate::diagrams::railroad::parse_railroad_peg)),
            combined: Some(ordered(9, crate::diagrams::railroad::parse_railroad_peg_json_and_editor_facts)),
            typed: Some(ordered(15, render_railroad_peg)),
        },
        DiagramTypeId::Venn => variant! {
            feature: "diagram-venn",
            identity: *identity,
            detector: Some(ordered(38, crate::detect::detector_venn)),
            canonical: semantic::canonical_venn,
            semantic: Some(ordered(37, crate::diagrams::venn::parse_venn)),
            combined: Some(ordered(35, crate::diagrams::venn::parse_venn_json_and_editor_facts)),
            typed: Some(ordered(37, render_venn)),
        },
        DiagramTypeId::Wardley => variant! {
            feature: "diagram-wardley",
            identity: *identity,
            detector: Some(ordered(39, crate::detect::detector_wardley)),
            canonical: semantic::canonical_wardley,
            semantic: Some(ordered(40, crate::diagrams::wardley::parse_wardley)),
            combined: Some(ordered(39, crate::diagrams::wardley::parse_wardley_json_and_editor_facts)),
            typed: Some(ordered(40, render_wardley)),
        },
        DiagramTypeId::Cynefin => variant! {
            feature: "diagram-cynefin",
            identity: *identity,
            detector: Some(ordered(40, crate::detect::detector_cynefin)),
            canonical: semantic::canonical_cynefin,
            semantic: Some(ordered(10, crate::diagrams::cynefin::parse_cynefin)),
            combined: Some(ordered(13, crate::diagrams::cynefin::parse_cynefin_json_and_editor_facts)),
            typed: Some(ordered(11, render_cynefin)),
        },
        DiagramTypeId::Usecase => variant! {
            feature: "diagram-usecase",
            identity: *identity,
            detector: Some(ordered(42, crate::detect::detector_usecase)),
            canonical: semantic::canonical_usecase,
            semantic: Some(ordered(42, crate::diagrams::usecase::parse_usecase)),
            combined: Some(ordered(42, crate::diagrams::usecase::parse_usecase_json_and_editor_facts)),
            typed: Some(ordered(42, render_usecase)),
        },
        DiagramTypeId::Agentflow => variant! {
            feature: "diagram-agentflow",
            identity: *identity,
            detector: Some(ordered(43, crate::detect::detector_agentflow)),
            canonical: semantic::canonical_agentflow,
            semantic: Some(ordered(43, crate::diagrams::agentflow::parse_agentflow)),
            warning_semantic: crate::diagrams::agentflow::parse_agentflow_with_warning_facts,
            combined: Some(ordered(43, crate::diagrams::agentflow::parse_agentflow_json_and_editor_facts)),
            typed: Some(ordered(43, render_agentflow)),
        },
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[cfg(feature = "presentation")]
    use crate::MermaidConfig;
    use serde_json::Value;
    use std::collections::{BTreeSet, HashSet};

    #[test]
    fn closed_shared_keys_bind_every_source_variant_once() {
        let expected = DiagramTypeId::all()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut actual = BTreeSet::new();
        for (family, variant) in variants() {
            let identity = variant.identity;
            assert!(actual.insert(identity.key));
            assert_eq!(identity.key.as_str(), identity.id);
            assert_eq!(diagram_type_family_id(identity.id), Some(family.id));
            let portable = merman_model::family::model_family_capabilities()
                .iter()
                .find(|capability| capability.diagram_type == identity.id)
                .expect("every source binding has a portable identity");
            assert_eq!(portable.logical_family_kind, family.id.as_str());
            assert_eq!(
                portable.metadata_id,
                identity.metadata.map(|metadata| metadata.id)
            );
            assert_eq!(portable.render_model_kind, identity.render_model_kind);
        }
        assert_eq!(actual, expected);
        assert_eq!(diagram_family_capabilities().len(), actual.len());
    }

    #[test]
    #[cfg(not(feature = "diagram-flowchart"))]
    fn shared_model_features_do_not_create_a_flowchart_source_provider() {
        for key in [
            DiagramTypeId::Flowchart,
            DiagramTypeId::FlowchartV2,
            DiagramTypeId::FlowchartElk,
        ] {
            let capability = diagram_family_capabilities()
                .iter()
                .find(|capability| capability.diagram_type == key.as_str())
                .unwrap();
            assert!(!capability.has_semantic_parser);
            assert!(!capability.has_editor_parser);
            assert!(!capability.has_combined_parser);
            assert!(!capability.has_render_parser);
            assert_eq!(
                merman_model::family::diagram_type_render_model_kind(key.as_str()),
                Some("flowchart")
            );
        }
    }

    const EXPECTED_FAMILY_IDS: &[(DiagramFamilyId, &str)] = &[
        (DiagramFamilyId::ERROR, "error"),
        (DiagramFamilyId::FLOWCHART, "flowchart"),
        (DiagramFamilyId::SWIMLANE, "swimlane"),
        (DiagramFamilyId::MINDMAP, "mindmap"),
        (DiagramFamilyId::ARCHITECTURE, "architecture"),
        (DiagramFamilyId::ZENUML, "zenuml"),
        (DiagramFamilyId::SEQUENCE, "sequence"),
        (DiagramFamilyId::C4, "c4"),
        (DiagramFamilyId::KANBAN, "kanban"),
        (DiagramFamilyId::CLASS, "class"),
        (DiagramFamilyId::ER, "er"),
        (DiagramFamilyId::GANTT, "gantt"),
        (DiagramFamilyId::INFO, "info"),
        (DiagramFamilyId::PIE, "pie"),
        (DiagramFamilyId::REQUIREMENT, "requirement"),
        (DiagramFamilyId::TIMELINE, "timeline"),
        (DiagramFamilyId::GIT_GRAPH, "gitGraph"),
        (DiagramFamilyId::STATE, "state"),
        (DiagramFamilyId::JOURNEY, "journey"),
        (DiagramFamilyId::QUADRANT_CHART, "quadrantChart"),
        (DiagramFamilyId::SANKEY, "sankey"),
        (DiagramFamilyId::PACKET, "packet"),
        (DiagramFamilyId::XY_CHART, "xychart"),
        (DiagramFamilyId::BLOCK, "block"),
        (DiagramFamilyId::EVENT_MODELING, "eventmodeling"),
        (DiagramFamilyId::TREE_VIEW, "treeView"),
        (DiagramFamilyId::RADAR, "radar"),
        (DiagramFamilyId::ISHIKAWA, "ishikawa"),
        (DiagramFamilyId::TREEMAP, "treemap"),
        (DiagramFamilyId::RAILROAD, "railroad"),
        (DiagramFamilyId::VENN, "venn"),
        (DiagramFamilyId::WARDLEY, "wardley"),
        (DiagramFamilyId::CYNEFIN, "cynefin"),
        (DiagramFamilyId::USECASE, "usecase"),
        (DiagramFamilyId::AGENTFLOW, "agentflow"),
    ];

    #[test]
    fn family_ids_are_unique_round_trip_and_keep_string_wire_values() {
        let mut unique = HashSet::new();

        for &(family, id) in EXPECTED_FAMILY_IDS {
            assert!(unique.insert(family), "duplicate family id {id}");
            assert_eq!(family.as_str(), id);
            assert_eq!(family.to_string(), id);
            assert_eq!(DiagramFamilyId::from_id(id), Some(family));
            assert_eq!(
                serde_json::to_value(family).unwrap(),
                Value::String(id.into())
            );
        }

        assert_eq!(unique.len(), 35);
        assert_eq!(DiagramFamilyId::from_id("future-family"), None);
    }

    #[test]
    fn public_family_catalog_exactly_covers_the_typed_core_catalog() {
        let expected = EXPECTED_FAMILY_IDS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>();
        let catalog = FAMILY_CATALOG
            .iter()
            .map(|family| family.id)
            .collect::<Vec<_>>();

        assert_eq!(DiagramFamilyId::all(), expected.as_slice());
        assert_eq!(catalog.as_slice(), expected.as_slice());
        for family in FAMILY_CATALOG {
            for variant in family.variants {
                assert_eq!(diagram_type_family_id(variant.id), Some(family.id));
            }
        }
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn operation_family_id_uses_effective_layout_for_flowchart_and_swimlane() {
        for (diagram_type, layout, expected) in [
            ("flowchart", None, DiagramFamilyId::FLOWCHART),
            ("flowchart-v2", Some("dagre"), DiagramFamilyId::FLOWCHART),
            ("flowchart-elk", Some("elk"), DiagramFamilyId::FLOWCHART),
            ("flowchart", Some("swimlane"), DiagramFamilyId::SWIMLANE),
            ("swimlane", None, DiagramFamilyId::FLOWCHART),
            ("swimlane", Some("elk"), DiagramFamilyId::FLOWCHART),
            ("swimlane", Some("swimlane"), DiagramFamilyId::SWIMLANE),
        ] {
            let mut config = MermaidConfig::empty_object();
            if let Some(layout) = layout {
                config.set_value("layout", Value::String(layout.to_string()));
            }
            assert_eq!(
                operation_family_id(diagram_type, &config),
                Some(expected),
                "{diagram_type} with layout {layout:?}"
            );
        }

        #[cfg(feature = "presentation")]
        {
            let user_config = MermaidConfig::empty_object();
            let mut defaulted_swimlane = crate::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"layout": "dagre"}),
            );
            crate::config::resolve_appearance(
                "swimlane",
                &user_config,
                &MermaidConfig::empty_object(),
                None,
                &crate::generated::upstream_default_config(),
                &mut defaulted_swimlane,
            );
            assert_eq!(defaulted_swimlane.get_str("layout"), Some("swimlane"));
            assert_eq!(
                operation_family_id("swimlane", defaulted_swimlane.as_config()),
                Some(DiagramFamilyId::SWIMLANE)
            );

            let user_config = MermaidConfig::from_value(serde_json::json!({"layout": "elk"}));
            let mut configured_flowchart =
                crate::presentation::SourcePresentationConfig::from_config(user_config.clone());
            crate::config::resolve_appearance(
                "swimlane",
                &user_config,
                &MermaidConfig::empty_object(),
                None,
                &crate::generated::upstream_default_config(),
                &mut configured_flowchart,
            );
            assert_eq!(
                operation_family_id("swimlane", configured_flowchart.as_config()),
                Some(DiagramFamilyId::FLOWCHART)
            );
        }

        let config = MermaidConfig::from_value(serde_json::json!({"layout": "swimlane"}));
        assert_eq!(
            operation_family_id("mindmap", &config),
            Some(DiagramFamilyId::MINDMAP)
        );
        assert_eq!(operation_family_id("future-diagram", &config), None);
    }

    #[test]
    fn detector_capabilities_resolve_shared_models_and_public_metadata_names() {
        for (detected, metadata) in [
            ("gitGraph", "gitgraph"),
            ("quadrantChart", "quadrantchart"),
            ("swimlane", "flowchart"),
            ("classDiagram", "class"),
        ] {
            let capability = diagram_family_capabilities()
                .iter()
                .find(|capability| capability.diagram_type == detected)
                .unwrap();
            if capability.has_render_parser {
                let family = capability.typed_render_family().unwrap();
                if detected != "swimlane" || cfg!(feature = "diagram-flowchart") {
                    assert_eq!(family.diagram_type, metadata);
                } else {
                    assert_eq!(family.diagram_type, "swimlane");
                }
            } else {
                assert!(capability.typed_render_family().is_none());
            }
        }
    }

    #[test]
    fn selectors_cover_logical_families_once_without_infrastructure_or_aliases() {
        let selectors = merman_model::family::diagram_family_selectors();
        assert_eq!(selectors.len(), 34);
        let mut features = BTreeSet::new();
        let mut families = BTreeSet::new();
        for selector in selectors {
            assert!(features.insert(selector.feature));
            assert!(families.insert(selector.logical_family_kind));
            assert!(selector.feature.starts_with("diagram-"));
            assert!(
                selector
                    .feature
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            );
        }
        for family in FAMILY_CATALOG {
            assert_eq!(family.selector.is_some(), family.id.as_str() != "error");
            assert_eq!(
                families.contains(family.id.as_str()),
                family.id.as_str() != "error"
            );
        }
        for (id, expected) in [
            ("flowchart", "diagram-flowchart"),
            ("flowchart-v2", "diagram-flowchart"),
            ("flowchart-elk", "diagram-flowchart"),
            ("swimlane", "diagram-swimlane"),
            ("gitGraph", "diagram-git-graph"),
            ("quadrantChart", "diagram-quadrant-chart"),
            ("treeView", "diagram-tree-view"),
            ("eventmodeling", "diagram-event-modeling"),
        ] {
            let (family, _) = find_variant(id).expect("known family alias");
            assert_eq!(family.selector, Some(expected), "{id}");
        }
        assert!(!features.contains("diagram-error"));
        assert!(!features.contains("diagram-flowchart-elk"));
    }

    #[test]
    fn public_metadata_ids_are_catalog_owned_instead_of_derived_from_family_names() {
        assert_eq!(diagram_type_metadata_id("flowchart-v2"), Some("flowchart"));
        assert_eq!(diagram_type_metadata_id("gitGraph"), Some("gitgraph"));
        assert_eq!(
            diagram_type_metadata_id("quadrantChart"),
            Some("quadrantchart")
        );
        assert_eq!(diagram_type_metadata_id("treeView"), Some("treeView"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn editor_semantics_follow_family_identity_across_variant_ids() {
        for variant in ["flowchart-v2", "flowchart-elk", "flowchart"] {
            assert_eq!(
                diagram_type_family_id(variant),
                Some(DiagramFamilyId::FLOWCHART),
                "{variant} must retain typed Flowchart family identity"
            );
        }

        let flowchart = diagram_type_editor_semantics("flowchart-v2")
            .expect("flowchart-v2 has editor family semantics");
        for variant in ["flowchart-elk", "flowchart"] {
            assert_eq!(
                diagram_type_editor_semantics(variant),
                Some(flowchart),
                "{variant} must inherit flowchart family semantics"
            );
        }

        let class = diagram_type_editor_semantics("classDiagram")
            .expect("classDiagram has editor family semantics");
        assert_eq!(diagram_type_editor_semantics("class"), Some(class));
        assert_ne!(flowchart, class);
    }

    #[test]
    fn absent_implementations_preserve_the_complete_identity_projection() {
        let unavailable = variants()
            .map(|(family, variant)| {
                (
                    family,
                    FamilyVariantDefinition {
                        identity: variant.identity,
                        implementation: FamilyImplementation::default(),
                        ..variant
                    },
                )
            })
            .collect::<Vec<_>>();
        let projected = FamilyCatalogProjection::build(
            unavailable
                .iter()
                .map(|(family, variant)| (*family, *variant)),
        );
        let complete = family_catalog_projection();

        #[cfg(feature = "compat-json")]
        {
            assert!(projected.semantic_parser_facts.is_empty());
            #[cfg(feature = "editor-facts")]
            assert!(projected.combined_parser_facts.is_empty());
        }
        #[cfg(feature = "presentation")]
        assert!(projected.render_parser_facts.is_empty());
        assert!(projected.supported_diagram_metadata_ids.is_empty());
        assert!(projected.built_in_typed_render_families.is_empty());
        assert_eq!(
            projected.diagram_header_facts,
            complete.diagram_header_facts
        );
        assert_eq!(
            projected
                .detector_facts
                .iter()
                .map(|fact| fact.id)
                .collect::<Vec<_>>(),
            complete
                .detector_facts
                .iter()
                .map(|fact| fact.id)
                .collect::<Vec<_>>()
        );
        let expected = complete
            .diagram_family_capabilities
            .iter()
            .map(|fact| DiagramFamilyCapability {
                has_semantic_parser: false,
                has_editor_parser: false,
                has_combined_parser: false,
                has_render_parser: false,
                ..*fact
            })
            .collect::<Vec<_>>();
        assert_eq!(projected.diagram_family_capabilities, expected);
    }

    #[test]
    #[cfg(all(
        feature = "diagram-flowchart",
        any(feature = "compat-json", feature = "presentation")
    ))]
    fn parser_and_typed_model_enumerations_follow_their_own_callbacks() {
        let (family, variant) = find_variant("flowchart-v2").unwrap();
        #[cfg(feature = "compat-json")]
        {
            let semantic_only = FamilyVariantDefinition {
                identity: variant.identity,
                implementation: FamilyImplementation {
                    semantic: variant.implementation.semantic,
                    ..FamilyImplementation::default()
                },
                ..variant
            };
            let projected = FamilyCatalogProjection::build([(family, semantic_only)]);
            assert_eq!(projected.supported_diagram_metadata_ids, ["flowchart"]);
            assert!(projected.built_in_typed_render_families.is_empty());
            assert!(projected.diagram_family_capabilities[0].has_semantic_parser);
            assert!(!projected.diagram_family_capabilities[0].has_editor_parser);
            assert!(!projected.diagram_family_capabilities[0].has_render_parser);
        }

        #[cfg(feature = "presentation")]
        {
            let typed_only = FamilyVariantDefinition {
                identity: variant.identity,
                implementation: FamilyImplementation {
                    typed_render: variant.implementation.typed_render,
                    ..FamilyImplementation::default()
                },
                ..variant
            };
            let projected = FamilyCatalogProjection::build([(family, typed_only)]);
            assert!(projected.supported_diagram_metadata_ids.is_empty());
            assert_eq!(
                projected.built_in_typed_render_families,
                [BuiltInTypedRenderFamily {
                    diagram_type: "flowchart",
                    render_model_kind: "flowchart",
                }]
            );
            assert!(!projected.diagram_family_capabilities[0].has_semantic_parser);
            assert!(projected.diagram_family_capabilities[0].has_render_parser);
        }
    }

    #[test]
    #[cfg(all(feature = "diagram-swimlane", feature = "presentation"))]
    fn shared_typed_models_do_not_admit_an_unavailable_logical_family() {
        let projected_variants = variants()
            .map(|(family, mut variant)| {
                if family.id.as_str() != "swimlane" {
                    variant.implementation = FamilyImplementation::default();
                }
                (family, variant)
            })
            .collect::<Vec<_>>();
        let projected = FamilyCatalogProjection::build(
            projected_variants
                .iter()
                .map(|(family, variant)| (*family, *variant)),
        );
        assert_eq!(projected.supported_diagram_metadata_ids, ["swimlane"]);
        assert_eq!(
            projected.built_in_typed_render_families,
            [BuiltInTypedRenderFamily {
                diagram_type: "swimlane",
                render_model_kind: "flowchart",
            }]
        );
        #[cfg(feature = "compat-json")]
        assert!(
            projected.diagram_family_capabilities.iter().all(|fact| {
                fact.has_semantic_parser == (fact.logical_family_kind == "swimlane")
            })
        );
    }

    #[test]
    fn catalog_ids_orders_and_family_policy_are_internally_consistent() {
        let mut ids = BTreeSet::new();
        let mut catalog_orders = BTreeSet::new();
        let mut detector_orders = BTreeSet::new();
        #[cfg(feature = "compat-json")]
        let mut semantic_orders = BTreeSet::new();
        #[cfg(feature = "compat-json")]
        #[cfg(feature = "editor-facts")]
        let mut combined_orders = BTreeSet::new();
        #[cfg(feature = "presentation")]
        let mut render_orders = BTreeSet::new();
        let mut metadata_orders = BTreeSet::new();
        let mut header_orders = BTreeSet::new();

        for (_, variant) in variants() {
            assert_ne!(
                variant.identity.id, "---",
                "frontmatter guard is not a family"
            );
            assert!(
                ids.insert(variant.identity.id),
                "duplicate catalog id {}",
                variant.identity.id
            );
            assert!(
                catalog_orders.insert(variant.identity.catalog_order),
                "duplicate catalog order {}",
                variant.identity.catalog_order
            );
            #[cfg(feature = "presentation")]
            assert!(
                variant.implementation.typed_render.is_none()
                    || variant.identity.render_model_kind.is_some(),
                "{} typed parser requires a known render kind",
                variant.identity.id
            );
            assert!(
                variant.identity.metadata.is_none() || variant.identity.render_model_kind.is_some(),
                "{} metadata requires a known render kind",
                variant.identity.id
            );
            #[cfg(feature = "compat-json")]
            #[cfg(feature = "editor-facts")]
            assert!(
                variant.implementation.combined.is_none()
                    || variant.implementation.semantic.is_some(),
                "{} combined parsing requires a semantic adapter",
                variant.identity.id
            );

            if let Some(fact) = variant.detector {
                assert!(detector_orders.insert(fact.order));
            }
            #[cfg(feature = "compat-json")]
            if let Some(fact) = variant.implementation.semantic {
                assert!(semantic_orders.insert(fact.order));
            }
            #[cfg(feature = "compat-json")]
            #[cfg(feature = "editor-facts")]
            if let Some(fact) = variant.implementation.combined {
                assert!(combined_orders.insert(fact.order));
            }
            #[cfg(feature = "presentation")]
            if let Some(fact) = variant.implementation.typed_render {
                assert!(render_orders.insert(fact.order));
            }
            if let Some(order) = variant
                .identity
                .metadata
                .and_then(|metadata| metadata.order)
            {
                assert!(metadata_orders.insert(order));
            }
            for fact in variant.identity.headers {
                assert!(header_orders.insert(fact.order));
            }
        }

        let config_orders = FAMILY_CATALOG
            .iter()
            .filter_map(|family| family.config.map(|config| config.frontmatter_order))
            .collect::<BTreeSet<_>>();
        let config_count = FAMILY_CATALOG
            .iter()
            .filter(|family| family.config.is_some())
            .count();
        assert_eq!(config_orders.len(), config_count);
        assert_eq!(ids.len(), diagram_family_capabilities().len());
    }
}
