#[cfg(all(test, feature = "compat-json"))]
use crate::ManagedSemanticJson;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, source_span::trailing_ascii_whitespace_slot,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, SourceSpan,
};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_structured::block::{
    BlockFieldKind, BlockFieldSyntax, BlockObservation, BlockSpannedText,
    parse_block_source_with_observer_controlled,
};
use merman_core_structured::block::{
    BlockIssueLocation, ParsedBlock, parse_block_source_controlled,
};
#[cfg(test)]
pub(crate) use merman_core_structured::block::{
    MAX_BLOCK_SPACE_EXPANSION_ITEMS, block_syntax_construction_count,
    reset_block_syntax_construction_count,
};
#[cfg(all(test, feature = "compat-json"))]
use serde_json::{Value, json};

pub(crate) use merman_model::families::block::BlockDiagramModel;
#[cfg(feature = "presentation")]
pub(crate) use merman_model::families::block::BlockDiagramRenderModel;

struct BlockSemanticSource {
    parsed: ParsedBlock,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct BlockParseFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Box<EditorSemanticFacts>>,
    #[cfg(feature = "editor-facts")]
    span: Option<SourceSpan>,
}

impl BlockParseFailure {
    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        let mut facts = self.editor_facts.map(|facts| *facts).unwrap_or_default();
        facts.mark_recovered_from_parse_error(
            format!("block parser recovered after parse error: {}", self.error),
            self.span,
        );
        (*self.error, facts)
    }
}

