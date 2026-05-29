use merman_ascii::{AsciiError, AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};

fn render_class(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("class diagram should parse")
        .expect("class diagram should be detected");

    render_model(&parsed.model, options)
}

#[test]
fn class_parser_single_class_renders_ascii_box() {
    let rendered = render_class("classDiagram\nclass Animal", &AsciiRenderOptions::ascii())
        .expect("class diagram should render");

    assert_eq!(rendered, "+--------+\n| Animal |\n+--------+\n");
}

#[test]
fn class_parser_single_class_renders_unicode_box() {
    let rendered = render_class("classDiagram\nclass Animal", &AsciiRenderOptions::unicode())
        .expect("class diagram should render");

    assert_eq!(rendered, "┌────────┐\n│ Animal │\n└────────┘\n");
}

#[test]
fn class_parser_members_and_methods_render_ascii_sections() {
    let rendered = render_class(
        "classDiagram\nclass Animal {\n  +String name\n  +eat(food) bool\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+-------------------+\n",
            "| Animal            |\n",
            "+-------------------+\n",
            "| +String name      |\n",
            "+-------------------+\n",
            "| +eat(food) : bool |\n",
            "+-------------------+\n",
        )
    );
}

#[test]
fn class_parser_extension_relation_renders_parent_above_child() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+--------+\n",
            "| Animal |\n",
            "+--------+\n",
            "     ^\n",
            "     |\n",
            "  +-----+\n",
            "  | Dog |\n",
            "  +-----+\n",
        )
    );
}

#[test]
fn class_parser_relationship_labels_are_explicitly_unsupported() {
    let err = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : extends",
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("relationship label support should be explicit");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationship labels",
        }
    );
}

#[test]
fn class_parser_relationship_layouts_with_unrelated_classes_are_explicitly_unsupported() {
    let err = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog",
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("unrelated classes must not be silently omitted");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "class relationship layouts with unrelated classes",
        }
    );
}
