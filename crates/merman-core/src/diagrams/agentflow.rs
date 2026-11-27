//! Core entry points and editor projection for Mermaid's Agentflow diagram.

#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorFamilySemantics, EditorRenamePolicy,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol,
};
use crate::{Error, OperationControl, OperationControlResult, family};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
pub(crate) use merman_core_flow::agentflow::is_valid_editor_node_id;
#[cfg(feature = "presentation")]
use merman_core_flow::agentflow::parse_agentflow_source_for_render_controlled;
#[cfg(feature = "editor-facts")]
use merman_core_flow::agentflow::{
    AgentflowIdentifierKind, AgentflowObservation, parse_agentflow_source_with_observer_controlled,
};
use merman_core_flow::agentflow::{
    AgentflowSyntaxIssue, ParsedAgentflow, parse_agentflow_source_controlled,
};
#[cfg(test)]
use merman_core_flow::agentflow::{
    agentflow_syntax_construction_count, reset_agentflow_syntax_construction_count,
};
use merman_model::families::agentflow::{AgentflowDiagnosticPosition, AgentflowDiagramModel};
#[cfg(feature = "presentation")]
use merman_model::families::agentflow::{AgentflowDiagramRenderModel, AgentflowVisuals};
#[cfg(test)]
use merman_model::families::agentflow::{AgentflowEdgeSemantic, AgentflowVertexKind};
#[cfg(all(test, feature = "compat-json"))]
use serde_json::Value;
#[cfg(feature = "presentation")]
use std::collections::HashMap;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

pub(crate) fn sanitize_common_db_fields(
    model: &mut AgentflowDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct Construction {
    model: AgentflowDiagramModel,
    #[cfg(feature = "presentation")]
    visuals: Option<HashMap<String, usize>>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

/// Parse the compatibility JSON path used by the public Mermaid facade.
#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn parse_agentflow(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let control = OperationControl::new();
    parse_agentflow_compatibility_model_controlled(code, meta, &control)
        .expect("a private parse control cannot be cancelled")?
        .into_compatibility_json_controlled(&meta.compat_json_metadata(), &control)
        .expect("a private projection control cannot be cancelled")
        .map(|(model, _)| model.into_unmanaged_value())
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_agentflow_compatibility_model_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<family::FamilySemanticModel>> {
    let construction = construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(construction
        .map(|construction| family::FamilySemanticModel::Agentflow(Box::new(construction.model)))
        .map_err(family::CombinedSemanticFailure::into_error))
}

pub(crate) fn parse_agentflow_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<AgentflowDiagramModel>> {
    let construction = construct_source(
        code,
        &meta.diagram_type,
        &meta.semantic_config,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(construction) => {
            let mut model = construction.model;
            sanitize_common_db_fields(&mut model, &meta.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts: construction.editor_facts,
            }
        }
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand.needs_recovery() => {
            let (error, editor_facts) = failure.into_parts();
            SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err(failure.into_error()),
            #[cfg(feature = "editor-facts")]
            editor_facts: None,
        },
    })
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_agentflow_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<AgentflowDiagramRenderModel>> {
    Ok(finish_construction(
        parse_agentflow_source_for_render_controlled(
            code,
            meta.effective_config.as_config(),
            meta.title.as_deref(),
            control,
        )?,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )?
    .map(|construction| AgentflowDiagramRenderModel {
        model: construction.model,
        visuals: AgentflowVisuals::from_node_dom_indices(
            construction
                .visuals
                .expect("full rendering requests DOM indices"),
        ),
    })
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_agentflow_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        EditorFactsDemand::Capture,
        control,
    )?;
    let (model, editor_facts) = match construction {
        Ok(construction) => (
            Ok(family::FamilySemanticModel::Agentflow(Box::new(
                construction.model,
            ))),
            construction
                .editor_facts
                .expect("Agentflow combined parse requests editor facts"),
        ),
        Err(failure) => {
            let (error, editor_facts) = failure.into_parts();
            (Err(error), editor_facts)
        }
    };
    Ok(family::CombinedSemanticParse::from_parts(
        model,
        editor_facts,
    ))
}

#[cfg(all(test, feature = "compat-json"))]
fn render_model_to_compat_json(
    model: &AgentflowDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::agentflow::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

fn construct_source(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    title: Option<&str>,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    #[cfg(feature = "editor-facts")]
    let mut editor = None;
    #[cfg(feature = "editor-facts")]
    let result = if editor_demand.captures_rich() {
        parse_agentflow_source_with_observer_controlled(
            code,
            config,
            title,
            &mut |observation| {
                editor
                    .get_or_insert_with(AgentflowEditorFacts::new)
                    .observe(observation);
            },
            control,
        )
    } else {
        parse_agentflow_source_controlled(code, config, title, control)
    }?;
    #[cfg(not(feature = "editor-facts"))]
    let result = parse_agentflow_source_controlled(code, config, title, control)?;
    finish_construction(
        result,
        diagram_type,
        config,
        #[cfg(feature = "editor-facts")]
        editor_demand.finish_facts(editor.map(|editor| editor.facts)),
        control,
    )
}

fn finish_construction(
    result: std::result::Result<ParsedAgentflow, AgentflowSyntaxIssue>,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] mut facts: Option<EditorSemanticFacts>,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    Ok(match result {
        Ok(parsed) => match super::grid::validate_inline_grid(
            config,
            diagram_type,
            &parsed.grid_coordinates,
            control,
        )? {
            Ok(()) => Ok(Construction {
                model: parsed.model,
                #[cfg(feature = "presentation")]
                visuals: parsed.node_dom_indices,
                #[cfg(feature = "editor-facts")]
                editor_facts: facts,
            }),
            Err(error) => Err(family::CombinedSemanticFailure::new(
                error,
                #[cfg(feature = "editor-facts")]
                facts,
            )),
        },
        Err(AgentflowSyntaxIssue { message, span }) => {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = &mut facts {
                facts.mark_recovered_from_parse_error(message.clone(), Some(span));
            }
            Err(family::CombinedSemanticFailure::new(
                Error::diagram_parse_exact(diagram_type.to_string(), message, span),
                #[cfg(feature = "editor-facts")]
                facts,
            ))
        }
    })
}

#[cfg(feature = "editor-facts")]
struct AgentflowEditorFacts {
    facts: EditorSemanticFacts,
    declared_entities: HashSet<String>,
}

#[cfg(feature = "editor-facts")]
impl AgentflowEditorFacts {
    fn new() -> Self {
        #[cfg(test)]
        crate::editor::test_work::record_collection();
        let mut facts = EditorSemanticFacts::new();
        facts.family_semantics = EditorFamilySemantics::new(EditorSemanticKind::Object);
        Self {
            facts,
            declared_entities: HashSet::new(),
        }
    }

    fn observe(&mut self, observation: AgentflowObservation<'_>) {
        let (name, kind, span) = match observation {
            AgentflowObservation::Started => return,
            AgentflowObservation::Header(span) => {
                self.facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    span,
                ));
                return;
            }
            AgentflowObservation::Identifier { name, kind, span } => (name, kind, span),
        };
        let (kind, reference) = match kind {
            AgentflowIdentifierKind::NodeDeclaration => (EditorSemanticKind::Object, false),
            AgentflowIdentifierKind::NodeReference => (EditorSemanticKind::Object, true),
            AgentflowIdentifierKind::NodeReferenceIfDeclared => (
                EditorSemanticKind::Object,
                self.declared_entities.contains(name),
            ),
            AgentflowIdentifierKind::ContainerDeclaration => (EditorSemanticKind::Namespace, false),
            AgentflowIdentifierKind::ContainerReference => (EditorSemanticKind::Namespace, true),
            AgentflowIdentifierKind::ClassDefinition => {
                self.facts
                    .push_symbol(EditorSemanticSymbol::class_definition(
                        name,
                        None,
                        EditorSemanticKind::Class,
                        span,
                        span,
                    ));
                return;
            }
        };
        if !reference {
            self.declared_entities.insert(name.to_string());
        }
        let symbol = if reference {
            EditorSemanticSymbol::reference(name, None, kind, span, span)
        } else {
            EditorSemanticSymbol::new(name, None, kind, span, span)
        };
        self.facts
            .push_symbol(symbol.with_rename_policy(EditorRenamePolicy::AgentflowNodeId));
    }
}

