#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::family;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, SourceSpan};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "presentation")]
use merman_core_structured::treemap::TreemapParsedInput;
#[cfg(feature = "editor-facts")]
use merman_core_structured::treemap::{
    ItemType, SpannedText, TreemapRow, TreemapSyntaxEvent, TreemapSyntaxObserver,
};
use merman_core_structured::treemap::{
    TreemapSemanticSource, TreemapSyntax, scan_treemap_controlled,
    treemap_semantic_source_from_parsed_controlled,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(all(test, feature = "compat-json"))]
use merman_model::families::treemap::TreemapClassDef;
pub(crate) use merman_model::families::treemap::TreemapDiagramModel;
#[cfg(test)]
pub(crate) use merman_model::families::treemap::TreemapNode;
#[cfg(feature = "presentation")]
pub(crate) use merman_model::families::treemap::{
    TreemapDiagramRenderModel, TreemapOutputPresence, TreemapVisuals,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut TreemapDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(all(test, feature = "compat-json"))]
use merman_model::families::treemap::compat::TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES;

struct TreemapParseOutcome {
    syntax: TreemapSyntax,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[cfg(feature = "presentation")]
impl TreemapParseOutcome {
    fn into_strict(self, diagram_type: &str) -> Result<TreemapParsedInput> {
        self.syntax
            .into_strict()
            .map_err(|issue| treemap_error(diagram_type, issue.message, issue.span))
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn into_combined_controlled(
        self,
        meta: &SourcePresentationMetadata,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<family::CombinedSemanticParse> {
        let Self {
            syntax,
            editor_facts,
        } = self;
        let model;
        let projection_meta = meta.compat_json_metadata();
        let construction = match syntax.into_strict() {
            Err(issue) => Err(family::CombinedSemanticFailure::new(
                treemap_error(&meta.diagram_type, issue.message, issue.span),
                Some(editor_facts),
            )),
            Ok(parsed) => {
                let source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
                model = into_render_model_controlled(source, control)?;
                match merman_model::families::treemap::compat::PreparedProjection::new_controlled(
                    &model,
                    &projection_meta,
                    control,
                )? {
                    Ok(projection) => Ok((projection, editor_facts)),
                    Err(error) => Err(family::CombinedSemanticFailure::new(
                        error.into(),
                        Some(editor_facts),
                    )),
                }
            }
        };
        let combined = family::CombinedSemanticParse::from_construction_controlled(
            construction,
            control,
            |(projection, editor_facts)| {
                projection
                    .project_controlled(control)
                    .map(|model| (model.map_err(Into::into), editor_facts))
            },
            family::CombinedSemanticFailure::into_parts,
        )?;
        control.checkpoint()?;
        Ok(combined)
    }
}

#[cfg(feature = "editor-facts")]
fn push_treemap_entity(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
    statement_span: SourceSpan,
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
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        statement_span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_treemap_payload(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_treemap_row_editor_facts(facts: &mut EditorSemanticFacts, row: &TreemapRow) {
    match row {
        TreemapRow::ClassDef(class_def) => {
            facts.push_symbol(EditorSemanticSymbol::class_definition(
                class_def.class_name.text.clone(),
                Some("treemap class definition".to_string()),
                EditorSemanticKind::Class,
                class_def.span,
                class_def.class_name.span,
            ));
            if let Some(style) = class_def.style_text.as_ref() {
                push_treemap_payload(
                    facts,
                    style,
                    "treemap class style",
                    EditorSemanticKind::String,
                );
            }
        }
        TreemapRow::Item(item) => {
            let (detail, kind) = match item.item_type {
                ItemType::Section => ("treemap section", EditorSemanticKind::Namespace),
                ItemType::Leaf => ("treemap leaf", EditorSemanticKind::Variable),
            };
            push_treemap_entity(facts, &item.name, item.span, detail, kind);
            if let Some(class_selector) = item.class_selector.as_ref() {
                push_treemap_payload(
                    facts,
                    class_selector,
                    "treemap class selector",
                    EditorSemanticKind::String,
                );
            }
            if let Some(value) = item.value.as_ref() {
                push_treemap_payload(
                    facts,
                    &value.source,
                    "treemap value",
                    EditorSemanticKind::String,
                );
            }
        }
    }
}

fn parse_treemap_outcome_controlled(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<TreemapParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("treemap");

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = EditorSemanticFacts::new();
    #[cfg(feature = "editor-facts")]
    let mut observe = |event: TreemapSyntaxEvent<'_>| match event {
        TreemapSyntaxEvent::Common(fact) => {
            push_langium_common_editor_fact(&mut editor_facts, fact, "treemap");
        }
        TreemapSyntaxEvent::Row(row) => push_treemap_row_editor_facts(&mut editor_facts, row),
        TreemapSyntaxEvent::Issue(issue) => {
            editor_facts.mark_recovered_from_parse_error(issue.message.clone(), issue.span);
        }
    };
    let syntax = scan_treemap_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand
            .needs_recovery()
            .then_some(TreemapSyntaxObserver {
                captures_rich: editor_demand.captures_rich(),
                observe: &mut observe,
            }),
        control,
    )?;
    Ok(TreemapParseOutcome {
        syntax,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_treemap(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = parse_treemap_semantic_source(code, meta)?;
    let model = into_render_model_controlled(source, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled");
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_treemap_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?
    .into_combined_controlled(meta, control)?;
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_treemap_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<TreemapDiagramRenderModel> {
    let source = parse_treemap_semantic_source(code, meta)?;
    Ok(
        into_render_model_controlled(source, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled"),
    )
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_treemap_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<TreemapDiagramRenderModel>> {
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let parsed = match parsed.into_strict(&meta.diagram_type) {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
    control.checkpoint()?;
    Ok(Ok(into_render_model_controlled(source, control)?))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &TreemapDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::treemap::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(all(test, feature = "compat-json"))]
fn render_model_to_compat_json_controlled(
    model: &TreemapDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<crate::ManagedSemanticJson>> {
    Ok(merman_model::families::treemap::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}

#[cfg(feature = "presentation")]
fn into_render_model_controlled(
    source: TreemapSemanticSource,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<TreemapDiagramRenderModel> {
    let output_presence = if source.is_present() {
        TreemapOutputPresence::Model
    } else {
        TreemapOutputPresence::Empty
    };
    let semantic = source.into_semantic_model_controlled(control)?;
    Ok(TreemapDiagramRenderModel::from_parts(
        semantic,
        TreemapVisuals::default(),
        output_presence,
    ))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_treemap_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<TreemapSemanticSource> {
    construct_treemap_semantic_source(code, meta)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_treemap_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<TreemapSemanticSource> {
    let control = crate::OperationControl::new();
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &control,
    )
    .expect("a private parse control cannot be cancelled")
    .into_strict(&meta.diagram_type)?;
    Ok(
        treemap_semantic_source_from_parsed_controlled(parsed, &control)
            .expect("a private parse control cannot be cancelled"),
    )
}

pub(crate) fn parse_treemap_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<TreemapDiagramModel>> {
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = editor_demand
        .needs_recovery()
        .then_some(parsed.editor_facts);
    let model = match parsed.syntax.into_strict() {
        Err(issue) => Err(treemap_error(&meta.diagram_type, issue.message, issue.span)),
        Ok(parsed) => {
            let source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
            let mut model = source.into_semantic_model_controlled(control)?;
            sanitize_common_db_fields(&mut model, &meta.semantic_config);
            Ok(model)
        }
    };
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn treemap_error(
    diagram_type: &str,
    message: impl Into<String>,
    span: Option<SourceSpan>,
) -> Error {
    let message = message.into();
    match span {
        Some(span) => Error::diagram_parse_exact(diagram_type, &message, span),
        None => Error::diagram_parse_fallback(diagram_type, &message),
    }
}

#[cfg(all(test, feature = "presentation"))]
mod presentation_data_tests {
    use super::*;

    #[test]
    fn full_construction_keeps_empty_presence_without_json_selection() {
        let meta = SourcePresentationMetadata {
            diagram_type: "treemap".into(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let control = crate::OperationControl::new();
        let empty = parse_treemap_model_for_render_controlled("", &meta, &control)
            .unwrap()
            .unwrap();
        let header = parse_treemap_model_for_render_controlled("treemap\n", &meta, &control)
            .unwrap()
            .unwrap();
        assert_eq!(empty.output_presence(), TreemapOutputPresence::Empty);
        assert_eq!(header.output_presence(), TreemapOutputPresence::Model);
        assert_eq!(
            serde_json::to_value(&empty.semantic).unwrap(),
            serde_json::to_value(&header.semantic).unwrap()
        );
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "all-diagrams")]
    use crate::tree_lifecycle_test_support::{
        assert_in_progress_export_deadline, run_lifecycle_child,
    };
    use crate::{
        ManagedSemanticJson, MermaidConfig, MermaidEngine, ParseOptions, RenderSemanticModel,
        SourcePresentationMetadata,
    };

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "treemap".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn treemap_ordinary_projections_do_not_collect_editor_facts() {
        let source = r#"treemap
"Leaf": 1
"#;
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_treemap_model_for_render_controlled(source, &meta(), &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let projected = crate::ManagedSemanticJson::from(parse_treemap(source, &meta()).unwrap());
        assert!(projected.as_value().is_object());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn treemap_accepts_treemap_beta_header() {
        let model = parse("treemap-beta\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_accepts_treemap_header() {
        let model = parse("treemap\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = MermaidEngine::new();
        let input = r#"treemap
title Treemap Title
accTitle: Treemap accTitle
accDescr: Treemap accDescr
"Root"
  "Leaf": 42
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.metadata().diagram_type, "treemap");
        match parsed.model() {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(model.title.as_deref(), Some("Treemap Title"));
                assert_eq!(model.acc_title.as_deref(), Some("Treemap accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Treemap accDescr"));
                assert_eq!(model.root.name, "");
                let root_children = model.root.children.as_ref().unwrap();
                assert_eq!(root_children.len(), 1);
                let root = &model.nodes[root_children[0]];
                assert_eq!(root.name, "Root");
                assert_eq!(model.nodes[root.children.as_ref().unwrap()[0]].name, "Leaf");
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("treemap"));
        assert_eq!(parsed_json.model["title"], json!("Treemap Title"));
        assert_eq!(parsed_json.model["accTitle"], json!("Treemap accTitle"));
        assert_eq!(
            parsed_json.model["root"]["children"][0]["name"],
            json!("Root")
        );
        assert_eq!(
            parsed_json.model["root"]["children"][0]["children"][0]["value"],
            json!(42)
        );
        assert!(parsed_json.model.get("config").is_some());
    }

    fn parse_error(text: &str) -> String {
        let engine = MermaidEngine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        err.to_string()
    }

    fn deep_treemap_chain(depth: usize) -> String {
        let mut input = String::from("treemap\n");
        for level in 0..depth {
            input.push_str(&" ".repeat(level));
            input.push('"');
            input.push_str(&format!("section{level}"));
            input.push_str("\"\n");
        }
        input.push_str(&" ".repeat(depth));
        input.push_str("\"leaf\": 1\n");
        input
    }

    #[test]
    fn treemap_deep_chain_semantic_and_render_model_use_heap_traversal() {
        const DEPTH: usize = 1200;
        let input = deep_treemap_chain(DEPTH);

        let model = parse(&input);
        let nodes = model["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), DEPTH + 1);
        assert_eq!(nodes[0]["name"], json!("section0"));
        assert_eq!(
            nodes
                .last()
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str),
            Some("leaf")
        );

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        match parsed.model() {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(model.nodes.len() + 1, DEPTH + 2);
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }
    }

    #[test]
    fn treemap_errors_on_trailing_whitespace_only_line() {
        let msg = parse_error("treemap\n\"A\": 1\n    \n");
        assert!(
            msg.contains("unexpected trailing whitespace-only line"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_trailing_comment_is_not_a_whitespace_only_line() {
        let model = parse("treemap\n\"A\": 1\n    %% trailing comment\n");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_rejects_header_with_colon() {
        let msg = parse_error("treemap:\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_rejects_header_with_suffix_tokens() {
        let msg = parse_error("treemap utilities\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_allows_whitespace_only_lines_in_the_middle() {
        let model = parse("treemap\n\"A\": 1\n    \n\"B\": 2\n");
        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn treemap_does_not_treat_unicode_nbsp_as_indentation() {
        let model = parse("treemap\n\"Root\"\n\u{00A0}\"Leaf\": 1\n");
        let children = model["root"]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["name"], json!("Root"));
        assert_eq!(children[1]["name"], json!("Leaf"));
    }

    #[test]
    fn treemap_parses_basic_hierarchy_from_docs() {
        let model = parse(
            r#"treemap-beta
"Section 1"
    "Leaf 1.1": 12
    "Section 1.2"
      "Leaf 1.2.1": 12
"Section 2"
    "Leaf 2.1": 20
    "Leaf 2.2": 25
"#,
        );

        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
        assert_eq!(model["root"]["children"][0]["name"], json!("Section 1"));
        assert_eq!(
            model["root"]["children"][0]["children"][0]["name"],
            json!("Leaf 1.1")
        );
        assert_eq!(
            model["root"]["children"][0]["children"][0]["value"],
            json!(12)
        );
        assert_eq!(model["root"]["children"][1]["name"], json!("Section 2"));
        assert_eq!(
            model["root"]["children"][1]["children"][1]["value"],
            json!(25)
        );
    }

    #[test]
    fn treemap_classdef_applies_compiled_styles() {
        let model = parse(
            r#"treemap-beta
"Main":::important
  "A": 20

classDef important fill:#f96,stroke:#333,stroke-width:2px;
"#,
        );
        assert_eq!(
            model["classes"]["important"]["styles"][0],
            json!("fill:#f96")
        );
        assert_eq!(
            model["root"]["children"][0]["cssCompiledStyles"][0],
            json!("fill:#f96;stroke:#333;stroke-width:2px")
        );
    }

    #[test]
    fn treemap_duplicate_classdefs_append_styles_in_source_order() {
        let model = parse(
            r#"treemap
"Root":::important
classDef important fill:#f96
classDef important stroke:#333
"Leaf": 1:::important
"#,
        );

        assert_eq!(
            model["classes"]["important"]["styles"],
            json!(["fill:#f96", "stroke:#333"])
        );
        assert_eq!(
            model["root"]["children"][0]["cssCompiledStyles"],
            json!(["fill:#f96;stroke:#333"])
        );
    }

    fn model_with_shared_class_styles(
        node_count: usize,
        style_bytes: usize,
    ) -> (TreemapDiagramRenderModel, String) {
        let style = format!("fill:{}", "a".repeat(style_bytes.saturating_sub(5)));
        let nodes = (0..node_count)
            .map(|index| TreemapNode {
                name: format!("node-{index}"),
                value: Some(json!(1).into()),
                class_selector: Some("shared".to_string()),
                ..Default::default()
            })
            .collect();
        (
            TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
                root: TreemapNode {
                    children: Some((0..node_count).collect()),
                    ..Default::default()
                },
                nodes,
                classes: std::collections::BTreeMap::from([(
                    "shared".to_string(),
                    TreemapClassDef {
                        id: "shared".to_string(),
                        styles: vec![style.clone()],
                        text_styles: Vec::new(),
                    },
                )]),
                ..Default::default()
            }),
            style,
        )
    }

    #[test]
    fn treemap_compatibility_projection_accepts_bounded_shared_class_styles() {
        let (model, style) = model_with_shared_class_styles(512, 256);
        let output = render_model_to_compat_json(&model, &meta())
            .expect("bounded repeated classDef styles should remain compatible");

        assert_eq!(output["nodes"].as_array().unwrap().len(), 512);
        assert_eq!(output["nodes"][0]["cssCompiledStyles"], json!([style]));
        assert_eq!(
            output["root"]["children"][511]["cssCompiledStyles"],
            output["nodes"][511]["cssCompiledStyles"]
        );
    }

    #[test]
    fn treemap_compatibility_projection_rejects_repeated_class_styles_before_value_build() {
        let (model, _) = model_with_shared_class_styles(2_048, 4_096);
        let error = render_model_to_compat_json(&model, &meta())
            .expect_err("repeated classDef styles beyond the output budget must fail");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured compatibility output budget error");
        };
        assert!(
            diagnostic
                .message()
                .contains("treemap compatibility JSON output budget exceeded"),
            "{}",
            diagnostic.message()
        );
        assert!(diagnostic.message().contains(&format!(
            "maximum is {TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES}"
        )));
    }

    #[test]
    fn treemap_compatibility_projection_rejects_large_nodes_before_value_build() {
        const NODE_COUNT: usize = 64_000;
        const NAME_BYTES: usize = 128;
        let name_suffix = "x".repeat(NAME_BYTES.saturating_sub(12));
        let nodes = (0..NODE_COUNT)
            .map(|index| TreemapNode {
                name: format!("node-{index:06}-{name_suffix}"),
                value: Some(json!(1).into()),
                ..Default::default()
            })
            .collect();
        let model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some((0..NODE_COUNT).collect()),
                ..Default::default()
            },
            nodes,
            ..Default::default()
        });

        let error = render_model_to_compat_json(&model, &meta())
            .expect_err("large nested and flat node output must fail before Value materialization");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured compatibility output budget error");
        };
        assert!(
            diagnostic
                .message()
                .contains("treemap compatibility JSON output budget exceeded"),
            "{}",
            diagnostic.message()
        );
        assert!(diagnostic.message().contains("estimated "));
    }

    #[test]
    fn treemap_compatibility_preflight_observes_cancellation() {
        let (model, _) = model_with_shared_class_styles(512, 256);
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            render_model_to_compat_json_controlled(&model, &meta(), &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn treemap_compatibility_projection_observes_cancellation_after_preflight() {
        let (model, _) = model_with_shared_class_styles(512, 256);
        let control = crate::OperationControl::new();
        let metadata = meta();
        let projection =
            merman_model::families::treemap::compat::PreparedProjection::new_controlled(
                &model,
                &metadata.compat_json_metadata(),
                &control,
            )
            .expect("preflight control should remain active")
            .expect("bounded model should pass preflight");
        control.cancel();

        assert!(matches!(
            projection.project_controlled(&control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_combined_compatibility_budget_error_preserves_editor_facts() {
        const NODE_COUNT: usize = 2_048;
        const STYLE_BYTES: usize = 4_096;
        let style = format!("fill:{}", "a".repeat(STYLE_BYTES.saturating_sub(5)));
        let mut text = String::from("treemap\n");
        for index in 0..NODE_COUNT {
            text.push_str(&format!("\"node-{index}\": 1:::shared\n"));
        }
        text.push_str("classDef shared ");
        text.push_str(&style);
        text.push('\n');

        let parsed =
            parse_treemap_json_and_editor_facts(&text, &meta(), &crate::OperationControl::new())
                .expect("budget rejection must remain a semantic result");
        let (model, facts) = parsed.into_parts();
        let error = model.expect_err("oversized compatibility output must not be truncated");
        assert!(
            error
                .to_string()
                .contains("treemap compatibility JSON output budget exceeded")
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "shared" && symbol.detail.as_deref() == Some("treemap class definition")
        }));
    }

    #[test]
    fn treemap_explicit_styles_override_shared_class_styles_in_both_projections() {
        let (mut model, _) = model_with_shared_class_styles(1, 32);
        for styles in [Vec::new(), vec!["fill:#123".to_string()]] {
            model.visuals.node_styles = vec![Some(styles.clone())];
            let output = render_model_to_compat_json(&model, &meta()).unwrap();
            assert_eq!(
                output["root"]["children"][0]["cssCompiledStyles"],
                json!(styles)
            );
            assert_eq!(output["nodes"][0]["cssCompiledStyles"], json!(styles));
            assert_eq!(output["nodes"][0]["classSelector"], json!("shared"));
        }
    }

    #[test]
    fn treemap_compatibility_preflight_rejects_repeated_relationships() {
        let (mut model, _) = model_with_shared_class_styles(1, 32);
        model.root.children = Some(vec![0, 0]);
        let error = merman_model::families::treemap::compat::PreparedProjection::new_controlled(
            &model,
            &meta().compat_json_metadata(),
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cyclic or repeated treemap child relationship")
        );
    }

    #[test]
    fn treemap_classdef_rejects_bare_label_style_tokens_like_mermaid_parser() {
        let msg = parse_error(
            r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#,
        );
        assert!(
            msg.contains("invalid classDef style token `color`"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_build_hierarchy_matches_upstream_utils_test() {
        let source = r#"treemap
"Root"
    "Branch 1"
        "Leaf 1.1": 10
        "Leaf 1.2": 15
    "Branch 2"
        "Leaf 2.1": 20
        "Leaf 2.2": 25
        "Leaf 2.3": 30
"#;
        let projected = parse_treemap(source, &meta()).unwrap();
        let root_value = projected["root"]["children"].as_array().unwrap();
        assert_eq!(
            root_value,
            &vec![json!({
                "name": "Root",
                "children": [
                    {
                        "name": "Branch 1",
                        "children": [
                            { "name": "Leaf 1.1", "value": 10 },
                            { "name": "Leaf 1.2", "value": 15 },
                        ]
                    },
                    {
                        "name": "Branch 2",
                        "children": [
                            { "name": "Leaf 2.1", "value": 20 },
                            { "name": "Leaf 2.2", "value": 25 },
                            { "name": "Leaf 2.3", "value": 30 },
                        ]
                    }
                ]
            })]
        );
    }

    #[cfg(feature = "all-diagrams")]
    #[test]
    fn treemap_deep_clone_drop_export_child() {
        run_lifecycle_child(
            "diagrams::treemap::tests::treemap_deep_clone_drop_export_child",
            || {
                let source = deep_treemap_chain(5_000);
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let RenderSemanticModel::Treemap(model) = parsed.model() else {
                    panic!("typed treemap");
                };
                assert_eq!(model.nodes.len(), 5_001);
                let json =
                    ManagedSemanticJson::from(render_model_to_compat_json(model, &meta()).unwrap());
                let mut output = Vec::new();
                json.write_json(&mut output).unwrap();
                assert_in_progress_export_deadline(&json);
                assert!(output.windows(10).any(|bytes| bytes == b"\"value\":1}"));
                drop(json.clone());
                drop(json);
                drop(parsed.clone());
                drop(parsed);
                assert!(
                    MermaidEngine::new()
                        .parse_diagram_sync("treemap\n\"leaf\": 1\n", ParseOptions::strict())
                        .is_ok()
                );
            },
        );
    }

    #[cfg(feature = "all-diagrams")]
    #[test]
    fn treemap_projection_cancels_after_deep_completed_subtree_child() {
        run_lifecycle_child(
            "diagrams::treemap::tests::treemap_projection_cancels_after_deep_completed_subtree_child",
            || {
                const DEPTH: usize = 3_000;
                let mut source = deep_treemap_chain(DEPTH);
                source.push_str("\"sibling\": 2\n");
                let model = parse_treemap_model_for_render(&source, &meta()).unwrap();
                let control = crate::OperationControl::new();
                let metadata = meta();
                let projection =
                    merman_model::families::treemap::compat::PreparedProjection::new_controlled(
                        &model,
                        &metadata.compat_json_metadata(),
                        &control,
                    )
                    .unwrap()
                    .unwrap();
                // The checked projection begins one checkpoint before root construction.
                control.cancel_after_checkpoints(4 * DEPTH + 6);
                assert!(matches!(
                    projection.project_controlled(&control),
                    Err(crate::OperationCancelled { .. })
                ));
                let projection =
                    render_model_to_compat_json(&model, &meta()).map(ManagedSemanticJson::from);
                assert!(projection.is_ok());
                let mut invalid = model.clone();
                let sibling = invalid.root.children.as_ref().unwrap()[1];
                invalid.nodes[sibling].children = Some(vec![usize::MAX]);
                assert!(render_model_to_compat_json(&invalid, &meta()).is_err());
                drop(invalid);
                drop(model);
            },
        );
    }

    #[test]
    fn treemap_projection_handles_deep_canonical_records() {
        let boundary = parse_treemap_model_for_render(&deep_treemap_chain(62), &meta()).unwrap();
        let json =
            ManagedSemanticJson::from(render_model_to_compat_json(&boundary, &meta()).unwrap());
        let mut output = Vec::new();
        json.write_json(&mut output).unwrap();
        assert!(!output.is_empty());
        let manual = TreemapDiagramRenderModel::default();
        let projected = render_model_to_compat_json(&manual, &meta()).unwrap();
        assert_eq!(projected["type"], "treemap");
        assert_eq!(projected["root"]["name"], "");
        assert_eq!(projected["nodes"], json!([]));
        let empty = TreemapDiagramRenderModel::from_parts(
            manual.semantic,
            manual.visuals,
            TreemapOutputPresence::Empty,
        );
        assert_eq!(
            render_model_to_compat_json(&empty, &meta()).unwrap(),
            json!({})
        );
    }

    #[test]
    fn treemap_projection_rejects_invalid_ids_and_cycles() {
        let mut model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some(vec![0]),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(render_model_to_compat_json(&model, &meta()).is_err());
        model.nodes.push(TreemapNode {
            name: "cycle".to_string(),
            children: Some(vec![0]),
            ..Default::default()
        });
        assert!(render_model_to_compat_json(&model, &meta()).is_err());
    }

    #[test]
    fn treemap_parser_can_cancel_between_rows() {
        let mut text = String::from("treemap\n");
        for index in 0..512 {
            text.push_str(&format!("\"item-{index}\": 1\n"));
        }
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);

        assert!(matches!(
            parse_treemap_outcome_controlled(
                &text,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_combined_parse_constructs_source_once_and_preserves_projections() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "accTitle: Product areas\n",
            "accDescr: Product hierarchy\n",
            "\"Root\nArea\":::important\n",
            "  \"Leaf\n雪\": .00\n",
            "  \"Rounded\": 9007199254740993\n",
            "  \"Boundary\": 9223372036854775808\n",
            "classDef important fill:#f96,stroke:#333;\n",
        );
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_treemap_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1,
            "one combined request must construct Treemap syntax once"
        );

        let standalone_json = parse_treemap(text, &meta).unwrap();
        assert_eq!(combined_json, standalone_json);
        assert_eq!(
            combined_json["root"]["children"][0]["name"],
            json!("Root\nArea")
        );
        assert_eq!(
            combined_json["root"]["children"][0]["children"][0]["value"],
            json!(0)
        );
        assert_eq!(
            combined_json["root"]["children"][0]["children"][1]["value"],
            json!(9_007_199_254_740_992_i64)
        );
        assert_eq!(
            combined_json["root"]["children"][0]["children"][2]["value"].as_f64(),
            Some(2_f64.powi(63))
        );
        assert_eq!(
            combined_editor.completeness,
            crate::EditorSemanticCompleteness::Complete
        );
        for name in [
            "Root\nArea",
            "Leaf\n雪",
            ".00",
            "9007199254740993",
            "9223372036854775808",
        ] {
            let start = text.find(name).unwrap();
            assert!(combined_editor.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.selection == SourceSpan::new(start, start + name.len())
            }));
        }
    }

    #[test]
    fn treemap_typed_and_json_projections_share_the_same_semantics() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "\"Root\":::important\n",
            "  \"Leaf\": 42\n",
            "classDef important fill:#f96,stroke:#333;\n",
            "classDef unused fill:#abc;\n",
        );
        let meta = meta();

        let compat = parse_treemap(text, &meta).unwrap();
        let typed = parse_treemap_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        let first = typed.root.children.as_ref().unwrap()[0];
        assert!(typed.compiled_styles(Some(first)).is_none());
        assert_eq!(
            typed.nodes[first].class_selector.as_deref(),
            Some("important")
        );
        assert_eq!(
            compat["root"]["children"][0]["cssCompiledStyles"][0],
            json!("fill:#f96;stroke:#333")
        );
        assert_eq!(compat["type"], json!("treemap"));
        assert!(compat["config"].is_object());
        assert_eq!(compat["accTitle"], Value::Null);
        assert_eq!(compat["accDescr"], Value::Null);
        assert!(compat["classes"].get("unused").is_some());
        assert!(typed.classes.contains_key("unused"));

        let empty = parse_treemap_model_for_render("", &meta).unwrap();
        assert_eq!(
            render_model_to_compat_json(&empty, &meta).unwrap(),
            json!({})
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_editor_projection_uses_exact_statement_spans() {
        let text = "treemap\n\"Leaf\": 42 :::hot\nclassDef hot fill:#f96;\n";
        let facts = crate::family::test_support::editor_facts(
            parse_treemap_json_and_editor_facts,
            text,
            &meta(),
        );

        for (name, detail) in [
            ("Leaf", "treemap leaf"),
            ("42", "treemap value"),
            ("hot", "treemap class selector"),
            ("fill:#f96", "treemap class style"),
        ] {
            let start = text.find(name).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection == SourceSpan::new(start, start + name.len())
            }));
        }

        let class_definition_start = text.rfind("hot").expect("classDef name");
        let class_definition = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "hot"
                    && symbol.detail.as_deref() == Some("treemap class definition")
                    && symbol.selection.start == class_definition_start
            })
            .expect("treemap class definition fact");
        assert_eq!(
            class_definition.role,
            crate::EditorSemanticRole::ClassDefinition
        );
    }

    #[test]
    fn treemap_incomplete_statement_recovers_prior_facts_with_exact_error_span() {
        let text = "treemap\n\"Root\"\n  \"Broken\":\n";
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must reject the leaf");
        assert!(error.to_string().contains("expected number"));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                text,
                &meta,
            );

            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Root"));
            let diagnostic = facts
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.message.contains("expected number"))
                .expect("recovery diagnostic");
            assert!(diagnostic.message.contains("expected number"));
            let start = text.find("\"Broken\":").unwrap();
            assert_eq!(
                diagnostic.span,
                Some(SourceSpan::new(start, start + "\"Broken\":".len()))
            );
        }
    }

    #[test]
    fn treemap_recoverable_outcome_keeps_error_prefix_and_later_safe_lines() {
        let text = concat!(
            "treemap\r\n",
            "\"Before\": 1\r\n",
            "  \"Broken\": 12 :::\r\n",
            "classDef hot fill:#f00; trailing\r\n",
            "\"After\": 2\r\n",
        );
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must use the first error");
        assert!(error.to_string().contains("expected class selector"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                text,
                &meta,
            );
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| { diagnostic.message.contains("expected class selector") })
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("unexpected tokens after classDef")
            }));

            assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
        }
    }

    #[test]
    fn treemap_multiline_acc_descr_uses_common_syntax_and_recovers_when_unterminated() {
        let complete = "treemap\naccDescr {\nline one\nline two\n}\n\"Root\"\n";
        let meta = meta();
        #[cfg(feature = "editor-facts")]
        let (json, facts) = crate::family::test_support::into_result(
            parse_treemap_json_and_editor_facts(complete, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        #[cfg(not(feature = "editor-facts"))]
        let json = parse_treemap(complete, &meta).unwrap();
        assert_eq!(json["accDescr"], json!("line one\nline two"));
        #[cfg(feature = "editor-facts")]
        {
            let payload_start = complete.find("line one").unwrap();
            let payload = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "line one\nline two")
                .expect("multiline accessibility payload");
            assert_eq!(
                payload.selection,
                SourceSpan::new(payload_start, payload_start + "line one\nline two".len())
            );
        }

        let incomplete = "treemap\naccDescr {\nline one\n";
        assert!(parse_treemap(incomplete, &meta).is_err());
        #[cfg(feature = "editor-facts")]
        {
            let recovered = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                incomplete,
                &meta,
            );
            assert_eq!(
                recovered.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(recovered.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("unterminated accDescr block")
                    && diagnostic.span == Some(SourceSpan::new(incomplete.len(), incomplete.len()))
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "treemap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn treemap_number2_finite_values_follow_binary64_prefix_conversion() {
        let precision_boundary = (1_u64 << 53) as f64;
        let integer_boundary = 2_f64.powi(63);
        for (token, expected) in [
            (".0", 0.0),
            (".00", 0.0),
            (".5", 0.5),
            ("1.", 1.0),
            ("1_234", 1.0),
            ("12.5.6", 12.5),
            ("1,234.50", 1234.5),
            ("9007199254740991", precision_boundary - 1.0),
            ("9007199254740992", precision_boundary),
            ("9007199254740993", precision_boundary),
            ("9007199254740993.0", precision_boundary),
            ("9007199254740993.5", precision_boundary + 2.0),
            ("9007199254740995", precision_boundary + 4.0),
            ("9223372036854775807", integer_boundary),
            ("9223372036854775808", integer_boundary),
            ("18446744073709551616", integer_boundary * 2.0),
        ] {
            let source = format!("treemap\n\"Weight\": {token}\n");
            let model = parse_treemap_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            let node = model.root.children.as_ref().unwrap()[0];
            assert_eq!(
                model.nodes[node].value.as_ref().unwrap().as_f64(),
                Some(expected),
                "token {token} must round before entering the model"
            );
        }
    }

    #[test]
    fn treemap_number2_rejects_invalid_or_nonfinite_values() {
        let overflowing = "9".repeat(400);
        for token in [
            ".",
            "..1",
            "_",
            ",,",
            "-1",
            "+1",
            "1e3",
            "0x10",
            "NaN",
            overflowing.as_str(),
        ] {
            let source = format!("treemap\n\"Weight\": {token}\n");
            let result = parse_treemap_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap();
            assert!(result.model.is_err(), "invalid token {token}");
        }
    }

    #[test]
    fn treemap_multiline_string2_preserves_literal_values_indent_and_spans() {
        for eol in ["\n", "\r\n"] {
            for quote in ['"', '\''] {
                let section_name = format!("Group{eol}    子");
                let leaf_name = format!("雪🦀\\n\\path{eol}%% literal{eol}    title literal");
                let source = format!(
                    "treemap{eol}{quote}{section_name}{quote}:::hot{eol}  {quote}{leaf_name}{quote}: .00:::hot{eol}  {quote}Sibling{quote}: 2{eol}{quote}After{quote}: 3{eol}classDef hot fill:#f96;{eol}"
                );
                crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
                let construction = parse_treemap_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                assert_eq!(
                    crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
                    1
                );
                let model = construction.model.as_ref().unwrap();
                assert_eq!(model.nodes.len(), 4);
                assert_eq!(model.root.children.as_deref(), Some([0, 3].as_slice()));
                assert_eq!(model.nodes[0].name, section_name);
                assert_eq!(model.nodes[0].children.as_deref(), Some([1, 2].as_slice()));
                assert_eq!(model.nodes[1].name, leaf_name);
                assert_eq!(model.nodes[1].value.as_ref().unwrap().as_i64(), Some(0));
                assert_eq!(model.nodes[1].class_selector.as_deref(), Some("hot"));
                assert_eq!(model.nodes[2].name, "Sibling");
                assert_eq!(model.nodes[3].name, "After");
                assert_eq!(model.title, None);
                #[cfg(feature = "editor-facts")]
                {
                    let facts = construction.editor_facts.as_ref().unwrap();
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Complete
                    );
                    assert!(facts.diagnostics.is_empty());
                    for (name, detail) in [
                        (section_name.as_str(), "treemap section"),
                        (leaf_name.as_str(), "treemap leaf"),
                        (".00", "treemap value"),
                    ] {
                        let start = source.find(name).unwrap();
                        assert!(facts.symbols.iter().any(|symbol| {
                            symbol.name == name
                                && symbol.detail.as_deref() == Some(detail)
                                && symbol.selection == SourceSpan::new(start, start + name.len())
                        }));
                    }
                    let selector = source.find(".00:::hot").unwrap() + ".00:::".len();
                    assert!(facts.symbols.iter().any(|symbol| {
                        symbol.detail.as_deref() == Some("treemap class selector")
                            && symbol.selection == SourceSpan::new(selector, selector + "hot".len())
                    }));
                }
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn treemap_malformed_multiline_string2_keeps_following_editor_facts() {
        for eol in ["\n", "\r\n"] {
            for quote in ['"', '\''] {
                let broken = format!("{quote}Broken{eol}  suffix{quote}: _");
                let source = format!(
                    "treemap{eol}{quote}Before{quote}: 1{eol}{broken}{eol}{quote}After{quote}: 2{eol}"
                );
                let outcome = parse_treemap_outcome_controlled(
                    &source,
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                let issue = outcome.syntax.into_strict().err().unwrap();
                assert_eq!(issue.message, "expected number");
                let start = source.find(&broken).unwrap();
                assert_eq!(
                    issue.span,
                    Some(SourceSpan::new(start, start + broken.len()))
                );
                let facts = outcome.editor_facts;
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                let start = source.find("After").unwrap();
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == "After"
                        && symbol.selection == SourceSpan::new(start, start + "After".len())
                }));
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn treemap_failed_string2_candidates_keep_later_editor_facts() {
        const ROWS: usize = 128;
        for quote in ['"', '\''] {
            let mut source = String::from("treemap\n");
            for index in 0..ROWS {
                source.push_str(&format!(
                    "{quote}Broken{index}\n{quote}After{index}{quote}: 1\n"
                ));
            }
            let other_quote = if quote == '"' { '\'' } else { '"' };
            source.push_str(&format!(
                "{quote}unclosed\n{other_quote}SafeSuffix{other_quote}: 2\n{other_quote}unclosed other\n"
            ));
            for index in 0..ROWS {
                source.push_str(&format!("classDef hot{index} fill:#f96;\n"));
            }
            source.push_str("title Recovered title\n");
            let outcome = parse_treemap_outcome_controlled(
                &source,
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            assert!(outcome.syntax.into_strict().is_err());
            let facts = outcome.editor_facts;
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            for name in ["After0", "After127", "SafeSuffix", "hot127"] {
                let start = source.find(name).unwrap();
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }));
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn treemap_recovery_only_demand_does_not_build_rich_facts() {
        let source = "treemap\ntitle Inventory\n\"Before\": 1\n\"Broken\":\n\"After\": 2\n";
        let outcome = parse_treemap_outcome_controlled(
            source,
            EditorFactsDemand::Recovery,
            &crate::OperationControl::new(),
        )
        .unwrap();
        let issue = outcome.syntax.into_strict().err().unwrap();
        let facts = outcome.editor_facts;
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(facts.diagnostics[0].message, issue.message);
        assert_eq!(facts.diagnostics[0].span, issue.span);
    }

    #[test]
    fn treemap_string2_does_not_treat_backslashes_as_quote_escapes() {
        for quote in ['"', '\''] {
            let source = format!("treemap\n{quote}bad\\{quote}suffix{quote}: 1\n");
            let result = parse_treemap_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap();
            assert!(result.model.is_err(), "a backslash cannot escape {quote}");
        }
    }

    #[test]
    fn treemap_multiline_string2_scan_can_cancel_before_the_token_ends() {
        for quote in ['"', '\''] {
            for closed in [false, true] {
                let suffix = if closed {
                    format!("{quote}: 1\n")
                } else {
                    String::new()
                };
                let source = format!(
                    "treemap\n{quote}start\n{}{suffix}",
                    "tail text\n".repeat(16 * 1024)
                );
                let control = crate::OperationControl::new();
                control.cancel_after_checkpoints(8);
                assert!(matches!(
                    parse_treemap_outcome_controlled(
                        &source,
                        #[cfg(feature = "editor-facts")]
                        EditorFactsDemand::Omit,
                        &control,
                    ),
                    Err(crate::OperationCancelled { .. })
                ));
            }
        }
    }

    #[test]
    fn treemap_semantics_preserve_hierarchy_and_authored_styles_without_editor_work() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_treemap_semantic_model_controlled(
            r#"treemap
title Inventory
"Group"
  "Leaf": 42:::hot
classDef hot fill:red
"#,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Inventory"));
        assert_eq!(model.root.children.as_deref(), Some([0usize].as_slice()));
        assert_eq!(model.nodes[0].name, "Group");
        assert_eq!(
            model.nodes[0].children.as_deref(),
            Some([1usize].as_slice())
        );
        assert_eq!(model.nodes[1].value.as_ref().unwrap().as_i64(), Some(42));
        assert_eq!(model.nodes[1].class_selector.as_deref(), Some("hot"));
        assert_eq!(model.classes["hot"].styles, ["fill:red"]);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn treemap_semantics_keep_empty_state_and_exact_errors_without_projection() {
        for source in [
            "", "treemap
",
        ] {
            let model = parse_treemap_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.root.children.as_deref(), Some([].as_slice()));
            assert!(model.nodes.is_empty());
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_treemap_semantic_model_controlled(
            "wrong
",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("structured error");
        };
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(0, 5)));
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            parse_treemap_semantic_model_controlled(
                r#"treemap
"Leaf": 1
"#,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }
}
