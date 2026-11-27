use crate::Error;
#[cfg(feature = "presentation")]
use crate::Result;
#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
#[cfg(feature = "editor-facts")]
use merman_core_structured::eventmodeling::{
    EventModelingEditorDemand, EventModelingFieldKind, EventModelingObservation,
    parse_eventmodeling_source_with_observer_controlled,
};
use merman_core_structured::eventmodeling::{
    EventModelingSourceIssue, ParsedEventModeling, parse_eventmodeling_source_controlled,
};
#[cfg(test)]
use merman_core_structured::eventmodeling::{
    eventmodeling_syntax_construction_count, reset_eventmodeling_syntax_construction_count,
};
use merman_model::families::event_modeling::EventModelingDiagramModel;
#[cfg(feature = "compat-json")]
use serde_json::Value;

pub(crate) fn sanitize_common_db_fields(
    model: &mut EventModelingDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}
#[cfg(feature = "compat-json")]
pub(crate) fn parse_eventmodeling(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    construct_eventmodeling_semantic_source(code, meta)
        .map_err(|failure| *failure.error)?
        .into_compat_json(meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_eventmodeling_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let construction = match construct_eventmodeling_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )? {
        Ok(mut source) => {
            let editor_facts = source.editor_facts.take().expect("requested editor facts");
            let json = source.into_compat_json_controlled(meta, control)?;
            Ok((json, editor_facts))
        }
        Err(failure) => Err(failure),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |(json, editor_facts)| (json, editor_facts),
        EventModelingParseFailure::into_error_and_editor_facts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &EventModelingDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::event_modeling::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_eventmodeling_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<EventModelingDiagramModel> {
    Ok(construct_eventmodeling_semantic_source(code, meta)
        .map_err(|failure| *failure.error)?
        .into_render_model(meta))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_eventmodeling_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<EventModelingDiagramModel>> {
    let construction = construct_eventmodeling_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(*failure.error)),
    };
    Ok(Ok(source.into_render_model_controlled(meta, control)?))
}

pub(crate) fn parse_eventmodeling_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<EventModelingDiagramModel>> {
    let construction = construct_eventmodeling_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let mut source = source;
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts.take();
            let model = source.into_model_controlled(&metadata.semantic_config, control)?;
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            {
                match editor_demand {
                    EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
                        let (error, facts) = failure.into_error_and_editor_facts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(*failure.error),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(*failure.error),
                }
            }
        }
    })
}