fn construct_block_semantic_source(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    presentation_requested: bool,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<BlockSemanticSource, BlockParseFailure>> {
    let runtime = crate::runtime::active_operation_context();
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let parsed = if let Some(facts) = editor_facts
        .as_mut()
        .filter(|_| editor_demand.captures_rich())
    {
        parse_block_source_with_observer_controlled(
            code,
            config,
            &mut |event| project_block_observation(facts, code, event),
            presentation_requested,
            &runtime,
            control,
        )
    } else {
        parse_block_source_controlled(code, config, presentation_requested, &runtime, control)
    }?;
    #[cfg(not(feature = "editor-facts"))]
    let parsed =
        parse_block_source_controlled(code, config, presentation_requested, &runtime, control)?;

    Ok(match parsed {
        Ok(parsed) => Ok(BlockSemanticSource {
            parsed,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => {
            let diagnostic = ParseDiagnostic::new(issue.message);
            let diagnostic = match issue.location {
                BlockIssueLocation::Exact(span) => {
                    diagnostic.with_span(span, ParseDiagnosticSpanKind::Exact)
                }
                BlockIssueLocation::InsertionPoint(offset) => diagnostic.with_span(
                    SourceSpan::new(offset, offset),
                    ParseDiagnosticSpanKind::InsertionPoint,
                ),
                BlockIssueLocation::Fallback => diagnostic,
            };
            #[cfg(feature = "editor-facts")]
            let span = diagnostic.span();
            Err(BlockParseFailure {
                error: Box::new(Error::diagram_parse_diagnostic("block", diagnostic)),
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_facts.map(Box::new),
                #[cfg(feature = "editor-facts")]
                span,
            })
        }
    })
}

#[cfg(feature = "editor-facts")]
fn project_block_observation(
    facts: &mut EditorSemanticFacts,
    code: &str,
    event: BlockObservation<'_>,
) {
    let (kind, span) = match event {
        BlockObservation::Directive { keyword, span } => {
            facts.push_directive_prefix(keyword);
            (EditorExpectedSyntaxKind::Directive, span)
        }
        BlockObservation::Expected { kind, span } => (block_syntax_kind(kind), span),
        BlockObservation::TrailingField { kind, start, end } => {
            let Some(span) = trailing_ascii_whitespace_slot(code, start, end) else {
                return;
            };
            (block_syntax_kind(kind), span)
        }
        BlockObservation::Node {
            text,
            detail,
            kind,
            is_entity,
        } => {
            let kind = block_semantic_kind(kind);
            if is_entity {
                push_block_entity(facts, text.clone(), detail, kind);
            } else {
                push_block_reference(facts, text.clone(), detail, kind);
            }
            return;
        }
        BlockObservation::ClassDefinition(text) => {
            push_block_class_definition(
                facts,
                text.clone(),
                "block class definition",
                EditorSemanticKind::Class,
            );
            return;
        }
        BlockObservation::Payload {
            text,
            detail,
            kind,
            syntax,
        } => {
            push_block_payload_as(
                facts,
                text.clone(),
                detail,
                block_semantic_kind(kind),
                block_syntax_kind(syntax),
            );
            return;
        }
        BlockObservation::Targets { text, detail } => {
            push_block_id_list(facts, text.clone(), detail, EditorSemanticKind::Object);
            return;
        }
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(kind, span));
}

#[cfg(feature = "editor-facts")]
fn block_semantic_kind(kind: BlockFieldKind) -> EditorSemanticKind {
    match kind {
        BlockFieldKind::Namespace => EditorSemanticKind::Namespace,
        BlockFieldKind::Object => EditorSemanticKind::Object,
        BlockFieldKind::Property => EditorSemanticKind::Property,
        BlockFieldKind::Class => EditorSemanticKind::Class,
        BlockFieldKind::String => EditorSemanticKind::String,
    }
}

#[cfg(feature = "editor-facts")]
fn block_syntax_kind(kind: BlockFieldSyntax) -> EditorExpectedSyntaxKind {
    match kind {
        BlockFieldSyntax::TargetList => EditorExpectedSyntaxKind::IdList,
        BlockFieldSyntax::StyleValue => EditorExpectedSyntaxKind::StyleValue,
        BlockFieldSyntax::ClassName => EditorExpectedSyntaxKind::ClassName,
        BlockFieldSyntax::Direction => EditorExpectedSyntaxKind::BlockDirectionValue,
        BlockFieldSyntax::Payload => EditorExpectedSyntaxKind::Payload,
    }
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_block_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<BlockDiagramRenderModel> {
    let source = construct_block_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(|failure| *failure.error)?;
    Ok(source
        .parsed
        .into_render_model(&OperationControl::new())
        .expect("a private conversion control cannot be cancelled"))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_block_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<BlockDiagramRenderModel>> {
    let construction = construct_block_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(*failure.error)),
    };
    control.checkpoint()?;
    Ok(Ok(source.parsed.into_render_model(control)?))
}

pub(crate) fn parse_block_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<BlockDiagramModel>> {
    let construction = construct_block_semantic_source(
        code,
        &meta.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        false,
        control,
    )?;
    let construction = match construction {
        Ok(source) => {
            let model = source.parsed.into_semantic_model(control)?;
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts: source.editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_error_and_editor_facts();
                control.checkpoint()?;
                return Ok(crate::family::SemanticConstruction {
                    model: Err(error),
                    editor_facts: Some(facts),
                });
            }
            crate::family::SemanticConstruction {
                model: Err(*failure.error),
                #[cfg(feature = "editor-facts")]
                editor_facts: None,
            }
        }
    };
    control.checkpoint()?;
    Ok(construction)
}

#[cfg(feature = "editor-facts")]
fn push_block_entity(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_block_reference(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::reference(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_block_class_definition(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ClassName,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::class_definition(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_block_payload_as(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
    expected_kind: EditorExpectedSyntaxKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(expected_kind, text.span));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_block_id_list(
    facts: &mut EditorSemanticFacts,
    ids: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if ids.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::IdList,
        ids.span,
    ));

    let mut cursor = 0usize;
    while cursor <= ids.text.len() {
        let next_comma = ids.text[cursor..]
            .find(',')
            .map(|offset| cursor + offset)
            .unwrap_or(ids.text.len());
        let raw = &ids.text[cursor..next_comma];
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trailing = raw.trim_end().len();
        if leading < trailing {
            push_block_reference(
                facts,
                BlockSpannedText {
                    text: ids.text[cursor + leading..cursor + trailing].to_string(),
                    span: SourceSpan::new(
                        ids.span.start + cursor + leading,
                        ids.span.start + cursor + trailing,
                    ),
                },
                detail,
                kind,
            );
        }

        if next_comma == ids.text.len() {
            break;
        }
        cursor = next_comma + 1;
    }
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_block_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_block_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        true,
        control,
    )?;
    let (model, editor_facts) = match construction {
        Ok(source) => {
            let model = source.parsed.into_render_model(control)?;
            (
                Ok(crate::family::FamilySemanticModel::Block(Box::new(model))),
                source.editor_facts.unwrap_or_default(),
            )
        }
        Err(failure) => {
            let (error, editor_facts) = failure.into_error_and_editor_facts();
            (Err(error), editor_facts)
        }
    };
    let parsed = crate::family::CombinedSemanticParse::from_parts(model, editor_facts);
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn render_model_to_compat_json(
    model: &BlockDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::block::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &BlockDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value>> {
    Ok(merman_model::families::block::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn parse_block(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let control = OperationControl::new();
    parse_block_compatibility_model_controlled(code, meta, &control)
        .expect("a private parse control cannot be cancelled")?
        .into_compatibility_json_controlled(&meta.compat_json_metadata(), &control)
        .expect("a private projection control cannot be cancelled")
        .map(|(model, _)| model.into_unmanaged_value())
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_block_compatibility_model_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<crate::family::FamilySemanticModel>> {
    let construction = construct_block_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(*failure.error)),
    };
    let model = source.parsed.into_render_model(control)?;
    control.checkpoint()?;
    Ok(Ok(crate::family::FamilySemanticModel::Block(Box::new(
        model,
    ))))
}
#[cfg(all(test, feature = "compat-json"))]
mod tests;

#[cfg(test)]
mod semantic_tests;
