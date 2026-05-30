use merman_ascii::{AsciiError, AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};

fn render_er(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("ER diagram should parse")
        .expect("ER diagram should be detected");

    render_model(&parsed.model, options)
}

#[test]
fn er_parser_single_entity_renders_ascii_box() {
    let rendered =
        render_er("erDiagram\nCUSTOMER", &AsciiRenderOptions::ascii()).expect("ER should render");

    assert_eq!(rendered, "+----------+\n| CUSTOMER |\n+----------+\n");
}

#[test]
fn er_parser_single_entity_renders_unicode_box() {
    let rendered =
        render_er("erDiagram\nCUSTOMER", &AsciiRenderOptions::unicode()).expect("ER should render");

    assert_eq!(rendered, "┌──────────┐\n│ CUSTOMER │\n└──────────┘\n");
}

#[test]
fn er_parser_attributes_render_in_entity_section() {
    let rendered = render_er(
        "erDiagram\nCUSTOMER {\n  string id PK\n  string name\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+--------------+\n",
            "| CUSTOMER     |\n",
            "+--------------+\n",
            "| string id PK |\n",
            "| string name  |\n",
            "+--------------+\n",
        )
    );
}

#[test]
fn er_parser_identifying_relationship_renders_cardinality_markers_and_label() {
    let rendered = render_er(
        "erDiagram\nCUSTOMER ||--o{ ORDER : places",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+----------+\n",
            "| CUSTOMER |\n",
            "+----------+\n",
            "     ||\n",
            "   places\n",
            "      |\n",
            "     o{\n",
            "  +-------+\n",
            "  | ORDER |\n",
            "  +-------+\n",
        )
    );
}

#[test]
fn er_parser_non_identifying_relationship_renders_dotted_line() {
    let rendered = render_er("erDiagram\nA ||..|{ B : refs", &AsciiRenderOptions::ascii())
        .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " ||\n", "refs\n", "  :\n", " |{\n", "+---+\n",
            "| B |\n", "+---+\n",
        )
    );
}

#[test]
fn er_parser_zero_or_one_cardinality_renders_marker() {
    let rendered = render_er(
        "erDiagram\nA ||--o| B : maybe",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " ||\n", "maybe\n", "  |\n", " o|\n", "+---+\n",
            "| B |\n", "+---+\n",
        )
    );
}

#[test]
fn er_parser_multiple_relationship_layouts_are_explicitly_unsupported() {
    let err = render_er(
        "erDiagram\nA ||--|| B : owns\nB ||--|| C : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("multiple ER relationship layout needs a graph layout pass");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "multiple ER relationships",
        }
    );
}

#[test]
fn er_parser_relationship_layouts_with_unrelated_entities_are_explicitly_unsupported() {
    let err = render_er(
        "erDiagram\nA ||--|| B : owns\nC",
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("unrelated ER entities need a graph layout pass");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "ER relationship layouts with unrelated entities",
        }
    );
}
