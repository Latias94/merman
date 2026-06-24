use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb, render_model,
};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_er(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("ER diagram should parse")
        .expect("ER diagram should be detected");

    render_model(&parsed.model, options)
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_html_spans(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("<span ") {
            index += rest.find('>').expect("span start tag should be closed") + 1;
            continue;
        }
        if rest.starts_with("</span>") {
            index += "</span>".len();
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("index should be on a char boundary");
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

#[test]
fn er_color_truecolor_emits_semantic_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::Text, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(3, 3, 3))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(4, 4, 4))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::new(5, 5, 5));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered =
        render_er("erDiagram\nCUSTOMER ||--o{ ORDER : places", &options).expect("ER should render");

    assert_eq!(
        strip_ansi(&rendered),
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
    for expected_code in [
        "\u{1b}[38;2;1;1;1m",
        "\u{1b}[38;2;2;2;2m",
        "\u{1b}[38;2;3;3;3m",
        "\u{1b}[38;2;4;4;4m",
        "\u{1b}[38;2;5;5;5m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn er_color_html_wraps_layered_relation_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x404040))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x505050))
        .with_role(AsciiColorRole::Junction, AsciiRgb::from_hex24(0x606060));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_er("erDiagram\nA ||--|| B : owns\nA ||--|| C : owns", &options)
        .expect("ER should render");

    assert_eq!(
        strip_html_spans(&rendered),
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
    for expected_fragment in [
        "<span style=\"color:#101010\">+---+</span>",
        "<span style=\"color:#202020\">A</span>",
        "<span style=\"color:#303030\">----</span>",
        "<span style=\"color:#404040\">||</span>",
        "<span style=\"color:#505050\">owns</span>",
        "<span style=\"color:#606060\">+</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
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
fn er_parser_identifying_relationship_renders_multiline_label() {
    let rendered = render_er(
        "erDiagram\nCUSTOMER ||--o{ ORDER : \"north<br>south\"",
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
            "    north\n",
            "    south\n",
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

#[test]
fn er_parser_parallel_relationship_layout_renders_each_lane() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nA ||..o{ B : contains",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel ER relationships should render distinct lanes");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            " ||      ||\n",
            "owns  contains\n",
            " |       :\n",
            " ||      o{\n",
            "     +---+\n",
            "     | B |\n",
            "     +---+\n",
        )
    );
}

#[test]
fn er_parser_mixed_parallel_relationship_layout_renders_each_lane() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : a\nA ||..o{ B : b\nA ||--|| C : c",
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed parallel ER relationships should render every lane");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "   || || ||\n",
            "  a |  b c:\n",
            "+---++.+++++\n",
            "||  o{    ||\n",
            "+---+    +---+\n",
            "| B |    | C |\n",
            "+---+    +---+\n",
        )
    );
}

#[test]
fn er_parser_spanning_level_relationship_layout_routes_around_intermediate_entity() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : a\nB ||--|| C : b\nA ||--|| C : c",
        &AsciiRenderOptions::ascii(),
    )
    .expect("spanning-level ER relationship should route around the intermediate entity");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "      ||   ||\n",
            "       a    c\n",
            "       |    |\n",
            "      ||    |\n",
            "     +---+  |\n",
            "     | B |  |\n",
            "     +---+  |\n",
            "      ||    |\n",
            "       b    |\n",
            "       |    |\n",
            "      ||   ||\n",
            "     +---+\n",
            "     | C |\n",
            "     +---+\n",
        )
    );
}

#[test]
fn er_parser_spanning_relationship_routes_around_wide_intermediate_entity() {
    let rendered = render_er(
        r#"erDiagram
USER ||--o{ ORDER : places
USER {
  int id PK
  string name
  string email
}
ORDER ||--|{ ORDER_ITEM : contains
ORDER {
  int id PK
  date created_at
  string status
}
ORDER_ITEM {
  int id PK
  int quantity
  float price
}
PRODUCT ||--o{ ORDER_ITEM : "ordered in"
PRODUCT {
  int id PK
  string name
  float price
}
"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("spanning ER relationship should render around intermediate entities");

    assert!(rendered.contains("ordered in"));
    assert!(rendered.contains("date created_at"));
    assert!(rendered.contains("string status"));
    assert!(!rendered.contains("int id P│"));
    assert!(!rendered.contains("date cre│ted_at"));
    assert!(!rendered.contains("string s│atus"));
}

#[test]
fn er_parser_cyclic_relationship_layout_renders_without_failing() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nB ||--|| C : owns\nC ||--|| A : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("cyclic ER relationships should render");

    assert!(rendered.contains("A"));
    assert!(rendered.contains("B"));
    assert!(rendered.contains("C"));
    assert!(rendered.contains("owns"));
}

#[test]
fn er_local_semantic_fixture_covers_dense_relationships() {
    let input = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/local-semantic/er/dense_relations.mmd"),
    )
    .expect("local semantic ER fixture must be readable");

    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("dense local semantic ER fixture should render");

    for expected in [
        "CUSTOMER", "ORDER", "INVOICE", "places", "billed", "invoices",
    ] {
        assert!(
            rendered.contains(expected),
            "dense semantic ER fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.lines().count() >= 6,
        "dense semantic ER fixture should produce a multi-line layout:\n{rendered}"
    );
}