#[cfg(all(test, feature = "compat-json"))]
fn construct(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )
}

pub(crate) fn offset_diagnostic_positions(
    model: &mut AgentflowDiagramModel,
    line_offset: usize,
    column_offset: usize,
) {
    for diagnostic in &mut model.diagnostics {
        if let Some(position) = &mut diagnostic.position {
            offset_diagnostic_position(position, line_offset, column_offset);
        }
    }
}

fn offset_diagnostic_position(
    position: &mut AgentflowDiagnosticPosition,
    line_offset: usize,
    column_offset: usize,
) {
    if position.start_line == 1 {
        position.start_column += column_offset;
    }
    if position.end_line == 1 {
        position.end_column += column_offset;
    }
    position.start_line += line_offset;
    position.end_line += line_offset;
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use serde_json::json;

    const SOURCE: &str = concat!(
        "agentflow-beta LR\n",
        "flow team[\"`**Team**`\"]\n",
        " A[\"`**Task**`\"]@{shape: task, instruction: run} -->|\"`**Call**`\"| B\n",
        "end\n",
        "connector api[API]\n",
        "B -.- api\n",
        "classDef hot fill:red\n",
        "class A hot\n",
        "style A stroke:blue\n",
        "click A href \"https://example.com\" \"Open\" _blank\n",
    );

    #[test]
    fn agentflow_headers_preserve_standalone_nodes_and_semicolon_direction() {
        let parser = crate::SemanticParser::new();
        for header in ["agentflow", "agentflow-beta"] {
            for (suffix, direction, vertices, edges) in [
                ("", "TB", &[][..], &[][..]),
                ("\nagentflow", "TB", &["agentflow"][..], &[][..]),
                ("\n; A", "TB", &["A"][..], &[][..]),
                (" LR %% comment\nA", "LR", &["A"][..], &[][..]),
                ("\n  agentflow direction TB\nB", "TB", &["B"][..], &[][..]),
                (
                    "\u{feff}LR; agentflow --> B",
                    "LR",
                    &["agentflow", "B"][..],
                    &[("agentflow", "B")][..],
                ),
            ] {
                let source = format!("{header}{suffix}");
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                assert_eq!(parsed.metadata.diagram_type, "agentflow", "{source:?}");
                let crate::SemanticDiagramModel::Agentflow(model) = parsed.model else {
                    panic!("expected Agentflow semantics for {source:?}");
                };
                assert_eq!(model.direction, direction, "{source:?}");
                assert_eq!(
                    model
                        .vertices
                        .iter()
                        .map(|node| node.id.as_str())
                        .collect::<Vec<_>>(),
                    vertices,
                    "{source:?}",
                );
                assert_eq!(
                    model
                        .edges
                        .iter()
                        .map(|edge| (edge.start.as_str(), edge.end.as_str()))
                        .collect::<Vec<_>>(),
                    edges,
                    "{source:?}",
                );
            }
        }
    }

    #[test]
    fn agentflow_headers_distinguish_detection_errors_from_grammar_errors() {
        let parser = crate::SemanticParser::new();
        for header in ["agentflow", "agentflow-beta"] {
            for suffix in ["-other\nA", ": LR\nA"] {
                let source = format!("{header}{suffix}");
                let error = parser.parse_diagram_sync(&source).unwrap_err();
                assert!(matches!(error, Error::DetectType(_)), "{source:?}: {error}");
            }
            for suffix in [
                "; A",
                "\u{00a0}; A",
                "%% comment\nA",
                " %% comment\nA",
                " LR extra\nA",
                " LR; direction TB; A\nB",
                " LR %% direction TB\nA",
                "\nagentflow-beta",
                "\nagentflow direction TB\nB",
                "\nagentflow; direction TB; A\nB",
            ] {
                let source = format!("{header}{suffix}");
                let error = parser.parse_diagram_sync(&source).unwrap_err();
                assert!(
                    matches!(error, Error::DiagramParse { .. }),
                    "{source:?}: {error}"
                );
            }
        }
        assert!(matches!(
            parser.parse_diagram_sync("\u{0085}agentflow\nA"),
            Err(Error::DetectType(_))
        ));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn canonical_agentflow_capture_retains_success_and_failure_facts() {
        for (source, valid) in [
            ("agentflow-beta\nA --> B\n", true),
            ("agentflow-beta\nA --> B\nC ==> D\n", false),
        ] {
            let metadata = crate::SemanticMetadata {
                diagram_type: "agentflow".to_string(),
                config: crate::MermaidConfig::empty_object(),
                semantic_config: crate::MermaidConfig::empty_object(),
                title: None,
            };
            reset_agentflow_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let parsed = parse_agentflow_semantic_model_controlled(
                source,
                &metadata,
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass Agentflow facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            assert_eq!(agentflow_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                facts.completeness,
                if valid {
                    crate::EditorSemanticCompleteness::Complete
                } else {
                    crate::EditorSemanticCompleteness::Recovered
                }
            );
            assert_eq!(facts.diagnostics.is_empty(), valid);
        }
    }

    #[test]
    fn canonical_agentflow_keeps_authored_facts_without_dom_or_editor_work() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_agentflow_syntax_construction_count();
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(SOURCE)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert_eq!(model.direction, "LR");
        assert_eq!(model.vertices[0].id, "A");
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Task);
        assert_eq!(model.vertices[0].shape.as_deref(), Some("roundedRect"));
        assert_eq!(model.vertices[0].metadata["instruction"], "run");
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("team"));
        assert_eq!(
            model.edges[1].edge_semantic,
            AgentflowEdgeSemantic::Reference
        );
        assert_eq!(model.connectors[0].id, "api");
        assert_eq!(model.sub_graphs[0].nodes, ["B", "A"]);
        let authored = serde_json::to_value(&model.authored).unwrap();
        assert_eq!(authored["nodes"]["A"]["label_type"], "markdown");
        assert_eq!(
            authored["nodes"]["A"]["classes"],
            json!(["hot", "clickable"])
        );
        assert_eq!(authored["nodes"]["A"]["styles"], json!(["stroke:blue"]));
        assert_eq!(authored["nodes"]["A"]["link"], "https://example.com/");
        assert_eq!(authored["nodes"]["A"]["link_target"], "_blank");
        assert_eq!(authored["subgraph_label_types"]["team"], "markdown");
        assert!(
            authored["nodes"]
                .as_object()
                .unwrap()
                .values()
                .all(|node| node.get("dom_index").is_none())
        );
        assert_eq!(agentflow_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            reset_agentflow_syntax_construction_count();
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "agentflow",
                    SOURCE,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Agentflow(full) = full.model() else {
                panic!("expected full Agentflow model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(&full.model).unwrap()
            );
            assert_eq!(agentflow_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn canonical_agentflow_preserves_security_limits_diagnostics_and_cancellation() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let source = "agentflow-beta\nA((Circle))\nclick A call test(\"one\", two) \"Run\"\n";
        for (security, expected_callback) in [("strict", false), ("loose", true)] {
            let parser = crate::SemanticParser::new().with_site_config(
                crate::MermaidConfig::from_value(json!({"securityLevel": security})),
            );
            let parsed = parser.parse_diagram_sync(source).unwrap();
            let model = match parsed.model {
                crate::SemanticDiagramModel::Agentflow(model) => model,
                #[allow(unreachable_patterns)]
                _ => panic!("expected Agentflow semantics"),
            };
            assert_eq!(model.diagnostics[0].id, "SHAPE_REMOVED");
            assert!(model.diagnostics[0].position.is_some());
            assert!(model.warning_facts[0].span.is_some());
            assert_eq!(model.authored.nodes["A"].have_callback, expected_callback);
        }
        let source_override = format!("%%{{init: {{securityLevel: 'loose'}}}}%%\n{source}");
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source_override)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert!(!model.authored.nodes["A"].have_callback);

        for source in [
            "agentflow-beta\nA ==> B\n",
            "agentflow-beta\nA@{shape: unknown}\n",
        ] {
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(source)
                .unwrap_err();
            assert!(matches!(error, Error::DiagramParse { .. }));
            #[cfg(feature = "compat-json")]
            assert_eq!(
                error.to_string(),
                crate::MermaidEngine::new()
                    .parse_diagram_sync(source, crate::ParseOptions::strict())
                    .unwrap_err()
                    .to_string()
            );
        }
        let error = crate::SemanticParser::new()
            .with_site_config(crate::MermaidConfig::from_value(json!({"maxEdges": 1})))
            .parse_diagram_sync("agentflow-beta\nA --> B --> C\n")
            .unwrap_err();
        assert!(error.to_string().contains("Edge limit exceeded"));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = OperationControl::new();
        control.cancel();
        let error = crate::SemanticParser::new()
            .parse_diagram_controlled_sync(SOURCE, &control)
            .unwrap_err();
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn full_agentflow_serialization_keeps_presentation_shape_and_dom_ordinals() {
        let source = SOURCE.replacen("agentflow-beta", "agentflow", 1);
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Agentflow(model) = parsed.model() else {
            panic!("expected full Agentflow model");
        };
        let value = serde_json::to_value(model).unwrap();
        assert!(value.get("authored").is_none());
        assert!(value.get("model").is_none());
        assert!(value.get("visuals").is_none());
        assert_eq!(value["presentation"]["nodes"]["A"]["dom_index"], 0);
        assert_eq!(
            value["presentation"]["nodes"]["A"]["label_type"],
            "markdown"
        );
        let roundtrip: AgentflowDiagramRenderModel = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&roundtrip).unwrap(), value);
        let (flow, context) = roundtrip.to_flowchart_model();
        assert_eq!(flow.keyword, "agentflow-beta");
        let node = flow.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!(node.classes, ["af-kind-task", "hot", "clickable"]);
        assert_eq!(node.styles, ["stroke:blue"]);
        assert_eq!(context.node_dom_index("A"), Some(0));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn agentflow_observes_editor_facts_only_for_combined_request() {
        let meta = SourcePresentationMetadata {
            diagram_type: "agentflow".into(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_agentflow_syntax_construction_count();
        let json = parse_agentflow(SOURCE, &meta).unwrap();
        assert!(json.get("authored").is_none());
        assert!(json.get("presentation").is_none());
        assert!(json["vertices"][0].get("parentId").is_none());
        assert_eq!(
            json["vertices"][0]["metadata"],
            json!({"instruction": "run"})
        );
        assert_eq!(agentflow_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_agentflow_syntax_construction_count();
        let combined =
            parse_agentflow_json_and_editor_facts(SOURCE, &meta, &OperationControl::new()).unwrap();
        let (combined_model, editor_facts) =
            crate::family::test_support::into_projected_result(Ok(combined), &meta).unwrap();
        assert_eq!(combined_model, json);
        assert!(editor_facts.symbols.iter().any(|symbol| symbol.name == "A"));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read().collections, 1);
        assert_eq!(agentflow_syntax_construction_count(), 1);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::SourceSpan;
    use crate::{MermaidConfig, OperationControl};
    use serde_json::json;

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "agentflow".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn agentflow_direction_is_scoped_to_flow_and_consumes_its_physical_line() {
        let source = "agentflow-beta TB\ndirection LR\nflow F\ndirection RL; A --> B\nend\nC; D\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["direction"], "TB");
        assert_eq!(model["subGraphs"][0]["direction"], "RL");
        assert_eq!(model["subGraphs"][0]["nodes"], json!([]));
        assert_eq!(model["edges"], json!([]));
        assert_eq!(model["vertices"].as_array().unwrap().len(), 2);
        assert_eq!(model["vertices"][0]["id"], "C");
        assert_eq!(model["vertices"][1]["id"], "D");
    }

    #[test]
    fn agentflow_accessibility_lines_preserve_semicolons_and_percent_signs() {
        let source =
            "agentflow-beta TB\naccTitle: hello; world %% literal\naccDescr: describe; all\nA\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["accTitle"], "hello; world %% literal");
        assert_eq!(model["accDescr"], "describe; all");
        assert_eq!(model["vertices"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn agentflow_plain_quoted_labels_preserve_literal_backslashes() {
        let source = r#"agentflow-beta TB
flow F["C:\new"]
A["C:\new"] -->|"C:\new"| B
end
"#;
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["vertices"][0]["label"], r"C:\new");
        assert_eq!(model["edges"][0]["label"], r"C:\new");
        assert_eq!(model["subGraphs"][0]["title"], r"C:\new");
    }

    #[test]
    fn agentflow_metadata_double_quoted_newlines_match_shape_data_lexer() {
        let source =
            "agentflow-beta TB\nA@{\n label: \"first\n second\",\n instruction: \"do\n next\"\n}\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["vertices"][0]["label"], "first<br/>second");
        assert_eq!(
            model["vertices"][0]["metadata"]["instruction"],
            "do<br/>next"
        );
        let source = "agentflow-beta TB\nA@{\n instruction: |\n  first\n  second\n}\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(
            model["vertices"][0]["metadata"]["instruction"],
            "first\nsecond\n"
        );
    }

    #[test]
    fn agentflow_markdown_labels_preserve_types_in_the_render_projection() {
        let source = r#"agentflow-beta TB
flow F["`**group**`"]
A["`**bold**`"] -->|"`**edge**`"| B["**plain**"]
B -- "`**split**`" --> C[plain]
end
connector K["`**connector**`"]
"#;
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        for (id, label, kind) in [
            ("A", "**bold**", "markdown"),
            ("B", "**plain**", "string"),
            ("C", "plain", "text"),
            ("K", "**connector**", "text"),
        ] {
            let node = flow.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.label.as_deref(), Some(label), "{id}");
            assert_eq!(node.label_type.as_deref(), Some(kind), "{id}");
        }
        for (index, label) in ["**edge**", "**split**"].into_iter().enumerate() {
            assert_eq!(flow.edges[index].label.as_deref(), Some(label));
            assert_eq!(flow.edges[index].label_type.as_deref(), Some("markdown"));
        }
        assert_eq!(flow.subgraphs[0].title, "**group**");
        assert_eq!(flow.subgraphs[0].label_type.as_deref(), Some("markdown"));
        let semantic = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(semantic["vertices"][0]["label"], "**bold**");
        for collection in ["vertices", "edges", "subGraphs", "connectors"] {
            assert!(
                semantic[collection]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|item| item.get("labelType").is_none())
            );
        }
    }

    #[test]
    fn agentflow_metadata_label_types_follow_upstream_default_and_update_rules() {
        let source = r#"agentflow-beta TB
Default@{label: "**default**"}
Text@{label: "**text**", labelType: text}
String@{label: "**string**", labelType: string}
Markdown@{label: "**markdown**", labelType: markdown}
Invalid@{label: "**invalid**", labelType: other}
Kept["`**kept**`"]
Kept@{label: "", labelType: text}
Kept@{labelType: string}
Default["plain"]
A -->|"`edge`"| B
L_A_B_0@{labelType: text}
flow Group["`**first**`"]
X
end
flow Group["**second**"]
Y
end
Group@{labelType: text}
"#;
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        for (id, kind) in [
            ("Default", "string"),
            ("Text", "text"),
            ("String", "string"),
            ("Markdown", "markdown"),
            ("Invalid", "markdown"),
            ("Kept", "markdown"),
        ] {
            let node = flow.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.label_type.as_deref(), Some(kind), "{id}");
        }
        assert_eq!(
            flow.nodes
                .iter()
                .find(|node| node.id == "Kept")
                .unwrap()
                .label
                .as_deref(),
            Some("**kept**")
        );
        assert_eq!(flow.edges[0].label_type.as_deref(), Some("markdown"));
        assert_eq!(flow.subgraphs[0].title, "**second**");
        assert_eq!(flow.subgraphs[0].label_type.as_deref(), Some("markdown"));
    }

    #[test]
    fn agentflow_header_requires_a_separator_after_direction() {
        for source in [
            "agentflow-beta TB extra\nA\n",
            "agentflow-beta LR A --> B\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err(), "{source}");
        }
        for source in [
            "agentflow-beta TB; A\n",
            "agentflow-beta TB %% comment\nA\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_ok(), "{source}");
        }
    }

    #[test]
    fn agentflow_accessibility_accepts_lexer_whitespace_before_colon() {
        let source = "agentflow-beta TB\naccTitle \t: Heading\naccDescr\u{3000}: direction LR describes the graph\nA\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["accTitle"], "Heading");
        assert_eq!(model["accDescr"], "direction LR describes the graph");
        assert_eq!(model["direction"], "TB");
    }

    #[test]
    fn agentflow_label_comments_preserve_quoted_percent_signs_and_source_spans() {
        for (label, expected) in [
            ("[one %%comment\n two]", "one \n two"),
            ("[one %% ] --> X @{ ignored\n two]", "one \n two"),
            ("[\"one %% literal\"]", "one %% literal"),
        ] {
            let source = format!("agentflow-beta TB\nA{label}\n");
            let model = parse_agentflow(&source, &meta()).unwrap();
            assert_eq!(model["vertices"][0]["label"], expected, "{source}");
            assert_eq!(model["vertices"].as_array().unwrap().len(), 1, "{source}");
        }
        assert!(parse_agentflow("agentflow-beta TB\nA[one %% hidden close]\n", &meta()).is_err());
    }

    #[test]
    fn agentflow_reserved_identifiers_follow_ascii_keyword_boundaries() {
        for id in [
            "end-user",
            "end注文",
            "global-user",
            "style-guide",
            "flow-user",
            "connector-user",
        ] {
            for statement in [id.to_string(), format!("A --> {id}")] {
                assert!(
                    parse_agentflow(&format!("agentflow-beta TB\n{statement}\n"), &meta()).is_err(),
                    "{statement}"
                );
            }
        }
        for id in [
            "end_user",
            "endUser",
            "End-user",
            "global_user",
            "flowUser",
            "friend-end",
        ] {
            let parsed = parse_agentflow(&format!("agentflow-beta TB\n{id}\n"), &meta()).unwrap();
            assert_eq!(parsed["vertices"][0]["id"], id);
        }
    }

    #[test]
    fn agentflow_direction_statements_follow_ordered_jison_rules() {
        for (statement, expected) in [
            ("direction\tLR", "LR"),
            ("direction\u{3000}RL", "RL"),
            ("direction LRignored", "LR"),
            ("prefixdirection BT", "BT"),
            ("direction LR direction TB", "TB"),
        ] {
            let source = format!("agentflow-beta TB\nflow F\n{statement}\nA\nend\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                model.sub_graphs[0].direction.as_deref(),
                Some(expected),
                "{statement}"
            );
        }
    }

    #[test]
    fn direction_does_not_hide_higher_priority_agentflow_statements() {
        for statement in [
            r#"connector C["direction LR"]"#,
            "classDef c fill:direction LR",
            "style A fill:direction LR",
            "global direction LR",
            r#"click A href "https://example.test" "direction LR""#,
            r#"default["direction LR"]"#,
            r#""direction LR""#,
        ] {
            let source = format!("agentflow-beta TB\n{statement}\n");
            assert!(parse_agentflow(&source, &meta()).is_err(), "{source}");
        }
    }

    #[test]
    fn direction_after_end_applies_to_the_remaining_context() {
        let source = "agentflow-beta TB\nflow Outer\nflow Inner\nA\nend direction LR\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Outer")
                .unwrap()
                .direction
                .as_deref(),
            Some("LR")
        );
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Inner")
                .unwrap()
                .direction
                .as_deref(),
            None
        );
    }

    #[test]
    fn consecutive_end_tokens_close_each_container_before_direction() {
        let source = "agentflow-beta TB\nflow Outer\nflow Inner\nA\nend end direction LR\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.sub_graphs.len(), 2);
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Inner")
                .unwrap()
                .direction,
            None
        );
        assert!(parse_agentflow(&format!("{source}end\n"), &meta()).is_err());
    }

    #[test]
    fn click_direction_text_preserves_the_link_instead_of_becoming_a_direction() {
        let source = "agentflow-beta TB\nA\nclick A href \"https://example.test/direction LR\"\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.nodes[0].link.as_deref(),
            Some("https://example.test/direction%20LR")
        );
    }

    #[test]
    fn indented_end_direction_preserves_the_open_container() {
        for indent in ["  ", "\u{00a0}"] {
            let source = format!("agentflow-beta TB\nflow O\nA\n{indent}end direction LR\nend\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(model.sub_graphs.len(), 1, "{source}");
            assert_eq!(model.sub_graphs[0].id, "O");
            assert_eq!(
                model.sub_graphs[0].direction.as_deref(),
                Some("LR"),
                "{source}"
            );
            assert_eq!(model.vertices[0].id, "A");
        }
    }

    #[test]
    fn indented_direction_precedes_click_and_classdef_keywords() {
        for statement in [
            r#"  click A href "https://example.test/direction LR""#,
            "  classDef c fill:direction LR",
        ] {
            let source = format!("agentflow-beta TB\nA\n{statement}\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            let (flow, _) = model.to_flowchart_model();
            assert_eq!(flow.nodes.len(), 1, "{source}");
            assert_eq!(flow.nodes[0].id, "A");
            assert_eq!(flow.nodes[0].link, None, "{source}");
            assert!(flow.class_defs.is_empty(), "{source}");
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn editor_edge_endpoints_declare_entities_once_and_preserve_occurrence_spans() {
        use crate::EditorSemanticRole::{Entity, Reference};

        let source = "agentflow-beta\nflow Team\nA[Worker]-->B-->A-->A\nend\n";
        let parsed = construct(source, &meta(), &OperationControl::new())
            .unwrap()
            .unwrap_or_else(|failure| panic!("{}", failure.into_error()));
        let symbols = &parsed.editor_facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("Team", Entity),
                ("A", Entity),
                ("B", Entity),
                ("A", Reference),
                ("A", Reference),
            ]
        );
        let edge_start = source.find("A[Worker]").unwrap();
        assert_eq!(
            symbols[1..]
                .iter()
                .map(|symbol| symbol.selection)
                .collect::<Vec<_>>(),
            [0, 12, 16, 20]
                .map(|offset| SourceSpan::new(edge_start + offset, edge_start + offset + 1))
        );
        for symbol in symbols {
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                symbol.name
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn editor_container_identifiers_do_not_select_keyword_prefixes_or_duplicate_symbols() {
        use crate::EditorSemanticRole::{Entity, Reference};

        let source = "agentflow-beta\nflow f[f]\nconnector c[c]\nc-->f\nend\nf@{view: collapsed}\n";
        let parsed = construct(source, &meta(), &OperationControl::new())
            .unwrap()
            .unwrap_or_else(|failure| panic!("{}", failure.into_error()));
        let symbols = &parsed.editor_facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("f", Entity),
                ("c", Entity),
                ("c", Reference),
                ("f", Reference),
                ("f", Reference),
            ]
        );
        assert_eq!(symbols[0].selection.start, source.find("f[f]").unwrap());
        assert_eq!(symbols[1].selection.start, source.find("c[c]").unwrap());
        assert_eq!(symbols[4].kind, EditorSemanticKind::Namespace);
        for symbol in symbols {
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                symbol.name
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn editor_presentation_symbols_use_lexer_spans_and_separate_class_definitions() {
        use crate::EditorSemanticRole::{ClassDefinition, Entity, Reference};

        let source = "agentflow-beta\nclassDef c fill:red\nstyle s fill:blue\nc-->s\nclass c,c c\nclick c,c \"https://example.com\"\n";
        let parsed = construct(source, &meta(), &OperationControl::new())
            .unwrap()
            .unwrap_or_else(|failure| panic!("{}", failure.into_error()));
        let symbols = &parsed.editor_facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("c", ClassDefinition),
                ("s", Entity),
                ("c", Entity),
                ("s", Reference),
                ("c", Reference),
                ("c", Reference),
                ("c", Reference),
                ("c", Reference),
            ]
        );
        let expected_starts = [
            source.find("c fill:red").unwrap(),
            source.find("s fill:blue").unwrap(),
            source.find("c-->s").unwrap(),
            source.find("c-->s").unwrap() + 4,
            source.find("class c,c").unwrap() + 6,
            source.find("class c,c").unwrap() + 8,
            source.find("click c,c").unwrap() + 6,
            source.find("click c,c").unwrap() + 8,
        ];
        for (symbol, start) in symbols.iter().zip(expected_starts) {
            assert_eq!(symbol.selection, SourceSpan::new(start, start + 1));
            assert_eq!(&source[start..start + 1], symbol.name);
        }
    }

    #[test]
    fn parses_domain_shapes_edges_and_nested_flow() {
        let source = "agentflow-beta LR\nflow agent[\"Agent\"]\n  input[\"Question\"]@{ shape: input, value: \"hello\" }\n  tool[\"Search\"]@{ shape: tool, params: \"q :: String\" }\n  input --> tool\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.direction, "LR");
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Input);
        assert_eq!(model.vertices[1].vertex_kind, AgentflowVertexKind::Tool);
        assert_eq!(
            model.edges[0].edge_semantic,
            AgentflowEdgeSemantic::Sequence
        );
        assert_eq!(model.sub_graphs[0].nodes, vec!["input", "tool"]);
        assert_eq!(model.vertices[0].metadata["value"], "hello");
    }

    #[test]
    fn keeps_unknown_metadata_and_assigns_edge_semantics() {
        let source = "agentflow-beta TB\na[\"A\"]@{ shape: decision, custom: 4 }\nb[\"B\"]@{shape: action}\na -- yes --> b\nb -.- a\na --x b\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Decision);
        assert_eq!(model.vertices[0].metadata["custom"], 4);
        assert_eq!(
            model
                .edges
                .iter()
                .map(|edge| edge.edge_semantic)
                .collect::<Vec<_>>(),
            vec![
                AgentflowEdgeSemantic::Sequence,
                AgentflowEdgeSemantic::Reference,
                AgentflowEdgeSemantic::Failure
            ]
        );
        assert_eq!(model.edges[0].label.as_deref(), Some("yes"));
    }

    #[test]
    fn parses_multiline_metadata_and_container_attachment() {
        let source = "agentflow-beta TB\nflow worker[\"Worker\"]\n  task[\"Task\"]@{\n    shape: task\n    instruction: \"do the work\"\n  }\nend\nworker@{ view: collapsed, algorithm: \"elk.layered\" }\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].metadata["instruction"], "do the work");
        assert_eq!(model.sub_graphs[0].metadata["view"], "collapsed");
        assert_eq!(model.sub_graphs[0].metadata["algorithm"], "elk.layered");
    }

    #[test]
    fn preserves_global_nodes_nested_containment_and_pipe_labels() {
        let source = "agentflow-beta TB\nglobal\n  shared[\"Shared\"]@{ shape: refdoc }\nend\nflow outer[\"Outer\"]\n  flow inner[\"Inner\"]\n    task[\"Task\"]\n  end\n  task -->|\"done\"| shared\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].id, "shared");
        assert_eq!(model.vertices[0].parent_id, None);
        assert_eq!(model.sub_graphs[0].nodes, vec!["task"]);
        assert_eq!(model.sub_graphs[1].nodes, vec!["inner"]);
        assert_eq!(model.edges[0].label.as_deref(), Some("done"));
    }

    #[test]
    fn preserves_explicit_edge_ids_and_edge_metadata_attachments() {
        let source = "agentflow-beta TB\na e1@--> b\ne1@{ instruction: \"hand off\", weight: 5 }\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.edges[0].id.as_deref(), Some("e1"));
        assert_eq!(model.edges[0].metadata["instruction"], "hand off");
        assert_eq!(model.edges[0].metadata["weight"], 5);
        assert_eq!(model.vertices.len(), 2);
        let (flow, _) = model.to_flowchart_model();
        assert!(flow.edges[0].is_user_defined_id);
    }

    #[test]
    fn preserves_literal_metadata_block_scalars() {
        let source =
            "agentflow-beta TB\na@{\n  instruction: |\n    do a thing,\n    then stop\n}\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(
            model.vertices[0].metadata["instruction"],
            "do a thing,\nthen stop\n"
        );
    }

    #[test]
    fn accepts_bom_and_crlf_source() {
        let source = "\u{feff}agentflow-beta TB\r\na[\"A\"] --> b[\"B\"]\r\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices.len(), 2);
        assert_eq!(model.edges.len(), 1);
    }
    #[test]
    fn unicode_edges_and_shorthand_shapes_preserve_domain_meaning() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\n判断{是否继续} -- 成功 --> 工具[[搜索]]\n工具 --x 失败\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Decision);
        assert_eq!(model.vertices[1].vertex_kind, AgentflowVertexKind::Tool);
        assert_eq!(model.vertices[1].label.as_deref(), Some("搜索"));
        assert_eq!(model.edges[0].label.as_deref(), Some("成功"));
        assert!(model.edges.iter().all(|edge| edge.length == 1));
    }

    #[test]
    fn statement_separators_preserve_quoted_and_metadata_content() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR; flow f[Worker]; a[\"甲; %% literal\"]@{instruction: \"read; write\"}; a --> b; end; %% ignored; c\nconnector api[API]; b --> api",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.vertices.len(), 2);
        assert_eq!(model.vertices[0].label.as_deref(), Some("甲; %% literal"));
        assert_eq!(model.vertices[0].metadata["instruction"], "read; write");
        assert_eq!(model.sub_graphs[0].nodes, ["a", "b"]);
        assert_eq!(model.edges.len(), 2);
        assert_eq!(model.connectors[0].id, "api");
    }

    #[test]
    fn endpoint_groups_expand_in_order_with_one_explicit_id() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta TD; a[A & label] & b 边@---> c & d next@--x e; c -..- |reference| e",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let links = model
            .edges
            .iter()
            .map(|edge| (edge.start.as_str(), edge.end.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            links,
            [
                ("a", "c"),
                ("a", "d"),
                ("b", "c"),
                ("b", "d"),
                ("c", "e"),
                ("d", "e"),
                ("c", "e")
            ]
        );
        assert_eq!(model.edges[2].id.as_deref(), Some("边"));
        assert_eq!(model.edges[5].id.as_deref(), Some("next"));
        assert_eq!(model.edges[0].length, 2);
        assert_eq!(model.edges[6].length, 2);
        assert_eq!(model.edges[6].label.as_deref(), Some("reference"));
        assert_eq!(model.direction, "TB");
        assert_eq!(model.vertices[0].label.as_deref(), Some("A & label"));
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.edges
                .iter()
                .filter(|edge| edge.is_user_defined_id)
                .count(),
            2
        );
    }

    #[test]
    fn endpoint_expansion_obeys_the_secure_edge_limit() {
        let mut meta = meta();
        meta.effective_config =
            crate::presentation::SourcePresentationConfig::from_value(json!({"maxEdges": 3}));
        let result = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na & b --> c & d",
            &meta,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Edge limit exceeded")
        );
    }

    #[test]
    fn metadata_uses_yaml_nesting_and_preserves_literal_commas() {
        let model = parse_agentflow_model_for_render_controlled(
            r#"agentflow-beta
a[Don't stop]@{
  shape: task,
  instruction: "don't \"stop\"", # trailing comma
  details:
    retries: 3
    enabled: true
  lines: |
    keep, commas,
}
a --> b
"#,
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let node = &model.vertices[0];
        assert_eq!(node.label.as_deref(), Some("Don't stop"));
        assert_eq!(node.metadata["instruction"], "don't \"stop\"");
        assert_eq!(
            node.metadata["details"],
            json!({"retries": 3, "enabled": true})
        );
        assert_eq!(node.metadata["lines"], "keep, commas,\n");
        assert_eq!(model.edges.len(), 1);
    }

    #[test]
    fn parser_retains_upstream_vertex_registration_ordinals() {
        // Replayed with the pinned Mermaid 12 AgentflowDB, including Jison's delayed metadata
        // reductions and connector replacement. Final edges cannot reconstruct these ordinals.
        let cases: &[(&str, &[(&str, usize)])] = &[
            (
                "agentflow-beta\nA\nA\nB --> C --> D\n",
                &[("A", 0), ("B", 2), ("C", 3), ("D", 4)],
            ),
            (
                "agentflow-beta\nA@{shape: task} & B@{shape: tool} --> C & D\nE\n",
                &[("A", 0), ("B", 1), ("C", 4), ("D", 5), ("E", 6)],
            ),
            (
                "agentflow-beta\nA[Old]@{shape: task}\nstyle A fill:red\nconnector A[API]\nconnector A[Again]\nA@{instruction: call}\nB\n",
                &[("A", 3), ("B", 5)],
            ),
            (
                "agentflow-beta\nflow F\n A --> B\nend\nF@{view: collapsed}\nA e@--> B\ne@{animate: true}\nC\n",
                &[("A", 0), ("B", 1), ("F", 2), ("C", 5)],
            ),
            (
                "agentflow-beta\nconnector api@{instruction: call}\nstyle X fill:red\napi --> Y\nZ\n",
                &[("api", 0), ("X", 1), ("Y", 3), ("Z", 4)],
            ),
        ];
        for (source, expected) in cases {
            let model = parse_agentflow_model_for_render_controlled(
                source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            let serialized = serde_json::to_value(&model).unwrap();
            let model: AgentflowDiagramRenderModel = serde_json::from_value(serialized).unwrap();
            let (_, context) = model.to_flowchart_model();
            for &(id, index) in *expected {
                assert_eq!(context.node_dom_index(id), Some(index), "{source}: {id}");
            }
        }
    }

    #[test]
    fn semantic_projection_strips_presentation_but_keeps_domain_metadata() {
        let json = parse_agentflow("agentflow-beta\nflow f@{ view: collapsed, instruction: run }\n a@{ shape: decision, icon: test, instruction: decide }\nend\na --> b\n", &meta()).unwrap();
        assert_eq!(json["vertices"][0]["shape"], "diamond");
        assert_eq!(
            json["vertices"][0]["metadata"],
            json!({"instruction": "decide"})
        );
        assert!(json["vertices"][0].get("parentId").is_none());
        assert_eq!(json["subGraphs"][0]["type"], "flow");
        assert_eq!(
            json["subGraphs"][0]["metadata"],
            json!({"instruction": "run"})
        );
        assert_eq!(json["edges"][0]["type"], "arrow_point");
    }

    #[test]
    fn connector_and_collapsed_flow_remain_renderable_without_semantic_duplication() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nconnector api[API]\nflow outer@{ view: collapsed }\n flow inner\n  a --> b\n end\nend\nb --> api\n",
            &meta(), &OperationControl::new()).unwrap().unwrap();
        assert!(model.vertices.iter().all(|node| node.id != "api"));
        assert_eq!(model.connectors.len(), 1);
        let (flow, context) = model.to_flowchart_model();
        let connector = flow.nodes.iter().find(|node| node.id == "api").unwrap();
        assert_eq!(connector.label.as_deref(), Some("API"));
        assert!(context.is_subgraph_collapsed("outer"));
        assert_eq!(context.collapsed_replacement("a"), Some("outer"));
        assert_eq!(context.collapsed_replacement("inner"), Some("outer"));
    }

    #[test]
    fn global_declaration_after_flow_releases_existing_membership() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR\nflow f\n direction TB\n a --> b\nend\nglobal\n a\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.direction, "LR");
        assert_eq!(model.sub_graphs[0].direction.as_deref(), Some("TB"));
        assert_eq!(model.sub_graphs[0].nodes, ["b"]);
        assert_eq!(model.vertices[0].parent_id, None);
    }

    #[test]
    fn unsupported_shapes_keep_source_meaning_and_use_upstream_render_fallback() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na((Circle))\nb@{ shape: cloud }\nc@{shape: task}\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].shape.as_deref(), Some("circle"));
        assert_eq!(model.diagnostics[0].id, "SHAPE_REMOVED");
        assert_eq!(model.diagnostics[1].id, "SHAPE_UNSUPPORTED");
        assert_eq!(model.diagnostics.len(), 2);
        let (flow, _) = model.to_flowchart_model();
        assert!(
            flow.nodes
                .iter()
                .all(|node| node.shape.as_deref() == Some("roundedRect"))
        );
    }
    #[test]
    fn collapsing_redirects_edges_without_dropping_authored_self_loops() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f@{view: collapsed}\n a --> b\n a --> a\nend\na --> external\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.edges[0].id.as_deref(), Some("L_a_b_0"));
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.edges.len(), 2);
        assert_eq!((&*flow.edges[0].from, &*flow.edges[0].to), ("f", "f"));
        assert_eq!(
            (&*flow.edges[1].from, &*flow.edges[1].to),
            ("f", "external")
        );
        assert_eq!(flow.nodes[0].classes, ["af-kind-task"]);
    }
    #[test]
    fn metadata_shape_validation_rejects_unknown_names_at_the_authored_block() {
        for shape in ["unknown", "roundedRect", "lean_right", "123", "true", "[]"] {
            let source = format!("agentflow-beta\n甲@{{ shape: {shape} }}\n");
            let failure = construct(&source, &meta(), &OperationControl::new())
                .unwrap()
                .err()
                .unwrap();
            let error = failure.into_error();
            assert!(
                error.to_string().contains("No such shape:"),
                "{shape}: {error}"
            );
        }
        let source = "agentflow-beta\n甲@{ shape: unknown }\n";
        let failure = construct(source, &meta(), &OperationControl::new())
            .unwrap()
            .err()
            .expect("unknown shape must fail");
        let Error::DiagramParse { diagnostic, .. } = failure.into_error() else {
            panic!("unknown shape must fail with a source span");
        };
        let span = diagnostic.span().expect("unknown shape diagnostic span");
        assert_eq!(&source[span.start..span.end], "@{ shape: unknown }");
    }

    #[test]
    fn input_syntax_resolves_the_mermaid_12_3_internal_shape_alias() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na[/Input/] --> b[Task]\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].shape.as_deref(), Some("lean-right"));
        assert!(model.diagnostics.is_empty());
        let (flow, _) = model.to_flowchart_model();
        assert!(
            flow.nodes[0]
                .classes
                .iter()
                .any(|class| class == "af-kind-input")
        );
    }

    #[test]
    fn shape_aliases_are_validated_before_resolution_and_container_metadata_is_not_a_vertex() {
        for (authored, resolved) in [
            ("task", "roundedRect"),
            ("tool", "subroutine"),
            ("input", "lean-right"),
            ("round", "rect"),
        ] {
            let source = format!("agentflow-beta\na@{{ shape: {authored} }}\n");
            let model = parse_agentflow(&source, &meta()).unwrap();
            assert_eq!(model["vertices"][0]["shape"], resolved);
        }
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f@{ shape: unknown }\n a[Task]\nend\nf@{ shape: 123 } --> b\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].metadata["shape"], 123);
        assert_eq!(model.vertices[0].shape.as_deref(), Some("square"));
        for shape in ["null", "false", "0", "''"] {
            assert!(
                parse_agentflow(
                    &format!("agentflow-beta\na@{{ shape: {shape} }}\n"),
                    &meta()
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn containers_complete_inside_out_and_keep_preorder_colors() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR\nflow outer[Outer]\n a\n flow\n  a --> b\n end\n flow inner\n  c\n end\nend\nflow\n d\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .map(|graph| graph.id.as_str())
                .collect::<Vec<_>>(),
            ["subGraph0", "inner", "outer", "subGraph3"]
        );
        assert_eq!(model.sub_graphs[0].nodes, ["b", "a"]);
        assert_eq!(model.sub_graphs[2].nodes, ["subGraph0", "inner"]);
        assert_eq!(model.sub_graphs[0].title.as_deref(), Some(""));
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("subGraph0"));
        let (_, context) = model.to_flowchart_model();
        for (ordinal, id) in ["outer", "subGraph0", "inner", "subGraph3"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(context.subgraph_color_ordinal(id), Some(ordinal));
        }
    }

    #[test]
    fn duplicate_containers_merge_at_completion_and_increment_anonymous_ids() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow same[First]\n a\nend\nflow same\n direction LR\n b\nend\nflow\n c\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs.len(), 2);
        assert_eq!(model.sub_graphs[0].nodes, ["a", "b"]);
        assert_eq!(model.sub_graphs[0].title.as_deref(), Some("First"));
        assert_eq!(model.sub_graphs[0].direction.as_deref(), Some("LR"));
        assert_eq!(model.sub_graphs[1].id, "subGraph2");
    }

    #[test]
    fn global_blocks_exempt_direct_members_without_exempting_nested_children() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow outer\n global\n  flow inner\n   a\n  end\n  b\n end\n a --> b\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs[0].id, "inner");
        assert_eq!(model.sub_graphs[0].nodes, ["a"]);
        assert!(model.sub_graphs[1].nodes.is_empty());
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("inner"));
        assert_eq!(model.vertices[1].parent_id, None);
    }

    #[test]
    fn edge_chains_prepend_target_groups_to_container_membership() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a & b --> c & d --> e\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["e", "c", "d", "a", "b"]);
    }

    #[test]
    fn endpoint_metadata_uses_the_same_dispatch_as_standalone_declarations() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a\nend\nconnector api[API]\nf@{ view: collapsed } --> api@{ instruction: call }\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs[0].metadata["view"], "collapsed");
        assert_eq!(model.connectors[0].metadata["instruction"], "call");
        let (_, context) = model.to_flowchart_model();
        assert!(context.is_subgraph_collapsed("f"));
        for source in [
            "agentflow-beta\na@{ instruction: first } b@{ instruction: second }\n",
            "agentflow-beta\na@{ instruction: run } garbage\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }

    #[test]
    fn multiline_accessibility_description_does_not_create_a_vertex() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\naccDescr {\n This diagram describes a workflow: [draft; \"pending\n}\na --> b\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            model.acc_descr.as_deref(),
            Some("This diagram describes a workflow: [draft; \"pending")
        );
        assert_eq!(model.vertices.len(), 2);
    }

    #[test]
    fn containment_cycles_keep_semantics_and_drop_only_the_render_cycle() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow A\n a --> B\nend\nflow B\n b --> A\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["B", "a"]);
        assert_eq!(model.sub_graphs[1].nodes, ["A", "b"]);
        assert_eq!(model.diagnostics.len(), 1);
        assert_eq!(model.diagnostics[0].id, "CONTAINMENT_VIOLATION");
        assert_eq!(model.diagnostics[0].node_id, "B");
        let (flow, context) = model.to_flowchart_model();
        assert_eq!(flow.subgraphs[0].nodes, ["a"]);
        assert_eq!(flow.subgraphs[1].nodes, ["A", "b"]);
        assert_eq!(context.subgraph_color_ordinal("B"), Some(0));
        assert_eq!(context.subgraph_color_ordinal("A"), Some(1));
        let mut collapsed = model;
        for graph in &mut collapsed.sub_graphs {
            graph.metadata.insert("view".into(), json!("collapsed"));
        }
        let (_, context) = collapsed.to_flowchart_model();
        assert_eq!(context.collapsed_replacement("A"), None);
        assert_eq!(context.collapsed_replacement("B"), Some("A"));
        assert_eq!(context.collapsed_replacement("a"), Some("A"));
        assert_eq!(context.collapsed_replacement("b"), Some("A"));
    }

    #[test]
    fn presentation_directives_preserve_order_without_changing_domain_membership() {
        let source = "agentflow-beta\nclass a hot\nclassDef hot fill:#ff0000,color:#123456\nclassDef hot stroke:#0000ff\nflow f[Worker]\n a[Task]:::hot --> b\n style ghost fill:#00ff00\nend\nclass f hot\nstyle a stroke-width:3px\nstyle a fill:#ffff00\nclick a href \"https://example.com\" \"Open task\" _blank\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["b", "a"]);
        assert!(
            model
                .vertices
                .iter()
                .any(|node| node.id == "ghost" && node.parent_id.is_none())
        );
        let semantic = render_model_to_compat_json(&model, &meta()).unwrap();
        assert!(semantic.get("presentation").is_none());
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.class_defs["hot"],
            ["fill:#ff0000", "color:#123456", "stroke:#0000ff"]
        );
        let node = flow.nodes.iter().find(|node| node.id == "a").unwrap();
        assert_eq!(node.classes, ["af-kind-task", "hot", "clickable"]);
        assert_eq!(node.styles, ["stroke-width:3px", "fill:#ffff00"]);
        assert_eq!(node.link.as_deref(), Some("https://example.com/"));
        assert_eq!(node.link_target.as_deref(), Some("_blank"));
        assert_eq!(flow.tooltips["a"], "Open task");
        assert_eq!(flow.subgraphs[0].classes, ["hot"]);
    }

    #[test]
    fn edge_style_defaults_and_metadata_follow_source_mutation_order() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na e@--> b\nlinkStyle default stroke:#ff0000\nlinkStyle 0 interpolate basis stroke:#0000ff\ne@{ animate: true, animation: fast, curve: linear }\nclassDef marked stroke-width:4px\nclass e marked\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.edge_defaults.as_ref().unwrap().style,
            ["stroke:#ff0000"]
        );
        assert_eq!(flow.edges[0].style, ["stroke:#0000ff", "fill:none"]);
        assert_eq!(flow.edges[0].classes, ["marked"]);
        assert_eq!(flow.edges[0].interpolate.as_deref(), Some("linear"));
        assert_eq!(flow.edges[0].animate, Some(true));
        assert_eq!(flow.edges[0].animation.as_deref(), Some("fast"));
        assert_eq!(model.edges[0].metadata["curve"], "linear");
        assert!(
            parse_agentflow("agentflow-beta\na --> b\nlinkStyle 1 stroke:red\n", &meta()).is_err()
        );
    }

    #[test]
    fn first_connector_declaration_replaces_vertex_state_and_later_declarations_preserve_it() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na[Old]:::red@{ shape: cloud, instruction: old }\nstyle a fill:red\nconnector a[API]\nclass a blue\nconnector a[Updated]\na[Final]@{ instruction: call }\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert!(model.vertices.is_empty());
        assert_eq!(model.connectors[0].title.as_deref(), Some("Final"));
        assert_eq!(
            model.connectors[0].metadata,
            serde_json::from_value(json!({"instruction":"call"})).unwrap()
        );
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.nodes[0].classes, ["af-kind-connector", "blue"]);
        assert!(flow.nodes[0].styles.is_empty());
        for source in [
            "agentflow-beta\na:::red[ignored]\n",
            "agentflow-beta\na\nclass a red[ignored]\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }

    #[test]
    fn default_curve_is_captured_when_an_edge_is_created() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nlinkStyle default interpolate basis\na --> b\nlinkStyle default interpolate linear\nb --> c\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.edges[0].interpolate.as_deref(), Some("basis"));
        assert_eq!(flow.edges[1].interpolate.as_deref(), Some("linear"));
    }

    #[test]
    fn container_vertex_styles_override_container_classes_and_inline_groups_keep_classes() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a:::red & b:::blue --> c:::green\nend\nclassDef first fill:red\nclass f first\nstyle f stroke:blue\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.subgraphs[0].styles, ["stroke:blue"]);
        assert!(flow.subgraphs[0].classes.is_empty());
        for (node, class) in flow.nodes.iter().zip(["red", "blue", "green"]) {
            assert_eq!(node.classes[1], class);
        }
        assert!(model.vertices.iter().all(|node| node.id != "f"));
    }

    #[test]
    fn click_callbacks_respect_security_and_directives_reject_unconsumed_content() {
        for (security, callback) in [("strict", false), ("loose", true)] {
            let mut metadata = meta();
            metadata.effective_config = crate::presentation::SourcePresentationConfig::from_value(
                json!({"securityLevel":security}),
            );
            let model = parse_agentflow_model_for_render_controlled(
                "agentflow-beta\na\nclick a call test(\"one\", two) \"Run\"\nclick unknown \"https://example.com\"\n",
                &metadata, &OperationControl::new(),
            ).unwrap().unwrap();
            let (flow, _) = model.to_flowchart_model();
            assert_eq!(flow.nodes.len(), 1);
            assert_eq!(flow.nodes[0].have_callback, callback);
            assert_eq!(flow.tooltips["a"], "Run");
        }
        for statement in [
            "classDef hot",
            "classDef hot[ignored] fill:red",
            "style a[ignored] fill:red",
            "style a",
            "linkStyle default",
            "class a hot garbage",
            "click a \"https://example.com\" garbage",
            "click a callback \"tip\" garbage",
            "click a \"https://example.com\" _unknown",
        ] {
            assert!(
                parse_agentflow(&format!("agentflow-beta\na\n{statement}\n"), &meta()).is_err(),
                "{statement}"
            );
        }
    }

    #[test]
    fn invalid_edges_and_unterminated_labels_do_not_silently_drop_source() {
        for source in ["agentflow-beta\na ==> b\n", "agentflow-beta\na[missing\n"] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }
}
