use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions, AsciiRgb,
    render_model,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::er::ErDiagramRenderModel;
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn parse_er_render_model(input: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("ER diagram should parse")
        .expect("ER diagram should be detected")
        .model
}

fn parse_er_model(input: &str) -> ErDiagramRenderModel {
    match parse_er_render_model(input) {
        RenderSemanticModel::Er(model) => model,
        other => panic!("expected ER render model, got {}", other.kind()),
    }
}

fn render_er(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let model = parse_er_render_model(input);

    render_model(&model, options)
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

fn read_local_semantic_fixture(path: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(path);
    std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()))
}

fn assert_unsupported_er_model(model: &ErDiagramRenderModel, feature: &'static str) {
    let err = merman_ascii::render_er(model, &AsciiRenderOptions::ascii())
        .expect_err("ER model should be rejected as unsupported");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature,
        }
    );
}

#[test]
fn er_parser_wide_attributes_and_summary_labels_preserve_relation_visibility() {
    let options = AsciiRenderOptions::ascii().with_max_grid_cells(1);

    let rendered = render_er(
        r#"erDiagram
CUSTOMER {
  string 名称
}
ORDER {
  string 状态🚀
}
AUDIT
CUSTOMER ||--o{ ORDER : "下单🚀"
ORDER ||--|| AUDIT : "记录数据"
"#,
        &options,
    )
    .expect("ER diagram with wide attributes and relation labels should render");

    for expected in [
        "CUSTOMER",
        "名称",
        "ORDER",
        "状态🚀",
        "AUDIT",
        "relations:",
        "CUSTOMER ||--o{ ORDER",
        "ORDER    ||--|| AUDIT",
        "下单🚀",
        "记录数据",
    ] {
        assert!(
            rendered.contains(expected),
            "wide ER fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("<br>"),
        "wide ER relation summary should not leak Mermaid break syntax:\n{rendered}"
    );
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
fn er_parser_attribute_keys_and_comments_render_in_entity_section() {
    let rendered = render_er(
        "erDiagram\nORDER {\n  int id PK\n  int customer_id FK \"owner id\"\n  string email UK\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    for expected in [
        "int id PK",
        "int customer_id FK owner id",
        "string email UK",
    ] {
        assert!(
            rendered.contains(expected),
            "ER attribute details should keep {expected:?} visible:\n{rendered}"
        );
    }
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
fn er_parser_reversed_one_or_more_cardinality_renders_normalized_marker() {
    let rendered = render_er("erDiagram\nA }|--|| B : has", &AsciiRenderOptions::ascii())
        .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " |{\n", " has\n", "  |\n", " ||\n", "+---+\n",
            "| B |\n", "+---+\n",
        )
    );
}

#[test]
fn er_parser_reversed_zero_or_more_cardinality_renders_normalized_marker() {
    let rendered = render_er("erDiagram\nA }o--|| B : has", &AsciiRenderOptions::ascii())
        .expect("ER should render");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", " o{\n", " has\n", "  |\n", " ||\n", "+---+\n",
            "| B |\n", "+---+\n",
        )
    );
}

#[test]
fn er_render_model_rejects_unknown_cardinality_markers() {
    let mut model = parse_er_model("erDiagram\nA ||--|| B : relates");
    model
        .relationships
        .first_mut()
        .expect("fixture should contain one relationship")
        .rel_spec
        .card_a = "MANY".to_string();

    assert_unsupported_er_model(&model, "unknown ER cardinality markers");
}

#[test]
fn er_render_model_rejects_unknown_relationship_identification_types() {
    let mut model = parse_er_model("erDiagram\nA ||--|| B : relates");
    model
        .relationships
        .first_mut()
        .expect("fixture should contain one relationship")
        .rel_spec
        .rel_type = "NEITHER".to_string();

    assert_unsupported_er_model(&model, "unknown ER relationship identification types");
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
fn er_parser_bidirectional_relationship_layout_renders_reverse_lanes() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba",
        &AsciiRenderOptions::ascii(),
    )
    .expect("bidirectional ER relationships should render distinct lanes");

    assert_eq!(
        rendered,
        concat!(
            "   +---+\n",
            "   | A |\n",
            "   +---+\n",
            " ||    ||\n",
            " ab     |\n",
            "  |    ba\n",
            " ||    ||\n",
            "   +---+\n",
            "   | B |\n",
            "   +---+\n",
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
            "        +---+\n",
            "        | A |\n",
            "        +---+\n",
            "      || || ||\n",
            "    a  |  b c:\n",
            "  +----++.+++++\n",
            " ||    o{    ||\n",
            "   +---+    +---+\n",
            "   | B |    | C |\n",
            "   +---+    +---+\n",
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
fn er_parser_cyclic_relationship_layout_routes_reverse_spanning_edge() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nB ||--|| C : owns\nC ||--|| A : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("cyclic ER relationships should render");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "      ||   ||\n",
            "     owns   |\n",
            "       |    |\n",
            "      ||    |\n",
            "     +---+  |\n",
            "     | B |  |\n",
            "     +---+  |\n",
            "      ||    |\n",
            "     owns   |\n",
            "       |  owns\n",
            "      ||   ||\n",
            "     +---+\n",
            "     | C |\n",
            "     +---+\n",
        )
    );
}

