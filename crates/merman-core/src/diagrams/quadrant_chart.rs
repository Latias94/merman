#[cfg(test)]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, family::CombinedSemanticFailure,
};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
use merman_core_structured::quadrant_chart::QuadrantSourceIssue;
#[cfg(not(feature = "editor-facts"))]
use merman_core_structured::quadrant_chart::parse_quadrant_source_controlled;
#[cfg(feature = "editor-facts")]
use merman_core_structured::quadrant_chart::{
    QuadrantObservation, QuadrantTextKind,
    parse_quadrant_source_with_observer_controlled as parse_quadrant_source_controlled,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static QUADRANT_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_quadrant_syntax_construction_count() {
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
fn quadrant_syntax_construction_count() -> usize {
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub(crate) use merman_model::families::quadrant_chart::QuadrantChartDiagramModel;
#[cfg(all(test, feature = "compat-json"))]
use merman_model::families::quadrant_chart::QuadrantChartStyles;

pub(crate) fn sanitize_common_db_fields(
    model: &mut QuadrantChartDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct QuadrantSemanticSource {
    model: QuadrantChartDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "editor-facts")]
fn project_quadrant_observation(
    facts: &mut EditorSemanticFacts,
    observation: QuadrantObservation<'_>,
) {
    match observation {
        QuadrantObservation::Directive(prefix) => facts.push_directive_prefix(prefix),
        QuadrantObservation::Text { kind, text, span } => {
            use QuadrantTextKind::{AccDescr, AccTitle, Quadrant, Title, XAxis, YAxis};

            let detail = match kind {
                Title => "quadrant chart title",
                AccTitle => "quadrant chart accessibility title",
                AccDescr => "quadrant chart accessibility description",
                XAxis => "quadrant chart x-axis",
                YAxis => "quadrant chart y-axis",
                Quadrant => "quadrant chart quadrant",
            };
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                span,
            ));
            let symbol = if matches!(kind, Title | AccTitle | AccDescr) {
                EditorSemanticSymbol::payload
            } else {
                EditorSemanticSymbol::outline
            };
            facts.push_symbol(symbol(
                text.to_string(),
                Some(detail.to_string()),
                EditorSemanticKind::String,
                span,
                span,
            ));
        }
        QuadrantObservation::ClassDefinition {
            name,
            statement,
            selection,
        } => {
            facts.push_directive_prefix("classDef");
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::NodeIdentifier,
                selection,
            ));
            facts.push_symbol(EditorSemanticSymbol::class_definition(
                name.to_string(),
                Some("quadrant chart class".to_string()),
                EditorSemanticKind::Class,
                statement,
                selection,
            ));
        }
        QuadrantObservation::PointLabel {
            text,
            statement,
            selection,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                selection,
            ));
            facts.push_symbol(EditorSemanticSymbol::outline(
                text.to_string(),
                Some("quadrant chart point".to_string()),
                EditorSemanticKind::Object,
                statement,
                selection,
            ));
        }
        QuadrantObservation::ClassSelector {
            name,
            statement,
            selection,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::ClassName,
                selection,
            ));
            if let Some(name) = name {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    name.to_string(),
                    Some("quadrant chart class".to_string()),
                    EditorSemanticKind::Class,
                    statement,
                    selection,
                ));
            }
        }
    }
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_quadrant_chart_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<QuadrantSemanticSource, CombinedSemanticFailure> {
    construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_quadrant_chart_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<QuadrantSemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.set(QUADRANT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    // Check the parser boundary before creating the editor journal.
    control.checkpoint()?;
    #[cfg(all(test, feature = "editor-facts"))]
    if editor_demand.captures_rich() {
        crate::editor::test_work::record_collection();
    }
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let mut observe = editor_facts
        .as_mut()
        .filter(|_| editor_demand.captures_rich())
        .map(|facts| {
            move |observation: QuadrantObservation<'_>| {
                project_quadrant_observation(facts, observation)
            }
        });

    let outcome = parse_quadrant_source_controlled(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        observe
            .as_mut()
            .map(|observe| observe as &mut dyn for<'a> FnMut(QuadrantObservation<'a>)),
        control,
    )?;
    Ok(match outcome {
        Ok(model) => Ok(QuadrantSemanticSource {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => {
            let error = match issue {
                QuadrantSourceIssue::Exact { span, message } => {
                    Error::diagram_parse_exact(diagram_type, message, span)
                }
                QuadrantSourceIssue::InsertionPoint { offset, message } => {
                    Error::diagram_parse_insertion_point(diagram_type, message, offset)
                }
            };
            Err(CombinedSemanticFailure::parser_recovery(
                #[cfg(feature = "editor-facts")]
                "quadrant chart",
                error,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            ))
        }
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_quadrant_chart(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = construct_quadrant_chart_semantic_source(code, meta)
        .map_err(CombinedSemanticFailure::into_error)?;
    render_model_to_compat_json(&source.model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_quadrant_chart_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |QuadrantSemanticSource {
             model,
             editor_facts,
         }| {
            (
                render_model_to_compat_json(&model, meta),
                editor_facts.unwrap_or_default(),
            )
        },
        CombinedSemanticFailure::into_parts,
    ))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &QuadrantChartDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::quadrant_chart::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_quadrant_chart_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<QuadrantChartDiagramModel> {
    construct_quadrant_chart_semantic_source(code, meta)
        .map(|source| source.model)
        .map_err(CombinedSemanticFailure::into_error)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_quadrant_chart_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<QuadrantChartDiagramModel>> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.model)),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

pub(crate) fn parse_quadrant_chart_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<QuadrantChartDiagramModel>> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
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
                        let (error, facts) = failure.into_parts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(failure.into_error()),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(failure.into_error()),
                }
            }
        }
    })
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::generated;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind, EditorSemanticRole};
    use crate::{MermaidEngine, ParseOptions, RenderSemanticModel};
    use futures::executor::block_on;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    fn axes(model: &Value) -> &Value {
        &model["axes"]
    }

    fn quadrants(model: &Value) -> &Value {
        &model["quadrants"]
    }

    fn points(model: &Value) -> Vec<Value> {
        model["points"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn errors_without_header() {
        let meta = SourcePresentationMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: MermaidConfig::default(),
            effective_config: generated::default_presentation_config(),
            title: None,
        };
        let err = parse_quadrant_chart("quadrant-1 do\n", &meta)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected quadrantChart"));
    }

    #[test]
    fn header_only_is_allowed() {
        let model = parse("quadrantChart\n");
        assert_eq!(model["type"].as_str().unwrap(), "quadrantChart");
        assert!(model["title"].is_null());
    }

    #[test]
    fn parses_x_axis_text_and_missing_right_side() {
        let model = parse("quadrantChart\nx-axis urgent --> not urgent\n");
        assert_eq!(axes(&model)["xAxisLeftText"].as_str().unwrap(), "urgent");
        assert_eq!(
            axes(&model)["xAxisRightText"].as_str().unwrap(),
            "not urgent"
        );

        let model = parse("quadrantChart\nx-AxIs \"Urgent(* +=[?\"  --> \n");
        assert_eq!(
            axes(&model)["xAxisLeftText"].as_str().unwrap(),
            "Urgent(* +=[? ⟶"
        );
        assert_eq!(axes(&model)["xAxisRightText"].as_str().unwrap(), "");
    }

    #[test]
    fn parses_y_axis_text_and_missing_top_side() {
        let model = parse("quadrantChart\ny-axis urgent --> not urgent\n");
        assert_eq!(axes(&model)["yAxisBottomText"].as_str().unwrap(), "urgent");
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "not urgent");

        let model = parse("quadrantChart\ny-AxIs \"Urgent(* +=[?\"  --> \n");
        assert_eq!(
            axes(&model)["yAxisBottomText"].as_str().unwrap(),
            "Urgent(* +=[? ⟶"
        );
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "");
    }

    #[test]
    fn parses_quadrant_text_and_title() {
        let model = parse("quadrantChart\nquadrant-1 Plan\nquadrant-2 \"Do(* +=[?\"\n");
        assert_eq!(quadrants(&model)["quadrant1Text"].as_str().unwrap(), "Plan");
        assert_eq!(
            quadrants(&model)["quadrant2Text"].as_str().unwrap(),
            "Do(* +=[?"
        );

        let model = parse("quadrantChart\ntitle \"this is title (* +=[?\"\n");
        assert_eq!(
            model["title"].as_str().unwrap(),
            "\"this is title (* +=[?\""
        );
    }

    #[test]
    fn parses_points_and_validates_coordinate_range() {
        let model = parse("quadrantChart\npoint1: [0.1, 0.4]\n");
        let pts = points(&model);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0]["text"].as_str().unwrap(), "point1");
        assert_eq!(pts[0]["x"].as_f64().unwrap(), 0.1);
        assert_eq!(pts[0]["y"].as_f64().unwrap(), 0.4);

        let model = parse("quadrantChart\n\"Point1 : (* +=[?\": [1, 0]\n");
        let pts = points(&model);
        assert_eq!(pts[0]["text"].as_str().unwrap(), "Point1 : (* +=[?");
        assert_eq!(pts[0]["x"].as_f64().unwrap(), 1.0);
        assert_eq!(pts[0]["y"].as_f64().unwrap(), 0.0);

        let err = parse_err("quadrantChart\nPoint1 : [1.2, 0.4]\n");
        assert!(err.contains("invalid point coordinate"));

        let err = parse_err("quadrantChart\nPoint1 : [0.2, 0.4, 0.6]\n");
        assert!(err.contains("invalid point coordinate"));
    }

    #[test]
    fn parses_point_styles_and_classes() {
        let model = parse(
            "quadrantChart\nclassDef class1 color: #109060, radius : 10, stroke-color: #310085, stroke-width: 10px\nPoint A:::class1: [0.9, 0.0]\n",
        );
        let classes = model["classes"].as_object().unwrap();
        let class1 = classes.get("class1").unwrap();
        assert_eq!(class1["color"].as_str().unwrap(), "#109060");
        assert_eq!(class1["radius"].as_i64().unwrap(), 10);
        assert_eq!(class1["strokeColor"].as_str().unwrap(), "#310085");
        assert_eq!(class1["strokeWidth"].as_str().unwrap(), "10px");

        let pts = points(&model);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0]["className"].as_str().unwrap(), "class1");

        let model = parse(
            "quadrantChart\nIncorta: [0.20, 0.30] radius: 10 ,color: #ff0000 ,stroke-color: #ff00ff ,stroke-width: 10px\n",
        );
        let pts = points(&model);
        let styles = &pts[0]["styles"];
        assert_eq!(styles["radius"].as_i64().unwrap(), 10);
        assert_eq!(styles["color"].as_str().unwrap(), "#ff0000");
        assert_eq!(styles["strokeColor"].as_str().unwrap(), "#ff00ff");
        assert_eq!(styles["strokeWidth"].as_str().unwrap(), "10px");
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn quadrant_class_definitions_and_uses_have_typed_roles() {
        let text = concat!(
            "quadrantChart\n",
            "classDef priority color: #109060\n",
            "Project A:::priority: [0.2, 0.8]\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("quadrantChart", text)
            .unwrap()
            .expect("quadrant editor facts");

        let class_definition = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "priority" && symbol.role == EditorSemanticRole::ClassDefinition
            })
            .expect("quadrant class definition");
        assert_eq!(class_definition.kind, EditorSemanticKind::Class);
        assert!(class_definition.role.contributes_completion());
        assert!(class_definition.role.contributes_outline());
        assert!(!class_definition.role.contributes_references());

        let class_use = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "priority" && symbol.role == EditorSemanticRole::Payload)
            .expect("quadrant class use");
        assert_eq!(class_use.kind, EditorSemanticKind::Class);
        assert!(!class_use.role.contributes_completion());
        assert!(!class_use.role.contributes_outline());
        assert!(!class_use.role.contributes_references());

        let class_names = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role == EditorSemanticRole::ClassDefinition)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(class_names, vec!["priority"]);

        let node_ids = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role == EditorSemanticRole::Entity)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            !node_ids.contains(&"priority"),
            "class names must not enter node-id completion"
        );

        let outline_names = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role.contributes_outline())
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(outline_names.contains(&"priority"));
        assert!(outline_names.contains(&"Project A"));
        assert_eq!(
            outline_names
                .iter()
                .filter(|name| **name == "priority")
                .count(),
            1,
            "class use must not create a second outline entry"
        );
    }

    #[test]
    fn parses_whole_chart_example() {
        let model = parse(
            "quadrantChart\n\
title Analytics and Business Intelligence Platforms\n\
x-axis \"Completeness of Vision ?\" --> \"x-axis-2\"\n\
y-axis Ability to Execute --> \"y-axis-2\"\n\
quadrant-1 Leaders\n\
quadrant-2 Challengers\n\
quadrant-3 Niche\n\
quadrant-4 Visionaries\n\
Microsoft: [0.75, 0.75]\n\
Salesforce: [0.55, 0.60]\n\
IBM: [0.51, 0.40]\n\
Incorta: [0.20, 0.30]\n",
        );
        assert_eq!(
            axes(&model)["xAxisLeftText"].as_str().unwrap(),
            "Completeness of Vision ?"
        );
        assert_eq!(axes(&model)["xAxisRightText"].as_str().unwrap(), "x-axis-2");
        assert_eq!(
            axes(&model)["yAxisBottomText"].as_str().unwrap(),
            "Ability to Execute"
        );
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "y-axis-2");
        assert_eq!(
            quadrants(&model)["quadrant1Text"].as_str().unwrap(),
            "Leaders"
        );
        assert_eq!(
            quadrants(&model)["quadrant4Text"].as_str().unwrap(),
            "Visionaries"
        );
        assert_eq!(points(&model).len(), 4);
    }

    #[test]
    fn parse_quadrant_chart_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = MermaidEngine::new();
        let input = r##"
