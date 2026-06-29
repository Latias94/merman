use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions, AsciiRgb,
    render_model,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn parse_class_render_model(input: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("class diagram should parse")
        .expect("class diagram should be detected")
        .model
}

fn parse_class_model(input: &str) -> ClassDiagram {
    match parse_class_render_model(input) {
        RenderSemanticModel::Class(model) => model,
        other => panic!("expected class render model, got {}", other.kind()),
    }
}

fn render_class(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let model = parse_class_render_model(input);

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

fn first_line_index_containing(rendered: &str, needle: &str) -> usize {
    rendered
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn assert_unsupported_class_model(model: &ClassDiagram, feature: &'static str) {
    let err = merman_ascii::render_class(model, &AsciiRenderOptions::ascii())
        .expect_err("class model should be rejected as unsupported");

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature,
        }
    );
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
fn class_parser_generic_class_titles_render_type_parameters() {
    let rendered = render_class(
        "classDiagram
    direction TB
    class Repository~T~
    <<interface>> Repository~T~
    class Service~T~ {
      +get(id: String) T
    }
    class SqlRepo~T~ {
      +get(id: String) T
    }
    Repository~T~ <|.. SqlRepo~T~
    Service~T~ ..> Repository~T~ : depends",
        &AsciiRenderOptions::ascii(),
    )
    .expect("generic class diagram should render");

    for expected in ["Repository<T>", "Service<T>", "SqlRepo<T>"] {
        assert!(
            rendered.contains(expected),
            "generic class title should keep {expected:?} visible:\n{rendered}"
        );
    }
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
fn class_local_semantic_fixture_covers_namespace_qualified_relationships() {
    let input = read_local_semantic_fixture("class/namespace_qualified_relationships.mmd");

    let rendered = render_class(&input, &AsciiRenderOptions::unicode())
        .expect("namespace-qualified class relationships should render");

    assert!(!rendered.contains("Platform.FFI.DartBinding"));
    assert!(!rendered.contains("Platform.FFI.PythonBinding"));
    assert!(!rendered.contains("Platform.Core.Renderer"));
    assert!(rendered.contains("DartBinding"));
    assert!(rendered.contains("PythonBinding"));
    assert!(rendered.contains("Renderer"));
    assert!(rendered.contains("calls"));
    assert!(
        rendered.lines().count() >= 4,
        "namespace-qualified class fixture should produce a non-trivial multi-line layout:\n{rendered}"
    );
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
fn class_parser_association_relation_renders_plain_line_without_marker() {
    let rendered = render_class(
        "classDiagram\nclass Student\nclass Course\nStudent -- Course : enrolls",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+---------+\n",
            "| Student |\n",
            "+---------+\n",
            "     |\n",
            "  enrolls\n",
            "     |\n",
            "+--------+\n",
            "| Course |\n",
            "+--------+\n",
        )
    );
}

#[test]
fn class_parser_dotted_association_relation_renders_plain_dotted_line_without_marker() {
    let rendered = render_class(
        "classDiagram\nclass Student\nclass Course\nStudent .. Course : observes",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+---------+\n",
            "| Student |\n",
            "+---------+\n",
            "     :\n",
            " observes\n",
            "     :\n",
            "+--------+\n",
            "| Course |\n",
            "+--------+\n",
        )
    );
}

#[test]
fn class_parser_endpoint_labels_render_near_relation_endpoints() {
    let rendered = render_class(
        "classDiagram\nclass Customer\nclass Order\nCustomer \"1\" --> \"*\" Order : places",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+----------+\n",
            "| Customer |\n",
            "+----------+\n",
            "      1\n",
            "      |\n",
            "   places\n",
            "      v\n",
            "      *\n",
            "  +-------+\n",
            "  | Order |\n",
            "  +-------+\n",
        )
    );
}

#[test]
fn class_parser_reverse_extension_endpoint_labels_follow_normalized_endpoints() {
    let rendered = render_class(
        "classDiagram\nclass Child\nclass Parent\nChild \"*\" --|> \"1\" Parent : extends",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+--------+\n",
            "| Parent |\n",
            "+--------+\n",
            "     1\n",
            "     ^\n",
            "  extends\n",
            "     |\n",
            "     *\n",
            " +-------+\n",
            " | Child |\n",
            " +-------+\n",
        )
    );
}

