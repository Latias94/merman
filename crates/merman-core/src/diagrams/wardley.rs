#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
use crate::{Error, OperationControl, OperationControlResult, family::CombinedSemanticFailure};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_structured::wardley::{
    WardleyFieldKind, WardleyObservation, parse_wardley_source_with_observer_controlled,
};
use merman_core_structured::wardley::{WardleyParseProblem, parse_wardley_source_controlled};
use merman_model::families::wardley::WardleyDiagramModel;
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(feature = "compat-json")]
pub(crate) fn parse_wardley(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    construct_wardley_semantic_source(code, meta)
        .map_err(CombinedSemanticFailure::into_error)?
        .into_compat_json(meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_wardley_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let construction = match construct_wardley_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )? {
        Ok(source) => Ok(source.into_combined_parts_controlled(meta, control)?),
        Err(error) => Err(error),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |parts| parts,
        CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_wardley_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<WardleyDiagramModel> {
    Ok(construct_wardley_semantic_source(code, meta)
        .map_err(CombinedSemanticFailure::into_error)?
        .into_render_model(meta))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_wardley_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<WardleyDiagramModel>> {
    let construction = construct_wardley_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    Ok(Ok(source.into_render_model_controlled(meta, control)?))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &WardleyDiagramModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value>> {
    Ok(merman_model::families::wardley::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}

pub(crate) fn parse_wardley_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<WardleyDiagramModel>> {
    let construction = construct_wardley_semantic_source_controlled(
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
            let editor_facts = editor_demand
                .needs_recovery()
                .then(|| std::mem::take(&mut source.editor_facts));
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

#[derive(Debug)]
struct WardleySemanticSource {
    model: WardleyDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

impl WardleySemanticSource {
    #[cfg(all(test, feature = "presentation"))]
    fn into_render_model(self, meta: &SourcePresentationMetadata) -> WardleyDiagramModel {
        self.into_render_model_controlled(meta, &OperationControl::new())
            .expect("a private parse control cannot be cancelled")
    }

    fn into_model_controlled(
        mut self,
        config: &crate::MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<WardleyDiagramModel> {
        control.checkpoint()?;
        crate::common_db::sanitize_optional_title(&mut self.model.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.model.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.model.acc_descr, config);
        control.checkpoint()?;
        Ok(self.model)
    }

    #[cfg(feature = "presentation")]
    fn into_render_model_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<WardleyDiagramModel> {
        self.into_model_controlled(meta.effective_config.as_config(), control)
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let control = OperationControl::new();
        self.into_compat_json_controlled(meta, &control)
            .expect("a private parse control cannot be cancelled")
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Value>> {
        let model = self.into_render_model_controlled(meta, control)?;
        render_model_to_compat_json_controlled(&model, meta, control)
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn into_combined_parts_controlled(
        mut self,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<(Result<Value>, EditorSemanticFacts)> {
        let editor_facts = std::mem::take(&mut self.editor_facts);
        let model = self.into_render_model_controlled(meta, control)?;
        let json = render_model_to_compat_json_controlled(&model, meta, control)?;
        control.checkpoint()?;
        Ok((json, editor_facts))
    }
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_wardley_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<WardleySemanticSource, CombinedSemanticFailure> {
    construct_wardley_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_wardley_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<WardleySemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("wardley");

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = EditorSemanticFacts::new();
    #[cfg(feature = "editor-facts")]
    let model = if editor_demand.captures_rich() {
        parse_wardley_source_with_observer_controlled(
            code,
            &mut |observation| project_wardley_observation(&mut editor_facts, observation),
            control,
        )
    } else {
        parse_wardley_source_controlled(code, control)
    }?;
    #[cfg(not(feature = "editor-facts"))]
    let model = parse_wardley_source_controlled(code, control)?;
    let model = match model {
        Ok(model) => model,
        Err(problem) => {
            return Ok(Err(wardley_failure(
                diagram_type,
                problem,
                #[cfg(feature = "editor-facts")]
                editor_facts,
                #[cfg(feature = "editor-facts")]
                editor_demand,
            )));
        }
    };

    Ok(Ok(WardleySemanticSource {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

fn wardley_failure(
    diagram_type: &str,
    problem: WardleyParseProblem,
    #[cfg(feature = "editor-facts")] editor_facts: EditorSemanticFacts,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> CombinedSemanticFailure {
    let error = Error::diagram_parse_exact(diagram_type.to_string(), problem.message, problem.span);
    CombinedSemanticFailure::parser_recovery(
        #[cfg(feature = "editor-facts")]
        "wardley",
        error,
        #[cfg(feature = "editor-facts")]
        editor_demand.needs_recovery().then_some(editor_facts),
    )
}

#[cfg(feature = "editor-facts")]
fn project_wardley_observation(
    facts: &mut EditorSemanticFacts,
    observation: WardleyObservation<'_>,
) {
    let (kind, text, detail, span, selection) = match observation {
        WardleyObservation::Started => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            return;
        }
        WardleyObservation::Directive(prefix) => {
            facts.push_directive_prefix(prefix);
            return;
        }
        WardleyObservation::Common(fact) => {
            push_langium_common_editor_fact(facts, fact, "wardley");
            return;
        }
        WardleyObservation::Field {
            kind,
            text,
            detail,
            span,
            selection,
        } => (kind, text, detail, span, selection),
    };
    let (kind, role, expected, quoted) = match kind {
        WardleyFieldKind::Definition { quoted } => (
            EditorSemanticKind::Object,
            EditorSemanticRole::Entity,
            EditorExpectedSyntaxKind::NodeIdentifier,
            quoted,
        ),
        WardleyFieldKind::Reference { quoted } => (
            EditorSemanticKind::Object,
            EditorSemanticRole::Reference,
            EditorExpectedSyntaxKind::NodeIdentifier,
            quoted,
        ),
        WardleyFieldKind::Outline => (
            EditorSemanticKind::Object,
            EditorSemanticRole::Outline,
            EditorExpectedSyntaxKind::Payload,
            false,
        ),
        WardleyFieldKind::Text => (
            EditorSemanticKind::String,
            EditorSemanticRole::Payload,
            EditorExpectedSyntaxKind::Payload,
            false,
        ),
        WardleyFieldKind::Number => (
            EditorSemanticKind::Property,
            EditorSemanticRole::Payload,
            EditorExpectedSyntaxKind::Payload,
            false,
        ),
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(expected, selection));
    let mut symbol = EditorSemanticSymbol::with_role(
        text,
        Some(detail.to_string()),
        kind,
        role,
        span,
        selection,
    );
    if matches!(
        role,
        EditorSemanticRole::Entity | EditorSemanticRole::Reference
    ) {
        let policy = if !quoted
            && text
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
        {
            EditorRenamePolicy::Identifier
        } else {
            EditorRenamePolicy::None
        };
        symbol = symbol.with_rename_policy(policy);
    }
    facts.push_symbol(symbol);
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind};
    use crate::{MermaidConfig, MermaidEngine, ParseOptions, RenderSemanticModel, SourceSpan};
    use merman_model::families::wardley::{
        WardleyFlowDirection, WardleyNode, WardleyPoint, WardleySize, WardleySourceStrategy,
    };

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "wardley".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    fn parse(source: &str) -> WardleyDiagramModel {
        parse_wardley_model_for_render(source, &meta()).unwrap()
    }

    #[test]
    fn wardley_json_projection_observes_cancellation_inside_large_model_collections() {
        let model = WardleyDiagramModel {
            nodes: (0..512)
                .map(|index| WardleyNode {
                    id: format!("node-{index}"),
                    label: format!("Node {index}"),
                    x: 50.0,
                    y: 50.0,
                    class_name: None,
                    label_offset_x: None,
                    label_offset_y: None,
                    in_pipeline: false,
                    is_pipeline_parent: false,
                    inertia: None,
                    source_strategy: None,
                })
                .collect(),
            ..WardleyDiagramModel::default()
        };
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(render_model_to_compat_json_controlled(&model, &meta(), &control).is_err());
    }

    #[test]
    fn parses_official_basic_map_with_upstream_coordinate_and_build_order() {
        let model = parse(
            r#"wardley-beta
title Tea Shop Value Chain

anchor Business [0.95, 0.63]
component Cup of Tea [0.79, 0.61]
component Tea [0.63, 0.81]
component Hot Water [0.52, 0.80]
component Kettle [0.43, 0.35]
component Power [0.10, 0.70]

Business -> Cup of Tea
Cup of Tea -> Tea
Cup of Tea -> Hot Water
Hot Water -> Kettle
Kettle -> Power

evolve Kettle 0.62
evolve Power 0.89

note "Standardising power allows Kettles to evolve faster" [0.30, 0.49]
"#,
        );

        assert_eq!(model.title.as_deref(), Some("Tea Shop Value Chain"));
        assert_eq!(
            model
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            [
                "Business",
                "Cup of Tea",
                "Tea",
                "Hot Water",
                "Kettle",
                "Power"
            ]
        );
        assert_eq!(model.nodes[0].class_name.as_deref(), Some("anchor"));
        assert_eq!((model.nodes[0].x, model.nodes[0].y), (63.0, 95.0));
        assert_eq!(model.links.len(), 5);
        assert_eq!(model.trends[0].node_id, "Kettle");
        assert_eq!(model.trends[0].target_x, 62.0);
        assert_eq!(model.notes[0].x, 49.0);
        assert_eq!(model.notes[0].y, 30.0);
    }

    #[test]
    fn parses_feature_rich_wardley_1116_semantics_without_renderer_reconstruction() {
        let model = parse(
            r#"wardley-beta
title Software Platform Strategy
accTitle: Platform map
accDescr: Strategic platform evolution
size [1100, 800]
evolution Genesis@0.25 / Concept -> Custom@0.5 / Emerging -> Product@0.75 / Converging -> Commodity@1.0 / Accepted

anchor Customer [0.90, 0.95]
component "Mobile App" [0.80, 0.85] (build)
component "Web App" [0.75, 0.80] label [-60, 10] (build)
component "API Gateway" [0.70, 0.65] (buy)
component Database [0.50, 0.45] (buy) (inertia)
component Cache [0.55, 0.50]

pipeline Database {
  component "File System" [0.25] label [-40, 20]
  component SQL DB [0.50]
}

Customer -> "Mobile App"; entry
"Mobile App" +> "API Gateway"
"API Gateway" +<> Cache
Cache +'backup'> Database
"API Gateway" -.-> Database
Database -> "File System"
evolve "API Gateway" 0.85
note "Build mobile-first experience" [0.85, 0.90]
annotations [0.10, 0.20]
annotation 1,[0.78, 0.82] "User touchpoints"
accelerator "Cloud Native" [0.20, 0.85]
deaccelerator "Legacy Data" [0.45, 0.35]
"#,
        );

        assert_eq!(model.acc_title.as_deref(), Some("Platform map"));
        assert_eq!(
            model.size,
            Some(WardleySize {
                width: 1100.0,
                height: 800.0
            })
        );
        assert_eq!(
            model.axes.stages,
            [
                "Genesis / Concept",
                "Custom / Emerging",
                "Product / Converging",
                "Commodity / Accepted",
            ]
        );
        assert_eq!(model.axes.stage_boundaries, [0.25, 0.5, 0.75, 1.0]);

        let database = model
            .nodes
            .iter()
            .find(|node| node.id == "Database")
            .unwrap();
        assert!(database.is_pipeline_parent);
        assert_eq!(database.source_strategy, Some(WardleySourceStrategy::Buy));
        assert_eq!(database.inertia, Some(true));
        let file_system = model
            .nodes
            .iter()
            .find(|node| node.label == "File System")
            .unwrap();
        assert_eq!(file_system.id, "Database_File System");
        assert!(file_system.in_pipeline);
        assert_eq!((file_system.x, file_system.y), (25.0, 50.0));
        assert_eq!(
            (file_system.label_offset_x, file_system.label_offset_y),
            (Some(-40.0), Some(20.0))
        );
        assert_eq!(
            model.pipelines[0].component_ids,
            ["Database_File System", "Database_SQL DB"]
        );

        assert_eq!(model.links[0].label.as_deref(), Some("entry"));
        assert_eq!(model.links[1].flow, Some(WardleyFlowDirection::Forward));
        assert_eq!(
            model.links[2].flow,
            Some(WardleyFlowDirection::Bidirectional)
        );
        assert_eq!(model.links[3].flow, Some(WardleyFlowDirection::Forward));
        assert_eq!(model.links[3].label.as_deref(), Some("backup"));
        assert!(model.links[4].dashed);
        assert_eq!(model.links[5].target, "Database_File System");
        assert_eq!(
            model.annotations_box,
            Some(WardleyPoint { x: 20.0, y: 10.0 })
        );
        assert_eq!(
            model.annotations[0].coordinates[0],
            WardleyPoint { x: 82.0, y: 78.0 }
        );
        assert_eq!(
            (model.accelerators[0].x, model.accelerators[0].y),
            (85.0, 20.0)
        );
        assert_eq!(
            (model.deaccelerators[0].x, model.deaccelerators[0].y),
            (35.0, 45.0)
        );
    }

    #[test]
    fn preserves_javascript_map_insertion_and_update_semantics() {
        let model = parse(
            r#"wardley-beta
component Shared [0.20, 0.20]
anchor Shared [0.90, 0.90]
anchor First [0.80, 0.80]
component Shared [0.40, 0.40] (market)
component Last [0.30, 0.30]
evolve Shared 0.60
evolve Shared 0.70
"#,
        );

        assert_eq!(
            model
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["Shared", "First", "Last"]
        );
        assert_eq!(model.nodes[0].class_name.as_deref(), Some("component"));
        assert_eq!((model.nodes[0].x, model.nodes[0].y), (40.0, 40.0));
        assert_eq!(
            model.nodes[0].source_strategy,
            Some(WardleySourceStrategy::Market)
        );
        assert_eq!(model.trends.len(), 1);
        assert_eq!(model.trends[0].target_x, 70.0);
    }

    #[test]
    fn covers_upstream_hyphen_no_space_and_pipeline_label_regressions() {
        let model = parse(
            r#"wardley-beta
component real-time processing [0.5, 0.5]
component end-user [0.8, 0.9]
component foo--bar [0.3, 0.4]
component foo- [0.2, 0.3]
real-time processing->end-user
foo--bar->foo-
pipeline real-time processing {
  component batch-loader [0.7] label [-40, 20]
}
batch-loader->end-user
"#,
        );

        assert!(model.nodes.iter().any(|node| node.id == "foo--bar"));
        assert!(model.nodes.iter().any(|node| node.id == "foo-"));
        let batch = model
            .nodes
            .iter()
            .find(|node| node.label == "batch-loader")
            .unwrap();
        assert_eq!(batch.id, "real-time processing_batch-loader");
        assert_eq!(
            (batch.label_offset_x, batch.label_offset_y),
            (Some(-40.0), Some(20.0))
        );
        assert_eq!(model.links[0].source, "real-time processing");
        assert_eq!(model.links[0].target, "end-user");
        assert_eq!(model.links[2].source, batch.id);
    }

    #[test]
    fn allows_blank_and_comment_lines_between_pipeline_components() {
        let model = parse(
            r#"wardley-beta
component Platform [0.5, 0.5]
pipeline Platform {
  component First [0.2]

  %% components may be separated by hidden comments
  component Second [0.8]
}
"#,
        );

        assert_eq!(
            model.pipelines[0].component_ids,
            ["Platform_First", "Platform_Second"]
        );
    }

    #[test]
    fn preserves_consecutive_horizontal_whitespace_in_unquoted_names() {
        let model = parse("wardley-beta\ncomponent Data  \t Platform [0.5, 0.5]\n");

        assert_eq!(model.nodes[0].id, "Data  \t Platform");
        assert_eq!(model.nodes[0].label, "Data  \t Platform");
    }

    #[test]
    fn allows_hidden_whitespace_inside_parenthesized_inertia() {
        let model = parse("wardley-beta\ncomponent API [0.5, 0.5] ( \t inertia \t )\n");

        assert_eq!(model.nodes[0].inertia, Some(true));
    }

    #[test]
    fn accepts_decimal_percentage_coordinates_but_rejects_integer_node_coordinates() {
        let model = parse("wardley-beta\ncomponent API [20.0, 75.0]\n");
        assert_eq!((model.nodes[0].x, model.nodes[0].y), (75.0, 20.0));

        let source = "wardley-beta\ncomponent API [1, 1]\n";
        let error = parse_wardley_model_for_render(source, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected wardley parse error");
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                source.find("1, 1").unwrap(),
                source.find("1, 1").unwrap() + 1,
            ))
        );
    }

    #[test]
    fn follows_optional_arrow_grammar_without_splitting_greedy_space_names() {
        let model = parse(
            "wardley-beta\ncomponent A [0.1, 0.1]\ncomponent B [0.2, 0.2]\n\"A\" \"B\"\nA->B+>\n",
        );
        assert_eq!(model.links.len(), 2);
        assert_eq!(
            (
                model.links[0].source.as_str(),
                model.links[0].target.as_str()
            ),
            ("A", "B")
        );
        assert_eq!(model.links[1].flow, Some(WardleyFlowDirection::Forward));

        let error = parse_wardley_model_for_render("wardley-beta\nA B\n", &meta()).unwrap_err();
        assert!(error.to_string().contains("second wardley link endpoint"));
    }

    #[test]
    fn rejects_out_of_range_coordinates_with_exact_source_span() {
        let source = "wardley-beta\ncomponent API [0.5, 101.0]\n";
        let error = parse_wardley_model_for_render(source, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected wardley parse error");
        };
        let start = source.find("101.0").unwrap();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 5)));
        assert!(diagnostic.message().contains("Component \"API\" evolution"));
    }

    #[test]
    fn rejects_pipeline_without_existing_parent_and_recovers_same_parser_facts() {
        let source = "wardley-beta\npipeline Missing {\n  component Child [0.5]\n}\n";
        let error = parse_wardley_model_for_render(source, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected wardley parse error");
        };
        let start = source.find("Missing").unwrap();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 7)));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_wardley_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Missing"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Child"));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(SourceSpan::new(start, start + 7))
            }));
        }
    }

    #[test]
    fn combined_engine_projection_constructs_once_and_exposes_typed_editor_contracts() {
        let source = "wardley-beta\ncomponent API [0.6, 0.7]\ncomponent DB [0.4, 0.5]\nAPI -> DB\n";
        #[cfg(feature = "editor-facts")]
        crate::diagrams::langium_common::reset_family_syntax_construction_count("wardley");
        let engine = MermaidEngine::new();
        #[cfg(feature = "editor-facts")]
        let combined = engine.parse_diagram_snapshot_sync(source).unwrap().unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("wardley"),
            1
        );
        #[cfg(feature = "editor-facts")]
        {
            let crate::ParsedEditorFacts::Available(facts) = combined.editor_facts() else {
                panic!("expected wardley editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert_eq!(
                facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == "API")
                    .count(),
                2
            );
            let api: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "API")
                .collect();
            assert_eq!(api[0].role, crate::EditorSemanticRole::Entity);
            assert_eq!(api[1].role, crate::EditorSemanticRole::Reference);
        }
        let typed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "wardley",
                source,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Wardley(model) = typed.model() else {
            panic!("expected typed wardley model");
        };
        assert_eq!(model.nodes.len(), 2);
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            &merman_model::families::wardley::compat::project(model, &typed.metadata().into(),)
                .unwrap(),
            combined
                .outcome()
                .parsed_model()
                .expect("expected parsed snapshot")
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn wardley_reference_roles_cover_links_evolve_and_pipeline_parents() {
        let source = concat!(
            "wardley-beta\n",
            "API -> DB\n",
            "component API [0.6, 0.7]\n",
            "component DB [0.4, 0.5]\n",
            "API -> DB\n",
            "evolve API 0.8\n",
            "pipeline DB {\n",
            "  component Storage [0.4]\n",
            "}\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_wardley_json_and_editor_facts,
            source,
            &meta(),
        );

        for name in ["API", "DB"] {
            let roles: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect();
            assert_eq!(roles[0], crate::EditorSemanticRole::Reference);
            assert_eq!(
                roles
                    .iter()
                    .filter(|role| **role == crate::EditorSemanticRole::Entity)
                    .count(),
                1
            );
            assert_eq!(
                roles
                    .iter()
                    .filter(|role| **role == crate::EditorSemanticRole::Reference)
                    .count(),
                roles.len() - 1
            );
        }
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Storage" && symbol.role == crate::EditorSemanticRole::Entity
        }));
    }

    #[test]
    fn recovery_keeps_confirmed_prefix_and_pipeline_suffix() {
        let source = concat!(
            "wardley-beta\r\n",
            "component Before [0.10, 0.20]\r\n",
            "component Broken [0.30, 1]\r\n",
            "pipeline Before {\r\n",
            "  component Child [0.40]\r\n",
            "  invalid pipeline syntax\r\n",
            "  component \"后续 子项\" [0.50]\r\n",
            "}\r\n",
            "component \"后来 🤓\" [0.60, 0.70]\r\n",
            "note \"未结束 🤓",
        );
        let invalid_number = source.find(", 1]").unwrap() + 2;
        let error = parse_wardley_model_for_render(source, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected wardley parse error");
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(invalid_number, invalid_number + 1))
        );

        #[cfg(feature = "editor-facts")]
        {
            crate::diagrams::langium_common::reset_family_syntax_construction_count("wardley");
            let facts = crate::family::test_support::editor_facts(
                parse_wardley_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("wardley"),
                1
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            for later in ["后续 子项", "后来 🤓"] {
                assert!(
                    facts.symbols.iter().any(|symbol| symbol.name == later),
                    "missing recovered symbol {later:?}: {:?}",
                    facts.symbols
                );
            }
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(SourceSpan::new(invalid_number, invalid_number + 1))
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use crate::SourceSpan;
    use merman_model::families::wardley::{WardleyPoint, WardleySourceStrategy};

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "wardley".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn semantic_sanitization_preserves_domain_payloads_and_accessibility_normalization() {
        let source = concat!(
            "wardley-beta\n",
            "title <script>secret</script><b>Header &amp; &#x3C;</b>\n",
            "accTitle: <script>secret</script>   Accessible\n",
            "accDescr {First\n    <b>Second &amp; &#x3C;</b>}\n",
            "component \"<script>raw</script>&amp;\" [0.5, 0.25]\n",
            "note \"<b>raw &amp;</b>\" [0.4, 0.3]\n",
        );
        for security_level in ["strict", "loose"] {
            let mut metadata = metadata();
            metadata.semantic_config = crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": security_level,
                "htmlLabels": true
            }));
            let model = parse_wardley_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.title.as_deref(), Some("<b>Header &amp; &lt;</b>"));
            assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("First\n<b>Second &amp; &lt;</b>")
            );
            assert_eq!(model.nodes[0].id, "<script>raw</script>&amp;");
            assert_eq!(model.nodes[0].label, "<script>raw</script>&amp;");
            assert_eq!(model.notes[0].text, "<b>raw &amp;</b>");
        }
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let metadata = metadata();
        let source = concat!(
            "wardley-beta\n",
            "accTitle: <script>secret</script>Public\n",
            "component API [0.5, 0.25] (buy)\n",
            "component DB [0.3, 0.5] (inertia)\n",
            "API -> DB\n",
            "evolve API 0.75\n",
            "annotation 1,[0.6, 0.4] \"Important\"\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("wardley");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_wardley_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("Public"));
        assert_eq!(model.nodes[0].id, "API");
        assert_eq!((model.nodes[0].x, model.nodes[0].y), (25.0, 50.0));
        assert_eq!(
            model.nodes[0].source_strategy,
            Some(WardleySourceStrategy::Buy)
        );
        assert_eq!(model.nodes[1].inertia, Some(true));
        assert_eq!(model.links[0].source, "API");
        assert_eq!(model.links[0].target, "DB");
        assert_eq!(model.trends[0].target_x, 75.0);
        assert_eq!(model.annotations[0].text.as_deref(), Some("Important"));
        assert_eq!(
            model.annotations[0].coordinates[0],
            WardleyPoint { x: 40.0, y: 60.0 }
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("wardley"),
            1,
        );
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
            parse_wardley_model_for_render_controlled(
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
                parse_wardley(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "wardley-beta\ncomponent API [0.5, 101.0]\n";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_wardley_semantic_model_controlled(
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
        let start = source.find("101.0").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "101.0".len())),
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_wardley_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
    }
}
