use crate::Error;
#[cfg(test)]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol,
};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
use merman_core_structured::venn::parse_venn_source_controlled;
#[cfg(feature = "editor-facts")]
use merman_core_structured::venn::{VennFieldKind, VennIssueKind, VennObservation};
pub(crate) use merman_model::families::venn::VennDiagramModel;
#[cfg(all(test, feature = "compat-json"))]
use merman_model::families::venn::{VennSubsetModel, VennTextNodeModel};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;

pub(crate) fn sanitize_common_db_fields(
    model: &mut VennDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct VennSemanticSource {
    model: VennDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct VennParseOutcome {
    source: VennSemanticSource,
    first_error: Option<Error>,
}

#[cfg(feature = "presentation")]
impl VennParseOutcome {
    fn into_strict_source(self) -> Result<VennSemanticSource> {
        match self.first_error {
            Some(error) => Err(error),
            None => Ok(self.source),
        }
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_venn(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = parse_venn_semantic_source(code, meta)?;
    render_model_to_compat_json(&source.model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_venn_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let VennParseOutcome {
        source,
        first_error,
    } = construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let construction = match first_error {
        Some(error) => Err(crate::family::CombinedSemanticFailure::new(
            error,
            source.editor_facts,
        )),
        None => Ok(source),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |source| {
            (
                render_model_to_compat_json(&source.model, meta),
                source.editor_facts.unwrap_or_default(),
            )
        },
        crate::family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_venn_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<VennDiagramModel> {
    Ok(parse_venn_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_venn_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<VennDiagramModel>> {
    let outcome = construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    Ok(outcome.into_strict_source().map(|source| source.model))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &VennDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::venn::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_venn_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<VennSemanticSource> {
    construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .into_strict_source()
}

pub(crate) fn parse_venn_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<VennDiagramModel>> {
    let VennParseOutcome {
        source,
        first_error,
    } = construct_venn_parse_outcome_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = source.editor_facts;
    let model = match first_error {
        Some(error) => Err(error),
        None => {
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            Ok(model)
        }
    };
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn construct_venn_parse_outcome_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<VennParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("venn");
    #[cfg(all(test, feature = "editor-facts"))]
    if editor_demand.captures_rich() {
        crate::editor::test_work::record_collection();
    }

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    #[cfg(feature = "editor-facts")]
    let mut observe = editor_facts.as_mut().map(|facts| {
        move |observation: VennObservation<'_>| project_venn_observation(facts, observation)
    });
    let outcome = parse_venn_source_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        #[cfg(feature = "editor-facts")]
        observe
            .as_mut()
            .map(|observe| observe as &mut dyn for<'a> FnMut(VennObservation<'a>)),
        control,
    )?;
    Ok(VennParseOutcome {
        source: VennSemanticSource {
            model: outcome.model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        },
        first_error: outcome.first_issue.map(|issue| {
            Error::diagram_parse_exact(diagram_type, issue.kind.message(), issue.span)
        }),
    })
}

#[cfg(feature = "editor-facts")]
fn project_venn_observation(facts: &mut EditorSemanticFacts, observation: VennObservation<'_>) {
    match observation {
        VennObservation::Directive(prefix) => facts.push_directive_prefix(prefix),
        VennObservation::IdList(span) => facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::IdList,
            span,
        )),
        VennObservation::Recovery(issue) => {
            let message = match &issue.kind {
                VennIssueKind::Statement(message) => {
                    format!("venn parser recovered after parse error: {message}")
                }
                _ => issue.kind.message().to_owned(),
            };
            facts.mark_recovered_from_parse_error(message, Some(issue.span));
        }
        VennObservation::Field {
            kind,
            text,
            span,
            selection,
        } => {
            use EditorSemanticRole::{Entity, Payload, Reference};

            let (detail, role) = match kind {
                VennFieldKind::Header => ("venn header", Payload),
                VennFieldKind::Title => ("venn title", Payload),
                VennFieldKind::Set => ("venn set", Entity),
                VennFieldKind::SetLabel => ("venn set label", Payload),
                VennFieldKind::Size => ("venn size", Payload),
                VennFieldKind::UnionSet => ("venn union set", Reference),
                VennFieldKind::UnionLabel => ("venn union label", Payload),
                VennFieldKind::TextSet => ("venn text set", Reference),
                VennFieldKind::TextNode => ("venn text node", Entity),
                VennFieldKind::TextLabel => ("venn text label", Payload),
                VennFieldKind::StyleTarget => ("venn style target", Reference),
                VennFieldKind::StyleValue => ("venn style value", Payload),
            };
            let expected = match role {
                Entity => Some(EditorExpectedSyntaxKind::NodeIdentifier),
                Payload if !text.is_empty() => Some(EditorExpectedSyntaxKind::Payload),
                _ => None,
            };
            if let Some(expected) = expected {
                facts.push_expected_syntax(EditorExpectedSyntax::new(expected, span));
            }
            if !text.is_empty() {
                let kind = if role == Payload {
                    EditorSemanticKind::String
                } else {
                    EditorSemanticKind::Namespace
                };
                facts.push_symbol(EditorSemanticSymbol::with_role(
                    text,
                    Some(detail.to_owned()),
                    kind,
                    role,
                    span,
                    selection,
                ));
            }
        }
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{
        MermaidConfig, MermaidEngine, ParseOptions, RenderSemanticModel, SourcePresentationMetadata,
    };

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "venn".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    fn parse(input: &str) -> VennDiagramModel {
        parse_venn_model_for_render(input, &meta()).unwrap()
    }

    #[test]
    fn venn_adjacent_sets_share_model_and_editor_semantics() {
        for (body, first_size) in [("set A:1set B", 1.0), ("set A set B", 10.0)] {
            let source = format!("venn-beta\n{body}\n");
            let model = parse(&source);
            assert_eq!(model.subsets.len(), 2);
            assert_eq!(model.subsets[0].sets, ["A"]);
            assert_eq!(model.subsets[0].size, first_size);
            assert_eq!(model.subsets[1].sets, ["B"]);
            assert_eq!(model.subsets[1].size, 10.0);
            let compat = parse_venn(&source, &meta()).unwrap();
            assert_eq!(compat["subsets"], json!(model.subsets));

            #[cfg(feature = "editor-facts")]
            {
                let (combined, facts) =
                    crate::family::test_support::into_result(parse_venn_json_and_editor_facts(
                        &source,
                        &meta(),
                        &crate::OperationControl::new(),
                    ))
                    .unwrap();
                assert_eq!(combined, compat);
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                assert!(facts.diagnostics.is_empty());
                for name in ["A", "B"] {
                    let start = source.find(name).unwrap();
                    let symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| {
                            symbol.name == name && symbol.detail.as_deref() == Some("venn set")
                        })
                        .unwrap();
                    assert_eq!(symbol.span, SourceSpan::new(start, start + 1));
                    assert_eq!(symbol.selection, symbol.span);
                }
            }
        }
    }

    #[test]
    fn venn_adjacent_union_text_and_styles_follow_grammar() {
        let source = concat!(
            "venn-beta\n",
            "set A[set union text style]set B[Beta]:2",
            "union A,B[Both]:.5text A note[set union text style]",
            "style A fill:rgb(1, 2, 3), stroke:red blue ",
            "style B fill:\"set A union text style\"set C\n",
        );
        let model = parse(source);
        assert_eq!(model.subsets.len(), 4);
        assert_eq!(
            model.subsets[0].label.as_deref(),
            Some("set union text style")
        );
        assert_eq!(model.subsets[1].size, 2.0);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
        assert_eq!(model.subsets[2].label.as_deref(), Some("Both"));
        assert_eq!(model.subsets[2].size, 0.5);
        assert_eq!(model.subsets[3].sets, ["C"]);
        assert_eq!(model.text_nodes.len(), 1);
        assert_eq!(model.text_nodes[0].sets, ["A"]);
        assert_eq!(model.text_nodes[0].id, "note");
        assert_eq!(
            model.text_nodes[0].label.as_deref(),
            Some("set union text style")
        );
        assert_eq!(model.style_entries.len(), 2);
        assert_eq!(model.style_entries[0].styles["fill"], "rgb(1, 2, 3)");
        assert_eq!(model.style_entries[0].styles["stroke"], "red blue");
        assert_eq!(
            model.style_entries[1].styles["fill"],
            "set A union text style"
        );
    }

    #[test]
    fn venn_adjacent_statements_preserve_bol_indent_mode() {
        let model = parse(concat!(
            "venn-beta\nset A:1set B\n",
            "  text\"one\" text A two\n",
            "  %% comment\n\n",
            "  text 1set C\n",
            "  text three\n",
            "  style A fill:red\n",
            "  text A explicit\n",
        ));
        let texts = model
            .text_nodes
            .iter()
            .map(|node| {
                (
                    node.sets.iter().map(String::as_str).collect::<Vec<_>>(),
                    node.id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            texts,
            vec![
                (vec!["B"], "one"),
                (vec!["A"], "two"),
                (vec!["B"], "1"),
                (vec!["C"], "three"),
                (vec!["A"], "explicit"),
            ]
        );
    }

    #[test]
    fn venn_adjacent_multiline_payloads_keep_keyword_text() {
        let source = concat!(
            "venn-beta\n",
            "set A[\"first\nset second\"]",
            "text A \"text\nstyle payload\"",
            "style A fill:\"union\nset payload\"set B\n",
        );
        let model = parse(source);
        assert_eq!(model.subsets.len(), 2);
        assert_eq!(model.subsets[0].label.as_deref(), Some("first\nset second"));
        assert_eq!(model.text_nodes[0].id, "text\nstyle payload");
        assert_eq!(model.style_entries[0].styles["fill"], "union\nset payload");
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_venn_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for (raw, name, detail, inset) in [
                (
                    "[\"first\nset second\"]",
                    "first\nset second",
                    "venn set label",
                    2,
                ),
                (
                    "\"text\nstyle payload\"",
                    "text\nstyle payload",
                    "venn text node",
                    1,
                ),
                (
                    "\"union\nset payload\"",
                    "union\nset payload",
                    "venn style value",
                    1,
                ),
            ] {
                let start = source.find(raw).unwrap();
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                    .unwrap();
                assert_eq!(symbol.span, SourceSpan::new(start, start + raw.len()));
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(start + inset, start + raw.len() - inset)
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_adjacent_public_ranges_preserve_bom_unicode_and_line_endings() {
        let engine = MermaidEngine::new();
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "\u{feff}%% préface{newline}venn-beta{newline}set \"集合\"[\"标签\"]:1set B{newline}union \"集合\",B[\"共同\"]:.5text \"集合\" \"注记\"[\"正文\"]style B fill:\"set 关键词\"set C{newline}"
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Venn(model) = parsed.model() else {
                panic!("expected Venn model");
            };
            assert_eq!(model.subsets.len(), 4);
            assert_eq!(model.subsets[0].sets, ["集合"]);
            assert_eq!(model.subsets[2].sets, ["B", "集合"]);
            assert_eq!(model.text_nodes[0].id, "注记");
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("venn", &source)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (raw, name, detail, inset) in [
                ("venn-beta", "venn-beta", "venn header", 0),
                ("\"集合\"", "集合", "venn set", 1),
                ("[\"标签\"]", "标签", "venn set label", 2),
                ("[\"共同\"]", "共同", "venn union label", 2),
                ("\"注记\"", "注记", "venn text node", 1),
                ("[\"正文\"]", "正文", "venn text label", 2),
                ("\"set 关键词\"", "set 关键词", "venn style value", 1),
            ] {
                let start = source.find(raw).unwrap();
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                    .unwrap_or_else(|| panic!("missing {detail}: {name}"));
                assert_eq!(symbol.span, SourceSpan::new(start, start + raw.len()));
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(start + inset, start + raw.len() - inset)
                );
            }
            for name in ["B", "C"] {
                let start = source.find(&format!("set {name}")).unwrap() + 4;
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == name && symbol.detail.as_deref() == Some("venn set")
                    })
                    .unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + 1));
            }
        }
    }

    #[test]
    fn venn_adjacent_recovery_preserves_completed_and_following_statements() {
        for newline in ["\n", "\r\n"] {
            for (body, invalid) in [
                ("set A:1union A,set C", "union A,"),
                ("set A:1set B invalid set C", "set B invalid"),
            ] {
                let source = format!("venn-beta{newline}{body}{newline}set D{newline}");
                let start = source.find(invalid).unwrap();
                let expected = SourceSpan::new(start, start + invalid.len());
                let Error::DiagramParse { diagnostic, .. } =
                    parse_venn(&source, &meta()).unwrap_err()
                else {
                    panic!("expected Venn parse error");
                };
                assert_eq!(diagnostic.span(), Some(expected));
                #[cfg(feature = "editor-facts")]
                {
                    let facts = crate::family::test_support::editor_facts(
                        parse_venn_json_and_editor_facts,
                        &source,
                        &meta(),
                    );
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                    assert!(
                        facts
                            .diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.span == Some(expected))
                    );
                    let sets = facts
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.detail.as_deref() == Some("venn set"))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        sets.iter()
                            .map(|symbol| symbol.name.as_str())
                            .collect::<Vec<_>>(),
                        ["A", "C", "D"]
                    );
                    for symbol in sets {
                        let offset = source.find(&format!("set {}", symbol.name)).unwrap() + 4;
                        assert_eq!(symbol.selection, SourceSpan::new(offset, offset + 1));
                    }
                }
            }
        }
    }

    #[test]
    fn venn_adjacent_tokens_do_not_relax_invalid_follow_rules() {
        for body in [
            "set A:1setB",
            "set A\ntext A 1[label]set B",
            "set A\nstyle A fill:\"a\" red set B",
            "set A\nstyle A fill:red \"a\" set B",
        ] {
            assert!(
                parse_venn(&format!("venn-beta\n{body}\n"), &meta()).is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn venn_adjacent_comments_preserve_utf16_prefix_rules_and_byte_ranges() {
        for prefix in ["é", "中", " "] {
            let source = format!("venn-beta\n{prefix}%% comment\nset A\n");
            assert_eq!(parse(&source).subsets[0].sets, ["A"]);
        }
        for newline in ["\n", "\r\n"] {
            let source = format!("venn-beta{newline}😀%% comment{newline}set A{newline}");
            let start = source.find('😀').unwrap();
            let expected = SourceSpan::new(start, start + '😀'.len_utf8());
            let Error::DiagramParse { diagnostic, .. } = parse_venn(&source, &meta()).unwrap_err()
            else {
                panic!("expected Venn lexical parse error");
            };
            assert_eq!(diagnostic.span(), Some(expected));
            #[cfg(feature = "editor-facts")]
            {
                let original = format!("\u{feff}{source}");
                let facts = MermaidEngine::new()
                    .parse_editor_semantic_facts_with_type_sync("venn", &original)
                    .unwrap()
                    .unwrap();
                let original_start = original.find('😀').unwrap();
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.diagnostics.iter().any(|diagnostic| {
                    diagnostic.span
                        == Some(SourceSpan::new(
                            original_start,
                            original_start + '😀'.len_utf8(),
                        ))
                }));
                let entity = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == "A" && symbol.detail.as_deref() == Some("venn set")
                    })
                    .unwrap();
                let entity_start = original.find("set A").unwrap() + 4;
                assert_eq!(
                    entity.selection,
                    SourceSpan::new(entity_start, entity_start + 1)
                );
            }
        }
    }

    #[test]
    fn parses_simple_sets_title_and_default_union_size() {
        let model = parse(
            r#"venn-beta
          title foo bar
          set A
          set B
          union A,B
      "#,
        );

        assert_eq!(model.title.as_deref(), Some("foo bar"));
        assert_eq!(
            model.subsets,
            vec![
                VennSubsetModel {
                    sets: vec!["A".to_string()],
                    size: 10.0,
                    label: None,
                },
                VennSubsetModel {
                    sets: vec!["B".to_string()],
                    size: 10.0,
                    label: None,
                },
                VennSubsetModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    size: 2.5,
                    label: None,
                },
            ]
        );
    }

    #[test]
    fn title_matches_the_upstream_lexer_boundary() {
        assert_eq!(
            parse("venn-beta\ntitle Valid title\n").title.as_deref(),
            Some("Valid title")
        );

        for source in [
            "venn-beta\ntitle: Invalid\n",
            "venn-beta\ntitle Invalid; suffix\n",
            "venn-beta\ntitle Invalid # suffix\n",
        ] {
            let error = parse_venn_model_for_render(source, &meta())
                .expect_err("the pinned Venn lexer rejects this title form");
            assert!(error.to_string().contains("title"), "{error}");
        }
    }

    #[test]
    fn parses_bracket_labels_and_size_suffixes() {
        let model = parse(
            r#"venn-beta
          title foo bar
          set A["Alpha"]:20
          set B[Beta]:12
          set C["Gamma"]:30
          union A,B["AB"]:5.3
          union C,A,B:1
      "#,
        );

        assert_eq!(model.subsets[0].label.as_deref(), Some("Alpha"));
        assert_eq!(model.subsets[0].size, 20.0);
        assert_eq!(model.subsets[1].label.as_deref(), Some("Beta"));
        assert_eq!(model.subsets[1].size, 12.0);
        assert_eq!(model.subsets[3].sets, ["A", "B"]);
        assert_eq!(model.subsets[3].label.as_deref(), Some("AB"));
        assert_eq!(model.subsets[3].size, 5.3);
        assert_eq!(model.subsets[4].sets, ["A", "B", "C"]);
    }

    #[test]
    fn parses_text_nodes_with_explicit_and_indented_forms() {
        let model = parse(
            r#"venn-beta
          set A["Frontend"]
            text A1["React"]
            text A2
          set B["Backend"]
            text B1
          union A,B["APIs"]
            text AB1["OpenAPI"]
      "#,
        );

        assert_eq!(
            model.text_nodes,
            vec![
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "A1".to_string(),
                    label: Some("React".to_string()),
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "A2".to_string(),
                    label: None,
                },
                VennTextNodeModel {
                    sets: vec!["B".to_string()],
                    id: "B1".to_string(),
                    label: None,
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    id: "AB1".to_string(),
                    label: Some("OpenAPI".to_string()),
                },
            ]
        );
    }

    #[test]
    fn parses_explicit_text_statement_and_numeric_text_id() {
        let model = parse(
            r#"venn-beta
set A
set B
union A,B
text A alpha["Alpha note"]
text A,B 42
      "#,
        );

        assert_eq!(
            model.text_nodes,
            vec![
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "alpha".to_string(),
                    label: Some("Alpha note".to_string()),
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    id: "42".to_string(),
                    label: None,
                },
            ]
        );
    }

    #[test]
    fn parses_style_entries() {
        let model = parse(
            r#"venn-beta
          set A
          set B
          union A,B
          style A fill:#ff6b6b, color:#333
          style A,B fill:rgb(255, 0, 128)
          style B fill:rgba(255, 0, 128, 0.5)
      "#,
        );

        assert_eq!(model.style_entries[0].targets, ["A"]);
        assert_eq!(
            model.style_entries[0]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("#ff6b6b")
        );
        assert_eq!(
            model.style_entries[0]
                .styles
                .get("color")
                .map(String::as_str),
            Some("#333")
        );
        assert_eq!(model.style_entries[1].targets, ["A", "B"]);
        assert_eq!(
            model.style_entries[1]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("rgb(255, 0, 128)")
        );
        assert_eq!(
            model.style_entries[2]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("rgba(255, 0, 128, 0.5)")
        );
    }

    #[test]
    fn rejects_invalid_set_and_union_shapes() {
        let err = parse_venn_model_for_render("venn-beta\nset A,B\n", &meta()).unwrap_err();
        assert!(err.to_string().contains("set requires single identifier"));

        let err = parse_venn_model_for_render("venn-beta\nunion A\n", &meta()).unwrap_err();
        assert!(
            err.to_string()
                .contains("union requires multiple identifiers")
        );

        let err = parse_venn_model_for_render("venn-beta\nset Foo\nunion Foo,Buz\n", &meta())
            .unwrap_err();
        assert!(err.to_string().contains("unknown set identifier"));
    }

    #[test]
    fn parses_quoted_identifiers_and_sorts_union_sets() {
        let model = parse(
            r#"venn-beta
        set "Foo Bar"
        set Buz
        union "Foo Bar",Buz
    "#,
        );

        assert_eq!(model.subsets[0].sets, ["Foo Bar"]);
        assert_eq!(model.subsets[2].sets, ["Buz", "Foo Bar"]);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn parse_venn_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
  text A1["Nested note"]
  text A1["Nested note"]
text A alpha["Alpha note"]
style A fill:#ff6b6b, color:#101010
style A,B fill:#00ffcc, color:#003333
"##;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("venn", text)
            .unwrap()
            .unwrap();

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for prefix in ["title", "set", "union", "text", "style"] {
            assert!(
                facts.directive_prefixes.iter().any(|value| value == prefix),
                "missing directive prefix {prefix}"
            );
        }

        let symbol_at = |name: &str, detail: &str, start: usize| {
            facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.selection.start == start
                })
                .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail} at {start}"))
        };

        let title_start = text.find("Product Surface").unwrap();
        let title = symbol_at("Product Surface", "venn title", title_start);
        assert_eq!(title.role, EditorSemanticRole::Payload);
        assert_eq!(title.kind, EditorSemanticKind::String);
        assert_eq!(title.selection.end, title_start + "Product Surface".len());

        let set_a_start = text.find("A[\"Core\"]").unwrap();
        let set_a = symbol_at("A", "venn set", set_a_start);
        assert_eq!(set_a.role, EditorSemanticRole::Entity);
        assert_eq!(set_a.kind, EditorSemanticKind::Namespace);
        assert_eq!(set_a.selection.end, set_a_start + "A".len());

        let set_label_start = text.find("Core").unwrap();
        let set_label = symbol_at("Core", "venn set label", set_label_start);
        assert_eq!(set_label.role, EditorSemanticRole::Payload);
        assert_eq!(set_label.kind, EditorSemanticKind::String);
        assert_eq!(set_label.selection.start, set_label_start);
        assert_eq!(set_label.selection.end, set_label_start + "Core".len());

        let union_a_start = text.find("union A,B").unwrap() + "union ".len();
        let union_a = symbol_at("A", "venn union set", union_a_start);
        assert_eq!(union_a.role, EditorSemanticRole::Reference);
        assert_eq!(union_a.kind, EditorSemanticKind::Namespace);

        let text_set_start = text.find("text A alpha").unwrap() + "text ".len();
        let text_set = symbol_at("A", "venn text set", text_set_start);
        assert_eq!(text_set.role, EditorSemanticRole::Reference);
        assert_eq!(text_set.kind, EditorSemanticKind::Namespace);

        let text_id_start = text.find("alpha").unwrap();
        let text_id = symbol_at("alpha", "venn text node", text_id_start);
        assert_eq!(text_id.role, EditorSemanticRole::Entity);
        assert_eq!(text_id.kind, EditorSemanticKind::Namespace);
        assert_eq!(text_id.selection.end, text_id_start + "alpha".len());

        let text_note_start = text.find("Alpha note").unwrap();
        let text_note = symbol_at("Alpha note", "venn text label", text_note_start);
        assert_eq!(text_note.role, EditorSemanticRole::Payload);

        let style_target_start = text.find("style A fill").unwrap() + "style ".len();
        let style_target = symbol_at("A", "venn style target", style_target_start);
        assert_eq!(style_target.role, EditorSemanticRole::Reference);
        assert_eq!(style_target.kind, EditorSemanticKind::Namespace);

        let fill_start = text.find("#ff6b6b").unwrap();
        let fill = symbol_at("#ff6b6b", "venn style value", fill_start);
        assert_eq!(fill.role, EditorSemanticRole::Payload);
        assert_eq!(fill.kind, EditorSemanticKind::String);
        assert_eq!(fill.selection.start, fill_start);

        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(set_a_start, set_a_start + "A".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::IdList
                && expected.span.start <= union_a_start
                && expected.span.end >= union_a_start + "A".len()
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span.start <= title_start
                && expected.span.end >= title_start + "Product Surface".len()
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_reference_roles_do_not_promote_occurrences_to_set_definitions() {
        let text = concat!(
            "venn-beta\n",
            "style Future fill:blue\n",
            "set Future\n",
            "set A\n",
            "set B\n",
            "union A,B\n",
            "text A note[\"Note\"]\n",
            "style A fill:red\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        let a_roles: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "A")
            .map(|symbol| symbol.role)
            .collect();
        assert_eq!(
            a_roles,
            [
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "note" && symbol.role == EditorSemanticRole::Entity
            })
        );
        let future_roles: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Future")
            .map(|symbol| symbol.role)
            .collect();
        assert_eq!(
            future_roles,
            [EditorSemanticRole::Reference, EditorSemanticRole::Entity]
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_combined_parse_constructs_once_and_preserves_projections() {
        let text = r##"venn-beta
title "Product overlap"
set "Frontend Team"["Frontend"]:.5
set Backend:12
union "Frontend Team",Backend["Shared"]:3
  text shared_note["API contracts"]
style "Frontend Team",Backend fill:rgba(255, 0, 128, 0.5), color:#101010
"##;
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_venn_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1,
            "one combined request must construct Venn syntax once"
        );

        assert_eq!(combined_json, parse_venn(text, &meta).unwrap());
        assert!(!combined_editor.symbols.is_empty());
    }

    #[test]
    fn venn_typed_and_json_projections_share_the_same_semantics() {
        let text = r#"venn-beta
title "Product overlap"
set A["Frontend"]:20
set B["Backend"]:12
union A,B["Shared"]:3
  text note["API contracts"]
style A,B fill:#ff6b6b, color:red
"#;

        let compat = parse_venn(text, &meta()).unwrap();
        let typed = parse_venn_model_for_render(text, &meta()).unwrap();

        assert_eq!(
            render_model_to_compat_json(&typed, &meta()).unwrap(),
            compat
        );
        assert_eq!(typed.title.as_deref(), Some("\"Product overlap\""));
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        assert_eq!(compat["subsets"], json!(typed.subsets));
        assert_eq!(compat["textNodes"], json!(typed.text_nodes));
        assert_eq!(compat["styleEntries"], json!(typed.style_entries));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_editor_projection_preserves_quoted_and_numeric_token_spans() {
        let text = "venn-beta\nset \"Frontend Team\"[\"Core\"]:.5\ntext \"Frontend Team\" 42\n";
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        let set_raw_start = text.find("\"Frontend Team\"").unwrap();
        let set = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "Frontend Team" && symbol.detail.as_deref() == Some("venn set")
            })
            .expect("quoted set fact");
        assert_eq!(
            set.span,
            SourceSpan::new(set_raw_start, set_raw_start + "\"Frontend Team\"".len())
        );
        assert_eq!(
            set.selection,
            SourceSpan::new(set_raw_start + 1, set_raw_start + "\"Frontend Team".len())
        );

        for (name, detail) in [(".5", "venn size"), ("42", "venn text node")] {
            let start = text.find(name).unwrap();
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                .unwrap_or_else(|| panic!("missing {detail} fact"));
            assert_eq!(symbol.span, SourceSpan::new(start, start + name.len()));
            assert_eq!(symbol.selection, symbol.span);
        }
    }

    #[test]
    fn venn_malformed_statement_recovers_prior_facts_with_the_strict_error_span() {
        let text = "venn-beta\nset A[Alpha]\nunion A,\n";
        let statement_start = text.find("union A,").unwrap();
        let statement_span = SourceSpan::new(statement_start, statement_start + "union A,".len());

        let error = parse_venn(text, &meta()).expect_err("strict parse must reject the union");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Venn parse error");
        };
        assert!(diagnostic.message().contains("expected identifier"));
        assert_eq!(diagnostic.span(), Some(statement_span));
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_venn_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "A" && symbol.detail.as_deref() == Some("venn set")
            }));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("expected identifier")
                    && diagnostic.span == Some(statement_span)
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_recovery_preserves_later_semantic_facts_with_crlf_spans() {
        let text = concat!(
            "venn-beta\r\n",
            "  union A, %% missing identifier\r\n",
            "set Later[\"ok\"]:12\r\n",
            "title Done\r\n",
        );
        let invalid = "union A,";
        let invalid_start = text.find(invalid).unwrap();
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Later" && symbol.detail.as_deref() == Some("venn set")
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        invalid_start,
                        invalid_start + invalid.len(),
                    ))
        }));
    }

    #[test]
    fn venn_numeric_and_color_edges_follow_the_upstream_jison_grammar() {
        let model = parse("venn-beta\nset A:.5\nset B:+2\nunion A,B:-.25\n");
        assert_eq!(model.subsets[0].size, 0.5);
        assert_eq!(model.subsets[1].size, 2.0);
        assert_eq!(model.subsets[2].size, -0.25);

        let invalid_number = parse_venn("venn-beta\nset A:1.\n", &meta()).unwrap_err();
        assert!(
            invalid_number
                .to_string()
                .contains("unexpected trailing venn tokens")
        );

        let invalid_rgb =
            parse_venn("venn-beta\nset A\nstyle A fill:rgb(red, 0, 0)\n", &meta()).unwrap_err();
        assert!(invalid_rgb.to_string().contains("expected style value"));
    }

    #[test]
    fn render_model_entrypoint_returns_typed_venn_model() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\nset A\nset B\nunion A,B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "venn");
        let RenderSemanticModel::Venn(model) = parsed.model() else {
            panic!("expected Venn render model");
        };
        assert_eq!(model.subsets.len(), 3);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "venn".to_string(),
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
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_preserves_venn_error_without_state_observer_facts() {
        let source = "venn-beta\ntitle Example\nset A\nunion A,\nset B\n";
        let capture = |demand| {
            parse_venn_semantic_model_controlled(
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
        crate::editor::test_work::reset();
        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        let recovery = capture(EditorFactsDemand::Recovery);
        assert_eq!(
            recovery.model.unwrap_err().to_string(),
            full.model.unwrap_err().to_string()
        );
        let facts = recovery.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert!(!facts.diagnostics.is_empty());
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1
        );
    }

    #[test]
    fn semantic_subsets_text_and_styles_construct_once_without_editor_work() {
        let source = concat!(
            "venn-beta\n",
            "title <b>Overlap</b>\n",
            "set B[\"Backend\"]:12set A[\"Frontend\"]:20",
            "union B,A[\"Shared\"]:3\n",
            "  text note[\"API contracts\"]\n",
            "style B,A fill:#ff0000, color:blue\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_venn_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("Overlap"));
        assert_eq!(model.subsets.len(), 3);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
        assert_eq!(model.subsets[2].size, 3.0);
        assert_eq!(model.subsets[2].label.as_deref(), Some("Shared"));
        assert_eq!(model.text_nodes[0].id, "note");
        assert_eq!(model.text_nodes[0].sets, ["A", "B"]);
        assert_eq!(model.text_nodes[0].label.as_deref(), Some("API contracts"));
        assert_eq!(model.style_entries[0].targets, ["A", "B"]);
        assert_eq!(model.style_entries[0].styles["fill"], "#ff0000");
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_union_error_retains_the_exact_crlf_statement_span_without_editor_work() {
        let source = "venn-beta\r\nset A\r\n  union A, %% missing identifier\r\nset Later\r\n";
        let start = source.find("union A,").unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_venn_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a Venn parse error");
        };
        assert!(diagnostic.message().contains("expected identifier"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "union A,".len()))
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
    fn semantic_entry_can_cancel_between_statements() {
        let source = "venn-beta\nset A set B set C set D\n";
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(matches!(
            parse_venn_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_typed_and_json_requests_omit_editor_observers() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "venn".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "venn-beta\nset A\nset B\nunion A,B\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_venn(source, &meta).unwrap();
        parse_venn_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