#[test]
fn er_parser_dense_crossing_relationships_fall_back_to_relation_summary() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba\nA ||--|| C : ac\nC ||--|| A : ca\nB ||--|| C : bc\nC ||--|| B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense ER relationships should render through relation summary fallback");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n",
            "| A |\n",
            "+---+\n",
            "\n",
            "+---+\n",
            "| B |\n",
            "+---+\n",
            "\n",
            "+---+\n",
            "| C |\n",
            "+---+\n",
            "\n",
            "relations:\n",
            "A ||--|| B : ab\n",
            "B ||--|| A : ba\n",
            "A ||--|| C : ac\n",
            "C ||--|| A : ca\n",
            "B ||--|| C : bc\n",
            "C ||--|| B : cb\n",
        )
    );
}

#[test]
fn er_parser_relationship_layout_falls_back_to_summary_when_grid_budget_is_tight() {
    let options = AsciiRenderOptions::ascii().with_max_grid_cells(1);

    let rendered = render_er(
        "erDiagram\nCUSTOMER\nORDER\nINVOICE\nCUSTOMER ||--o{ ORDER : \"places<br>orders\"\nORDER ||--|| INVOICE : bills",
        &options,
    )
    .expect("ER relationships should fall back to relation summary when grid budget is tight");

    for expected in [
        "CUSTOMER",
        "ORDER",
        "INVOICE",
        "relations:",
        "CUSTOMER ||--o{ ORDER",
        "||--|| INVOICE",
        "places",
        "bills",
        "orders",
    ] {
        assert!(
            rendered.contains(expected),
            "tight-budget ER relation summary should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains(" / "),
        "tight-budget ER relation summary should keep multiline labels as continuation rows:\n{rendered}"
    );
}

#[test]
fn er_color_html_wraps_dense_relation_summary_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x505050));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba\nA ||--|| C : ac\nC ||--|| A : ca\nB ||--|| C : bc\nC ||--|| B : cb",
        &options,
    )
    .expect("dense ER diagram should render");

    assert_eq!(
        strip_html_spans(&rendered),
        concat!(
            "+---+\n",
            "| A |\n",
            "+---+\n",
            "\n",
            "+---+\n",
            "| B |\n",
            "+---+\n",
            "\n",
            "+---+\n",
            "| C |\n",
            "+---+\n",
            "\n",
            "relations:\n",
            "A ||--|| B : ab\n",
            "B ||--|| A : ba\n",
            "A ||--|| C : ac\n",
            "C ||--|| A : ca\n",
            "B ||--|| C : bc\n",
            "C ||--|| B : cb\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+---+</span>",
        "<span style=\"color:#202020\">A</span>",
        "<span style=\"color:#303030\">relations:</span>",
        "<span style=\"color:#505050\">A ||--|| B : ab</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

#[test]
fn er_local_semantic_fixture_covers_dense_relationships() {
    let input = read_local_semantic_fixture("er/dense_relations.mmd");

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

#[test]
fn er_local_semantic_fixture_covers_dense_multiline_relation_summary() {
    let input = read_local_semantic_fixture("er/dense_multiline_relations.mmd");

    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("dense multiline local semantic ER fixture should render");

    for expected in [
        "CUSTOMER",
        "ORDER",
        "INVOICE",
        "PAYMENT",
        "relations:",
        "CUSTOMER ||--o{ ORDER",
        "places",
        "orders",
        "belongs",
        "to",
        "reconciles",
        "payment",
        "captures",
        "funds",
    ] {
        assert!(
            rendered.contains(expected),
            "dense multiline semantic ER fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains(" / "),
        "dense multiline semantic ER fixture should keep label lines structured instead of slash-joining them:\n{rendered}"
    );
    assert!(
        !rendered.contains("<br>"),
        "dense multiline semantic ER fixture should not leak Mermaid break syntax:\n{rendered}"
    );
}

#[test]
fn er_local_semantic_fixture_covers_routed_schema_with_attributes() {
    let input = read_local_semantic_fixture("er/routed_schema_with_attributes.mmd");

    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("routed schema ER fixture should render");

    for expected in [
        "CUSTOMER",
        "ORDER",
        "LINE_ITEM",
        "PRODUCT",
        "string id PK",
        "string email UK",
        "int quantity",
        "places",
        "contains",
        "supplies",
    ] {
        assert!(
            rendered.contains(expected),
            "routed schema fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "routed schema fixture should remain a routed grid, not a summary:\n{rendered}"
    );
}

#[test]
fn er_local_semantic_fixture_covers_disconnected_components() {
    let input = read_local_semantic_fixture("er/disconnected_components.mmd");

    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("disconnected ER fixture should render");

    for expected in ["CUSTOMER", "ORDER", "AUDIT_LOG", "places"] {
        assert!(
            rendered.contains(expected),
            "disconnected ER fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "disconnected ER fixture should stay as a routed grid, not a summary:\n{rendered}"
    );

    let line_index = |needle: &str| {
        rendered
            .lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
    };
    assert!(
        line_index("CUSTOMER") < line_index("AUDIT_LOG"),
        "isolated ER entity should remain visually separate from the connected component:\n{rendered}"
    );
}