#[test]
fn class_parser_endpoint_labels_are_routed_without_fallback_summary() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA \"1\" --> \"*\" B : ab\nB \"1\" --> \"*\" C : bc",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert!(
        !rendered.contains("relations:"),
        "endpoint-label fixture should stay routed, not summarize:\n{rendered}"
    );
    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "| A |\n", "+---+\n", "  1\n", "  |\n", " av\n", "  *\n", "+---+\n",
            "| B |\n", "+---+\n", "  1\n", "  |\n", " bv\n", "  *\n", "+---+\n", "| C |\n",
            "+---+\n",
        )
    );
}

#[test]
fn class_local_semantic_fixture_covers_wide_members_and_summary_labels() {
    let input = read_local_semantic_fixture("class/wide_members_and_summary_labels.mmd");
    let options = AsciiRenderOptions::ascii().with_max_grid_cells(1);

    let rendered = render_class(&input, &options)
        .expect("class diagram with wide member and relation labels should render");

    for expected in [
        "User",
        "名称",
        "Order",
        "状态🚀",
        "Audit",
        "relations:",
        "User  --> Order",
        "Order --> Audit",
        "创建🚀",
        "记录数据",
    ] {
        assert!(
            rendered.contains(expected),
            "wide class fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("<br>"),
        "wide class relation summary should not leak Mermaid break syntax:\n{rendered}"
    );
}

#[test]
fn class_local_semantic_fixture_covers_annotation_methods() {
    let input = read_local_semantic_fixture("class/annotation_methods.mmd");
    let rendered = render_class(&input, &AsciiRenderOptions::ascii())
        .expect("class annotation and methods fixture should render");

    for expected in [
        "<<abstract>>",
        "Shape",
        "+draw() : void",
        "Circle",
        "+radius int",
        "^",
    ] {
        assert!(
            rendered.contains(expected),
            "class annotation fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    assert!(
        first_line_index_containing(&rendered, "Shape")
            < first_line_index_containing(&rendered, "Circle"),
        "inheritance should keep Shape before Circle in the routed terminal layout:\n{rendered}"
    );
}

#[test]
fn class_parser_lollipop_relation_renders_interface_node() {
    let rendered = render_class(
        "classDiagram\nIService ()-- Service",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+---------------+\n",
            "| <<interface>> |\n",
            "| IService      |\n",
            "+---------------+\n",
            "        o\n",
            "        |\n",
            "   +---------+\n",
            "   | Service |\n",
            "   +---------+\n",
        )
    );
}

#[test]
fn class_local_semantic_fixture_covers_note_for_link() {
    let input = read_local_semantic_fixture("class/note_for_service.mmd");
    let rendered =
        render_class(&input, &AsciiRenderOptions::ascii()).expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+----------+\n",
            "| note     |\n",
            "| Handles  |\n",
            "| requests |\n",
            "+----------+\n",
            "      :\n",
            " +---------+\n",
            " | Service |\n",
            " +---------+\n",
        )
    );
}

#[test]
fn class_local_semantic_fixture_covers_standalone_note() {
    let input = read_local_semantic_fixture("class/standalone_note.mmd");
    let rendered =
        render_class(&input, &AsciiRenderOptions::ascii()).expect("class diagram should render");

    assert_eq!(
        rendered,
        concat!(
            "+------------+\n",
            "| note       |\n",
            "| Standalone |\n",
            "+------------+\n",
        )
    );
}

