#![cfg(feature = "diagram-quadrant-chart")]

use merman_core::SemanticParser;
use merman_model::{
    families::quadrant_chart::QuadrantChartDiagramModel, semantic::SemanticDiagramModel,
};

fn parse_quadrant(source: &str) -> QuadrantChartDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid QuadrantChart source");
    let SemanticDiagramModel::QuadrantChart(model) = parsed.model else {
        panic!("expected the canonical QuadrantChart model");
    };
    model
}

#[test]
fn coordinate_padding_uses_the_javascript_whitespace_set() {
    for newline in ["\n", "\r\n"] {
        for padding in ["\u{feff}", "\u{00a0}", "\u{2003}"] {
            let source = format!("quadrantChart\nP: [{padding}0.5{padding},{padding}1{padding}]\n")
                .replace('\n', newline);
            let model = parse_quadrant(&source);
            assert_eq!(model.points.len(), 1);
            assert_eq!((model.points[0].x, model.points[0].y), (0.5, 1.0));
        }
        for coordinates in [
            "\u{0085}0.5,1",
            "0.5\u{0085},1",
            "0.5,\u{0085}1",
            "0.5,1\u{0085}",
        ] {
            let source = format!("quadrantChart\nP: [{coordinates}]\n").replace('\n', newline);
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "U+0085 must not be discarded from coordinates: {source:?}",
            );
        }
    }
}

#[test]
fn point_and_class_styles_read_only_the_first_value_component() {
    // quadrantDb.ts destructures [key, value] from split(/\s*:\s*/).
    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "quadrantChart\n",
            "classDef C radius: 0002 :ignored,color:#fff:ignored,stroke-width:2px:ignored\n",
            "P:::C: [0.5,0.5] radius:1,radius:3:ignored,stroke-color:#123:ignored\n",
        )
        .replace('\n', newline);
        let model = parse_quadrant(&source);
        let class = &model.classes["C"];
        assert_eq!(class.radius, Some(2));
        assert_eq!(class.color.as_deref(), Some("#fff"));
        assert_eq!(class.stroke_width.as_deref(), Some("2px"));
        let point = &model.points[0];
        assert_eq!(point.class_name.as_deref(), Some("C"));
        assert_eq!(point.styles.radius, Some(3));
        assert_eq!(point.styles.stroke_color.as_deref(), Some("#123"));
    }
}

#[test]
fn ignored_style_components_still_obey_the_style_lexer() {
    for suffix in [
        "foo_bar",
        "a.b",
        "a+b",
        "原稿",
        "\u{feff}",
        "\u{0085}",
        "\u{00a0}",
        "classDef",
        "CLASSDEF",
        "title",
        "x-axis",
        "y-axis",
        "quadrant-1",
        "quadrant-2",
        "quadrant-3",
        "quadrant-4",
        "quadrantChart",
        "accTitle:ignored",
        "accDescr:ignored",
        "1classDef",
        "-title",
        "::C",
    ] {
        for statement in [
            format!("P: [0.5,0.5] radius:1:{suffix}"),
            format!("classDef C radius:1:{suffix}"),
        ] {
            let source = format!("quadrantChart\n{statement}\n");
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "DB destructuring must not admit invalid style tokens: {source:?}",
            );
        }
    }
    for suffix in [
        "ignored",
        "color",
        "aclassDef",
        "1ignored",
        "classDefinition",
        "titles",
        ":C",
        ":C:ignored",
    ] {
        let source = format!(
            "quadrantChart\nclassDef C radius:1:{suffix}\nP:::C: [0.5,0.5] radius:2:{suffix}\n"
        );
        let model = parse_quadrant(&source);
        assert_eq!(model.classes["C"].radius, Some(1));
        assert_eq!(model.points[0].styles.radius, Some(2));
    }
    for padding in ["\u{feff}", "\u{0085}", "\u{00a0}", "\u{2003}"] {
        for statement in [
            format!("P: [0.5,0.5] radius:{padding}1{padding}"),
            format!("classDef C radius:{padding}1{padding}"),
        ] {
            let source = format!("quadrantChart\n{statement}\n");
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "UNICODE_TEXT is not a styleComponent: {source:?}",
            );
        }
    }
}

#[test]
fn empty_and_missing_radius_values_keep_the_upstream_validation_error() {
    use merman_core::Error;

    for (style, value) in [("radius:", ""), ("radius::1", ""), ("radius", "undefined")] {
        for statement in [
            format!("P: [0.5,0.5] {style}"),
            format!("classDef C {style}"),
        ] {
            let source = format!("quadrantChart\n{statement}\n");
            let Error::DiagramParse { diagnostic, .. } = SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect_err("empty or missing radius must not be converted to zero")
            else {
                panic!("invalid styles must remain a parse error");
            };
            assert_eq!(
                diagnostic.message(),
                format!("value for radius {value} is invalid, please use a valid number"),
            );
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn accepted_bom_padding_keeps_original_point_and_class_selections() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "---\ntitle: 原稿😀\n---\nquadrantChart\n",
            "classDef C radius:2:ignored\n",
            "\"点😀\":::C: [\u{feff}0.5\u{feff},\u{feff}1\u{feff}]\n",
        )
        .replace('\n', newline);
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("valid QuadrantChart must produce a snapshot");
        };
        assert!(snapshot.outcome().parsed_model().is_some());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("requested QuadrantChart facts must be available");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        let start = source.find("点😀").unwrap();
        let point = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "点😀")
            .unwrap();
        assert_eq!(
            point.selection,
            SourceSpan::new(start, start + "点😀".len())
        );
        let definition = source.find("classDef C").unwrap() + "classDef ".len();
        let reference = source.find(":::C").unwrap() + ":::".len();
        let expected = [
            SourceSpan::new(definition, definition + 1),
            SourceSpan::new(reference, reference + 1),
        ];
        let selections: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "C")
            .map(|symbol| symbol.selection)
            .collect();
        assert_eq!(selections, expected);
    }
}
