use merman_ascii::{AsciiRenderOptions, render_model};
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
fn er_parser_relationship_chain_renders_each_cardinality_and_label() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nB ||--|| C : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " ||\n", "owns\n", "  |\n", " ||\n", "+---+\n",
            "| B |\n", "+---+\n", " ||\n", "owns\n", "  |\n", " ||\n", "+---+\n", "| C |\n",
            "+---+\n",
        )
    );
}

#[test]
fn er_parser_relationship_star_renders_each_label_and_leaf_cardinality() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nA ||--|| C : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "      ||\n",
            "  owns owns\n",
            "  +----+---+\n",
            " ||       ||\n",
            "+---+    +---+\n",
            "| B |    | C |\n",
            "+---+    +---+\n",
        )
    );
}

#[test]
fn er_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge() {
    let rendered = render_er(
        "erDiagram\nA ||--|| D : owns\nB ||--|| C : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("crossing ER relationships should render by reordering the lower layer");

    assert_eq!(
        rendered,
        concat!(
            "+---+    +---+\n",
            "| A |    | B |\n",
            "+---+    +---+\n",
            " ||       ||\n",
            "owns     owns\n",
            "  |        |\n",
            " ||       ||\n",
            "+---+    +---+\n",
            "| D |    | C |\n",
            "+---+    +---+\n",
        )
    );
}

#[test]
fn er_parser_relationship_layouts_render_unrelated_entities_as_components() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nC",
        &AsciiRenderOptions::ascii(),
    )
    .expect("unrelated ER entities should render as separate components");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " ||\n", "owns\n", "  |\n", " ||\n", "+---+\n",
            "| B |\n", "+---+\n", "\n", "+---+\n", "| C |\n", "+---+\n",
        )
    );
}
