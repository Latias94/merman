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
fn class_parser_extension_relation_renders_label() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : extends",
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
            "  extends\n",
            "     |\n",
            "  +-----+\n",
            "  | Dog |\n",
            "  +-----+\n",
        )
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

#[test]
fn class_parser_multiple_relationships_are_explicitly_unsupported() {
    let err = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog\nAnimal <|-- Cat",
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("multiple class relationship layout needs a graph layout pass");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "multiple class relationships",
        }
    );
}

#[test]
fn class_parser_reverse_extension_orients_marker_toward_parent() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nDog --|> Animal",
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
fn class_parser_aggregation_relation_renders_hollow_diamond_marker() {
    let rendered = render_class(
        "classDiagram\nclass Whole\nclass Part\nWhole o-- Part : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+-------+\n",
            "| Whole |\n",
            "+-------+\n",
            "    o\n",
            "  owns\n",
            "    |\n",
            "+------+\n",
            "| Part |\n",
            "+------+\n",
        )
    );
}

#[test]
fn class_parser_composition_relation_renders_filled_diamond_marker() {
    let rendered = render_class(
        "classDiagram\nclass Whole\nclass Part\nWhole *-- Part : contains",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+-------+\n",
            "| Whole |\n",
            "+-------+\n",
            "    *\n",
            "contains\n",
            "    |\n",
            "+------+\n",
            "| Part |\n",
            "+------+\n",
        )
    );
}

#[test]
fn class_parser_composition_relation_renders_unicode_marker() {
    let rendered = render_class(
        "classDiagram\nclass Whole\nclass Part\nWhole *-- Part",
        &AsciiRenderOptions::unicode(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "┌───────┐\n",
            "│ Whole │\n",
            "└───────┘\n",
            "    ◆\n",
            "    │\n",
            "┌──────┐\n",
            "│ Part │\n",
            "└──────┘\n",
        )
    );
}

#[test]
fn class_parser_dependency_relation_renders_dotted_arrow_marker() {
    let rendered = render_class(
        "classDiagram\nclass Service\nclass Repo\nService ..> Repo : uses",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+---------+\n",
            "| Service |\n",
            "+---------+\n",
            "     :\n",
            "   uses\n",
            "     v\n",
            " +------+\n",
            " | Repo |\n",
            " +------+\n",
        )
    );
}
