#![cfg(feature = "diagram-class")]

use merman_core::SemanticParser;
use merman_model::{families::class::ClassDiagramModel, semantic::SemanticDiagramModel};

fn class_model(source: &str) -> ClassDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Class source");
    let SemanticDiagramModel::Class(model) = parsed.model else {
        panic!("expected the canonical Class model");
    };
    model
}

#[test]
fn block_members_keep_sanitizer_exposed_spaces_when_reading_modifiers() {
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "classDiagram{newline}class C {{{newline}+value$ <!--hint-->{newline}+abstract* <!--hint-->{newline}<!--hint--> +other{newline}<!--hint--> +get() Result{newline}}}{newline}"
        );
        let model = class_model(&source);
        let class = &model.classes["C"];
        assert_eq!(class.members.len(), 3);
        for (member, id, visibility, display) in [
            (&class.members[0], "value$", "+", "+value$"),
            (&class.members[1], "abstract*", "+", "+abstract*"),
            (&class.members[2], "other", "+", "+other"),
        ] {
            assert_eq!(member.id, id, "{source:?}");
            assert_eq!(member.visibility, visibility);
            assert_eq!(member.classifier, "");
            assert_eq!(member.css_style, "");
            assert_eq!(member.display_text, display);
        }
        assert_eq!(class.methods.len(), 1);
        let method = &class.methods[0];
        assert_eq!(method.id, "get");
        assert_eq!(method.visibility, "+");
        assert_eq!(method.parameters, "");
        assert_eq!(method.return_type, "Result");
        assert_eq!(method.classifier, "");
        assert_eq!(method.display_text, "+get() : Result");
    }
}

#[test]
fn colon_members_keep_their_earlier_label_cleanup_before_modifier_parsing() {
    let model = class_model(concat!(
        "classDiagram\nclass C\n",
        "C: +value$ <!--hint-->\n",
        "C: <!--hint--> +get() Result\n",
    ));
    let class = &model.classes["C"];
    let member = &class.members[0];
    assert_eq!(member.id, "value");
    assert_eq!(member.visibility, "+");
    assert_eq!(member.classifier, "$");
    assert_eq!(member.css_style, "text-decoration:underline;");
    assert_eq!(member.display_text, "+value");
    let method = &class.methods[0];
    assert_eq!(method.id, "get");
    assert_eq!(method.visibility, "+");
    assert_eq!(method.return_type, "Result");
    assert_eq!(method.display_text, "+get() : Result");
}

#[test]
fn ordinary_block_members_keep_visibility_and_static_or_abstract_classifiers() {
    let model = class_model(concat!(
        "classDiagram\nclass C {\n",
        "+value$\n+abstract*\n+get() Result\n+getStatic() Result$\n}\n",
    ));
    let class = &model.classes["C"];
    for (member, id, classifier, css_style) in [
        (
            &class.members[0],
            "value",
            "$",
            "text-decoration:underline;",
        ),
        (&class.members[1], "abstract", "*", "font-style:italic;"),
    ] {
        assert_eq!(member.id, id);
        assert_eq!(member.visibility, "+");
        assert_eq!(member.classifier, classifier);
        assert_eq!(member.css_style, css_style);
    }
    for (method, id, classifier, css_style) in [
        (&class.methods[0], "get", "", ""),
        (
            &class.methods[1],
            "getStatic",
            "$",
            "text-decoration:underline;",
        ),
    ] {
        assert_eq!(method.id, id);
        assert_eq!(method.visibility, "+");
        assert_eq!(method.return_type, "Result");
        assert_eq!(method.classifier, classifier);
        assert_eq!(method.css_style, css_style);
    }
}