quadrantChart
title Typed Quadrant
accTitle: Quadrant accTitle
accDescr: Quadrant accDescription
x-axis Low --> High
y-axis Bottom --> Top
quadrant-1 Expand
quadrant-2 Maintain
quadrant-3 Evaluate
quadrant-4 Retire
classDef priority color: #109060, radius : 10, stroke-color: #310085, stroke-width: 10px
Project A:::priority : [0.2, 0.8]
"##;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "quadrantChart");
        match parsed.model() {
            RenderSemanticModel::QuadrantChart(model) => {
                assert_eq!(model.title.as_deref(), Some("Typed Quadrant"));
                assert_eq!(model.acc_title.as_deref(), Some("Quadrant accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Quadrant accDescription"));
                assert_eq!(model.axes.x_axis_left_text, "Low");
                assert_eq!(model.axes.x_axis_right_text, "High");
                assert_eq!(model.quadrants.quadrant1_text, "Expand");
                assert_eq!(model.points.len(), 1);
                assert_eq!(model.points[0].text, "Project A");
                assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
                assert_eq!(model.classes["priority"].radius, Some(10.0));
            }
            other => panic!("quadrantChart render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("quadrantChart"));
        assert_eq!(parsed_json.model["title"], json!("Typed Quadrant"));
        assert_eq!(parsed_json.model["accTitle"], json!("Quadrant accTitle"));
        assert_eq!(parsed_json.model["axes"]["xAxisLeftText"], json!("Low"));
        assert_eq!(
            parsed_json.model["quadrants"]["quadrant1Text"],
            json!("Expand")
        );
        assert_eq!(parsed_json.model["points"][0]["text"], json!("Project A"));
        assert_eq!(
            parsed_json.model["classes"]["priority"]["radius"],
            json!(10)
        );
        assert!(parsed_json.model.get("config").is_some());
    }

    #[test]
    fn entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "title Delivery portfolio; quadrant-1 Hidden\n",
            "accTitle: Portfolio\n",
            "x-axis Low --> High\n",
            "y-axis Bottom --> Top\n",
            "quadrant-1 \"Invest\nCarefully\"\n",
            "classDef priority color: #109060, radius: 10\n",
            "\"`Project\nA`\":::priority: [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Quadrant JSON parse succeeds")
            .expect("standalone Quadrant JSON parse returns a diagram");
        reset_quadrant_syntax_construction_count();
        parse_quadrant_chart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Quadrant JSON projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        let typed = parse_quadrant_chart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Quadrant typed projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_quadrant_chart_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &OperationControl::new(),
            ))
            .expect("Quadrant combined projection succeeds");
        #[cfg(not(feature = "editor-facts"))]
        let combined_json = parse_quadrant_chart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .unwrap();
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        #[cfg(feature = "editor-facts")]
        assert!(!combined_editor.symbols.is_empty());
        assert_eq!(
            merman_model::families::quadrant_chart::compat::project(
                &typed,
                &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta)
            )
            .unwrap(),
            combined_json
        );
        assert_eq!(combined_json["type"], json!("quadrantChart"));
        assert!(combined_json["config"].is_object());
        assert_eq!(combined_json["accDescr"], Value::Null);

        let compat_classes: std::collections::BTreeMap<String, QuadrantChartStyles> =
            serde_json::from_value(combined_json["classes"].clone())
                .expect("Quadrant compatibility classes deserialize into typed styles");
        assert_eq!(typed.classes.len(), compat_classes.len());
        for (name, styles) in &typed.classes {
            let compat = compat_classes
                .get(name)
                .expect("every typed Quadrant class remains in compatibility JSON");
            assert_eq!(
                (
                    styles.radius,
                    styles.color.as_deref(),
                    styles.stroke_color.as_deref(),
                    styles.stroke_width.as_deref(),
                ),
                (
                    compat.radius,
                    compat.color.as_deref(),
                    compat.stroke_color.as_deref(),
                    compat.stroke_width.as_deref(),
                ),
                "Quadrant class {name} drift",
            );
        }
        let typed = serde_json::to_value(typed).expect("Quadrant typed model serializes");
        for field in [
            "title",
            "accTitle",
            "accDescr",
            "quadrants",
            "axes",
            "points",
        ] {
            assert_eq!(typed[field], combined_json[field], "Quadrant {field} drift");
        }
    }

    #[test]
    fn quoted_semicolons_and_inline_acc_descr_block_share_statement_scanner() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "title \"Plan; Execute\"\n",
            "accDescr {first %% literal; second}; quadrant-1 \"Build; Learn\"\n",
            "\"Point; A\": [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("quoted semicolons parse")
            .expect("quadrant model");
        #[cfg(feature = "editor-facts")]
        let facts = crate::family::test_support::editor_facts_from_frozen(
            parse_quadrant_chart_json_and_editor_facts,
            text,
            &parsed.meta,
        );

        assert_eq!(parsed.model["title"], json!("\"Plan; Execute\""));
        assert_eq!(parsed.model["accDescr"], json!("first %% literal; second"));
        assert_eq!(
            parsed.model["quadrants"]["quadrant1Text"],
            json!("Build; Learn")
        );
        assert_eq!(parsed.model["points"][0]["text"], json!("Point; A"));
        #[cfg(feature = "editor-facts")]
        for expected in [
            "\"Plan; Execute\"",
            "first %% literal; second",
            "Build; Learn",
            "Point; A",
        ] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == expected),
                "missing source-backed fact for {expected}"
            );
        }
    }

    #[test]
    fn multiline_acc_descr_preserves_original_source_span() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "accDescr {\n",
            "  First line\n",
            "  Second line\n",
            "}\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("multiline accDescr parses")
            .expect("quadrant model");
        assert_eq!(parsed.model["accDescr"], json!("First line\nSecond line"));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts_from_frozen(
                parse_quadrant_chart_json_and_editor_facts,
                text,
                &parsed.meta,
            );
            let description = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("quadrant chart accessibility description")
                })
                .expect("multiline accessibility description fact");
            let content_start = text.find('{').expect("opening brace") + 1;
            let content_end = text.rfind('}').expect("closing brace");

            assert_eq!(
                description.selection,
                SourceSpan::new(content_start, content_end)
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_point_recovers_prior_parser_facts() {
        let engine = MermaidEngine::new();
        let text = "quadrantChart\nx-axis Low --> High\nBroken: [1.2, 0.4]\n";
        let statement_start = text.find("Broken").expect("malformed statement");
        let statement_end = statement_start + "Broken: [1.2, 0.4]".len();
        reset_quadrant_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("quadrantChart", text)
            .expect("quadrant editor recovery succeeds")
            .expect("quadrant editor facts are available");

        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "Low" && symbol.role == EditorSemanticRole::Outline
            })
        );
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(statement_start, statement_end))
        }));
    }

    #[test]
    fn eof_rejects_unterminated_multiline_acc_descr_like_pinned_jison() {
        let text = "quadrantChart\naccDescr {\npartial description\n";

        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let snapshot = MermaidEngine::new()
            .parse_diagram_snapshot_with_type_sync("quadrantChart", text)
            .expect("Quadrant snapshot operation")
            .expect("Quadrant snapshot");
        #[cfg(not(feature = "editor-facts"))]
        assert!(
            MermaidEngine::new()
                .parse_diagram_sync(text, ParseOptions::strict())
                .is_err()
        );
        assert_eq!(quadrant_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            assert!(matches!(
                snapshot.outcome(),
                crate::DiagramParseOutcome::Failed(_)
            ));
            let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Quadrant recovery facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(!facts.diagnostics.is_empty());
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "partial description" && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict",
                "htmlLabels": true,
                "dompurifyConfig": { "FORBID_TAGS": ["b"] }
            })),
            title: None,
        }
    }

    #[test]
    fn raw_line_terminals_keep_semicolons_and_literal_comment_markers() {
        let source = concat!(
            "quadrantChart\n",
            "title Plan; quadrant-1 Hidden %% literal \"\n",
            "accTitle: Accessible; title Literal\n",
            "accDescr: Details; quadrant-2 Hidden\n",
            "quadrant-1 Visible\n",
        );
        reset_quadrant_syntax_construction_count();
        let model = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            model.title.as_deref(),
            Some("Plan; quadrant-1 Hidden %% literal \"")
        );
        assert_eq!(
            model.acc_title.as_deref(),
            Some("Accessible; title Literal")
        );
        assert_eq!(
            model.acc_descr.as_deref(),
            Some("Details; quadrant-2 Hidden")
        );
        assert_eq!(model.quadrants.quadrant1_text, "Visible");
        assert!(model.quadrants.quadrant2_text.is_empty());
        assert_eq!(quadrant_syntax_construction_count(), 1);
    }

    #[test]
    fn multiline_text_tokens_share_axes_points_and_legal_text_suffixes() {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            let source = format!(
                "quadrantChart\nx-axis {open}Low\nEffort{close} --> High\n\
                 quadrant-1 {open}First\nSecond{close} ???\n\
                 quadrant-2 {open}Literal\\nValue{close}\n\
                 {open}Point\nName{close}:::priority: [0.2, 0.8]\n\
                 classDef priority color:#abc\n"
            );
            let model = parse_quadrant_chart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();

            assert_eq!(model.axes.x_axis_left_text, "Low\nEffort");
            assert_eq!(model.axes.x_axis_right_text, "High");
            assert_eq!(model.quadrants.quadrant1_text, "First\nSecond ???");
            assert_eq!(model.quadrants.quadrant2_text, r"Literal\nValue");
            assert_eq!(model.points[0].text, "Point\nName");
            assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
        }
    }

    #[test]
    fn mixed_text_rejects_reserved_tokens_without_banning_alpha_suffixes() {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            for (suffix, expected) in [
                (" title B", None),
                (" tail", Some("A tail")),
                (" untitled", Some("A untitled")),
            ] {
                let source = format!("quadrantChart\nquadrant-1 {open}A{close}{suffix}\n");
                let construction = parse_quadrant_chart_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &OperationControl::new(),
                )
                .unwrap();
                match expected {
                    Some(expected) => {
                        assert_eq!(
                            construction.model.unwrap().quadrants.quadrant1_text,
                            expected
                        );
                    }
                    None => assert!(construction.model.is_err(), "{source}"),
                }
            }
        }
    }

    #[test]
    fn malformed_text_rejects_extra_tokens_and_unclosed_strings() {
        for text in [
            "\"First\nSecond\" [junk]",
            "\"First\nSecond\" \"another token\"",
            "\"`First\nSecond`\" [junk]",
            "\"First\nSecond",
            "\"`First\nSecond",
            "\"First\\\"Second\"",
        ] {
            let source = format!("quadrantChart\nquadrant-1 {text}\n");
            let construction = parse_quadrant_chart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap();
            assert!(
                construction.model.is_err(),
                "accepted malformed text: {text}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_multiline_text_recovers_following_facts_once() {
        let source = concat!(
            "quadrantChart\n",
            "quadrant-1 \"Broken\nCaption\" [junk]\n",
            "quadrant-2 \"Valid\nCaption\"\n",
            "\"`Next\nPoint`\": [0.2, 0.8]\n",
        );
        reset_quadrant_syntax_construction_count();
        let construction = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(construction.model.is_err());
        let facts = construction.editor_facts.unwrap();
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        for name in ["Valid\nCaption", "Next\nPoint"] {
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .unwrap();
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
        }
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| symbol.name.contains("Broken"))
        );
        assert!(!facts.diagnostics.is_empty());
    }

    #[test]
    fn semantic_points_are_authored_domain_coordinates_without_editor_work() {
        let source = concat!(
            "quadrantChart\n",
            "title Priorities\n",
            "x-axis Low --> High\n",
            "quadrant-1 Plan\n",
            "classDef priority radius:10,color:#ff0000\n",
            "<b>Idea</b>:::priority: [0.25, 0.75]\n",
        );
        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("Priorities"));
        assert_eq!(model.axes.x_axis_left_text, "Low");
        assert_eq!(model.axes.x_axis_right_text, "High");
        assert_eq!(model.quadrants.quadrant1_text, "Plan");
        assert_eq!(model.points.len(), 1);
        assert_eq!(model.points[0].text, "Idea");
        assert_eq!((model.points[0].x, model.points[0].y), (0.25, 0.75));
        assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
        assert_eq!(model.classes["priority"].radius, Some(10.0));
        assert_eq!(quadrant_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_coordinate_errors_preserve_exact_spans_without_editor_work() {
        let source = "quadrantChart\r\n  Broken: [1.2, 0.4]\r\n";
        let start = source.find("Broken").unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a quadrant parse error");
        };
        assert!(diagnostic.message().contains("invalid point coordinate"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "Broken: [1.2, 0.4]".len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_entry_checks_cancellation_before_construction() {
        let control = OperationControl::new();
        control.cancel();
        reset_quadrant_syntax_construction_count();
        assert!(matches!(
            parse_quadrant_chart_semantic_model_controlled(
                "quadrantChart\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
        assert_eq!(quadrant_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_typed_and_json_requests_omit_editor_observers() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "quadrantChart\nx-axis Low --> High\nA: [0.2, 0.8]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_quadrant_chart(source, &meta).unwrap();
        parse_quadrant_chart_model_for_render_controlled(
            source,
            &meta,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