struct EventModelingSemanticSource {
    syntax: ParsedEventModeling,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct EventModelingParseFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Box<EditorSemanticFacts>>,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

impl EventModelingSemanticSource {
    #[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
    fn into_render_model(self, meta: &SourcePresentationMetadata) -> EventModelingDiagramModel {
        self.into_render_model_controlled(meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")
    }

    fn into_model_controlled(
        self,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<EventModelingDiagramModel> {
        let mut model = self.syntax.into_model_controlled(config, control)?;
        sanitize_common_db_fields(&mut model, config);
        Ok(model)
    }

    #[cfg(feature = "presentation")]
    fn into_render_model_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<EventModelingDiagramModel> {
        self.into_model_controlled(meta.effective_config.as_config(), control)
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let model = self.into_render_model(meta);
        render_model_to_compat_json(&model, meta)
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn into_compat_json_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Result<Value>> {
        let model = self.into_render_model_controlled(meta, control)?;
        control.checkpoint()?;
        Ok(render_model_to_compat_json(&model, meta))
    }
}

impl EventModelingParseFailure {
    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        let mut facts = *self.editor_facts.expect("requested editor facts");
        facts.mark_recovered_from_parse_error(
            format!(
                "eventmodeling parser recovered after parse error: {}",
                self.error
            ),
            Some(self.span),
        );
        (*self.error, facts)
    }
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_eventmodeling_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<EventModelingSemanticSource, EventModelingParseFailure> {
    construct_eventmodeling_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_eventmodeling_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<EventModelingSemanticSource, EventModelingParseFailure>,
> {
    #[cfg(not(feature = "editor-facts"))]
    let syntax = parse_eventmodeling_source_controlled(code, control)?;
    #[cfg(feature = "editor-facts")]
    let (syntax, editor_facts) = match editor_demand {
        EditorFactsDemand::Omit => (parse_eventmodeling_source_controlled(code, control)?, None),
        EditorFactsDemand::Recovery | EditorFactsDemand::Capture => {
            let demand = if editor_demand.captures_rich() {
                EventModelingEditorDemand::Capture
            } else {
                EventModelingEditorDemand::Recovery
            };
            let mut facts = EditorSemanticFacts::new();
            let syntax = parse_eventmodeling_source_with_observer_controlled(
                code,
                demand,
                &mut |observation| push_eventmodeling_observation(&mut facts, observation),
                control,
            )?;
            (syntax, Some(facts))
        }
    };
    Ok(match syntax {
        Ok(syntax) => Ok(EventModelingSemanticSource {
            syntax,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => {
            #[cfg(feature = "editor-facts")]
            let span = issue.span();
            Err(EventModelingParseFailure {
                error: Box::new(eventmodeling_source_error(diagram_type, issue)),
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_facts.map(Box::new),
                #[cfg(feature = "editor-facts")]
                span,
            })
        }
    })
}

fn eventmodeling_source_error(diagram_type: &str, issue: EventModelingSourceIssue) -> Error {
    match issue {
        EventModelingSourceIssue::Exact { message, span } => {
            Error::diagram_parse_exact(diagram_type, message, span)
        }
        EventModelingSourceIssue::InsertionPoint { message, offset } => {
            Error::diagram_parse_insertion_point(diagram_type, message, offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn push_eventmodeling_observation(
    facts: &mut EditorSemanticFacts,
    observation: EventModelingObservation<'_>,
) {
    use EditorRenamePolicy as Rename;
    use EditorSemanticKind as Kind;
    use EditorSemanticRole as Role;
    use EventModelingFieldKind as Field;

    let (field, text, span) = match observation {
        EventModelingObservation::CollectionStarted => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            return;
        }
        EventModelingObservation::Common(common) => {
            push_langium_common_editor_fact(facts, common, "eventmodeling");
            return;
        }
        EventModelingObservation::ValidationIssue { message, span } => {
            facts.push_diagnostic(message, Some(span));
            return;
        }
        EventModelingObservation::Field { kind, text, span } => (kind, text, span),
    };
    if matches!(
        field,
        Field::Header | Field::Frame | Field::DataBlock | Field::NoteBlock
    ) {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
    }
    let (detail, kind, role, rename) = match field {
        Field::Header => (
            "eventmodeling header",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::ModelEntity => (
            "eventmodeling model entity",
            Kind::Object,
            Role::Entity,
            Rename::QualifiedIdentifier,
        ),
        Field::Frame => (
            "eventmodeling frame",
            Kind::Namespace,
            Role::Entity,
            Rename::EventModelingFrameId,
        ),
        Field::EntityType => (
            "eventmodeling entity type",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::EntityIdentifier => (
            "eventmodeling entity identifier",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::SourceFrame => (
            "eventmodeling source frame",
            Kind::Namespace,
            Role::Reference,
            Rename::EventModelingFrameId,
        ),
        Field::DataReference => (
            "eventmodeling data reference",
            Kind::Namespace,
            Role::Reference,
            Rename::EventModelingId,
        ),
        Field::DataType => (
            "eventmodeling data type",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::InlineData => (
            "eventmodeling inline data",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::DataEntity => (
            "eventmodeling data entity",
            Kind::Namespace,
            Role::Entity,
            Rename::EventModelingId,
        ),
        Field::DataBlock => (
            "eventmodeling data block",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::NoteSourceFrame => (
            "eventmodeling note source frame",
            Kind::Namespace,
            Role::Reference,
            Rename::EventModelingFrameId,
        ),
        Field::NoteBlock => (
            "eventmodeling note block",
            Kind::String,
            Role::Payload,
            Rename::None,
        ),
        Field::GwtSourceFrame => (
            "eventmodeling gwt source frame",
            Kind::Namespace,
            Role::Reference,
            Rename::EventModelingFrameId,
        ),
        Field::GwtEntityReference => (
            "eventmodeling gwt entity reference",
            Kind::Object,
            Role::Reference,
            Rename::EventModelingId,
        ),
    };
    facts.push_symbol(
        EditorSemanticSymbol::with_role(text, Some(detail.to_string()), kind, role, span, span)
            .with_rename_policy(rename),
    );
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::MermaidEngine;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{MermaidConfig, ParseDiagnosticSpanKind, SourcePresentationMetadata};

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "eventmodeling".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn eventmodeling_parser_can_cancel_inside_a_gwt_group() {
        let mut text = String::from("eventmodeling\ntf 001 evt Root\ngwt 001 given ");
        for index in 0..512 {
            text.push_str(&format!("evt Entity{index} "));
        }
        text.push_str("then evt Entity0\n");
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(40);

        assert!(matches!(
            construct_eventmodeling_semantic_source_controlled(
                &text,
                &meta().diagram_type,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn eventmodeling_projection_can_cancel_between_frames() {
        let mut text = String::from("eventmodeling\n");
        for index in 0..512 {
            text.push_str(&format!("tf {:03} evt Entity{index}\n", index % 1000));
        }
        let source = construct_eventmodeling_semantic_source(&text, &meta())
            .unwrap_or_else(|_| panic!("large eventmodeling source"));
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(20);

        assert!(matches!(
            source.into_render_model_controlled(&meta(), &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn parses_simple_model_with_full_syntax() {
        let model = parse_eventmodeling_model_for_render(
            "eventmodeling\ntimeframe 01 event Start\n",
            &meta(),
        )
        .unwrap();

        assert_eq!(model.frames.len(), 1);
        let frame = &model.frames[0];
        assert_eq!(frame.name, "01");
        assert_eq!(frame.model_entity_type, "event");
        assert_eq!(frame.entity_identifier, "Start");
    }

    #[test]
    fn parses_reset_frames_qualified_names_and_sources() {
        let model = parse_eventmodeling_model_for_render(
            r#"eventmodeling
tf 02 ui UI
resetframe 01 evt Product.PriceChanged
tf 03 evt Cart.ItemAdded ->> 01 ->> 02
"#,
            &meta(),
        )
        .unwrap();

        assert_eq!(model.frames.len(), 3);
        assert_eq!(model.frames[1].frame_kind, "resetframe");
        assert_eq!(model.frames[1].entity_identifier, "Product.PriceChanged");
        assert_eq!(model.frames[2].source_frames, ["01", "02"]);
    }

    #[test]
    fn qualified_names_allow_hidden_trivia_around_dots_and_normalize_segments() {
        let text = concat!(
            "eventmodeling\n",
            "entity Sales /* bounded context */ . Order\n",
            "tf 01 evt Sales \t.\n Order\n",
        );
        let model = parse_eventmodeling_model_for_render(text, &meta()).unwrap();

        assert_eq!(model.frames[0].entity_identifier, "Sales.Order");

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "Sales.Order")
            );
        }
    }

    #[test]
    fn yaml_is_hidden_where_the_upstream_terminal_can_start() {
        let text = concat!(
            "eventmodeling ---\n",
            "title: hidden document metadata\n",
            "---\n",
            "entity Cart\n",
            "tf 01 evt Cart\n",
        );
        let model = parse_eventmodeling_model_for_render(text, &meta()).unwrap();

        assert_eq!(model.frames[0].entity_identifier, "Cart");
    }

    #[test]
    fn captures_inline_data_and_data_blocks() {
        let model = parse_eventmodeling_model_for_render(
            r#"eventmodeling
tf 01 cmd AddItem { productId: 7 }
tf 02 evt ItemAdded [[ItemAddedData]]
tf 03 evt QuotedData `json`" { "ok": true } "

data ItemAddedData {
  productId: 7
}
"#,
            &meta(),
        )
        .unwrap();

        assert_eq!(
            model.frames[0].data_inline_value.as_deref(),
            Some("{ productId: 7 }")
        );
        assert_eq!(
            model.frames[1].data_reference.as_deref(),
            Some("ItemAddedData")
        );
        assert_eq!(
            model.frames[2].data_inline_value.as_deref(),
            Some("\" { \"ok\": true } \"")
        );
        assert_eq!(model.data_entities.len(), 1);
        assert!(
            model.data_entities[0]
                .data_block_value
                .contains("productId")
        );
    }

    #[test]
    fn combined_parse_constructs_once_and_preserves_all_projections() {
        let text = concat!(
            "eventmodeling\r\n",
            "tf 01 cmd AddItem { productId: 7 }\r\n",
            "tf 02 evt ItemAdded ->> 01 [[ItemAddedData]]\r\n",
            "data ItemAddedData {\r\n",
            "  productId: 7\r\n",
            "}\r\n",
        );
        let expected_json = parse_eventmodeling(text, &meta()).unwrap();
        let expected_model = parse_eventmodeling_model_for_render(text, &meta()).unwrap();

        reset_eventmodeling_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (json, facts) =
            crate::family::test_support::into_result(parse_eventmodeling_json_and_editor_facts(
                text,
                &meta(),
                &crate::OperationControl::new(),
            ))
            .unwrap();

        #[cfg(not(feature = "editor-facts"))]
        let json = parse_eventmodeling(text, &meta()).unwrap();
        assert_eq!(eventmodeling_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta()).unwrap(),
            expected_json
        );
        assert_eq!(
            json["frames"],
            serde_json::to_value(&expected_model.frames).unwrap()
        );
        assert_eq!(
            json["dataEntities"],
            serde_json::to_value(&expected_model.data_entities).unwrap()
        );

        #[cfg(feature = "editor-facts")]
        {
            let block_start = text.find("{\r\n").unwrap();
            let block_end = text.rfind('}').unwrap() + 1;
            let block = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("eventmodeling data block"))
                .expect("missing multiline data block fact");
            assert_eq!(block.span, SourceSpan::new(block_start, block_end));
            assert_eq!(block.selection, block.span);
            assert_eq!(block.name, &text[block_start..block_end]);
        }
    }

    #[test]
    fn data_block_closing_brace_accepts_pinned_trailing_whitespace() {
        for suffix in ["  \n", "\t"] {
            let text = format!("eventmodeling\ndata Payload {{\nvalue\n}}{suffix}");
            #[cfg(feature = "editor-facts")]
            let block_start = text.find('{').expect("opening brace");
            #[cfg(feature = "editor-facts")]
            let block_end = text.rfind('}').expect("closing brace") + 1;

            #[cfg(feature = "editor-facts")]
            let (json, facts) = crate::family::test_support::into_result(
                parse_eventmodeling_json_and_editor_facts(
                    &text,
                    &meta(),
                    &crate::OperationControl::new(),
                ),
            )
            .expect("EM_DATA_BLOCK accepts whitespace after its closing brace");
            #[cfg(not(feature = "editor-facts"))]
            let json = parse_eventmodeling(&text, &meta())
                .expect("EM_DATA_BLOCK accepts whitespace after its closing brace");
            let typed = parse_eventmodeling_model_for_render(&text, &meta())
                .expect("typed projection accepts the same block");
            #[cfg(feature = "editor-facts")]
            let block = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("eventmodeling data block"))
                .expect("data block semantic fact");

            #[cfg(feature = "editor-facts")]
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert_eq!(json["dataEntities"][0]["name"], "Payload");
            assert_eq!(typed.data_entities[0].name, "Payload");
            #[cfg(feature = "editor-facts")]
            assert_eq!(block.span, SourceSpan::new(block_start, block_end));
            #[cfg(feature = "editor-facts")]
            assert_eq!(block.name, text[block_start..block_end]);
        }
    }

    #[test]
    fn incomplete_data_block_recovers_from_the_single_construction() {
        let text = concat!(
            "eventmodeling\n",
            "tf 01 cmd AddItem\n",
            "data Broken {\n",
            "  productId: 7\n",
        );
        let Error::DiagramParse { diagnostic, .. } =
            parse_eventmodeling(text, &meta()).unwrap_err()
        else {
            panic!("expected eventmodeling parse error");
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_eventmodeling_syntax_construction_count();
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(eventmodeling_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "01"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Broken"));
            let block_start = text.find('{').unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.detail.as_deref() == Some("eventmodeling data block")
                    && symbol.span == SourceSpan::new(block_start, text.len())
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn parse_eventmodeling_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r#"eventmodeling
tf 01 cmd AddItem { productId: 7 }
tf 02 evt ItemAdded ->> 01 [[ItemAddedData]]

data ItemAddedData {
  productId: 7
}
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("eventmodeling", text)
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "01"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "ItemAddedData")
        );
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "ItemAdded")
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "AddItem"));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "01" && symbol.rename_policy == EditorRenamePolicy::EventModelingFrameId
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "ItemAddedData"
                && symbol.rename_policy == EditorRenamePolicy::EventModelingId
        }));

        let frame_start = text.find("01").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(frame_start, frame_start + "01".len())
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn editor_facts_assign_entity_reference_and_payload_roles_explicitly() {
        let text = concat!(
            "eventmodeling\n",
            "entity CartUpdated\n",
            "tf 001 cmd UpdateCart\n",
            "tf 002 evt Cart.Updated ->> 001 [[CartData]] `json`{ \"ok\": true }\n",
            "data CartData `json` {\n",
            "  { \"ok\": true }\n",
            "}\n",
            "note 002 `md` {\n",
            "  Cart changed\n",
            "}\n",
            "gwt 002\n",
            "  given\n",
            "    evt CartUpdated\n",
            "  then\n",
            "    evt CartUpdated\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_eventmodeling_json_and_editor_facts,
            text,
            &meta(),
        );

        for (name, detail) in [
            ("CartUpdated", "eventmodeling model entity"),
            ("001", "eventmodeling frame"),
            ("002", "eventmodeling frame"),
            ("CartData", "eventmodeling data entity"),
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.role == EditorSemanticRole::Entity
            }));
        }
        for (name, detail) in [
            ("001", "eventmodeling source frame"),
            ("CartData", "eventmodeling data reference"),
            ("002", "eventmodeling note source frame"),
            ("002", "eventmodeling gwt source frame"),
            ("CartUpdated", "eventmodeling gwt entity reference"),
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.role == EditorSemanticRole::Reference
            }));
        }
        for detail in [
            "eventmodeling entity type",
            "eventmodeling entity identifier",
            "eventmodeling data type",
            "eventmodeling inline data",
            "eventmodeling data block",
            "eventmodeling note block",
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.detail.as_deref() == Some(detail)
                    && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }

    #[test]
    fn parses_complete_pinned_langium_grammar_from_one_source() {
        let text = r#"eventmodeling title Checkout flow
entity CartUpdated accTitle: Checkout accessibility title
entity ProductChanged accDescr {
  Checkout accessibility description
}
tf 001 cmd UpdateCart
tf 002 evt Cart.Updated ->> 001 [[CartData]] `json`{ "ok": true }
data CartData `json` {
  { "ok": true }
}
note 002 `md` {
  Cart changed
}
gwt 002
  given
    evt CartUpdated
  when
    cmd ProductChanged
  then
    evt CartUpdated
"#;

        let model = parse_eventmodeling_model_for_render(text, &meta()).unwrap();
        assert_eq!(model.title.as_deref(), Some(" title Checkout flow"));
        assert_eq!(
            model.acc_title.as_deref(),
            Some("accTitle: Checkout accessibility title")
        );
        assert_eq!(
            model.acc_descr.as_deref(),
            Some(" accDescr {\nCheckout accessibility description\n}")
        );
        assert_eq!(model.frames.len(), 2);
        assert_eq!(model.data_entities.len(), 1);

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (name, detail) in [
                ("CartUpdated", "eventmodeling model entity"),
                ("CartData", "eventmodeling data entity"),
                ("002", "eventmodeling note source frame"),
                ("ProductChanged", "eventmodeling gwt entity reference"),
            ] {
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == name && symbol.detail.as_deref() == Some(detail)
                }));
            }
            for directive in ["title", "accTitle", "accDescr"] {
                assert!(
                    facts
                        .directive_prefixes
                        .iter()
                        .any(|item| item == directive)
                );
            }
        }
    }

    #[test]
    fn metadata_matches_selected_runtime_raw_token_values() {
        let mut metadata = meta();
        metadata.effective_config = crate::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({ "securityLevel": "strict" }),
        );
        for (source, title, acc_title, acc_descr) in [
            (
                "eventmodeling title Value\ntf 01 event Start\n",
                Some(" title Value"),
                None,
                None,
            ),
            (
                "eventmodeling accTitle: Accessible <!--a11y-->\ntf 01 event Start\n",
                None,
                Some("accTitle: Accessible "),
                None,
            ),
            (
                "eventmodeling accDescr { First <!--a11y--> }\ntf 01 event Start\n",
                None,
                None,
                Some(" accDescr { First  }"),
            ),
            (
                "eventmodeling title Value %% comment\ntf 01 event Start\n",
                Some(" title Value "),
                None,
                None,
            ),
            (
                "eventmodeling\ntf 01 event Start accTitle: Accessible\n",
                None,
                Some("accTitle: Accessible"),
                None,
            ),
            (
                "eventmodeling\taccTitle: Accessible\n",
                None,
                Some("accTitle: Accessible"),
                None,
            ),
            (
                "eventmodeling accDescr {\nFirst\n<!--a11y-->\n}\ntf 01 event Start\n",
                None,
                None,
                Some(" accDescr {\nFirst\n}"),
            ),
            (
                "eventmodeling accTitle: <!--a11y-->\u{0085}<!--a11y-->\ntf 01 event Start\n",
                None,
                Some("accTitle: \u{0085}"),
                None,
            ),
            (
                "eventmodeling accTitle: Accessible %% first %% second\ntf 01 event Start\n",
                None,
                Some("accTitle: Accessible "),
                None,
            ),
            (
                "eventmodeling accDescr{One} accDescr{Two}\ntf 01 event Start\n",
                None,
                None,
                Some(" accDescr{Two}"),
            ),
            (
                "eventmodeling accDescr{One} accTitle: Two\n",
                None,
                Some("accTitle: Two"),
                Some(" accDescr{One}"),
            ),
            (
                "eventmodeling  \t accTitle: Value\n",
                None,
                Some("accTitle: Value"),
                None,
            ),
            (
                "eventmodeling accTitle: a \"quote\" %% rest\n",
                None,
                Some("accTitle: a \"quote\" "),
                None,
            ),
            ("eventmodeling title\n", Some(" title"), None, None),
            ("eventmodeling accTitle:\n", None, Some("accTitle:"), None),
            (
                "eventmodeling accDescr{}\n",
                None,
                None,
                Some(" accDescr{}"),
            ),
            (
                "eventmodeling accDescr {\nFirst\n<!--a11y-->\n} accTitle: Accessible <!--a11y-->\ntf 01 event Start\n",
                None,
                Some("accTitle: Accessible "),
                Some(" accDescr {\nFirst\n}"),
            ),
            (
                "eventmodeling accDescr {\n<!--a11y-->\u{0085}<!--a11y-->\n} accTitle: <!--a11y-->\u{0085}<!--a11y-->\ntf 01 event Start\n",
                None,
                Some("accTitle: \u{0085}"),
                Some(" accDescr {\n\u{0085}\n}"),
            ),
            (
                "eventmodeling accDescr {\n\n} accTitle: \ntf 01 event Start\n",
                None,
                Some("accTitle: "),
                Some(" accDescr {\n}"),
            ),
        ] {
            let model = parse_eventmodeling_model_for_render(source, &metadata).unwrap();
            assert_eq!(model.title.as_deref(), title, "source: {source:?}");
            assert_eq!(model.acc_title.as_deref(), acc_title, "source: {source:?}");
            assert_eq!(model.acc_descr.as_deref(), acc_descr, "source: {source:?}");
            let json = parse_eventmodeling(source, &metadata).unwrap();
            assert_eq!(
                render_model_to_compat_json(&model, &metadata).unwrap(),
                json
            );
        }
    }

    #[test]
    fn metadata_boundaries_match_selected_runtime_rejections() {
        for source in [
            "eventmodelingaccTitle: Value\n",
            "eventmodeling accDescr{One}accTitle: Two\n",
            "eventmodeling accDescr{One}\n accTitle: Two\n",
            "eventmodeling accDescr{Oops\n",
            "eventmodeling\u{feff}accTitle: Nope\n",
            "eventmodeling\ntitle Value\ntf 01 event Start\n",
            "eventmodeling\naccDescr {Help}\ntf 01 event Start\n",
            "eventmodeling\u{feff} accTitle: Accessible\n",
        ] {
            assert!(parse_eventmodeling(source, &meta()).is_err(), "{source:?}");
        }
    }

    #[test]
    fn metadata_blocks_allow_following_statements_without_eol_and_last_assignment_wins() {
        let text = concat!(
            "eventmodeling accDescr { First }entity Seen title First\r\n",
            "tf 01 evt Seen accDescr { Second }tf 02 cmd Next title Final\r\n",
        );
        let model = parse_eventmodeling_model_for_render(text, &meta()).unwrap();
        assert_eq!(model.title.as_deref(), Some(" title Final"));
        assert_eq!(model.acc_descr.as_deref(), Some(" accDescr { Second }"));
        assert_eq!(model.frames.len(), 2);
        assert_eq!(model.frames[0].entity_identifier, "Seen");
        assert_eq!(model.frames[1].entity_identifier, "Next");
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn metadata_editor_facts_keep_whole_raw_token_ranges() {
        let text = concat!(
            "/* 前置 🤓 */\r\neventmodeling",
            " accDescr { First %% literal 🤓 } accTitle: Accessible %% hidden \" %%\r\n",
            "entity Seen title Final\t value\r\n",
            "tf 01 evt Seen\r\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_eventmodeling_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (raw, detail, directive) in [
            (
                " accDescr { First %% literal 🤓 }",
                "eventmodeling accessibility description",
                "accDescr",
            ),
            (
                " accTitle: Accessible ",
                "eventmodeling accessibility title",
                "accTitle",
            ),
            (" title Final\t value", "eventmodeling title", "title"),
        ] {
            let start = text.find(raw).unwrap();
            let span = SourceSpan::new(start, start + raw.len());
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some(detail))
                .unwrap();
            assert_eq!(symbol.name, raw);
            assert_eq!(symbol.span, span);
            assert_eq!(symbol.selection, span);
            assert_eq!(symbol.role, EditorSemanticRole::Payload);
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == span
            }));
            assert!(
                facts
                    .directive_prefixes
                    .iter()
                    .any(|item| item == directive)
            );
        }
    }

