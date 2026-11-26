#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonDiagnostic;
#[allow(
    unused_imports,
    reason = "The selected Mermaid families use different shared Langium terminals."
)]
pub(crate) use merman_core_preprocess::scan::langium::{
    LangiumCommonFact, LangiumCommonFacts, LangiumCommonField, LangiumCommonParse,
    convert_langium_escape_character, is_ecmascript_line_terminator, parse_langium_common,
    parse_langium_string, strip_langium_inline_comment,
};
#[cfg(test)]
use std::{cell::RefCell, collections::BTreeMap};

#[cfg(test)]
thread_local! {
    static FAMILY_SYNTAX_CONSTRUCTION_COUNTS: RefCell<BTreeMap<&'static str, usize>> =
        const { RefCell::new(BTreeMap::new()) };
}

#[cfg(test)]
pub(crate) fn record_family_syntax_construction(family: &'static str) {
    FAMILY_SYNTAX_CONSTRUCTION_COUNTS.with_borrow_mut(|counts| {
        *counts.entry(family).or_default() += 1;
    });
}

#[cfg(test)]
pub(crate) fn reset_family_syntax_construction_count(family: &'static str) {
    FAMILY_SYNTAX_CONSTRUCTION_COUNTS.with_borrow_mut(|counts| {
        counts.remove(family);
    });
}

#[cfg(test)]
pub(crate) fn family_syntax_construction_count(family: &'static str) -> usize {
    FAMILY_SYNTAX_CONSTRUCTION_COUNTS
        .with_borrow(|counts| counts.get(family).copied().unwrap_or_default())
}

#[cfg(feature = "editor-facts")]
pub(crate) fn push_langium_common_editor_fact(
    facts: &mut EditorSemanticFacts,
    fact: &LangiumCommonFact,
    family_name: &str,
) {
    facts.push_directive_prefix(fact.field.directive());
    if fact.value.is_empty() {
        return;
    }

    let detail = match fact.field {
        LangiumCommonField::Title => format!("{family_name} title"),
        LangiumCommonField::AccTitle => format!("{family_name} accessibility title"),
        LangiumCommonField::AccDescr => format!("{family_name} accessibility description"),
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        fact.value_span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        fact.value.clone(),
        Some(detail),
        EditorSemanticKind::String,
        fact.raw_span,
        fact.value_span,
    ));
}

#[cfg(feature = "editor-facts")]
pub(crate) fn push_langium_common_recovery(
    facts: &mut EditorSemanticFacts,
    diagnostic: &LangiumCommonDiagnostic,
) {
    facts.mark_recovered_from_parse_error(&diagnostic.message, Some(diagnostic.span));
}

