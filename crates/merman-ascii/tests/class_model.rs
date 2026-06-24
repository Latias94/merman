use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb, render_model,
};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_class(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("class diagram should parse")
        .expect("class diagram should be detected");

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
fn class_color_truecolor_emits_semantic_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::Text, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(3, 3, 3))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(4, 4, 4))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::new(5, 5, 5));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : extends",
        &options,
    )
    .expect("class diagram should render");

    assert_eq!(
        strip_ansi(&rendered),
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
fn class_color_html_wraps_layered_relation_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x404040))
        .with_role(AsciiColorRole::Junction, AsciiRgb::from_hex24(0x505050));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog\nAnimal <|-- Cat",
        &options,
    )
    .expect("class diagram should render");

    assert_eq!(
        strip_html_spans(&rendered),
        concat!(
            "    +--------+\n",
            "    | Animal |\n",
            "    +--------+\n",
            "         ^\n",
            "         |\n",
            "   +-----+----+\n",
            "+-----+    +-----+\n",
            "| Dog |    | Cat |\n",
            "+-----+    +-----+\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+--------+</span>",
        "<span style=\"color:#202020\">Animal</span>",
        "<span style=\"color:#303030\">|</span>",
        "<span style=\"color:#404040\">^</span>",
        "<span style=\"color:#505050\">+</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

#[test]
fn class_color_html_wraps_parallel_relation_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x404040))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x505050));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : parent\nAnimal <|-- Dog : base",
        &options,
    )
    .expect("class diagram should render");

    assert_eq!(
        strip_html_spans(&rendered),
        concat!(
            " +--------+\n",
            " | Animal |\n",
            " +--------+\n",
            "  ^      ^\n",
            "parent  base\n",
            "  |      |\n",
            "   +-----+\n",
            "   | Dog |\n",
            "   +-----+\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+--------+</span>",
        "<span style=\"color:#202020\">Animal</span>",
        "<span style=\"color:#303030\">|</span>",
        "<span style=\"color:#404040\">^</span>",
        "<span style=\"color:#505050\">parent</span>",
        "<span style=\"color:#505050\">base</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
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
fn class_parser_extension_relation_renders_multiline_label() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : north<br>south",
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
            "   north\n",
            "   south\n",
            "     |\n",
            "  +-----+\n",
            "  | Dog |\n",
            "  +-----+\n",
        )
    );
}

#[test]
fn class_parser_relationship_layouts_render_unrelated_classes_as_components() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog",
        &AsciiRenderOptions::ascii(),
    )
    .expect("unrelated classes should render as separate components");

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
            "\n",
            "+-----+\n",
            "| Cat |\n",
            "+-----+\n",
        )
    );
}

#[test]
fn class_parser_parallel_relationship_layout_renders_each_lane() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : parent\nAnimal <|-- Dog : base",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel class relationships should render distinct lanes");

    assert_eq!(
        rendered,
        concat!(
            " +--------+\n",
            " | Animal |\n",
            " +--------+\n",
            "  ^      ^\n",
            "parent  base\n",
            "  |      |\n",
            "   +-----+\n",
            "   | Dog |\n",
            "   +-----+\n",
        )
    );
}

#[test]
fn class_parser_bidirectional_relationship_layout_renders_reverse_lanes() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nA --> B : ab\nB --> A : ba",
        &AsciiRenderOptions::ascii(),
    )
    .expect("bidirectional class relationships should render distinct lanes");

    assert_eq!(
        rendered,
        concat!(
            "   +---+\n",
            "   | A |\n",
            "   +---+\n",
            "  |     v\n",
            " ab     |\n",
            "  |    ba\n",
            "  v     |\n",
            "   +---+\n",
            "   | B |\n",
            "   +---+\n",
        )
    );
}

#[test]
fn class_parser_mixed_parallel_relationship_layout_renders_each_lane() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog\nAnimal <|-- Dog\nAnimal <|-- Cat",
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed parallel class relationships should render every lane");

    assert_eq!(
        rendered,
        concat!(
            "       +--------+\n",
            "       | Animal |\n",
            "       +--------+\n",
            "         ^  ^  ^\n",
            "         |  |  |\n",
            "   +-----+--+--+-+\n",
            "   +-----+    +-----+\n",
            "   | Dog |    | Cat |\n",
            "   +-----+    +-----+\n",
        )
    );
}