#[test]
fn class_render_model_rejects_relationships_with_multiple_markers() {
    let mut model = parse_class_model("classDiagram\nclass A\nclass B\nA <|-- B");
    let aggregation = model.constants.relation_type.aggregation;
    let composition = model.constants.relation_type.composition;
    let relation = model
        .relations
        .first_mut()
        .expect("fixture should contain one relation");
    relation.relation.type1 = aggregation;
    relation.relation.type2 = composition;

    assert_unsupported_class_model(&model, "class relationships with multiple markers");
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
fn class_parser_dense_realization_relationships_keep_dotted_summary_connector() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA ..|> B : ab\nB ..|> A : ba\nA ..> C : ac\nC ..> A : ca\nB --> C : bc\nC --> B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense realization relationships should render through relation summary fallback");

    assert!(
        rendered.contains("relations:"),
        "dense realization fixture should use relation summary:\n{rendered}"
    );
    for expected in [
        "B <|.. A : ab",
        "A <|.. B : ba",
        "A ..>  C : ac",
        "B -->  C : bc",
    ] {
        assert!(
            rendered.contains(expected),
            "dense realization summary should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("<|--"),
        "dense realization summary should not collapse dotted realization to solid inheritance:\n{rendered}"
    );
}

#[test]
fn class_parser_dense_plain_associations_keep_summary_connector() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA -- B : ab\nB -- A : ba\nA -- C : ac\nC -- A : ca\nB -- C : bc\nC -- B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense plain associations should render through relation summary fallback");

    assert!(
        rendered.contains("relations:"),
        "dense plain association fixture should use relation summary:\n{rendered}"
    );
    for expected in ["A --", "B --", "C --", ": ab", ": ba", ": bc", ": cb"] {
        assert!(
            rendered.contains(expected),
            "dense plain association summary should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("-->") && !rendered.contains("<--"),
        "dense plain association summary should not invent arrowheads:\n{rendered}"
    );
}

#[test]
fn class_parser_relation_layout_falls_back_to_summary_when_grid_budget_is_tight() {
    let options = AsciiRenderOptions::ascii().with_max_grid_cells(1);

    let rendered = render_class(
        "classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes<br>through\nService --> Repo : stores",
        &options,
    )
    .expect("class relationships should fall back to relation summary when grid budget is tight");

    for expected in [
        "Gateway",
        "Service",
        "Repo",
        "relations:",
        "Gateway --> Service : routes",
        "Service --> Repo",
        "stores",
        "through",
    ] {
        assert!(
            rendered.contains(expected),
            "tight-budget class relation summary should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains(" / "),
        "tight-budget class relation summary should keep multiline labels as continuation rows:\n{rendered}"
    );
}

#[test]
fn class_color_truecolor_marks_dense_relation_summary_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x505050));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb",
        &options,
    )
    .expect("dense class diagram should render");

    assert_eq!(
        strip_ansi(&rendered),
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
    for expected_fragment in [
        "\u{1b}[38;2;16;16;16m",
        "\u{1b}[38;2;32;32;32m",
        "\u{1b}[38;2;48;48;48mrelations:",
        "\u{1b}[38;2;80;80;80mA --> B : ab",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

#[test]
fn class_local_semantic_fixture_covers_dense_relationships() {
    let input = read_local_semantic_fixture("class/dense_relations.mmd");

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

#[test]
fn class_local_semantic_fixture_covers_dense_multiline_relation_summary() {
    let input = read_local_semantic_fixture("class/dense_multiline_relations.mmd");

    let rendered = render_class(&input, &AsciiRenderOptions::ascii())
        .expect("dense multiline local semantic class fixture should render");

    for expected in [
        "Gateway",
        "Service",
        "Repo",
        "Cache",
        "relations:",
        "Gateway --> Service : receives",
        "request",
        "Service --> Gateway : returns",
        "response",
        "persists",
        "state",
        "invalidates",
        "entry",
    ] {
        assert!(
            rendered.contains(expected),
            "dense multiline semantic class fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains(" / "),
        "dense multiline semantic class fixture should keep label lines structured instead of slash-joining them:\n{rendered}"
    );
    assert!(
        !rendered.contains("<br>"),
        "dense multiline semantic class fixture should not leak Mermaid break syntax:\n{rendered}"
    );
}

#[test]
fn class_local_semantic_fixture_covers_routed_relationship_variants() {
    let input = read_local_semantic_fixture("class/routed_relationship_variants.mmd");

    let rendered = render_class(&input, &AsciiRenderOptions::ascii())
        .expect("routed relationship variant class fixture should render");

    for expected in [
        "Shape",
        "<<interface>>",
        "Circle",
        "radius",
        "draw",
        "implements",
        "paints",
        "loads",
        "keeps",
        "contains",
    ] {
        assert!(
            rendered.contains(expected),
            "routed relationship variant fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "routed relationship variant fixture should remain a routed grid, not a summary:\n{rendered}"
    );
}

#[test]
fn class_local_semantic_fixture_covers_disconnected_components() {
    let input = read_local_semantic_fixture("class/disconnected_components.mmd");

    let rendered = render_class(&input, &AsciiRenderOptions::ascii())
        .expect("disconnected class fixture should render");

    for expected in ["Service", "Repo", "Logger", "Isolated", "fetch", "log"] {
        assert!(
            rendered.contains(expected),
            "disconnected class fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "disconnected class fixture should stay as a routed grid, not a summary:\n{rendered}"
    );

    let line_index = |needle: &str| {
        rendered
            .lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
    };
    assert!(
        line_index("Service") < line_index("Isolated"),
        "isolated class component should remain visually separate from the connected component:\n{rendered}"
    );
}