/// Parses one `TitleAndAccessibilities` terminal at `offset` in the complete parser input.
///
/// A successful result consumes the terminal and its required `EOL` (`NEWLINE+ | EOF`). An
/// unterminated block `accDescr` is returned as a recovered fact so editor projections can retain
/// its prefix and partial payload; strict family parsers must reject results with a diagnostic.
#[cfg(all(test, feature = "all-diagrams", feature = "presentation"))]
mod tests {
    #[cfg(all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    use crate::EditorSemanticCompleteness;
    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    use crate::{Error, Result, SourceSpan};
    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    use crate::{MermaidConfig, SourcePresentationMetadata};
    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    use serde_json::Value;

    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    type JsonParser = fn(&str, &SourcePresentationMetadata) -> Result<Value>;
    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    struct FamilyCase {
        id: &'static str,
        header: &'static str,
        tail: &'static str,
        #[cfg(feature = "compat-json")]
        parser: JsonParser,
        #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
        combined: crate::family::CombinedSemanticParser,
        #[cfg(feature = "compat-json")]
        retains_title: bool,
    }

    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    fn family_cases() -> [FamilyCase; 7] {
        [
            FamilyCase {
                id: "architecture",
                header: "architecture-beta",
                tail: "service api\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::architecture::parse_architecture,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::architecture::parse_architecture_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
            FamilyCase {
                id: "cynefin",
                header: "cynefin-beta",
                tail: "clear\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::cynefin::parse_cynefin,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::cynefin::parse_cynefin_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
            FamilyCase {
                id: "gitGraph",
                header: "gitGraph",
                tail: "commit id: \"c1\"\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::git_graph::parse_git_graph,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::git_graph::parse_git_graph_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
            FamilyCase {
                id: "info",
                header: "info showInfo",
                tail: "",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::info::parse_info,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::info::parse_info_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: false,
            },
            FamilyCase {
                id: "packet",
                header: "packet",
                tail: "0-7: \"byte\"\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::packet::parse_packet,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::packet::parse_packet_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
            FamilyCase {
                id: "pie",
                header: "pie showData",
                tail: "\"A\": 1\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::pie::parse_pie,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::pie::parse_pie_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
            FamilyCase {
                id: "radar",
                header: "radar-beta",
                tail: "axis a\r\ncurve c { 1 }\r\n",
                #[cfg(feature = "compat-json")]
                parser: crate::diagrams::radar::parse_radar,
                #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
                combined: crate::diagrams::radar::parse_radar_json_and_editor_facts,
                #[cfg(feature = "compat-json")]
                retains_title: true,
            },
        ]
    }

    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    fn metadata(id: &str) -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: id.to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        }
    }

    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    fn common_family_source(case: &FamilyCase) -> String {
        format!(
            "{}\r\n  title First\r\n  title Final   title %% hidden\r\n  accTitle:  Accessible   map\r\n  accDescr {{\r\n    First   line\r\n\r\n    \tSecond\t\tline  \r\n  }}\r\n{}",
            case.header, case.tail
        )
    }

    #[test]
    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    fn importing_families_share_common_values_spans_and_db_capabilities() {
        for case in family_cases() {
            let source = common_family_source(&case);
            let meta = metadata(case.id);
            let model = (case.parser)(&source, &meta)
                .unwrap_or_else(|error| panic!("{} common parse failed: {error}", case.id));

            if case.retains_title {
                assert_eq!(model["title"], "Final title", "family: {}", case.id);
            } else {
                assert!(model.get("title").is_none(), "family: {}", case.id);
            }
            if case.id == "info" {
                assert!(model.get("accTitle").is_none());
                assert!(model.get("accDescr").is_none());
            } else {
                assert_eq!(model["accTitle"], "Accessible map", "family: {}", case.id);
                assert_eq!(
                    model["accDescr"], "First line\nSecond line",
                    "family: {}",
                    case.id
                );
            }

            #[cfg(feature = "editor-facts")]
            {
                let editor =
                    crate::family::test_support::editor_facts(case.combined, &source, &meta);
                assert_eq!(
                    editor.completeness,
                    EditorSemanticCompleteness::Complete,
                    "family: {}",
                    case.id
                );
                let detail = format!("{} accessibility description", case.id);
                let descr = editor
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail.as_str()))
                    .unwrap_or_else(|| panic!("{} missing common editor payload", case.id));
                assert_eq!(descr.name, "First line\nSecond line");
                let raw = &source[descr.span.start..descr.span.end];
                assert!(raw.starts_with("  accDescr {"), "family: {}", case.id);
                assert!(raw.ends_with("  }"), "family: {}", case.id);
                let selected = &source[descr.selection.start..descr.selection.end];
                assert!(selected.starts_with("First"), "family: {}", case.id);
                assert!(selected.contains("Second\t\tline"), "family: {}", case.id);
            }
        }
    }

    #[test]
    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    fn importing_families_keep_common_comments_and_ecmascript_boundary_values() {
        let comment = "%% first %% \"unclosed ' %% final";
        for case in family_cases() {
            for boundary in ["", "\u{0085}", "\u{feff}"] {
                let expected_boundary = if boundary == "\u{0085}" { boundary } else { "" };
                let title = format!("{expected_boundary}Value{expected_boundary}");
                let acc_title = format!("{expected_boundary}Accessible{expected_boundary}");
                #[cfg(feature = "editor-facts")]
                let scalar = format!("{expected_boundary}Scalar{expected_boundary}");
                let descr = format!(
                    "{expected_boundary}Help %% \"literal %%\u{2028}Next{expected_boundary}"
                );
                #[cfg(feature = "editor-facts")]
                let descr_source = format!(
                    "{expected_boundary}Help %% \"literal %%\u{2028}  Next{expected_boundary}"
                );
                let source = format!(
                    "{}\r\n  title {boundary}Value{boundary} {comment}\r\n  accTitle: {boundary}Accessible{boundary} {comment}\r\n  accDescr: {boundary}Scalar{boundary} {comment}\r\n \t\r\n  {comment}\r\n  accDescr\u{feff}{{ {boundary}Help %% \"literal %%\u{2028}  Next{boundary} }} {comment}\r\n{}",
                    case.header, case.tail
                );
                let meta = metadata(case.id);
                let model = (case.parser)(&source, &meta)
                    .unwrap_or_else(|error| panic!("{} common parse failed: {error}", case.id));

                if case.retains_title {
                    assert_eq!(model["title"], title, "family: {}", case.id);
                } else {
                    assert!(model.get("title").is_none(), "family: {}", case.id);
                }
                if case.id == "info" {
                    assert!(model.get("accTitle").is_none());
                    assert!(model.get("accDescr").is_none());
                } else {
                    assert_eq!(model["accTitle"], acc_title, "family: {}", case.id);
                    assert_eq!(model["accDescr"], descr, "family: {}", case.id);
                }

                #[cfg(feature = "editor-facts")]
                {
                    let editor =
                        crate::family::test_support::editor_facts(case.combined, &source, &meta);
                    assert_eq!(editor.completeness, EditorSemanticCompleteness::Complete);
                    assert!(editor.diagnostics.is_empty(), "family: {}", case.id);
                    for (expected, payload_source) in [
                        (&title, title.as_str()),
                        (&acc_title, acc_title.as_str()),
                        (&scalar, scalar.as_str()),
                        (&descr, descr_source.as_str()),
                    ] {
                        let start = source.find(payload_source).unwrap();
                        let symbol = editor
                            .symbols
                            .iter()
                            .find(|symbol| &symbol.name == expected)
                            .unwrap_or_else(|| panic!("{} missing {expected:?}", case.id));
                        assert_eq!(
                            symbol.selection,
                            SourceSpan::new(start, start + payload_source.len()),
                            "family: {}",
                            case.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(all(feature = "all-diagrams", feature = "presentation"))]
    fn importing_family_typed_models_receive_the_same_common_values() {
        let cases = family_cases();
        let source = |id: &str| {
            let case = cases.iter().find(|case| case.id == id).unwrap();
            common_family_source(case)
        };

        let architecture = crate::diagrams::architecture::parse_architecture_model_for_render(
            &source("architecture"),
            &metadata("architecture"),
        )
        .unwrap();
        assert_eq!(architecture.title.as_deref(), Some("Final title"));
        assert_eq!(
            architecture.acc_descr.as_deref(),
            Some("First line\nSecond line")
        );
        assert_eq!(architecture.nodes.len(), 1);

        let cynefin = crate::diagrams::cynefin::parse_cynefin_model_for_render(
            &source("cynefin"),
            &metadata("cynefin"),
        )
        .unwrap();
        assert_eq!(cynefin.title.as_deref(), Some("Final title"));
        assert_eq!(cynefin.domains.len(), 1);

        let git_graph = crate::diagrams::git_graph::parse_git_graph_model_for_render(
            &source("gitGraph"),
            &metadata("gitGraph"),
        )
        .unwrap();
        assert_eq!(git_graph.title.as_deref(), Some("Final title"));
        assert_eq!(git_graph.acc_title.as_deref(), Some("Accessible map"));
        assert_eq!(
            git_graph.acc_descr.as_deref(),
            Some("First line\nSecond line")
        );
        assert_eq!(git_graph.commits.len(), 1);

        let (info, _) =
            crate::diagrams::info::parse_info_model_for_render(&source("info"), &metadata("info"))
                .unwrap();
        assert!(info.show_info);

        let (packet, _) = crate::diagrams::packet::parse_packet_model_for_render(
            &source("packet"),
            &metadata("packet"),
        )
        .unwrap();
        assert_eq!(packet.title.as_deref(), Some("Final title"));
        assert_eq!(packet.packet.len(), 1);

        let (pie, _) =
            crate::diagrams::pie::parse_pie_model_for_render(&source("pie"), &metadata("pie"))
                .unwrap();
        assert_eq!(pie.title.as_deref(), Some("Final title"));
        assert!(pie.show_data);
        assert_eq!(pie.sections.len(), 1);

        let (radar, _) = crate::diagrams::radar::parse_radar_model_for_render(
            &source("radar"),
            &metadata("radar"),
        )
        .unwrap();
        assert_eq!(radar.title.as_deref(), Some("Final title"));
        assert_eq!(radar.axes.len(), 1);
        assert_eq!(radar.curves.len(), 1);
    }

    #[test]
    #[cfg(all(feature = "all-diagrams", feature = "compat-json"))]
    fn importing_families_reject_unterminated_blocks_and_recover_editor_payloads() {
        for case in family_cases() {
            let source = format!("{}\naccDescr {{\n  partial", case.header);
            let meta = metadata(case.id);
            let error = match (case.parser)(&source, &meta) {
                Ok(_) => panic!("{} accepted unterminated accDescr", case.id),
                Err(error) => error,
            };
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("{} returned non-parser error", case.id);
            };
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(source.len(), source.len()))
            );

            #[cfg(feature = "editor-facts")]
            {
                let editor =
                    crate::family::test_support::editor_facts(case.combined, &source, &meta);
                assert_eq!(editor.completeness, EditorSemanticCompleteness::Recovered);
                assert!(
                    editor
                        .directive_prefixes
                        .iter()
                        .any(|prefix| prefix == "accDescr")
                );
                assert!(editor.symbols.iter().any(|symbol| {
                    symbol.name == "partial"
                        && symbol.selection
                            == SourceSpan::new(source.find("partial").unwrap(), source.len())
                }));
                assert!(editor.diagnostics.iter().any(|entry| {
                    entry.message.contains("unterminated accDescr block")
                        && entry.span == Some(SourceSpan::new(source.len(), source.len()))
                }));
            }
        }
    }
}
