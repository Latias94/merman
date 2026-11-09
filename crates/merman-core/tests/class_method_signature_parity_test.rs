#![cfg(feature = "diagram-class")]

use merman_core::SemanticParser;
use merman_model::{families::class::ClassDiagramModel, semantic::SemanticDiagramModel};

fn parse(source: &str) -> ClassDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Class source");
    let SemanticDiagramModel::Class(model) = parsed.model else {
        panic!("Class source must retain its canonical model");
    };
    model
}

#[test]
fn method_fields_keep_ecmascript_whitespace_and_post_classifier_return_spaces() {
    for newline in ["\n", "\r\n"] {
        for (member, id, visibility, parameters, return_type, classifier) in [
            (
                "+run(\u{85}arg\u{85}) \u{85}Result\u{85}$",
                "run",
                "+",
                "\u{85}arg\u{85}",
                "\u{85}Result\u{85}",
                "$",
            ),
            (
                "+run(\u{feff}arg\u{feff}) \u{feff}Result\u{feff}$",
                "run",
                "+",
                "arg",
                "Result\u{feff}",
                "$",
            ),
            ("+run(arg) Result  $", "run", "+", "arg", "Result  ", "$"),
            (
                "\u{85}+run(arg) Result\u{85}",
                "\u{85}+run",
                "",
                "arg",
                "Result\u{85}",
                "",
            ),
            (
                "\u{feff}+run(arg) Result\u{feff}",
                "run",
                "+",
                "arg",
                "Result",
                "",
            ),
            (
                "+run(arg)\u{85}Result",
                "run",
                "+",
                "arg",
                "\u{85}Result",
                "",
            ),
        ] {
            let source =
                format!("classDiagram{newline}class C {{{newline}{member}{newline}}}{newline}");
            let model = parse(&source);
            let method = &model.classes["C"].methods[0];
            assert_eq!(method.id, id, "{source:?}");
            assert_eq!(method.visibility, visibility, "{source:?}");
            assert_eq!(method.parameters, parameters, "{source:?}");
            assert_eq!(method.return_type, return_type, "{source:?}");
            assert_eq!(method.classifier, classifier, "{source:?}");
        }
    }
}

#[test]
fn method_search_respects_js_line_terminators_and_greedy_parentheses() {
    for separator in ["\u{2028}", "\u{2029}"] {
        for (member, id, parameters, return_type, visibility) in [
            (
                format!("+old{separator}run(arg) Result"),
                "run",
                "arg",
                "Result",
                "",
            ),
            (
                format!("+run(arg) R{separator}tail"),
                "run",
                "arg",
                "R",
                "+",
            ),
            (
                format!("+run(arg){separator}Result"),
                "run",
                "arg",
                "Result",
                "+",
            ),
            (
                format!("+run(arg) {separator}Result"),
                "run",
                "arg",
                "",
                "+",
            ),
            (
                format!("broken(a{separator}run(arg) Result"),
                "run",
                "arg",
                "Result",
                "",
            ),
        ] {
            let model = parse(&format!("classDiagram\nclass C {{\n{member}\n}}\n"));
            let method = &model.classes["C"].methods[0];
            assert_eq!(method.id, id, "{member:?}");
            assert_eq!(method.parameters, parameters, "{member:?}");
            assert_eq!(method.return_type, return_type, "{member:?}");
            assert_eq!(method.visibility, visibility, "{member:?}");
        }
    }
    let model = parse("classDiagram\nclass C {\n+a(b(c)d) Result\n+()\n}\n");
    let methods = &model.classes["C"].methods;
    assert_eq!(methods[0].id, "a(b");
    assert_eq!(methods[0].parameters, "c)d");
    assert_eq!(methods[1].id, "+");
    assert_eq!(methods[1].visibility, "");
}

#[test]
fn attribute_and_annotation_boundaries_use_ecmascript_trimming() {
    let model = parse(concat!(
        "classDiagram\nclass C {\n",
        "+value\u{85}$\n+value\u{feff}$\n\u{85}+literal\u{85}\n\u{feff}+trimmed\u{feff}\n",
        "\u{feff}<<service>>\u{feff}\n)literal\n}\n",
    ));
    let class = &model.classes["C"];
    assert_eq!(
        class
            .members
            .iter()
            .map(|member| member.id.as_str())
            .collect::<Vec<_>>(),
        [
            "value\u{85}",
            "value",
            "\u{85}+literal\u{85}",
            "trimmed",
            ")literal"
        ]
    );
    assert_eq!(class.members[2].visibility, "");
    assert_eq!(class.annotations, ["service"]);
    assert!(class.methods.is_empty());
}

#[test]
fn invalid_method_signatures_fail_semantic_construction() {
    for member in ["method)", "()", "run(a\u{2028}b)", "run(a\u{2029}b)"] {
        let source = format!("classDiagram\nclass C {{\n{member}\n}}\n");
        let error = SemanticParser::new()
            .parse_diagram_sync(&source)
            .expect_err("selected DB rejects a method without a valid signature");
        assert!(
            error.to_string().contains("Invalid class method signature"),
            "{source:?}: {error}"
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn member_facts_keep_authored_boundaries_after_signature_failure() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    for broken in [false, true] {
        let member = "\u{85}+run(\u{feff}arg\u{feff}) Result\u{85}";
        let source = format!(
            concat!(
                "\u{feff}---\r\ntitle: Classes 😀\r\n---\r\nclassDiagram\r\nclass C {{\r\n",
                "\u{feff}{member}\u{feff}\r\n{}\r\n}}\r\nclass Later\r\nLater --> C\r\n",
            ),
            if broken { "invalid)" } else { "+ok() Result" },
            member = member
        );
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("Class retains a source snapshot");
        };
        assert_eq!(snapshot.outcome().parsed_model().is_some(), !broken);
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Class retains member facts");
        };
        assert_eq!(
            facts.completeness,
            if broken {
                EditorSemanticCompleteness::Recovered
            } else {
                EditorSemanticCompleteness::Complete
            }
        );
        let start = source.find(member).unwrap();
        let expected = SourceSpan::new(start, start + member.len());
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == member)
            .expect("NEL remains part of authored member selection");
        assert_eq!(symbol.selection, expected);
        assert_eq!(
            &source[symbol.selection.start..symbol.selection.end],
            member
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        for symbol in &facts.symbols {
            assert!(
                source.is_char_boundary(symbol.selection.start)
                    && source.is_char_boundary(symbol.selection.end)
            );
        }
    }
}
