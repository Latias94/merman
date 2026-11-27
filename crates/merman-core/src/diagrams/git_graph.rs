#[cfg(any(test, feature = "presentation"))]
use crate::Result;
#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
#[cfg(feature = "compat-json")]
use crate::family;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
#[cfg(feature = "editor-facts")]
use crate::{EditorRenamePolicy, EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol};
use crate::{Error, MermaidConfig, SemanticMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_graph::git_graph::{
    GitGraphArgumentKind, GitGraphArgumentRole, GitGraphSyntaxEvent,
};
use merman_core_graph::git_graph::{
    GitGraphDiagnostic, GitGraphDiagnosticLocation, GitGraphHeader,
};
pub(crate) use merman_model::families::git_graph::GitGraphDiagramModel;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::{Value, json};

#[cfg(feature = "editor-facts")]
pub(crate) use merman_core_graph::git_graph::is_gitgraph_reference as is_valid_editor_reference;

#[cfg(test)]
thread_local! {
    static GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn sanitize_common_db_fields(model: &mut GitGraphDiagramModel, config: &MermaidConfig) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct GitGraphSemanticSource {
    model: GitGraphDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct GitGraphParseFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Box<EditorSemanticFacts>>,
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn parse_git_graph(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let control = crate::OperationControl::new();
    parse_git_graph_compatibility_model_controlled(code, meta, &control)
        .expect("a private parse control cannot be cancelled")?
        .into_compatibility_json_controlled(&meta.compat_json_metadata(), &control)
        .expect("a private projection control cannot be cancelled")
        .map(|(model, _)| model.into_unmanaged_value())
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_git_graph_compatibility_model_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<family::FamilySemanticModel>> {
    let construction = construct_git_graph_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(construction
        .map(|source| family::FamilySemanticModel::GitGraph(Box::new(source.model)))
        .map_err(|failure| *failure.error))
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_git_graph_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    control.checkpoint()?;
    let construction = construct_git_graph_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    let (model, editor_facts) = match construction {
        Ok(source) => (
            Ok(family::FamilySemanticModel::GitGraph(Box::new(
                source.model,
            ))),
            source.editor_facts.unwrap_or_default(),
        ),
        Err(failure) => (
            Err(*failure.error),
            failure.editor_facts.map(|facts| *facts).unwrap_or_default(),
        ),
    };
    let parsed = family::CombinedSemanticParse::from_parts(model, editor_facts);
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_git_graph_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<GitGraphDiagramModel> {
    parse_git_graph_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_git_graph_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<GitGraphDiagramModel>> {
    let source = construct_git_graph_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(source
        .map(|source| source.model)
        .map_err(|failure| *failure.error))
}

#[cfg(feature = "editor-facts")]
fn push_gitgraph_syntax_event(facts: &mut EditorSemanticFacts, event: GitGraphSyntaxEvent<'_>) {
    let argument = match event {
        GitGraphSyntaxEvent::Common(common) => {
            push_langium_common_editor_fact(facts, common, "gitGraph");
            return;
        }
        GitGraphSyntaxEvent::Argument(argument) => argument,
    };
    if argument.text.is_empty() {
        return;
    }
    let (detail, kind) = match argument.kind {
        GitGraphArgumentKind::CommitMessage => {
            ("gitGraph commit message", EditorSemanticKind::String)
        }
        GitGraphArgumentKind::CommitId => ("gitGraph commit id", EditorSemanticKind::Object),
        GitGraphArgumentKind::CommitTag => ("gitGraph commit tag", EditorSemanticKind::String),
        GitGraphArgumentKind::CommitType => ("gitGraph commit type", EditorSemanticKind::String),
        GitGraphArgumentKind::BranchName => ("gitGraph branch", EditorSemanticKind::Variable),
        GitGraphArgumentKind::BranchOrder => ("gitGraph branch order", EditorSemanticKind::String),
        GitGraphArgumentKind::MergeBranch => {
            ("gitGraph merge branch", EditorSemanticKind::Variable)
        }
        GitGraphArgumentKind::MergeId => ("gitGraph merge id", EditorSemanticKind::Object),
        GitGraphArgumentKind::MergeTag => ("gitGraph merge tag", EditorSemanticKind::String),
        GitGraphArgumentKind::MergeType => ("gitGraph merge type", EditorSemanticKind::String),
        GitGraphArgumentKind::CherryPickId => {
            ("gitGraph cherry-pick id", EditorSemanticKind::Object)
        }
        GitGraphArgumentKind::CherryPickParent => {
            ("gitGraph cherry-pick parent", EditorSemanticKind::Object)
        }
        GitGraphArgumentKind::CherryPickTag => {
            ("gitGraph cherry-pick tag", EditorSemanticKind::String)
        }
    };
    let name = argument.text.clone();
    let detail = Some(detail.to_string());
    let span = argument.span;
    let symbol = match argument.role {
        GitGraphArgumentRole::Entity => EditorSemanticSymbol::new(name, detail, kind, span, span)
            .with_rename_policy(EditorRenamePolicy::GitGraphReference),
        GitGraphArgumentRole::Reference => {
            EditorSemanticSymbol::reference(name, detail, kind, span, span)
                .with_rename_policy(EditorRenamePolicy::GitGraphReference)
        }
        GitGraphArgumentRole::Payload => {
            EditorSemanticSymbol::payload(name, detail, kind, span, span)
        }
    };
    facts.push_symbol(symbol);
}

pub(crate) fn parse_git_graph_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<SemanticConstruction<GitGraphDiagramModel>> {
    let construction = construct_git_graph_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    control.checkpoint()?;
    Ok(match construction {
        Ok(mut source) => {
            sanitize_common_db_fields(&mut source.model, &metadata.semantic_config);
            SemanticConstruction {
                model: Ok(source.model),
                #[cfg(feature = "editor-facts")]
                editor_facts: source.editor_facts,
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err(*failure.error),
            #[cfg(feature = "editor-facts")]
            editor_facts: failure.editor_facts.map(|facts| *facts),
        },
    })
}

fn construct_git_graph_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    effective_config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::result::Result<GitGraphSemanticSource, GitGraphParseFailure>>
{
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("gitGraph");

    let header = match GitGraphHeader::parse(code, control)? {
        Ok(header) => header,
        Err(diagnostic) => {
            return Ok(Err(gitgraph_parse_failure(
                gitgraph_parse_error(diagram_type, diagnostic),
                #[cfg(feature = "editor-facts")]
                new_gitgraph_editor_facts(editor_demand),
                #[cfg(feature = "editor-facts")]
                SourceSpan::new(0, code.len()),
            )));
        }
    };
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = new_gitgraph_editor_facts(editor_demand);
    #[cfg(feature = "editor-facts")]
    let syntax = if let Some(facts) = editor_facts
        .as_mut()
        .filter(|_| editor_demand.captures_rich())
    {
        header.into_syntax_with_events(control, |event| push_gitgraph_syntax_event(facts, event))?
    } else {
        header.into_syntax(control)?
    };
    #[cfg(not(feature = "editor-facts"))]
    let syntax = header.into_syntax(control)?;

    let model = match syntax {
        Ok(syntax) => {
            #[cfg(test)]
            GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT
                .set(GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.get() + 1);
            syntax.into_model_controlled(
                diagram_type,
                effective_config,
                crate::runtime::generated_id_hex,
                control,
            )?
        }
        Err(diagnostic) => Err(diagnostic),
    };
    Ok(match model {
        Ok(model) => Ok(GitGraphSemanticSource {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(diagnostic) => Err(gitgraph_parse_failure(
            gitgraph_parse_error(diagram_type, diagnostic),
            #[cfg(feature = "editor-facts")]
            editor_facts,
            #[cfg(feature = "editor-facts")]
            SourceSpan::new(0, code.len()),
        )),
    })
}

fn gitgraph_parse_error(diagram_type: &str, diagnostic: GitGraphDiagnostic) -> Error {
    match diagnostic.location {
        GitGraphDiagnosticLocation::Unspanned => {
            Error::diagram_parse_fallback("gitGraph", diagnostic.message)
        }
        GitGraphDiagnosticLocation::Exact(span) => {
            Error::diagram_parse_exact("gitGraph", diagnostic.message, span)
        }
        GitGraphDiagnosticLocation::CommonInsertionPoint(offset) => {
            Error::diagram_parse_insertion_point(diagram_type, diagnostic.message, offset)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn new_gitgraph_editor_facts(demand: EditorFactsDemand) -> Option<EditorSemanticFacts> {
    demand.needs_recovery().then(|| {
        #[cfg(test)]
        crate::editor::test_work::record_collection();
        EditorSemanticFacts::new()
    })
}

fn gitgraph_parse_failure(
    error: Error,
    #[cfg(feature = "editor-facts")] mut editor_facts: Option<EditorSemanticFacts>,
    #[cfg(feature = "editor-facts")] fallback_span: SourceSpan,
) -> GitGraphParseFailure {
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = editor_facts.as_mut() {
        let (message, span) = match &error {
            Error::DiagramParse { diagnostic, .. } => (
                diagnostic.message().to_string(),
                diagnostic.span().or(Some(fallback_span)),
            ),
            other => (other.to_string(), Some(fallback_span)),
        };
        facts.mark_recovered_from_parse_error(message, span);
    }
    GitGraphParseFailure {
        error: Box::new(error),
        #[cfg(feature = "editor-facts")]
        editor_facts: editor_facts.map(Box::new),
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, RenderSemanticModel};
    use futures::executor::block_on;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn test_meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "gitGraph".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        }
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    fn parse_with_seed(text: &str, seed: i64) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new().with_site_config(
            crate::presentation::SourcePresentationConfig::from_value(
                json!({ "gitGraph": { "seed": seed } }),
            ),
        );
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn commit_ids(model: &Value) -> Vec<String> {
        model["commits"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c["id"].as_str().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn git_graph_parser_can_cancel_inside_commit_arguments() {
        let mut text = String::from("gitGraph\ncommit id:\"root\"");
        for index in 0..512 {
            text.push_str(&format!(" tag:\"tag-{index}\""));
        }
        text.push('\n');
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(20);

        assert!(matches!(
            construct_git_graph_semantic_source_controlled(
                &text,
                &test_meta().diagram_type,
                test_meta().effective_config.as_config(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn gitgraph_fractional_main_order_preserves_numeric_sort_and_stable_ties() {
        for (order, expected) in [
            (0.5, ["early", "main", "late"]),
            (-0.5, ["main", "early", "late"]),
            (0.0, ["main", "early", "late"]),
            (2.0, ["early", "main", "late"]),
        ] {
            let engine = MermaidEngine::new().with_site_config(
                crate::presentation::SourcePresentationConfig::from_value(json!({
                    "gitGraph": { "mainBranchOrder": order }
                })),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    "gitGraph\ncommit id:\"root\"\nbranch early\nbranch late order:2\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let RenderSemanticModel::GitGraph(model) = parsed.model() else {
                panic!("expected GitGraph model");
            };
            assert_eq!(
                model
                    .branches
                    .iter()
                    .map(|branch| branch.name.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "mainBranchOrder={order}"
            );
        }
    }

    #[test]
    fn gitgraph_quoted_fields_span_lines_without_turning_payload_into_commands() {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "gitGraph{newline}commit id:\"C\" msg:'first{newline}commit id:\"fake\"' tag:\"first{newline}second %% literal\" tag:'last'{newline}branch feature{newline}commit id:\"F\"{newline}checkout main{newline}merge feature id:\"M\" tag:'merge{newline}tag'{newline}"
            );
            let model = parse(&source);
            assert_eq!(commit_ids(&model), ["C", "F", "M"]);
            assert_eq!(model["commits"][0]["message"], "first\ncommit id:\"fake\"");
            assert_eq!(
                model["commits"][0]["tags"],
                json!(["first\nsecond %% literal", "last"])
            );
            assert_eq!(model["commits"][2]["tags"], json!(["merge\ntag"]));
        }
        for source in [
            "gitGraph\ncommit tag:\"first\nsecond\n",
            "gitGraph\ncommit tag:\"first\\\nsecond\"\n",
            "gitGraph\r\ncommit tag:'first\\\r\nsecond'\r\n",
            "gitGraph\ncommit\ntag:\"separate statement\"\n",
        ] {
            assert!(!parse_err(source).is_empty(), "{source:?}");
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_multiline_editor_payloads_keep_original_utf8_and_crlf_spans() {
        let source = "gitGraph\r\ncommit id:\"C\" tag:\"第一\r\nsecond %% literal\"\r\nbranch feature\r\ncheckout feature\r\n";
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("gitGraph", source)
            .unwrap()
            .unwrap();
        assert!(facts.diagnostics.is_empty());
        for (name, detail) in [
            ("第一\nsecond %% literal", "gitGraph commit tag"),
            ("feature", "gitGraph branch"),
        ] {
            let symbols = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                .collect::<Vec<_>>();
            assert!(!symbols.is_empty());
            for symbol in symbols {
                assert_eq!(
                    source[symbol.selection.start..symbol.selection.end].replace("\r\n", "\n"),
                    name
                );
            }
        }
        assert_eq!(
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "feature")
                .count(),
            2
        );
    }

    #[test]
    fn gitgraph_only_ascii_horizontal_whitespace_separates_tokens() {
        for whitespace in [
            '\u{00a0}', '\u{2003}', '\u{2028}', '\u{2029}', '\u{000b}', '\u{000c}',
        ] {
            for source in [
                format!("gitGraph\ncommit{whitespace}id:\"C\"\n"),
                format!("gitGraph\ncommit id:{whitespace}\"C\"\n"),
                format!("gitGraph\ncommit id:\"C\"{whitespace}\n"),
                format!("gitGraph\ncommit type:HIGHLIGHT{whitespace}\n"),
                format!("gitGraph\n{whitespace}commit id:\"C\"\n"),
            ] {
                assert!(!parse_err(&source).is_empty(), "{source:?}");
            }
        }
        let model = parse("gitGraph\r\n\tcommit\tid:\t\"C\" tag:\"a\u{00a0}b\"\r\n");
        assert_eq!(commit_ids(&model), ["C"]);
        assert_eq!(model["commits"][0]["tags"], json!(["a\u{00a0}b"]));
    }

    #[test]
    fn gitgraph_merge_tags_preserve_literal_markup_while_commit_tags_are_sanitized() {
        let model = parse(concat!(
            "gitGraph\n",
            "commit id:\"root\" tag:\"<unknown>root</unknown>\"\n",
            "branch feature\ncommit id:\"F\"\ncheckout main\n",
            "merge feature id:\"M\" tag:\"<unknown>release</unknown>\" tag:'<script>alert(1)</script>'\n",
        ));
        assert_eq!(model["commits"][0]["tags"], json!(["root"]));
        assert_eq!(
            model["commits"][2]["tags"],
            json!(["<unknown>release</unknown>", "<script>alert(1)</script>"])
        );
    }

    #[test]
    fn should_handle_gitgraph_definition_and_defaults() {
        let model = parse("gitGraph:\n commit\n");
        assert_eq!(model["commits"].as_array().unwrap().len(), 1);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "main");
        assert_eq!(model["direction"].as_str().unwrap(), "LR");
        assert_eq!(model["branches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn parse_gitgraph_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = MermaidEngine::new().with_site_config(
            crate::presentation::SourcePresentationConfig::from_value(json!({
                "gitGraph": { "seed": 1 }
            })),
        );
        let input = r#"
gitGraph TB:
title <script>alert(1)</script><b>Git title</b>
accTitle: Git accTitle
accDescr: Git accDescription
commit id:"C0"
branch feature
checkout feature
commit id:"F1" tag:"v1"
checkout main
merge feature id:"M1"
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "gitGraph");
        match parsed.model() {
            RenderSemanticModel::GitGraph(model) => {
                assert_eq!(model.diagram_type, "gitGraph");
                assert_eq!(model.direction, "TB");
                assert_eq!(model.current_branch, "main");
                assert_eq!(model.title.as_deref(), Some("<b>Git title</b>"));
                assert_eq!(model.acc_title.as_deref(), Some("Git accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Git accDescription"));
                assert_eq!(model.branches.len(), 2);
                assert_eq!(model.branches[0].name, "main");
                assert_eq!(model.commits.len(), 3);
                assert_eq!(model.commits[1].id, "F1");
                assert_eq!(model.commits[1].tags, vec!["v1".to_string()]);
                assert_eq!(model.commits[2].commit_type, 3);
            }
            other => panic!("gitGraph render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("gitGraph"));
        assert_eq!(parsed_json.model["direction"], json!("TB"));
        assert_eq!(parsed_json.model["currentBranch"], json!("main"));
        assert_eq!(parsed_json.model["title"], json!("<b>Git title</b>"));
        assert_eq!(parsed_json.model["accTitle"], json!("Git accTitle"));
        assert_eq!(parsed_json.model["branches"][0]["name"], json!("main"));
        assert_eq!(parsed_json.model["commits"][1]["id"], json!("F1"));
        assert_eq!(parsed_json.model["commits"][1]["tags"], json!(["v1"]));
        assert!(parsed_json.model.get("config").is_some());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn parse_gitgraph_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "gitGraph TB\n",
            "accTitle: Git title\n",
            "accDescr: Git description\n",
            "branch feature order: 2\n",
            "commit id:\"C1\" msg:\"commit message\" tag:\"v1\" type: HIGHLIGHT\n",
            "checkout feature\n",
            "merge feature id:\"M1\" tag:\"merge tag\"\n",
            "cherry-pick id:\"C1\" parent:\"P1\" tag:\"pick tag\"\n",
        );
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("gitGraph", text)
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "feature"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "C1"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "M1"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "P1"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "commit message")
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_usage_occurrences_are_typed_references_without_polluting_entities() {
        let text = concat!(
            "gitGraph\n",
            "commit id:\"ROOT\"\n",
            "branch feature\n",
            "checkout feature\n",
            "commit id:\"F1\"\n",
            "switch main\n",
            "commit id:\"M0\"\n",
            "merge feature id:\"M1\"\n",
            "branch release\n",
            "cherry-pick id:\"M1\" parent:\"M0\"\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("gitGraph", text)
            .unwrap()
            .expect("gitGraph editor facts");

        let roles_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            roles_for("feature"),
            vec![
                crate::EditorSemanticRole::Entity,
                crate::EditorSemanticRole::Reference,
                crate::EditorSemanticRole::Reference,
            ]
        );
        assert_eq!(
            roles_for("main"),
            vec![crate::EditorSemanticRole::Reference]
        );
        assert_eq!(
            roles_for("M1"),
            vec![
                crate::EditorSemanticRole::Entity,
                crate::EditorSemanticRole::Reference,
            ]
        );
        assert_eq!(
            roles_for("M0"),
            vec![
                crate::EditorSemanticRole::Entity,
                crate::EditorSemanticRole::Reference,
            ]
        );
        assert_eq!(
            roles_for("release"),
            vec![crate::EditorSemanticRole::Entity]
        );

        let feature_symbols = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "feature")
            .collect::<Vec<_>>();
        assert!(feature_symbols[0].role.contributes_completion());
        assert!(feature_symbols[0].role.contributes_outline());
        for reference in &feature_symbols[1..] {
            assert!(!reference.role.contributes_completion());
            assert!(!reference.role.contributes_outline());
            assert!(reference.role.contributes_references());
            assert_eq!(reference.kind, EditorSemanticKind::Variable);
        }

        for name in ["M1", "M0"] {
            let symbols = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .collect::<Vec<_>>();
            assert_eq!(symbols[0].kind, EditorSemanticKind::Object);
            assert_eq!(symbols[1].kind, EditorSemanticKind::Object);
            assert!(symbols[1].role.contributes_references());
            assert!(!symbols[1].role.contributes_completion());
            assert!(!symbols[1].role.contributes_outline());
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_parser_recovery_keeps_later_semantic_facts_without_rescanning() {
        let text = concat!(
            "gitGraph\r\n",
            "commit id:\"C1\"\r\n",
            "checkout main trailing\r\n",
            "commit id:\"C2\" msg:\"后来\"\r\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("gitGraph");
        let facts = crate::family::test_support::editor_facts(
            parse_git_graph_json_and_editor_facts,
            text,
            &test_meta(),
        );

        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("gitGraph"),
            1
        );
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "C1"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "C2"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "后来"));
    }

    #[test]
    fn gitgraph_unknown_command_reports_exact_command_span() {
        let engine = MermaidEngine::new();
        let text = "gitGraph\n  frobnicate branch\n";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected gitGraph parse error");
        };

        let command_start = text.find("frobnicate").unwrap();
        assert_eq!(diagnostic.message(), "Unknown statement: frobnicate");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                command_start,
                command_start + "frobnicate".len()
            ))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn seeded_auto_commit_ids_use_one_semantic_construction() {
        let model = parse_with_seed("gitGraph:\ncommit\n", 1);
        let ids = commit_ids(&model);
        assert_eq!(ids, vec!["0-4ab40cd".to_string()]);
    }

    #[test]
    fn seeded_auto_commit_ids_are_direction_invariant() {
        let base = commit_ids(&parse_with_seed("gitGraph:\ncommit\n", 1));
        let tb = commit_ids(&parse_with_seed("gitGraph TB:\ncommit\n", 1));
        let bt = commit_ids(&parse_with_seed("gitGraph BT:\ncommit\n", 1));
        assert_eq!(base, tb);
        assert_eq!(base, bt);
        assert_eq!(base, vec!["0-4ab40cd".to_string()]);
    }

    #[test]
    fn auto_commit_ids_are_deterministic_for_default_engine() {
        let first = commit_ids(&parse("gitGraph:\ncommit\ncommit\n"));
        let second = commit_ids(&parse("gitGraph:\ncommit\ncommit\n"));

        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_ne!(first[0], first[1]);
        assert!(first[0].starts_with("0-"));
        assert!(first[1].starts_with("1-"));
    }

    #[test]
    fn should_handle_set_direction_tb_and_bt() {
        let model = parse("gitGraph TB:\ncommit\n");
        assert_eq!(model["direction"].as_str().unwrap(), "TB");
        let model = parse("gitGraph BT:\ncommit\n");
        assert_eq!(model["direction"].as_str().unwrap(), "BT");
    }

    #[test]
    fn should_checkout_and_switch_branch() {
        let model = parse("gitGraph:\nbranch new\ncheckout new\n");
        assert_eq!(model["commits"].as_array().unwrap().len(), 0);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "new");

        let model = parse("gitGraph:\nbranch new\nswitch new\n");
        assert_eq!(model["commits"].as_array().unwrap().len(), 0);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "new");
    }

    #[test]
    fn should_add_commits_to_checked_out_branch() {
        let model = parse("gitGraph:\nbranch new\ncheckout new\ncommit\ncommit\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "new");
        assert_eq!(commits[0]["branch"].as_str().unwrap(), "new");
        assert_eq!(commits[1]["branch"].as_str().unwrap(), "new");
        assert_eq!(commits[1]["parents"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn should_handle_commit_with_args_and_message_variants() {
        let model = parse("gitGraph:\ncommit \"a commit\"\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0]["message"].as_str().unwrap(), "a commit");

        let model = parse("gitGraph:\ncommit msg: \"test commit\"\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits[0]["message"].as_str().unwrap(), "test commit");

        let model = parse("gitGraph:\ncommit id:\"1111\"\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits[0]["id"].as_str().unwrap(), "1111");

        let model = parse("gitGraph:\ncommit tag:\"test\"\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(
            commits[0]["tags"].as_array().unwrap()[0].as_str().unwrap(),
            "test"
        );

        let model = parse("gitGraph:\ncommit tag:\"a\" tag:\"b\"\n");
        let commits = model["commits"].as_array().unwrap();
        let tags = commits[0]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str().unwrap(), "a");
        assert_eq!(tags[1].as_str().unwrap(), "b");

        let model = parse("gitGraph:\ncommit type: HIGHLIGHT\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits[0]["type"].as_i64().unwrap(), 2);

        let model = parse("gitGraph:\ncommit id:\"1111\" tag: \"test tag\"\n");
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits[0]["id"].as_str().unwrap(), "1111");
        assert_eq!(
            commits[0]["tags"].as_array().unwrap()[0].as_str().unwrap(),
            "test tag"
        );

        let model = parse(
            "gitGraph:\ncommit id:\"1111\" type:REVERSE tag: \"test tag\" msg:\"test msg\"\n",
        );
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits[0]["id"].as_str().unwrap(), "1111");
        assert_eq!(commits[0]["type"].as_i64().unwrap(), 1);
        assert_eq!(commits[0]["message"].as_str().unwrap(), "test msg");
        assert_eq!(
            commits[0]["tags"].as_array().unwrap()[0].as_str().unwrap(),
            "test tag"
        );
    }

    #[test]
    fn gitgraph_commit_body_preserves_mixed_order_fields() {
        let model =
            parse("gitGraph:\ncommit \"mixed message\" id:\"C1\" tag:\"v1\" type:REVERSE\n");
        let commit = &model["commits"][0];

        assert_eq!(commit["id"], "C1");
        assert_eq!(commit["message"], "mixed message");
        assert_eq!(commit["tags"], json!(["v1"]));
        assert_eq!(commit["type"], 1);
    }

    #[test]
    fn gitgraph_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let model = parse(concat!(
            "gitGraph\n",
            "commit id:'root\\n' msg:\"message\\r100%% complete\" tag:'tag\\t' %% outside comment\n",
            "branch \"side\\nbranch\"\n",
            "commit id:\"second\" tag:\"slash\\\\tail\"\n",
        ));
        let commit = &model["commits"][0];

        assert_eq!(commit["id"], "root\n");
        assert_eq!(commit["message"], "message\r100%% complete");
        assert_eq!(commit["tags"], json!(["tag\t"]));
        assert_eq!(model["commits"][1]["branch"], "side\nbranch");
        assert_eq!(model["commits"][1]["tags"], json!(["slash\\tail"]));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_escaped_branch_references_keep_decoded_identity_and_authored_ranges() {
        let source = concat!(
            "gitGraph\ncommit id:\"root\"\nbranch \"left\\nright\"\n",
            "commit id:\"work\"\ncheckout main\nmerge \"left\\nright\" id:\"merged\"\n",
            "checkout \"left\\nright\"\n",
        );
        let model = parse(source);
        assert_eq!(model["branches"][1]["name"], "left\nright");
        assert_eq!(model["commits"][1]["branch"], "left\nright");
        assert_eq!(model["commits"][2]["parents"], json!(["root", "work"]));
        assert_eq!(model["currentBranch"], "left\nright");

        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("gitGraph", source)
            .unwrap()
            .unwrap();
        assert!(facts.diagnostics.is_empty());
        let references = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "left\nright")
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 3);
        for symbol in references {
            assert!(matches!(
                symbol.detail.as_deref(),
                Some("gitGraph branch" | "gitGraph merge branch")
            ));
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                r"left\nright"
            );
        }
    }

    #[test]
    fn gitgraph_rejects_tokens_outside_the_langium_command_grammar() {
        let invalid_inputs = [
            "gitGraph\ncommit id:C1\n",
            "gitGraph\ncommit type:\"NORMAL\"\n",
            "gitGraph\ncommit \"message\" trailing\n",
            "gitGraph\nbranch feature order:\"2\"\n",
            "gitGraph\nbranch feature unknown:\"value\"\n",
            "gitGraph\nbranch feature\ncheckout feature trailing\n",
            concat!(
                "gitGraph\n",
                "commit id:\"C0\"\n",
                "branch feature\n",
                "commit id:\"F1\"\n",
                "checkout main\n",
                "merge feature unknown:\"value\"\n",
            ),
            "gitGraph\ncherryPick id:\"C1\"\n",
        ];

        for input in invalid_inputs {
            let _ = parse_err(input);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_recovery_reports_parser_diagnostic_with_exact_crlf_span() {
        let text = concat!(
            "gitGraph\r\n",
            "commit id:\"C1\"\r\n",
            "  checkout main trailing %% hidden\r\n",
            "commit id:\"C2\"\r\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_git_graph_json_and_editor_facts,
            text,
            &SourcePresentationMetadata {
                diagram_type: "gitGraph".to_string(),
                config: MermaidConfig::default(),
                effective_config: crate::presentation::SourcePresentationConfig::default(),
                title: None,
            },
        );
        let invalid = "checkout main trailing";
        let start = text.find(invalid).unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "C1"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "C2"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
                && diagnostic.message.contains("unexpected checkout argument")
        }));
    }

    #[test]
    fn gitgraph_editor_recovery_reports_database_validation_errors() {
        let text = concat!(
            "gitGraph\r\n",
            "commit id:\"C1\"\r\n",
            "  checkout missing  \r\n",
        );
        let invalid = "checkout missing";
        let start = text.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("checkout of an unknown branch must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected gitGraph parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("gitGraph", text)
                .unwrap()
                .expect("gitGraph editor recovery facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "C1"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "missing"));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(expected_span)
                    && diagnostic.message.contains("not yet created")
            }));
        }
    }

    #[test]
    fn cherry_pick_filters_tags_that_sanitize_to_empty() {
        let model = parse(concat!(
            "gitGraph\n",
            "commit id:\"ZERO\"\n",
            "branch feature\n",
            "commit id:\"A\"\n",
            "checkout main\n",
            "cherry-pick id:\"A\" tag:\"<script>alert(1)</script>\" tag:\"kept\"\n",
        ));
        let commits = model["commits"].as_array().unwrap();

        assert_eq!(commits.last().unwrap()["tags"], json!(["kept"]));
    }

    #[test]
    fn commit_errors_on_unknown_fields() {
        let err =
            parse_err("gitGraph\ncommit id:\"2\" msg:\"Malformed commit\" oops:\"ignored\"\n");
        assert_eq!(err, "unexpected commit argument: oops:\"ignored\"");
    }

    #[test]
    fn should_handle_three_straight_commits() {
        let model = parse("gitGraph:\ncommit\ncommit\ncommit\n");
        assert_eq!(model["commits"].as_array().unwrap().len(), 3);
        assert_eq!(model["branches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn should_handle_new_branch_creation_and_names() {
        let model = parse("gitGraph:\ncommit\nbranch testBranch\n");
        assert_eq!(model["commits"].as_array().unwrap().len(), 1);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "testBranch");
        assert_eq!(model["branches"].as_array().unwrap().len(), 2);

        let model = parse("gitGraph:\ncommit\nbranch azAZ_-./test\n");
        assert_eq!(model["currentBranch"].as_str().unwrap(), "azAZ_-./test");
        assert_eq!(model["branches"].as_array().unwrap().len(), 2);

        let model = parse("gitGraph:\ncommit\nbranch 1.0.1\n");
        assert_eq!(model["currentBranch"].as_str().unwrap(), "1.0.1");
        assert_eq!(model["branches"].as_array().unwrap().len(), 2);

        let model = parse("gitGraph:\ncommit\nbranch release-\n");
        assert_eq!(model["currentBranch"].as_str().unwrap(), "release-");

        for invalid in ["release/", "release."] {
            let error = parse_err(&format!("gitGraph:\ncommit\nbranch {invalid}\n"));
            assert!(
                error.contains("invalid gitGraph reference"),
                "{invalid}: {error}"
            );
        }
    }

    #[test]
    fn should_allow_quoted_branch_names_and_merge() {
        let model = parse(
            "gitGraph:\ncommit\nbranch \"branch\"\ncheckout \"branch\"\ncommit\ncheckout main\nmerge \"branch\"\n",
        );
        assert_eq!(model["commits"].as_array().unwrap().len(), 3);
        assert_eq!(model["currentBranch"].as_str().unwrap(), "main");
        assert_eq!(model["branches"].as_array().unwrap().len(), 2);
        assert_eq!(
            model["branches"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|b| b["name"].as_str())
                .collect::<Vec<_>>(),
            vec!["main", "branch"]
        );
    }

    #[test]
    fn should_handle_branch_order_sorting() {
        let model = parse(
            "gitGraph:\ncommit\nbranch test1 order: 3\nbranch test2 order: 2\nbranch test3 order: 1\n",
        );
        assert_eq!(
            model["branches"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|b| b["name"].as_str())
                .collect::<Vec<_>>(),
            vec!["main", "test3", "test2", "test1"]
        );

        let model = parse("gitGraph:\ncommit\nbranch test1 order: 1\nbranch test2\nbranch test3\n");
        assert_eq!(
            model["branches"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|b| b["name"].as_str())
                .collect::<Vec<_>>(),
            vec!["main", "test2", "test3", "test1"]
        );
    }

    #[test]
    fn should_handle_merge_with_two_parents() {
        let model = parse(
            "gitGraph:\ncommit\nbranch testBranch\ncheckout testBranch\ncommit\ncheckout main\ncommit\nmerge testBranch\n",
        );
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 4);
        let merge = &commits[3];
        assert_eq!(merge["branch"].as_str().unwrap(), "main");
        assert_eq!(merge["parents"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn should_support_cherry_picking_commits() {
        let model = parse(
            "gitGraph\ncommit id: \"ZERO\"\nbranch develop\ncommit id:\"A\"\ncheckout main\ncherry-pick id:\"A\"\n",
        );
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[2]["branch"].as_str().unwrap(), "main");
        assert_eq!(
            commits[2]["tags"].as_array().unwrap()[0].as_str().unwrap(),
            "cherry-pick:A"
        );

        let model = parse(
            "gitGraph\ncommit id: \"ZERO\"\nbranch develop\ncommit id:\"A\"\ncheckout main\ncherry-pick id:\"A\" tag:\"MyTag\"\n",
        );
        let commits = model["commits"].as_array().unwrap();
        assert_eq!(
            commits[2]["tags"].as_array().unwrap()[0].as_str().unwrap(),
            "MyTag"
        );

        let model = parse(
            "gitGraph\ncommit id: \"ZERO\"\nbranch develop\ncommit id:\"A\"\ncheckout main\ncherry-pick id:\"A\" tag:\"\"\n",
        );
        let commits = model["commits"].as_array().unwrap();
        assert!(commits[2]["tags"].as_array().unwrap().is_empty());
    }

    #[test]
    fn should_support_cherry_picking_merge_commits_and_validate_parent() {
        let err = parse_err(
            "gitGraph\ncommit id: \"ZERO\"\nbranch feature\nbranch release\ncheckout feature\ncommit id: \"A\"\ncommit id: \"B\"\ncheckout main\nmerge feature id: \"M\"\ncheckout release\ncommit id: \"C\"\ncherry-pick id:\"M\"\n",
        );
        assert!(err.contains("Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified."));

        let err = parse_err(
            "gitGraph\ncommit id: \"ZERO\"\nbranch feature\nbranch release\ncheckout feature\ncommit id: \"A\"\ncommit id: \"B\"\ncheckout main\nmerge feature id: \"M\"\ncheckout release\ncommit id: \"C\"\ncherry-pick id:\"M\" parent: \"A\"\n",
        );
        assert!(err.contains("Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit."));
    }

    #[test]
    fn should_throw_error_when_try_to_branch_existing_branch() {
        let err = parse_err("gitGraph\ncommit\nbranch testBranch\ncommit\nbranch main\n");
        assert!(err.contains("Trying to create an existing branch."));

        let err = parse_err("gitGraph\ncommit\nbranch testBranch\ncommit\nbranch testBranch\n");
        assert!(err.contains("Trying to create an existing branch."));
    }

    #[test]
    fn should_throw_error_when_try_to_checkout_unknown_branch() {
        let err = parse_err("gitGraph\ncommit\ncheckout testBranch\n");
        assert_eq!(
            err,
            "Trying to checkout branch which is not yet created. (Help try using \"branch testBranch\")"
        );
    }

    #[test]
    fn should_throw_error_when_trying_to_merge_without_commits_or_unknown_branch() {
        let err = parse_err("gitGraph\nmerge testBranch\n");
        assert_eq!(
            err,
            "Incorrect usage of \"merge\". Current branch (main)has no commits"
        );

        let err = parse_err("gitGraph\ncommit\nmerge testBranch\n");
        assert_eq!(
            err,
            "Incorrect usage of \"merge\". Branch to be merged (testBranch) does not exist"
        );

        let err = parse_err("gitGraph\nbranch test1\ncheckout main\ncommit\nmerge test1\n");
        assert_eq!(
            err,
            "Incorrect usage of \"merge\". Branch to be merged (test1) has no commits"
        );
    }

    #[test]
    fn should_throw_error_when_trying_to_merge_branch_to_itself() {
        let err = parse_err("gitGraph\ncommit\nbranch testBranch\nmerge testBranch\n");
        assert_eq!(
            err,
            "Incorrect usage of \"merge\". Cannot merge a branch to itself"
        );
    }

    #[test]
    fn should_throw_error_when_using_existing_id_as_merge_id() {
        let err = parse_err(
            "gitGraph\ncommit id: \"1-111\"\nbranch testBranch\ncommit id: \"2-222\"\ncheckout main\nmerge testBranch id: \"1-111\"\n",
        );
        assert!(err.contains("Incorrect usage of \"merge\". Commit with id:1-111 already exists, use different custom id"));
    }

    #[test]
    fn should_throw_error_when_trying_to_merge_branches_having_same_heads() {
        let err =
            parse_err("gitGraph\ncommit\nbranch testBranch\ncheckout main\nmerge testBranch\n");
        assert_eq!(
            err,
            "Incorrect usage of \"merge\". Both branches have same head"
        );
    }

    #[test]
    fn should_handle_accessibility_title_and_description() {
        let model = parse(
            "gitGraph:\naccTitle: This is a title\naccDescr: This is a description\ncommit\n",
        );
        assert_eq!(model["accTitle"].as_str().unwrap(), "This is a title");
        assert_eq!(model["accDescr"].as_str().unwrap(), "This is a description");

        let model = parse(
            "gitGraph:\naccTitle: This is a title\naccDescr {\n  This is a description\n  using multiple lines\n}\ncommit\n",
        );
        assert_eq!(model["accTitle"].as_str().unwrap(), "This is a title");
        assert_eq!(
            model["accDescr"].as_str().unwrap(),
            "This is a description\nusing multiple lines"
        );
    }

    #[test]
    fn should_work_with_unsafe_properties_as_ids_and_branch_names() {
        for prop in ["__proto__", "constructor"] {
            let model = parse(&format!(
                "gitGraph\ncommit id:\"{prop}\"\nbranch {prop}\ncheckout {prop}\ncommit\ncheckout main\nmerge {prop}\n"
            ));
            assert_eq!(model["commits"].as_array().unwrap().len(), 3);
            assert_eq!(commit_ids(&model)[0], prop);
            assert_eq!(model["currentBranch"].as_str().unwrap(), "main");
            assert_eq!(model["branches"].as_array().unwrap().len(), 2);
        }
    }

    #[test]
    fn should_log_warning_when_two_commits_have_same_id() {
        let text = "gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n";
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("gitGraph compatibility parse succeeds")
            .expect("gitGraph compatibility parse returns a diagram");
        let typed = parse_git_graph_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("gitGraph typed parse succeeds");
        let projection = merman_model::families::git_graph::compat::project(
            &typed,
            &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
        )
        .expect("gitGraph compatibility projection succeeds");
        let model = parsed.model;

        assert_eq!(projection, model);
        assert_eq!(projection["type"], json!("gitGraph"));
        assert!(projection["config"].is_object());
        assert!(projection.get("title").is_none());
        let warnings = model["warningFacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.get("message").and_then(|message| message.as_str()))
            .collect::<Vec<_>>();
        assert!(warnings.contains(&"Commit ID working on MDR already exists"));
        assert_eq!(
            model["warnings"],
            json!(["Commit ID working on MDR already exists"])
        );
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use serde_json::json;

    fn canonical(source: &str) -> Result<GitGraphDiagramModel> {
        crate::SemanticParser::new()
            .with_site_config(MermaidConfig::from_value(
                json!({ "gitGraph": { "seed": 1 } }),
            ))
            .parse_diagram_sync(source)
            .map(|parsed| match parsed.model {
                crate::SemanticDiagramModel::GitGraph(model) => model,
                #[allow(unreachable_patterns)]
                _ => panic!("expected GitGraph semantics"),
            })
    }

    #[cfg(feature = "editor-facts")]
    fn capture(source: &str) -> SemanticConstruction<GitGraphDiagramModel> {
        let metadata = SemanticMetadata {
            diagram_type: "gitGraph".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::from_value(json!({ "gitGraph": { "seed": 1 } })),
            title: None,
        };
        parse_git_graph_semantic_model_controlled(
            source,
            &metadata,
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap()
    }

    fn assert_gitgraph_diagnostic(source: &str, span_text: &str, message: &str) {
        let error = canonical(source).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected GitGraph parse diagnostic for {source:?}");
        };
        let start = source.rfind(span_text).unwrap();
        let span = SourceSpan::new(start, start + span_text.len());
        assert_eq!(diagnostic.message(), message, "{source:?}");
        assert_eq!(diagnostic.span(), Some(span), "{source:?}");
        #[cfg(feature = "compat-json")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_sync(source, crate::ParseOptions::strict())
                .unwrap_err();
            assert_eq!(error.to_string(), full.to_string());
        }
        #[cfg(feature = "editor-facts")]
        {
            let captured = capture(source);
            assert!(captured.model.is_err());
            let facts = captured.editor_facts.unwrap();
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let diagnostic = facts
                .diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                })
                .unwrap();
            assert_eq!(diagnostic.message, message);
            assert_eq!(diagnostic.span, Some(span));
        }
    }

    #[test]
    fn gitgraph_quote_boundaries_preserve_models_and_editor_spans() {
        for newline in ["\n", "\r\n"] {
            for statement in ["commit\"hello\"", "commit type:NORMAL\"hello\""] {
                let source = format!("gitGraph{newline}{statement}{newline}");
                let model = canonical(&source).unwrap();
                assert_eq!(model.commits.len(), 1);
                assert_eq!(model.commits[0].message, "hello");
                assert_eq!(model.commits[0].commit_type, 0);
            }
            for quote in ['\'', '"'] {
                let source = concat!(
                    "gitGraph\n",
                    "commit\"root message\" id:\"R\"\n",
                    "branch\"topic\" order:2\n",
                    "commit type:HIGHLIGHT\"topic message\" id:\"F\"\n",
                    "checkout\"main\"\n",
                    "merge\"topic\" id:\"M\" type:REVERSE tag:\"first\" tag:\"last\"\n",
                    "switch\"topic\"\n",
                )
                .replace('"', &quote.to_string())
                .replace('\n', newline);
                let model = canonical(&source).unwrap();
                assert_eq!(model.current_branch, "topic");
                assert_eq!(model.commits.len(), 3);
                assert_eq!(model.commits[0].message, "root message");
                assert_eq!(model.commits[1].message, "topic message");
                assert_eq!(model.commits[1].commit_type, 2);
                assert_eq!(model.commits[2].parents, ["R", "F"]);
                assert_eq!(model.commits[2].commit_type, 3);
                assert_eq!(model.commits[2].custom_type, Some(1));
                assert_eq!(model.commits[2].tags, ["first", "last"]);
                #[cfg(feature = "editor-facts")]
                {
                    let captured = capture(&source);
                    assert!(captured.model.is_ok());
                    let facts = captured.editor_facts.unwrap();
                    assert!(facts.diagnostics.is_empty());
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Complete
                    );
                    let topic_roles = facts
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.name == "topic")
                        .map(|symbol| symbol.role)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        topic_roles,
                        [
                            crate::EditorSemanticRole::Entity,
                            crate::EditorSemanticRole::Reference,
                            crate::EditorSemanticRole::Reference,
                        ]
                    );
                    for symbol in &facts.symbols {
                        assert_eq!(
                            &source[symbol.selection.start..symbol.selection.end],
                            symbol.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn gitgraph_repeated_scalars_keep_winning_bindings_and_cumulative_tags() {
        for newline in ["\n", "\r\n"] {
            let source = concat!(
                "gitGraph\n",
                "commit msg:\"old message\" id:\"stale-root\" tag:\"root-a\" id:\"R\" ",
                "msg:\"root message\" type:REVERSE type:NORMAL tag:\"root-b\"\n",
                "branch release\ncheckout main\nbranch feature\ncommit id:\"F\"\n",
                "checkout main\n",
                "merge feature id:\"stale-merge\" tag:\"merge-a\" id:\"M\" ",
                "type:REVERSE type:HIGHLIGHT tag:\"merge-b\"\n",
                "checkout release\n",
                "cherry-pick id:\"stale-source\" tag:\"pick-a\" parent:\"stale-parent\" ",
                "id:\"M\" parent:\"R\" tag:\"\" tag:\"pick-b\"\n",
            )
            .replace('\n', newline);
            let model = canonical(&source).unwrap();
            assert_eq!(model.commits.len(), 4);
            assert_eq!(model.commits[0].id, "R");
            assert_eq!(model.commits[0].message, "root message");
            assert_eq!(model.commits[0].commit_type, 0);
            assert_eq!(model.commits[0].tags, ["root-a", "root-b"]);
            assert_eq!(model.commits[2].id, "M");
            assert_eq!(model.commits[2].custom_type, Some(2));
            assert_eq!(model.commits[2].tags, ["merge-a", "merge-b"]);
            assert_eq!(model.commits[3].parents, ["R", "M"]);
            assert_eq!(model.commits[3].commit_type, 4);
            assert_eq!(model.commits[3].tags, ["pick-a", "pick-b"]);
            #[cfg(feature = "editor-facts")]
            {
                use crate::EditorSemanticRole::{Entity, Payload, Reference};

                let captured = capture(&source);
                assert!(captured.model.is_ok());
                let facts = captured.editor_facts.unwrap();
                assert!(facts.diagnostics.is_empty());
                let bindings = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.role.contributes_references())
                    .map(|symbol| (symbol.name.as_str(), symbol.role))
                    .collect::<Vec<_>>();
                assert_eq!(
                    bindings,
                    [
                        ("R", Entity),
                        ("release", Entity),
                        ("main", Reference),
                        ("feature", Entity),
                        ("F", Entity),
                        ("main", Reference),
                        ("feature", Reference),
                        ("M", Entity),
                        ("release", Reference),
                        ("M", Reference),
                        ("R", Reference),
                    ]
                );
                for name in ["stale-root", "stale-merge", "stale-source", "stale-parent"] {
                    let symbols = facts
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.name == name)
                        .collect::<Vec<_>>();
                    assert_eq!(symbols.len(), 1);
                    assert_eq!(symbols[0].role, Payload);
                    assert_eq!(symbols[0].rename_policy, EditorRenamePolicy::None);
                    assert!(!symbols[0].role.contributes_completion());
                    assert!(!symbols[0].role.contributes_outline());
                }
                for symbol in &facts.symbols {
                    assert_eq!(
                        &source[symbol.selection.start..symbol.selection.end],
                        symbol.name
                    );
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gitgraph_scalar_binding_spans_handle_equal_empty_and_recovered_winners() {
        for newline in ["\n", "\r\n"] {
            for (winner, trailing) in [("same", ""), ("", ""), ("same", " invalid")] {
                let statement = format!("commit id:\"same\" id:\"{winner}\"{trailing}");
                let source =
                    format!("gitGraph{newline}{statement}{newline}commit id:\"later\"{newline}");
                let captured = capture(&source);
                assert_eq!(captured.model.is_ok(), trailing.is_empty());
                let facts = captured.editor_facts.unwrap();
                let ids = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.detail.as_deref() == Some("gitGraph commit id"))
                    .collect::<Vec<_>>();
                assert_eq!(ids[0].role, crate::EditorSemanticRole::Payload);
                assert_eq!(ids[0].rename_policy, EditorRenamePolicy::None);
                let bindings = ids
                    .iter()
                    .filter(|symbol| symbol.role == crate::EditorSemanticRole::Entity)
                    .collect::<Vec<_>>();
                if winner.is_empty() {
                    let model = captured.model.unwrap();
                    assert!(model.commits[0].id.starts_with("0-"));
                    assert_eq!(bindings.len(), 1);
                } else {
                    assert_eq!(bindings.len(), 2);
                    let start = source.rfind("id:\"same\"").unwrap() + "id:\"".len();
                    assert_eq!(
                        bindings[0].selection,
                        SourceSpan::new(start, start + winner.len())
                    );
                    assert_eq!(
                        bindings[0].rename_policy,
                        EditorRenamePolicy::GitGraphReference
                    );
                }
                assert_eq!(bindings.last().unwrap().name, "later");
                assert_eq!(facts.diagnostics.is_empty(), trailing.is_empty());
                if !trailing.is_empty() {
                    assert_gitgraph_diagnostic(
                        &source,
                        &statement,
                        "unexpected commit argument: invalid",
                    );
                }
            }
        }
    }

    #[test]
    fn gitgraph_quote_boundaries_reject_keyword_prefixes_and_missing_commands() {
        let cases = [
            (
                "commit02\"hello\"",
                "commit02",
                "Unknown statement: commit02",
            ),
            (
                "branch02\"topic\"",
                "branch02",
                "Unknown statement: branch02",
            ),
            (
                "checkout02\"main\"",
                "checkout02",
                "Unknown statement: checkout02",
            ),
            (
                "switch02\"main\"",
                "switch02",
                "Unknown statement: switch02",
            ),
            ("merge02\"topic\"", "merge02", "Unknown statement: merge02"),
            (
                "commit type:NORMALsuffix\"hello\"",
                "commit type:NORMALsuffix\"hello\"",
                "Unknown commit type: NORMALsuffix",
            ),
            (
                "commit type:\"NORMAL\"",
                "commit type:\"NORMAL\"",
                "expected commit type",
            ),
            ("\"hello\"", "\"hello\"", "expected gitGraph command"),
            (
                "merge\"topic\" type:NORMAL\"hello\"",
                "merge\"topic\" type:NORMAL\"hello\"",
                "unexpected merge argument: \"hello\"",
            ),
            (
                "branch\"topic\" order:1\"extra\"",
                "branch\"topic\" order:1\"extra\"",
                "unexpected branch argument: \"extra\"",
            ),
        ];
        for newline in ["\n", "\r\n"] {
            for (statement, span_text, message) in cases {
                let source = format!("gitGraph{newline}  {statement} %% hidden{newline}");
                assert_gitgraph_diagnostic(&source, span_text, message);
            }
        }
    }

    #[test]
    fn gitgraph_merge_errors_follow_source_validation_order() {
        let cases = [
            (
                "commit id:\"R\"\n  merge main\n",
                "merge main",
                "Cannot merge branch 'main' into itself.",
            ),
            (
                "commit id:\"R\"\nbranch topic\ncommit id:\"F\"\n  merge topic\n",
                "merge topic",
                "Cannot merge branch 'topic' into itself.",
            ),
            (
                "commit id:\"R\"\nbranch topic\n  merge topic\n",
                "merge topic",
                "Incorrect usage of \"merge\". Cannot merge a branch to itself",
            ),
            (
                "  merge main\n",
                "merge main",
                "Incorrect usage of \"merge\". Cannot merge a branch to itself",
            ),
            (
                "commit id:\"R\"\nbranch topic\ncheckout main\n  merge topic\n",
                "merge topic",
                "Incorrect usage of \"merge\". Both branches have same head",
            ),
            (
                "branch empty\ncheckout main\ncommit id:\"R\"\n  merge empty\n",
                "merge empty",
                "Incorrect usage of \"merge\". Branch to be merged (empty) has no commits",
            ),
            (
                "  merge missing\n",
                "merge missing",
                "Incorrect usage of \"merge\". Current branch (main)has no commits",
            ),
            (
                "commit id:\"R\"\n  merge missing\n",
                "merge missing",
                "Incorrect usage of \"merge\". Branch to be merged (missing) does not exist",
            ),
        ];
        for newline in ["\n", "\r\n"] {
            for (body, statement, message) in cases {
                let source = format!("gitGraph\n{body}").replace('\n', newline);
                assert_gitgraph_diagnostic(&source, statement, message);
            }
        }
    }

    #[test]
    fn gitgraph_cherry_pick_preserves_tags_parents_and_errors() {
        let prefix = concat!(
            "gitGraph\ncommit id:\"R\"\nbranch release\ncheckout main\n",
            "branch feature\ncommit id:\"F\" msg:\"feature message\"\n",
            "checkout main\nmerge feature id:\"M\"\ncheckout release\n",
        );
        for newline in ["\n", "\r\n"] {
            let picks = concat!(
                "cherry-pick id:\"F\"\n",
                "cherry-pick id:\"M\" parent:\"R\"\n",
                "cherry-pick id:\"M\" parent:\"F\" tag:\"\"\n",
                "cherry-pick id:\"M\" parent:\"R\" tag:\"first\" tag:\"last\"\n",
            );
            let source = format!("{prefix}{picks}").replace('\n', newline);
            let model = canonical(&source).unwrap();
            assert_eq!(model.commits.len(), 7);
            for (index, source_id) in [(3, "F"), (4, "M"), (5, "M"), (6, "M")] {
                let commit = &model.commits[index];
                let head = &model.commits[if index == 3 { 0 } else { index - 1 }];
                assert_eq!(commit.parents, [head.id.as_str(), source_id]);
                assert_eq!(commit.seq, index as i64);
                assert_eq!(commit.branch, "release");
                assert_eq!(commit.commit_type, 4);
            }
            assert_eq!(
                model.commits[3].message,
                "cherry-picked feature message into release"
            );
            assert_eq!(model.commits[3].tags, ["cherry-pick:F"]);
            assert_eq!(model.commits[4].tags, ["cherry-pick:M|parent:R"]);
            assert!(model.commits[5].tags.is_empty());
            assert_eq!(model.commits[6].tags, ["first", "last"]);

            let cases = [
                (
                    "gitGraph\ncherry-pick id:\"missing\" parent:\"wrong\"\n".to_string(),
                    "cherry-pick id:\"missing\" parent:\"wrong\"",
                    "Incorrect usage of \"cherryPick\". Source commit id should exist and provided",
                ),
                (
                    "gitGraph\ncommit id:\"R\"\ncherry-pick id:\"R\"\n".to_string(),
                    "cherry-pick id:\"R\"",
                    "Incorrect usage of \"cherryPick\". Source commit is already on current branch",
                ),
                (
                    "gitGraph\nbranch feature\ncommit id:\"F\"\ncheckout main\ncherry-pick id:\"F\"\n".to_string(),
                    "cherry-pick id:\"F\"",
                    "Incorrect usage of \"cherry-pick\". Current branch (main)has no commits",
                ),
                (
                    format!("{prefix}cherry-pick id:\"M\"\n"),
                    "cherry-pick id:\"M\"",
                    "Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.",
                ),
                (
                    format!("{prefix}cherry-pick id:\"M\" parent:\"wrong\"\n"),
                    "cherry-pick id:\"M\" parent:\"wrong\"",
                    "Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.",
                ),
            ];
            for (source, statement, message) in cases {
                assert_gitgraph_diagnostic(&source.replace('\n', newline), statement, message);
            }
        }
    }

    fn reset_construction_counts() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("gitGraph");
        GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.set(0);
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    fn assert_one_construction_without_editor() {
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("gitGraph"),
            1
        );
        assert_eq!(GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.get(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_keeps_database_failure_without_command_facts() {
        for source in [
            "gitGraph\ncommit id:\"root\"\n",
            "gitGraph\ncommit id:\"root\"\ncheckout missing\n",
        ] {
            let metadata = SemanticMetadata {
                diagram_type: "gitGraph".to_string(),
                config: MermaidConfig::empty_object(),
                semantic_config: MermaidConfig::from_value(json!({"gitGraph": {"seed": 1}})),
                title: None,
            };
            let capture = |demand| {
                parse_git_graph_semantic_model_controlled(
                    source,
                    &metadata,
                    demand,
                    &crate::OperationControl::new(),
                )
                .unwrap()
            };
            let full = capture(EditorFactsDemand::Capture);
            let full_facts = full.editor_facts.unwrap();
            assert!(!full_facts.symbols.is_empty());
            reset_construction_counts();
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
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("gitGraph"),
                1
            );
            assert_eq!(GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.get(), 1);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn canonical_gitgraph_capture_retains_facts_on_success_and_database_failure() {
        for (source, valid) in [
            ("gitGraph\ncommit id:\"root\"\n", true),
            ("gitGraph\ncommit id:\"root\"\ncheckout missing\n", false),
        ] {
            let metadata = SemanticMetadata {
                diagram_type: "gitGraph".to_string(),
                config: MermaidConfig::empty_object(),
                semantic_config: MermaidConfig::from_value(json!({"gitGraph": {"seed": 1}})),
                title: None,
            };
            reset_construction_counts();
            let parsed = parse_git_graph_semantic_model_controlled(
                source,
                &metadata,
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass GitGraph facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("gitGraph"),
                1
            );
            assert_eq!(GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.get(), 1);
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
    fn canonical_gitgraph_keeps_authored_semantics_and_normalizes_common_fields() {
        reset_construction_counts();
        let source = concat!(
            "gitGraph TB:\n",
            "title <script>alert(1)</script><b>Canonical title</b>\n",
            "accTitle: Git history\n",
            "accDescr: A feature merge\n",
            "commit id:\"C0\" msg:\"Root\" tag:\"v1\"\n",
            "branch feature order:2\n",
            "commit id:\"F1\" type:HIGHLIGHT\n",
            "checkout main\n",
            "merge feature id:\"M1\" type:REVERSE tag:\"merged\"\n",
        );
        let model = canonical(source).unwrap();
        assert_eq!(model.diagram_type, "gitGraph");
        assert_eq!(model.direction, "TB");
        assert_eq!(model.title.as_deref(), Some("<b>Canonical title</b>"));
        assert_eq!(model.acc_title.as_deref(), Some("Git history"));
        assert_eq!(model.acc_descr.as_deref(), Some("A feature merge"));
        assert_eq!(
            model
                .branches
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            ["main", "feature"]
        );
        assert_eq!(model.current_branch, "main");
        assert_eq!(model.commits.len(), 3);
        assert_eq!(model.commits[0].tags, ["v1"]);
        assert_eq!(model.commits[1].commit_type, 2);
        assert_eq!(model.commits[2].parents, ["C0", "F1"]);
        assert_eq!(model.commits[2].custom_type, Some(1));
        assert_eq!(model.commits[2].custom_id, Some(true));
        assert_one_construction_without_editor();

        #[cfg(feature = "presentation")]
        {
            let parsed = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::GitGraph(full) = parsed.model() else {
                panic!("expected GitGraph render model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(full).unwrap()
            );
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn runtime_gitgraph_generated_ids_use_replay_ordinals_for_commit_merge_and_cherry_pick() {
        let context = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_seed(23)
            .begin_operation()
            .unwrap();
        let generated = [1, 3, 4].map(|ordinal| {
            format!(
                "{ordinal}-{}",
                context.derive_hex("git-graph.commit-id", ordinal, 7)
            )
        });
        let source = format!(
            "gitGraph\ncommit id:\"root\"\nbranch feature\ncommit\ncheckout main\ncommit id:\"main-head\"\nmerge feature\ncherry-pick id:\"{}\"\n",
            generated[0]
        );
        reset_construction_counts();
        let parsed = crate::SemanticParser::new()
            .with_operation_context(context)
            .parse_diagram_sync(&source)
            .unwrap();
        let crate::SemanticDiagramModel::GitGraph(model) = parsed.model else {
            panic!("expected GitGraph semantics");
        };
        assert_eq!(
            model
                .commits
                .iter()
                .map(|commit| commit.id.as_str())
                .collect::<Vec<_>>(),
            [
                "root",
                generated[0].as_str(),
                "main-head",
                generated[1].as_str(),
                generated[2].as_str()
            ]
        );
        assert_eq!(
            model.commits[4].parents,
            [generated[1].as_str(), generated[0].as_str()]
        );
        assert_one_construction_without_editor();
    }

    #[test]
    fn seeded_canonical_gitgraph_constructs_once_and_can_reference_its_generated_id() {
        reset_construction_counts();
        let source = concat!(
            "gitGraph\n",
            "commit id:\"ROOT\"\n",
            "branch feature\n",
            "commit\n",
            "checkout main\n",
            "cherry-pick id:\"1-4ab40cd\"\n",
        );
        let model = canonical(source).unwrap();
        assert_eq!(
            model
                .commits
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            ["ROOT", "1-4ab40cd", "2-a5b722b"]
        );
        assert_eq!(model.commits[2].parents, ["ROOT", "1-4ab40cd"]);
        assert_eq!(model.commits[2].tags, ["cherry-pick:1-4ab40cd"]);
        assert_one_construction_without_editor();
    }

    #[test]
    fn seeded_canonical_gitgraph_uses_one_identity_domain_for_custom_merge_validation() {
        reset_construction_counts();
        let source = concat!(
            "gitGraph\n",
            "commit\n",
            "branch feature\n",
            "commit id:\"F1\"\n",
            "checkout main\n",
            "merge feature id:\"0-5b722bd\"\n",
        );
        let model = canonical(source).unwrap();
        assert_eq!(
            model
                .commits
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            ["0-4ab40cd", "F1", "0-5b722bd"]
        );
        assert_eq!(model.commits[2].parents, ["0-4ab40cd", "F1"]);
        assert_one_construction_without_editor();
    }

    #[test]
    fn seeded_canonical_gitgraph_rejects_its_actual_generated_id_as_custom_merge_id() {
        reset_construction_counts();
        let source = concat!(
            "gitGraph\n",
            "commit\n",
            "branch feature\n",
            "commit id:\"F1\"\n",
            "checkout main\n",
            "merge feature id:\"0-4ab40cd\"\n",
        );
        let error = canonical(source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Commit with id:0-4ab40cd already exists")
        );
        assert_one_construction_without_editor();
    }

    #[test]
    fn seeded_canonical_gitgraph_preserves_generated_id_collision_warnings() {
        reset_construction_counts();
        let source = concat!(
            "\u{feff}---\r\ntitle: History\r\n---\r\ngitGraph\r\ncommit\r\n",
            "commit id:\"0-4ab40cd\" msg:\"Replacement\"\r\n",
        );
        let model = canonical(source).unwrap();
        assert_eq!(model.commits.len(), 1);
        assert_eq!(model.commits[0].id, "0-4ab40cd");
        assert_eq!(model.commits[0].seq, 1);
        assert_eq!(model.commits[0].message, "Replacement");
        assert_eq!(model.commits[0].parents, ["0-4ab40cd"]);
        assert_eq!(model.warning_facts.len(), 1);
        let span = model.warning_facts[0].span.expect("original warning span");
        assert_eq!(
            &source[span.start..span.end],
            "commit id:\"0-4ab40cd\" msg:\"Replacement\""
        );
        assert_one_construction_without_editor();
    }

    #[test]
    fn canonical_gitgraph_preserves_exact_errors_and_cooperative_cancellation() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let source = "gitGraph\r\ncommit id:\"C0\"\r\n  checkout missing  \r\n";
        let error = canonical(source).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected diagram diagnostic");
        };
        let start = source.find("checkout missing").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "checkout missing".len()))
        );
        assert!(diagnostic.message().contains("Trying to checkout branch"));
        #[cfg(feature = "compat-json")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_sync(source, crate::ParseOptions::strict())
                .unwrap_err();
            assert_eq!(error.to_string(), full.to_string());
        }
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        let mut source = String::from("gitGraph\ncommit id:\"root\"");
        for index in 0..512 {
            source.push_str(&format!(" tag:\"tag-{index}\""));
        }
        source.push('\n');
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(20);
        assert!(matches!(
            crate::SemanticParser::new().parse_diagram_controlled_sync(&source, &control),
            Err(crate::OperationCancelled { .. })
        ));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn combined_gitgraph_capture_uses_one_seeded_construction_and_keeps_parser_spans() {
        reset_construction_counts();
        let source = "gitGraph\r\ncommit id:\"ROOT\"\r\nbranch feature\r\ncommit id:\"F1\"\r\ncheckout main\r\nmerge feature id:\"M1\"\r\n";
        let meta = SourcePresentationMetadata {
            diagram_type: "gitGraph".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::from_value(
                json!({ "gitGraph": { "seed": 1 } }),
            ),
            title: None,
        };
        let (json, facts) = crate::family::test_support::into_projected_result(
            parse_git_graph_json_and_editor_facts(source, &meta, &crate::OperationControl::new()),
            &meta,
        )
        .unwrap();
        assert_eq!(json["commits"].as_array().unwrap().len(), 3);
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Complete
        );
        let start = source.find("F1").unwrap();
        assert!(
            facts.symbols.iter().any(|symbol| symbol.name == "F1"
                && symbol.selection == SourceSpan::new(start, start + 2))
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("gitGraph"),
            1
        );
        assert_eq!(GITGRAPH_SEMANTIC_CONSTRUCTION_COUNT.get(), 1);
        let counts = crate::editor::test_work::read();
        assert_eq!(counts.collections, 1);
        assert!(counts.symbols > 0);
    }
}
