use super::*;
#[cfg(feature = "editor-facts")]
use merman_core_structured::gantt::{
    GanttExpectedSyntax, GanttObservation, GanttSymbolKind, parse_gantt_syntax_observed_controlled,
};
use merman_core_structured::gantt::{
    GanttIssueLocation, GanttSource, GanttSourceIssue, parse_gantt_syntax_controlled,
};

use crate::family::{CombinedSemanticFailure, SemanticConstruction};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, family::EditorFactsDemand,
};
use crate::{OperationControl, OperationControlResult};

pub(crate) fn parse_gantt_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<GanttDiagramModel>> {
    let construction = construct_gantt_semantic_source_controlled(
        code,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        false,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand.needs_recovery() => {
            let (error, editor_facts) = failure.into_parts();
            return Ok(SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            });
        }
        Err(failure) => {
            return Ok(SemanticConstruction {
                model: Err(failure.into_error()),
                #[cfg(feature = "editor-facts")]
                editor_facts: None,
            });
        }
    };
    let GanttSemanticSource {
        source,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    } = source;
    let model = match source {
        Some(source) => source
            .into_semantic_model(control)?
            .map_err(gantt_source_error),
        None => Ok(GanttDiagramModel::default()),
    };
    Ok(SemanticConstruction {
        model: model.map(|mut model| {
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            model
        }),
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_gantt(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let Some(source) = parse_gantt_semantic_source(code, meta)?.source else {
        return Ok(json!({}));
    };
    let model = gantt_source_to_render_model(source)?;
    super::render_model_to_compat_json(&model, meta)
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn parse_gantt_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<GanttDiagramRenderModel> {
    let Some(source) = parse_gantt_semantic_source(code, meta)?.source else {
        return Ok(GanttDiagramRenderModel::from_output_data(
            GanttDiagramModel::default(),
            String::new(),
            GanttOutputData::Empty,
        ));
    };
    gantt_source_to_render_model(source)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_gantt_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<GanttDiagramRenderModel>> {
    let construction = construct_gantt_semantic_source_controlled(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    match source.source {
        Some(source) => Ok(source
            .into_render_model(control)?
            .map_err(gantt_source_error)),
        None => Ok(Ok(GanttDiagramRenderModel::from_output_data(
            GanttDiagramModel::default(),
            String::new(),
            GanttOutputData::Empty,
        ))),
    }
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_gantt_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = match construct_gantt_semantic_source_controlled(
        code,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        true,
        control,
    )? {
        Ok(source) => Ok(source.into_combined_parts_controlled(meta, control)?),
        Err(error) => Err(error),
    };
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |parts| parts,
        CombinedSemanticFailure::into_parts,
    ))
}

struct GanttSemanticSource {
    source: Option<GanttSource>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
impl GanttSemanticSource {
    fn into_combined_parts_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<(Result<Value>, EditorSemanticFacts)> {
        let Self {
            source,
            editor_facts,
        } = self;
        let model = match source {
            Some(source) => match source
                .into_render_model(control)?
                .map_err(gantt_source_error)
            {
                Ok(model) => render_model_to_compat_json_controlled(&model, meta, control)?,
                Err(error) => Err(error),
            },
            None => Ok(json!({})),
        };
        control.checkpoint()?;
        Ok((
            model,
            editor_facts.expect("Gantt combined parse requests editor facts"),
        ))
    }
}

#[cfg(feature = "compat-json")]
fn parse_gantt_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<GanttSemanticSource> {
    construct_gantt_semantic_source(code, meta).map_err(CombinedSemanticFailure::into_error)
}

#[cfg(feature = "compat-json")]
fn construct_gantt_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<GanttSemanticSource, CombinedSemanticFailure> {
    construct_gantt_semantic_source_controlled(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_gantt_semantic_source_controlled(
    code: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    capture_display_mode: bool,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<GanttSemanticSource, CombinedSemanticFailure>> {
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand.captures_rich().then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let syntax = if let Some(facts) = editor_facts.as_mut() {
        let syntax = parse_gantt_syntax_observed_controlled(
            code,
            config,
            &mut |observation| record_gantt_observation(facts, observation),
            capture_display_mode,
            control,
        )?;
        #[cfg(test)]
        crate::editor::test_work::record_collection();
        syntax
    } else {
        parse_gantt_syntax_controlled(code, config, capture_display_mode, control)?
    };
    #[cfg(not(feature = "editor-facts"))]
    let syntax = parse_gantt_syntax_controlled(code, config, capture_display_mode, control)?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = editor_demand.finish_facts(editor_facts);

    let source = match syntax {
        Ok(Some(syntax)) => {
            control.checkpoint()?;
            let runtime = crate::runtime::active_operation_context();
            syntax.into_source(runtime, control)?.map(Some)
        }
        Ok(None) => Ok(None),
        Err(issue) => Err(issue),
    };
    Ok(match source {
        Ok(source) => Ok(GanttSemanticSource {
            source,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => Err(CombinedSemanticFailure::parser_recovery(
            #[cfg(feature = "editor-facts")]
            "gantt",
            gantt_source_error(issue),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )),
    })
}

fn gantt_source_error(issue: GanttSourceIssue) -> Error {
    match issue.location {
        GanttIssueLocation::Fallback => Error::diagram_parse_fallback("gantt", issue.message),
        GanttIssueLocation::Exact(span) => Error::diagram_parse_exact("gantt", issue.message, span),
        GanttIssueLocation::InsertionPoint(offset) => {
            Error::diagram_parse_insertion_point("gantt", issue.message, offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn record_gantt_observation(facts: &mut EditorSemanticFacts, observation: GanttObservation<'_>) {
    match observation {
        GanttObservation::DirectivePrefix(prefix) => facts.push_directive_prefix(prefix),
        GanttObservation::ExpectedSyntax { kind, span } => {
            let kind = match kind {
                GanttExpectedSyntax::Payload => EditorExpectedSyntaxKind::Payload,
                GanttExpectedSyntax::InteractionAction => {
                    EditorExpectedSyntaxKind::InteractionAction
                }
                GanttExpectedSyntax::NodeIdentifier => EditorExpectedSyntaxKind::NodeIdentifier,
            };
            facts.push_expected_syntax(EditorExpectedSyntax::new(kind, span));
        }
        GanttObservation::Symbol {
            kind,
            text,
            statement,
            selection,
        } => {
            use EditorSemanticKind::{Function, Namespace, String, Variable};
            use EditorSemanticRole::{Entity, Outline, Payload, Reference};
            let (detail, symbol_kind, role) = match kind {
                GanttSymbolKind::DateFormat => ("gantt date format", String, Payload),
                GanttSymbolKind::AxisFormat => ("gantt axis format", String, Payload),
                GanttSymbolKind::TickInterval => ("gantt tick interval", String, Payload),
                GanttSymbolKind::Includes => ("gantt includes", String, Payload),
                GanttSymbolKind::Excludes => ("gantt excludes", String, Payload),
                GanttSymbolKind::TodayMarker => ("gantt today marker", String, Payload),
                GanttSymbolKind::Weekday => ("gantt weekday", String, Payload),
                GanttSymbolKind::Weekend => ("gantt weekend", String, Payload),
                GanttSymbolKind::Title => ("gantt title", String, Payload),
                GanttSymbolKind::Section => ("gantt section", Namespace, Outline),
                GanttSymbolKind::AccTitle => ("gantt accessibility title", String, Payload),
                GanttSymbolKind::AccDescr => ("gantt accessibility description", String, Payload),
                GanttSymbolKind::Task => ("gantt task", Variable, Entity),
                GanttSymbolKind::Dependency => ("gantt dependency", Variable, Reference),
                GanttSymbolKind::ClickTarget => ("gantt click target", Variable, Reference),
                GanttSymbolKind::ClickHref => ("gantt click href", String, Payload),
                GanttSymbolKind::ClickCallback => ("gantt click callback", Function, Payload),
                GanttSymbolKind::ClickCallbackArgs => {
                    ("gantt click callback args", String, Payload)
                }
            };
            facts.push_symbol(EditorSemanticSymbol::with_role(
                text,
                Some(detail.to_string()),
                symbol_kind,
                role,
                statement,
                selection,
            ));
        }
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &GanttDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::gantt::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &GanttDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value>> {
    Ok(merman_model::families::gantt::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}

#[cfg(feature = "compat-json")]
fn gantt_source_to_render_model(source: GanttSource) -> Result<GanttDiagramRenderModel> {
    let control = OperationControl::new();
    source
        .into_render_model(&control)
        .expect("a private parse control cannot be cancelled")
        .map_err(gantt_source_error)
}