#[test]
fn class_parser_spanning_level_relationship_layout_routes_around_intermediate_box() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA <|-- B\nB <|-- C\nA <|-- C",
        &AsciiRenderOptions::ascii(),
    )
    .expect("spanning-level class relationship should route around the intermediate box");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "       ^    ^\n",
            "       |    |\n",
            "       |    |\n",
            "     +---+  |\n",
            "     | B |  |\n",
            "     +---+  |\n",
            "       ^    |\n",
            "       |    |\n",
            "       |    |\n",
            "     +---+\n",
            "     | C |\n",
            "     +---+\n",
        )
    );
}

#[test]
fn class_parser_cyclic_relationship_layout_routes_reverse_spanning_edge() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> C : bc\nC --> A : ca",
        &AsciiRenderOptions::ascii(),
    )
    .expect("cyclic class relationships should render");

    assert_eq!(
        rendered,
        concat!(
            "     +---+\n",
            "     | A |\n",
            "     +---+\n",
            "       |    v\n",
            "      ab    |\n",
            "       |    |\n",
            "       v    |\n",
            "     +---+  |\n",
            "     | B |  |\n",
            "     +---+  |\n",
            "       |    |\n",
            "      bc    |\n",
            "       |   ca\n",
            "       v    |\n",
            "     +---+\n",
            "     | C |\n",
            "     +---+\n",
        )
    );
}

#[test]
fn class_parser_namespace_qualified_relationships_do_not_render_duplicate_classes() {
    let rendered = render_class(
        r#"classDiagram
namespace Platform["Platform Layer"] {
  namespace FFI {
    class DartBinding
    class PythonBinding
  }
  namespace Core {
    class Renderer
  }
}
Platform.FFI.DartBinding --> Platform.Core.Renderer : calls
Platform.FFI.PythonBinding --> Platform.Core.Renderer : calls
"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("namespace-qualified class relationships should render");

    assert!(!rendered.contains("Platform.FFI.DartBinding"));
    assert!(!rendered.contains("Platform.FFI.PythonBinding"));
    assert!(!rendered.contains("Platform.Core.Renderer"));
    assert!(rendered.contains("DartBinding"));
    assert!(rendered.contains("PythonBinding"));
    assert!(rendered.contains("Renderer"));
    assert!(rendered.contains("calls"));
}

#[test]
fn class_parser_extension_star_renders_all_children() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nclass Cat\nAnimal <|-- Dog\nAnimal <|-- Cat",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "    +--------+\n",
            "    | Animal |\n",
            "    +--------+\n",
            "         ^\n",
            "         |\n",
            "   +-----+----+\n",
            "+-----+    +-----+\n",
            "| Dog |    | Cat |\n",
            "+-----+    +-----+\n",
        )
    );
}

#[test]
fn class_parser_extension_chain_renders_each_relationship() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Mammal\nclass Dog\nAnimal <|-- Mammal\nMammal <|-- Dog",
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
            "     |\n",
            "+--------+\n",
            "| Mammal |\n",
            "+--------+\n",
            "     ^\n",
            "     |\n",
            "     |\n",
            "  +-----+\n",
            "  | Dog |\n",
            "  +-----+\n",
        )
    );
}

#[test]
fn class_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nA <|-- D\nB <|-- C",
        &AsciiRenderOptions::ascii(),
    )
    .expect("crossing class relationships should render by reordering the lower layer");

    assert_eq!(
        rendered,
        concat!(
            "+---+    +---+\n",
            "| A |    | B |\n",
            "+---+    +---+\n",
            "  ^        ^\n",
            "  |        |\n",
            "  |        |\n",
            "+---+    +---+\n",
            "| D |    | C |\n",
            "+---+    +---+\n",
        )
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

#[test]
fn class_parser_dense_crossing_relationships_fall_back_to_relation_summary() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense class relationships should render through relation summary fallback");

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
            "A --> B : ab\n",
            "B --> A : ba\n",
            "A --> C : ac\n",
            "C --> A : ca\n",
            "B --> C : bc\n",
            "C --> B : cb\n",
        )
    );
}

#[test]
fn class_local_semantic_fixture_covers_dense_relationships() {
    let input = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/local-semantic/class/dense_relations.mmd"),
    )
    .expect("local semantic class fixture must be readable");

    let rendered = render_class(&input, &AsciiRenderOptions::ascii())
        .expect("dense local semantic class fixture should render");

    for expected in [
        "Service", "Repo", "Cache", "Logger", "fetch", "read", "trace",
    ] {
        assert!(
            rendered.contains(expected),
            "dense semantic class fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.lines().count() >= 6,
        "dense semantic class fixture should produce a non-trivial multi-line layout:\n{rendered}"
    );
}