    #[test]
    fn metadata_shadowed_by_identifiers_rejects_and_recovers_later_records() {
        for gap in ["\n", "\n ", "\r\n\t", "\u{feff} "] {
            for directive in [
                "title Invalid 🤓",
                "accTitle: Invalid 🤓",
                "accDescr { Invalid 🤓 }",
            ] {
                let text = format!(
                    "/* 前置 🤓 */\r\neventmodeling{gap}{directive}\r\nentity After\r\ntf 02 evt Done\r\n"
                );
                let start = text.find(directive).unwrap();
                let span = SourceSpan::new(start, start + directive.len());
                let Error::DiagramParse { diagnostic, .. } =
                    parse_eventmodeling(&text, &meta()).unwrap_err()
                else {
                    panic!("expected metadata rejection for {text:?}");
                };
                assert_eq!(diagnostic.span(), Some(span), "source: {text:?}");
                assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

                #[cfg(feature = "editor-facts")]
                {
                    let facts = crate::family::test_support::editor_facts(
                        parse_eventmodeling_json_and_editor_facts,
                        &text,
                        &meta(),
                    );
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                    assert!(facts.directive_prefixes.is_empty());
                    assert!(facts.diagnostics.iter().any(|item| item.span == Some(span)));
                    for (name, detail) in [
                        ("After", "eventmodeling model entity"),
                        ("02", "eventmodeling frame"),
                        ("Done", "eventmodeling entity identifier"),
                    ] {
                        let start = text.find(name).unwrap();
                        let span = SourceSpan::new(start, start + name.len());
                        assert!(facts.symbols.iter().any(|symbol| {
                            symbol.name == name
                                && symbol.detail.as_deref() == Some(detail)
                                && symbol.span == span
                                && symbol.selection == span
                        }));
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn rejected_metadata_after_blocks_keeps_prefix_and_later_record_spans() {
        for gap in ["", "\n "] {
            let raw = " accDescr{One}";
            let rejected = "accTitle: Invalid 🤓";
            let text =
                format!("eventmodeling{raw}{gap}{rejected}\r\nentity After\r\ntf 02 evt Done\r\n");
            let start = text.find(rejected).unwrap();
            let rejected_span = SourceSpan::new(start, start + rejected.len());
            let Error::DiagramParse { diagnostic, .. } =
                parse_eventmodeling(&text, &meta()).unwrap_err()
            else {
                panic!("expected metadata rejection");
            };
            assert_eq!(diagnostic.span(), Some(rejected_span));
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                &text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|item| item.span == Some(rejected_span))
            );
            assert!(
                facts
                    .directive_prefixes
                    .iter()
                    .any(|item| item == "accDescr")
            );
            assert!(
                !facts
                    .directive_prefixes
                    .iter()
                    .any(|item| item == "accTitle")
            );
            for (name, detail) in [
                (raw, "eventmodeling accessibility description"),
                ("After", "eventmodeling model entity"),
                ("02", "eventmodeling frame"),
                ("Done", "eventmodeling entity identifier"),
            ] {
                let start = text.find(name).unwrap();
                let span = SourceSpan::new(start, start + name.len());
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.span == span
                        && symbol.selection == span
                }));
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_preserves_newline_priority_before_indented_metadata() {
        let text = concat!(
            "eventmodeling\r\n",
            "invalid statement\r\n",
            " accTitle: Unreachable\r\n",
            "entity After\r\n",
            "tf 02 evt Done\r\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_eventmodeling_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Done"));
    }

    #[test]
    fn malformed_metadata_tokens_do_not_admit_shared_common_syntax() {
        for directive in [
            " AccTitle: Wrong case",
            " accTitleSuffix: Invalid",
            " title: Invalid",
            " titleSuffix",
            " accDescr { Unterminated",
        ] {
            let text = format!("eventmodeling{directive}\ntf 01 evt After\n");
            assert!(parse_eventmodeling(&text, &meta()).is_err(), "{text:?}");
            #[cfg(feature = "editor-facts")]
            {
                let facts = crate::family::test_support::editor_facts(
                    parse_eventmodeling_json_and_editor_facts,
                    &text,
                    &meta(),
                );
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
            }
        }
    }

    #[test]
    fn malformed_middle_statement_keeps_later_semantic_facts() {
        let text = concat!(
            "eventmodeling\r\n",
            "entity Before\r\n",
            "tf 01 invalid Broken\r\n",
            "entity After\r\n",
            "tf 02 evt Done\r\n",
        );
        let invalid_start = text.find("invalid").unwrap();
        let invalid_span = SourceSpan::new(invalid_start, invalid_start + "invalid".len());

        let Error::DiagramParse { diagnostic, .. } =
            parse_eventmodeling(text, &meta()).expect_err("strict parse must keep the first error")
        else {
            panic!("expected eventmodeling parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(invalid_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Done"));
        }
    }

    #[test]
    fn cursor_skips_c_style_comments_with_crlf_unicode_content() {
        let text = concat!(
            "eventmodeling // 行内 🤓\r\n",
            "%% global preprocess comment\r\n",
            "/* 块注释\r\n",
            "   第二行 🤓 */\r\n",
            "entity After\r\n",
        );
        parse_eventmodeling(text, &meta()).expect("C-style comments are hidden family grammar");
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );

            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
        }
    }

    #[test]
    fn unterminated_block_comment_keeps_the_confirmed_comment_prefix() {
        let text = concat!(
            "eventmodeling\r\n",
            "entity Before\r\n",
            "/* 未闭合 🤓\r\n",
            "仍是注释",
        );
        let Error::DiagramParse { diagnostic, .. } = parse_eventmodeling(text, &meta())
            .expect_err("unterminated block comment must remain a strict parse error")
        else {
            panic!("expected eventmodeling parse diagnostic");
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        }
    }

    #[test]
    fn rejects_tokens_outside_the_pinned_langium_grammar_with_exact_spans() {
        for (statement, invalid) in [
            ("tf 1000 evt Started", "1000"),
            ("tf xx evt Started", "xx"),
            ("tf 01 invalid Started", "invalid"),
            ("tf 01 evt Product..Started", "Product..Started"),
            ("data Cart `yaml` {\n}\n", "yaml"),
            ("unknown value", "unknown value"),
        ] {
            let text = format!("eventmodeling\n{statement}\n");
            let Error::DiagramParse { diagnostic, .. } =
                parse_eventmodeling(&text, &meta()).unwrap_err()
            else {
                panic!("expected eventmodeling parse diagnostic for {statement:?}");
            };
            let start = text.find(invalid).unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + invalid.len())),
                "statement: {statement:?}"
            );
            assert_eq!(
                diagnostic.span_kind(),
                ParseDiagnosticSpanKind::Exact,
                "statement: {statement:?}"
            );
        }
    }

    #[test]
    fn reports_link_and_source_type_validation_without_rejecting_render_semantics() {
        let text = r#"eventmodeling
entity KnownEvent
tf 01 rmo ReadModel
tf 02 evt Changed ->> 01 [[MissingData]]
note 99 {
  unresolved frame
}
gwt 02
  given
    evt MissingEntity
  then
    evt KnownEvent
"#;

        parse_eventmodeling(text, &meta()).unwrap();
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_eventmodeling_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for expected in [
                "event can only receive input from a command",
                "unknown eventmodeling data reference 'MissingData'",
                "unknown eventmodeling frame reference '99'",
                "unknown eventmodeling model entity reference 'MissingEntity'",
            ] {
                assert!(
                    facts
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains(expected)),
                    "missing validation diagnostic containing {expected:?}: {:?}",
                    facts.diagnostics
                );
            }
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_keeps_validation_and_parse_diagnostics_without_rich_facts() {
        for source in [
            "eventmodeling\ntf 01 evt Changed ->> 99 [[MissingData]]\n",
            "eventmodeling\ntf 01 cmd Start\ntf 01 evt Changed\n",
        ] {
            let capture = |demand| {
                parse_eventmodeling_semantic_model_controlled(
                    source,
                    &metadata(),
                    demand,
                    &crate::OperationControl::new(),
                )
                .unwrap()
            };
            let full = capture(EditorFactsDemand::Capture);
            let full_facts = full.editor_facts.unwrap();
            assert!(!full_facts.symbols.is_empty());
            assert!(!full_facts.diagnostics.is_empty());
            crate::editor::test_work::reset();
            reset_eventmodeling_syntax_construction_count();
            let recovery = capture(EditorFactsDemand::Recovery);
            assert_eq!(recovery.model.is_ok(), full.model.is_ok());
            let facts = recovery.editor_facts.unwrap();
            assert_eq!(facts.completeness, full_facts.completeness);
            assert_eq!(facts.diagnostics, full_facts.diagnostics);
            assert!(facts.symbols.is_empty());
            assert!(facts.directive_prefixes.is_empty());
            assert!(facts.expected_syntax.is_empty());
            let work = crate::editor::test_work::read();
            assert_eq!(
                (work.symbols, work.directives, work.expected_syntax),
                (0, 0, 0)
            );
            assert_eq!(eventmodeling_syntax_construction_count(), 1);
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "eventmodeling".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn adjacent_hidden_comments_separate_scalar_tokens_without_preprocessing() {
        for separator in [
            " ",
            "\u{feff}",
            "\u{00a0}",
            "\u{2028}",
            "/**/",
            "/* boundary */",
            "// boundary\r\n",
            "%% boundary\r\n",
            "%%{init: {}}%%\r\n",
        ] {
            let source = format!("eventmodeling\ntf 01{separator}evt{separator}Started\n");
            let model = parse_eventmodeling_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap_or_else(|error| panic!("{source}: {error}"));
            assert_eq!(model.frames.len(), 1);
            assert_eq!(model.frames[0].name, "01");
            assert_eq!(model.frames[0].model_entity_type, "evt");
            assert_eq!(model.frames[0].entity_identifier, "Started");
        }
    }

    #[test]
    fn nel_is_not_an_eventmodeling_scalar_separator() {
        let result = parse_eventmodeling_semantic_model_controlled(
            "eventmodeling\ntf 01\u{0085}event Start\n",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(result.model.is_err());
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let metadata = metadata();
        let source = concat!(
            "eventmodeling accTitle: <script>secret</script>Public\n",
            "tf 01 cmd AddItem { productId: 7 }\n",
            "tf 02 evt ItemAdded ->> 01\n",
            "data ItemAddedData {\n",
            "  productId: 7\n",
            "}\n",
        );
        reset_eventmodeling_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_eventmodeling_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("accTitle: Public"));
        assert_eq!(model.frames[0].name, "01");
        assert_eq!(model.frames[0].model_entity_type, "cmd");
        assert_eq!(model.frames[0].entity_identifier, "AddItem");
        assert_eq!(
            model.frames[0].data_inline_value.as_deref(),
            Some("{ productId: 7 }")
        );
        assert_eq!(model.frames[1].source_frames, ["01"]);
        assert_eq!(model.data_entities[0].name, "ItemAddedData");
        assert!(
            model.data_entities[0]
                .data_block_value
                .contains("productId: 7")
        );
        assert_eq!(eventmodeling_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full_metadata = crate::SourcePresentationMetadata {
                diagram_type: metadata.diagram_type.clone(),
                config: metadata.config.clone(),
                effective_config: crate::presentation::SourcePresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ),
                title: metadata.title.clone(),
            };
            parse_eventmodeling_model_for_render_controlled(
                source,
                &full_metadata,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());

            #[cfg(feature = "compat-json")]
            {
                parse_eventmodeling(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "eventmodeling\ntf 01 invalid Started\n";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_eventmodeling_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured parse diagnostic");
        };
        let start = source.find("invalid").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "invalid".len())),
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_eventmodeling_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn unclosed_metadata_lookahead_keeps_prefix_and_later_recovery() {
        let source = concat!(
            "eventmodeling accDescr{Closed}\n",
            "tf 01 evt A accDescr{\n",
            "tf 02 evt B accDescr{\n",
            "entity Later\n",
            "tf 03 evt Final\n",
        );
        let control = crate::OperationControl::new();
        let Err(failure) = construct_eventmodeling_semantic_source_controlled(
            source,
            "eventmodeling",
            EditorFactsDemand::Capture,
            &control,
        )
        .unwrap() else {
            panic!("unclosed metadata must reject");
        };
        let (_, facts) = failure.into_error_and_editor_facts();
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        for payload in [" accDescr{Closed}", "Later", "Final"] {
            let start = source.find(payload).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.selection == SourceSpan::new(start, start + payload.len())
            }));
        }
    }

    #[test]
    fn semantic_metadata_scanning_can_cancel_inside_long_tokens() {
        for (prefix, payload, suffix) in [
            ("eventmodeling accTitle:", "🤓", "\n"),
            ("eventmodeling accDescr{", "🤓", "}\n"),
            ("eventmodeling", " ", "accTitle: Value\n"),
        ] {
            let source = format!("{prefix}{}{suffix}", payload.repeat(65_536));
            let control = crate::OperationControl::new();
            control.cancel_after_checkpoints(10);
            assert!(matches!(
                parse_eventmodeling_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &control,
                ),
                Err(crate::OperationCancelled { .. })
            ));
        }
    }
}
