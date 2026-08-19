mod support;

use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiRgb, TerminalWidthProfile,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{Engine, OperationControl, ParseOptions};
use std::path::Path;
use support::{render_controlled_model, render_model, render_model_with_resources};

fn parse_class_render_model(input: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("class diagram should parse")
        .expect("class diagram should be detected")
        .into_parts()
        .1
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

fn render_class_with_resources(
    input: &str,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let model = parse_class_render_model(input);

    render_model_with_resources(&model, options, resources)
}

fn render_class_model(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    render_model(&RenderSemanticModel::Class(model.clone()), options)
}

fn render_class_with_grid_limit(
    input: &str,
    options: &AsciiRenderOptions,
    max_grid_cells: usize,
) -> merman_ascii::Result<String> {
    let model = parse_class_render_model(input);
    let control = OperationControl::new();
    let context = Engine::new()
        .begin_operation()
        .expect("deterministic operation context should be available");
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, max_grid_cells)
        .expect("valid operation grid limit");

    render_controlled_model(&model, options, &control, &context, resources)
}

fn assert_unsupported_class_model(model: &ClassDiagram, feature: &'static str) {
    let error = render_class_model(model, &AsciiRenderOptions::ascii())
        .expect_err("class model should be rejected as unsupported");

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature,
        }
    );
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
        if rest.starts_with("&gt;") {
            output.push('>');
            index += "&gt;".len();
            continue;
        }
        if rest.starts_with("&lt;") {
            output.push('<');
            index += "&lt;".len();
            continue;
        }
        if rest.starts_with("&amp;") {
            output.push('&');
            index += "&amp;".len();
            continue;
        }
        if rest.starts_with("&quot;") {
            output.push('"');
            index += "&quot;".len();
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

fn line_and_column_containing(rendered: &str, needle: &str) -> (usize, usize) {
    rendered
        .lines()
        .enumerate()
        .find_map(|(line, text)| text.find(needle).map(|column| (line, column)))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn framed_class_summary_endpoint(id: &str) -> String {
    format!(r#"id(bytes={})="{id}""#, id.len())
}

fn framed_class_summary_relation(
    source: &str,
    connector: &str,
    target: &str,
    label: Option<&str>,
) -> String {
    let relation = format!(
        "{} {connector} {}",
        framed_class_summary_endpoint(source),
        framed_class_summary_endpoint(target),
    );
    match label {
        Some(label) => format!("{relation} : {label}"),
        None => relation,
    }
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

    let parent = framed_class_summary_relation("Animal", "<|--", "Dog", Some("parent"));
    let base = framed_class_summary_relation("Animal", "<|--", "Dog", Some("base"));
    assert_eq!(
        strip_html_spans(&rendered),
        format!(
            concat!(
                "+--------+\n",
                "| Animal |\n",
                "+--------+\n",
                "\n",
                "+-----+\n",
                "| Dog |\n",
                "+-----+\n",
                "\n",
                "relations:\n",
                "{}\n",
                "{}\n",
            ),
            parent, base,
        )
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+--------+</span>",
        "<span style=\"color:#202020\">Animal</span>",
        "<span style=\"color:#505050\">id(bytes=6)=&quot;Animal&quot; &lt;|-- id(bytes=3)=&quot;Dog&quot; : parent</span>",
        "<span style=\"color:#505050\">id(bytes=6)=&quot;Animal&quot; &lt;|-- id(bytes=3)=&quot;Dog&quot; : base</span>",
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
fn class_render_model_rejects_relations_without_endpoint_classes() {
    let mut model = parse_class_model("classDiagram\nclass A\nclass B\nA --> B : uses");
    model.classes.clear();

    assert_unsupported_class_model(&model, "relationships with missing endpoint classes");
}

#[test]
fn class_render_model_rejects_missing_relation_endpoints_before_namespace_rendering() {
    let mut model = parse_class_model(
        "classDiagram\nnamespace Domain {\n  class A\n}\nclass B\nA --> B : uses",
    );
    model.relations[0].id2 = "Missing".to_string();

    assert_unsupported_class_model(&model, "relationships with missing endpoint classes");
}

#[test]
fn class_render_model_rejects_missing_note_targets_before_namespace_rendering() {
    let mut model = parse_class_model(
        "classDiagram\nnamespace Domain {\n  class A\n  note for A \"linked\"\n}",
    );
    model.notes[0].class_id = Some("Missing".to_string());

    assert_unsupported_class_model(&model, "notes with missing target classes");
}

#[test]
fn class_render_model_rejects_missing_interface_targets() {
    let mut model = parse_class_model("classDiagram\nIService ()-- Service");
    model.interfaces[0].class_id = "Missing".to_string();

    assert_unsupported_class_model(&model, "interfaces with missing target classes");
}

#[test]
fn class_render_model_rejects_inconsistent_namespace_class_ownership() {
    let mut model = parse_class_model("classDiagram\nnamespace Domain {\n  class A\n}");
    model
        .namespaces
        .get_mut("Domain")
        .expect("Domain namespace should exist")
        .class_ids
        .clear();

    assert_unsupported_class_model(&model, "inconsistent class namespace ownership");
}

#[test]
fn class_render_model_rejects_inconsistent_namespace_note_ownership() {
    let mut model = parse_class_model(
        "classDiagram\nnamespace Domain {\n  class A\n  note for A \"linked\"\n}",
    );
    model
        .namespaces
        .get_mut("Domain")
        .expect("Domain namespace should exist")
        .note_ids
        .clear();

    assert_unsupported_class_model(&model, "inconsistent class namespace ownership");
}

#[test]
fn class_render_model_rejects_duplicate_rendered_ids() {
    let mut model = parse_class_model("classDiagram\nclass A\nclass B");
    model.classes.get_mut("B").expect("class B should exist").id = "A".to_string();

    assert_unsupported_class_model(&model, "duplicate rendered class ids");
}

#[test]
fn class_parser_class_and_namespace_same_id_keep_distinct_route_ownership() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "class Domain\n",
            "namespace Domain {\n",
            "  class A\n",
            "}\n",
            "Domain --> A : owns",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .expect("class and namespace semantic ids should occupy separate render-id domains");

    assert!(!rendered.contains("relations:"), "{rendered}");
    assert_eq!(
        rendered.matches("│ Domain │").count(),
        2,
        "the class and namespace titles should each render once:\n{rendered}"
    );
    assert_eq!(rendered.matches("│ A │").count(), 1, "{rendered}");
    assert!(rendered.contains("owns"), "{rendered}");
}

#[test]
fn class_parser_explicit_qualified_class_is_not_folded_into_namespace_member() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "namespace N {\n",
            "  class C\n",
            "}\n",
            "class N.C[\"Distinct\"]\n",
            "class D\n",
            "N.C --> D : calls\n",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .expect("an explicit qualified class must retain its own identity and route endpoint");

    assert_eq!(rendered.matches("│ C │").count(), 1, "{rendered}");
    assert_eq!(rendered.matches("Distinct").count(), 1, "{rendered}");
    assert!(rendered.contains("id(bytes=3)=\"N.C\""), "{rendered}");
    assert!(rendered.contains("calls"), "{rendered}");
    assert!(!rendered.contains("member(bytes=3)=\"N.C\""), "{rendered}");
}

#[test]
fn class_terminal_width_profile_preserves_complex_graphemes_and_ambiguous_width() {
    let mut model = parse_class_model("classDiagram\nclass A");
    let mut class = model
        .classes
        .shift_remove("A")
        .expect("class A should exist");
    class.id = "👩‍💻·".to_string();
    class.text = "👩‍💻·".to_string();
    model.classes.insert("👩‍💻·".to_string(), class);

    let unicode = render_class_model(
        &model,
        &AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Unicode),
    )
    .expect("class should render with Unicode terminal widths");
    let cjk = render_class_model(
        &model,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    )
    .expect("class should render with CJK terminal widths");

    assert_eq!(unicode, "+-----+\n| 👩‍💻· |\n+-----+\n");
    assert_eq!(cjk, "+------+\n| 👩‍💻· |\n+------+\n");
    assert!(
        !cjk.contains(['┌', '─', '┐', '│', '└', '┘']),
        "CJK width profiles must use single-cell structural glyphs: {cjk}"
    );
}

#[test]
fn class_terminal_normalization_discloses_the_authored_box_identity() {
    let render_identity = |identity: &str| {
        let mut model = parse_class_model("classDiagram\nclass A");
        let mut class = model
            .classes
            .shift_remove("A")
            .expect("class A should exist");
        class.id = identity.to_string();
        class.text = identity.to_string();
        model.classes.insert(identity.to_string(), class);
        render_class_model(&model, &AsciiRenderOptions::ascii())
            .expect("direct class identity should render")
    };

    let control = render_identity("\u{1b}");
    let authored_escape = render_identity(r"\u{1B}");

    assert!(control.contains(r#"id(bytes=1)="\u{1B}""#), "{control}");
    assert_ne!(control, authored_escape);
}

#[test]
fn class_single_line_display_projection_discloses_authored_text() {
    let render_display = |display: &str| {
        let mut model = parse_class_model("classDiagram\nclass A");
        model
            .classes
            .get_mut("A")
            .expect("class A should exist")
            .text = display.to_string();
        render_class_model(&model, &AsciiRenderOptions::ascii())
            .expect("direct class display text should render")
    };

    for (authored, projected_literal, disclosure) in [
        ("\u{1b}", r"\u{1B}", r#"text(bytes=1)="\u{1B}""#),
        ("\n", r"\u{A}", r#"text(bytes=1)="\n""#),
        ("&lt;", "<", r#"text(bytes=4)="&lt;""#),
    ] {
        let transformed = render_display(authored);
        let literal = render_display(projected_literal);

        assert!(
            transformed.contains(disclosure),
            "missing authored display disclosure {disclosure:?}:\n{transformed}"
        );
        assert!(
            transformed.contains(r#"id(bytes=1)="A""#),
            "the fixed class identity should remain disclosed:\n{transformed}"
        );
        assert_ne!(transformed, literal);
    }
}

#[test]
fn class_relationship_labels_preserve_complex_graphemes() {
    let mut model = parse_class_model("classDiagram\nclass A\nclass B\nA --> B : owns");
    model
        .classes
        .get_mut("A")
        .expect("class A should exist")
        .text = "Client 👩‍💻".to_string();
    model.relations[0].title = "owns 👩‍💻".to_string();
    let rendered = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("class relationship should render");

    assert!(rendered.contains("Client 👩‍💻"), "{rendered}");
    assert!(rendered.contains("owns 👩‍💻"), "{rendered}");
    assert!(!rendered.contains("relations:"), "{rendered}");
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
fn class_parser_member_classifiers_remain_visible() {
    let rendered = render_class(
        "classDiagram\nclass Service {\n  +abstractValue*\n  +staticValue$\n  +abstractCall()*\n  +staticCall()$\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("classified class members should render");

    for expected in [
        "+abstractValue*",
        "+staticValue$",
        "+abstractCall()*",
        "+staticCall()$",
    ] {
        assert!(
            rendered.contains(expected),
            "classified member {expected:?} should remain visible:\n{rendered}"
        );
    }
}

#[test]
fn class_render_model_reconstructs_members_when_display_text_is_empty() {
    let mut model =
        parse_class_model("classDiagram\nclass Service {\n  +value\n  #compute(input) Result\n}");
    let class = model
        .classes
        .get_mut("Service")
        .expect("Service class should exist");
    let member = class
        .members
        .first_mut()
        .expect("Service should have an attribute");
    member.visibility = "+".to_string();
    member.id = "items List~T~".to_string();
    member.classifier = "$".to_string();
    member.display_text.clear();
    let method = class
        .methods
        .first_mut()
        .expect("Service should have a method");
    method.visibility = "#".to_string();
    method.id = "compute~T~".to_string();
    method.parameters = "  items: List~List~T~~  ".to_string();
    method.return_type = "  Result~List~T~~  ".to_string();
    method.classifier = "*".to_string();
    method.display_text.clear();

    let rendered = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("typed member fields should reconstruct a terminal display");

    assert!(rendered.contains("+items List<T>$"), "{rendered}");
    assert!(
        rendered.contains("#compute<T>(items: List<List<T>>) : Result<List<T>>*"),
        "{rendered}"
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
fn class_parser_direction_controls_terminal_layout() {
    let render = |direction| {
        render_class(
            &format!("classDiagram\ndirection {direction}\nclass A\nclass B\nA --> B"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|err| panic!("class direction {direction} should render: {err}"))
    };

    let tb = render("TB");
    let bt = render("BT");
    let lr = render("LR");
    let rl = render("RL");

    assert!(
        first_line_index_containing(&tb, "| A |") < first_line_index_containing(&tb, "| B |"),
        "TB should place A above B:\n{tb}"
    );
    assert!(
        first_line_index_containing(&bt, "| B |") < first_line_index_containing(&bt, "| A |"),
        "BT should place B above A:\n{bt}"
    );

    let (lr_a_line, lr_a_column) = line_and_column_containing(&lr, "| A |");
    let (lr_b_line, lr_b_column) = line_and_column_containing(&lr, "| B |");
    assert_eq!(lr_a_line, lr_b_line, "LR should share one row:\n{lr}");
    assert!(
        lr_a_column < lr_b_column,
        "LR should place A left of B:\n{lr}"
    );

    let (rl_a_line, rl_a_column) = line_and_column_containing(&rl, "| A |");
    let (rl_b_line, rl_b_column) = line_and_column_containing(&rl, "| B |");
    assert_eq!(rl_a_line, rl_b_line, "RL should share one row:\n{rl}");
    assert!(
        rl_b_column < rl_a_column,
        "RL should place B left of A:\n{rl}"
    );

    assert_ne!(tb, bt);
    assert_ne!(tb, lr);
    assert_ne!(lr, rl);

    let mut lowercase_model =
        parse_class_model("classDiagram\ndirection LR\nclass A\nclass B\nA --> B");
    lowercase_model.direction = "lr".to_string();
    assert_eq!(
        render_class_model(&lowercase_model, &AsciiRenderOptions::ascii())
            .expect("lowercase direct-model class direction should render"),
        lr
    );
}

#[test]
fn class_bottom_up_summary_preserves_semantic_endpoint_roles() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction BT\n",
            "class A\n",
            "class B\n",
            "A \"source\" <|--|> \"target\" B : first\n",
            "A \"source2\" <|--|> \"target2\" B : second",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("bottom-up parallel class relationships should preserve semantic roles");

    let summary = rendered
        .split_once("relations:\n")
        .expect("parallel relationships should use structured fallback")
        .1;
    for (source_label, target_label) in [("source", "target"), ("source2", "target2")] {
        let line = summary
            .lines()
            .find(|line| line.contains(source_label))
            .unwrap_or_else(|| panic!("missing {source_label:?} summary row:\n{rendered}"));
        assert!(
            line.starts_with(&framed_class_summary_endpoint("A")),
            "{line}"
        );
        assert!(line.contains(&framed_class_summary_endpoint("B")), "{line}");
        assert!(line.contains("<|--|>"), "{line}");
        assert!(
            line.contains("endpoint1=[") && line.contains(source_label),
            "{line}"
        );
        assert!(
            line.contains("endpoint2=[") && line.contains(target_label),
            "{line}"
        );
    }
}

#[test]
fn class_direction_bytes_are_admitted_before_parsing() {
    let mut model = parse_class_model("classDiagram\nclass A");
    model.direction = format!("{}sideways", " ".repeat(1_024));
    let direction_bytes = model.direction.len();
    let model = RenderSemanticModel::Class(model);
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("class direction work limit should be valid");

    let error = render_model_with_resources(&model, &AsciiRenderOptions::ascii(), resources)
        .expect_err("direction bytes must be admitted before direction validation");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == direction_bytes
                && details.max == 1
    ));
}

#[test]
fn class_parser_horizontal_component_draws_each_class_once() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "class A\n",
            "class B\n",
            "class C\n",
            "A <|-- B : parent\n",
            "A ..> B : depends\n",
            "B --> C : next",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal class component should render");

    for title in ["| A |", "| B |", "| C |"] {
        assert_eq!(
            rendered.matches(title).count(),
            1,
            "a horizontal component must place {title:?} exactly once:\n{rendered}"
        );
    }
    for label in ["parent", "depends", "next"] {
        assert!(
            rendered.contains(label),
            "horizontal routing must preserve {label:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_horizontal_unrelated_edge_crossings_use_lossless_summary() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "class A\n",
            "class B\n",
            "class C\n",
            "class D\n",
            "A --> C : first\n",
            "B ..> D : second\n",
            "A --> B : bridge",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("unrelated horizontal crossings should remain recoverable");

    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in [
        framed_class_summary_relation("A", "-->", "C", Some("first")),
        framed_class_summary_relation("B", "..>", "D", Some("second")),
        framed_class_summary_relation("A", "-->", "B", Some("bridge")),
    ] {
        assert!(
            rendered.contains(&expected),
            "summary must preserve {expected:?} after owner crossing fallback:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_horizontal_shared_source_crossings_use_lossless_summary() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "class A\n",
            "class B\n",
            "class C\n",
            "A --> B : short\n",
            "A --> C : long",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("shared-source horizontal crossings should remain recoverable");

    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in [
        framed_class_summary_relation("A", "-->", "B", Some("short")),
        framed_class_summary_relation("A", "-->", "C", Some("long")),
    ] {
        assert!(
            rendered.contains(&expected),
            "summary must preserve {expected:?} after shared-source crossing fallback:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_horizontal_long_label_keeps_connector_attached_to_ports() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "A \"1\" <|--|> \"*\" B : relationship label that forces a wide lane",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("wide horizontal class relationship should render");

    let (_, a_column) = line_and_column_containing(&rendered, "| A |");
    let (_, b_column) = line_and_column_containing(&rendered, "| B |");
    let connector = rendered
        .lines()
        .find(|line| line.contains("<|") && line.contains("|>"))
        .unwrap_or_else(|| panic!("missing two-sided horizontal connector:\n{rendered}"));
    let between_boxes = &connector[a_column + "| A |".len()..b_column];

    assert!(
        !between_boxes.contains(' '),
        "the connector must span the complete port-to-port gap:\n{rendered}"
    );
    for expected in ["1", "*", "relationship label that forces a wide lane"] {
        assert!(
            rendered.contains(expected),
            "horizontal relation must preserve {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_horizontal_parallel_self_relations_share_one_box() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "class Node\n",
            "Node --> Node : next\n",
            "Node ..> Node : loads",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal parallel self relations should render");

    assert_eq!(
        rendered.matches("| Node |").count(),
        1,
        "parallel self relations must share one class box:\n{rendered}"
    );
    for label in ["next", "loads"] {
        assert!(
            rendered.contains(label),
            "parallel self relation must preserve {label:?}:\n{rendered}"
        );
    }
    assert!(
        rendered.matches('v').count() >= 2,
        "each self relation must retain its terminal marker:\n{rendered}"
    );
}

#[test]
fn class_parser_horizontal_mixed_self_and_normal_relations_use_lossless_summary() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction LR\n",
            "class A\n",
            "class B\n",
            "A --> A : self\n",
            "A --> B : next",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed horizontal class relations should remain recoverable");

    assert_eq!(rendered.matches("| A |").count(), 1, "{rendered}");
    assert_eq!(rendered.matches("| B |").count(), 1, "{rendered}");
    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in [
        framed_class_summary_relation("A", "-->", "A", Some("self")),
        framed_class_summary_relation("A", "-->", "B", Some("next")),
    ] {
        assert!(
            rendered.contains(&expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_horizontal_direction_propagates_resource_errors() {
    let input = "classDiagram\ndirection LR\nclass A\nclass B\nA --> B";

    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
    ] {
        let resources = AsciiResourcePolicy::default()
            .with_limit(limit, 1)
            .expect("horizontal class resource limit should be valid");
        let error = render_class_with_resources(input, &AsciiRenderOptions::ascii(), resources)
            .expect_err("horizontal class rendering must propagate resource errors");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details) if details.limit == limit
        ));
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

    for expected in [
        "Animal",
        "Dog",
        "north",
        "south",
        r#"authored(bytes=14)="north<br>south""#,
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
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
fn class_parser_parallel_relationship_layout_uses_lossless_summary_when_ports_do_not_fit() {
    let rendered = render_class(
        "classDiagram\nclass Animal\nclass Dog\nAnimal <|-- Dog : parent\nAnimal <|-- Dog : base",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel class relationships should preserve every relation");

    let parent = framed_class_summary_relation("Animal", "<|--", "Dog", Some("parent"));
    let base = framed_class_summary_relation("Animal", "<|--", "Dog", Some("base"));
    assert_eq!(
        rendered,
        format!(
            concat!(
                "+--------+\n",
                "| Animal |\n",
                "+--------+\n",
                "\n",
                "+-----+\n",
                "| Dog |\n",
                "+-----+\n",
                "\n",
                "relations:\n",
                "{}\n",
                "{}\n",
            ),
            parent, base,
        )
    );
}

#[test]
fn class_parser_bidirectional_relationship_layout_preserves_both_directions_in_summary() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nA --> B : ab\nB --> A : ba",
        &AsciiRenderOptions::ascii(),
    )
    .expect("bidirectional class relationships should remain recoverable");

    let ab = framed_class_summary_relation("A", "-->", "B", Some("ab"));
    let ba = framed_class_summary_relation("B", "-->", "A", Some("ba"));
    assert_eq!(
        rendered,
        format!(
            concat!(
                "+---+\n",
                "| A |\n",
                "+---+\n",
                "\n",
                "+---+\n",
                "| B |\n",
                "+---+\n",
                "\n",
                "relations:\n",
                "{}\n",
                "{}\n",
            ),
            ab, ba,
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
fn class_parser_spanning_level_relationship_layout_summarizes_invalid_outer_port() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA <|-- B\nB <|-- C\nA <|-- C",
        &AsciiRenderOptions::ascii(),
    )
    .expect("spanning-level class relationships should remain recoverable");

    let ab = framed_class_summary_relation("A", "<|--", "B", None);
    let bc = framed_class_summary_relation("B", "<|--", "C", None);
    let ac = framed_class_summary_relation("A", "<|--", "C", None);
    assert_eq!(
        rendered,
        format!(
            concat!(
                "+---+\n| A |\n+---+\n\n",
                "+---+\n| B |\n+---+\n\n",
                "+---+\n| C |\n+---+\n\n",
                "relations:\n",
                "{}\n",
                "{}\n",
                "{}\n",
            ),
            ab, bc, ac,
        )
    );
}

#[test]
fn class_parser_cyclic_relationship_layout_summarizes_disconnected_back_edge() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> C : bc\nC --> A : ca",
        &AsciiRenderOptions::ascii(),
    )
    .expect("cyclic class relationships should render");

    let ab = framed_class_summary_relation("A", "-->", "B", Some("ab"));
    let bc = framed_class_summary_relation("B", "-->", "C", Some("bc"));
    let ca = framed_class_summary_relation("C", "-->", "A", Some("ca"));
    assert_eq!(
        rendered,
        format!(
            concat!(
                "+---+\n| A |\n+---+\n\n",
                "+---+\n| B |\n+---+\n\n",
                "+---+\n| C |\n+---+\n\n",
                "relations:\n",
                "{}\n",
                "{}\n",
                "{}\n",
            ),
            ab, bc, ca,
        )
    );
}

#[test]
fn class_parser_parallel_relationship_layout_keeps_diagram_when_ports_fit() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "class VeryWideParent\n",
            "class VeryWideChild\n",
            "VeryWideParent <|-- VeryWideChild : p\n",
            "VeryWideParent <|-- VeryWideChild : b",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("wide parallel class relationships should keep distinct lanes");

    assert!(!rendered.contains("relations:"), "{rendered}");
    assert_eq!(rendered.matches('^').count(), 2, "{rendered}");
    for label in ["p", "b"] {
        assert!(rendered.contains(label), "missing {label:?}:\n{rendered}");
    }
}

#[test]
fn class_local_semantic_fixture_covers_namespace_qualified_relationships() {
    let input = read_local_semantic_fixture("class/namespace_qualified_relationships.mmd");

    let rendered = render_class(&input, &AsciiRenderOptions::unicode())
        .expect("namespace-qualified class relationships should render");

    for (framing, authored_endpoint) in [
        ("endpointRef(bytes=24)=", "Platform.FFI.DartBinding"),
        ("endpointRef(bytes=26)=", "Platform.FFI.PythonBinding"),
        ("endpointRef(bytes=22)=", "Platform.Core.Renderer"),
    ] {
        assert!(
            rendered.contains(framing) && rendered.contains(authored_endpoint),
            "qualified relation endpoint should retain {authored_endpoint:?}:\n{rendered}"
        );
    }
    assert!(rendered.contains("Platform Layer"));
    assert!(rendered.contains("FFI"));
    assert!(rendered.contains("Core"));
    assert!(rendered.contains("DartBinding"));
    assert!(rendered.contains("PythonBinding"));
    assert!(rendered.contains("Renderer"));
    assert!(rendered.contains("relations:"), "{rendered}");
    for endpoint in [
        "member(bytes=11)=\\\"DartBinding\\\"",
        "member(bytes=13)=\\\"PythonBinding\\\"",
        "member(bytes=8)=\\\"Renderer\\\"",
    ] {
        assert!(
            rendered.contains(endpoint),
            "namespace relation summaries should preserve injective endpoint framing {endpoint:?}:\n{rendered}"
        );
    }
    assert_eq!(rendered.matches("calls").count(), 2, "{rendered}");
    assert!(
        rendered.lines().count() >= 20,
        "namespace-qualified class fixture should produce a non-trivial multi-line layout:\n{rendered}"
    );
}

#[test]
fn class_parser_namespace_containers_render_without_relationships() {
    let rendered = render_class(
        "classDiagram
namespace Domain[\"Domain Layer\"] {
  class User
  namespace Persistence {
    class UserRepo
  }
}
class Outside",
        &AsciiRenderOptions::unicode(),
    )
    .expect("namespace containers should render");

    for expected in ["Domain Layer", "Persistence", "UserRepo", "User", "Outside"] {
        assert!(
            rendered.contains(expected),
            "namespace class output should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "namespace containers without relationships should not add a relation summary:\n{rendered}"
    );
    assert!(
        rendered.matches('┌').count() >= 4,
        "nested namespace output should contain nested terminal boxes:\n{rendered}"
    );
}

#[test]
fn class_parser_namespace_aliases_disclose_utf8_authored_identity() {
    let render = |root: &str| {
        render_class(
            &format!(
                "classDiagram\nnamespace {root}[\"Shared\"] {{\n  namespace API[\"Nested\"] {{\n    class C\n  }}\n}}"
            ),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("aliased namespace {root:?} should render: {error}"))
    };

    let domain = render("领域");
    let boundary = render("边界");

    assert_ne!(
        domain, boundary,
        "different authored namespace ids must not collapse behind the same visible aliases"
    );
    for expected in [
        r#"namespaceId(bytes=6)="领域""#,
        r#"namespaceId(bytes=10)="领域.API""#,
    ] {
        assert!(
            domain.contains(expected),
            "namespace identity should use UTF-8 byte framing for {expected:?}:\n{domain}"
        );
    }
    for expected in [
        r#"namespaceId(bytes=6)="边界""#,
        r#"namespaceId(bytes=10)="边界.API""#,
    ] {
        assert!(
            boundary.contains(expected),
            "nested namespace identity should retain its owning authored id {expected:?}:\n{boundary}"
        );
    }
}

#[test]
fn class_typed_root_dotted_namespace_discloses_full_identity_behind_leaf_label() {
    let render = |namespace_id: &str| {
        let mut model = parse_class_model("classDiagram\nnamespace Root {\n  class Member\n}");
        let mut namespace = model
            .namespaces
            .shift_remove("Root")
            .expect("Root namespace should exist");
        namespace.id = namespace_id.to_string();
        namespace.label = "C".to_string();
        model
            .classes
            .get_mut("Member")
            .expect("Member class should exist")
            .parent = Some(namespace.id.clone());
        model.namespaces.insert(namespace.id.clone(), namespace);

        render_class_model(&model, &AsciiRenderOptions::ascii()).unwrap_or_else(|error| {
            panic!("dotted namespace {namespace_id:?} should render: {error}")
        })
    };

    let left = render("A.C");
    let right = render("B.C");

    assert_ne!(
        left, right,
        "root dotted namespaces with the same leaf label must retain distinct identities"
    );
    assert!(left.contains(r#"namespaceId(bytes=3)="A.C""#), "{left}");
    assert!(right.contains(r#"namespaceId(bytes=3)="B.C""#), "{right}");
}

#[test]
fn class_typed_nested_namespace_discloses_identity_when_parent_path_cannot_recover_it() {
    let render = |namespace_id: &str| {
        let mut model = parse_class_model(
            "classDiagram\nnamespace Parent {\n  namespace Child {\n    class Member\n  }\n}",
        );
        let child_key = model
            .namespaces
            .keys()
            .find(|id| id.as_str() != "Parent")
            .cloned()
            .expect("nested Child namespace should exist");
        let mut namespace = model
            .namespaces
            .shift_remove(&child_key)
            .expect("nested Child namespace should exist");
        namespace.id = namespace_id.to_string();
        namespace.label = "Child".to_string();
        namespace.dom_id = "namespace-child".to_string();
        namespace.parent = Some("Parent".to_string());
        model
            .classes
            .get_mut("Member")
            .expect("Member class should exist")
            .parent = Some(namespace.id.clone());
        model.namespaces.insert(namespace.id.clone(), namespace);

        render_class_model(&model, &AsciiRenderOptions::ascii()).unwrap_or_else(|error| {
            panic!("nested namespace {namespace_id:?} should render: {error}")
        })
    };

    let foreign = render("Foreign.Child");
    let other = render("Other.Child");

    assert_ne!(foreign, other);
    assert!(
        foreign.contains(r#"namespaceId(bytes=13)="Foreign.Child""#),
        "{foreign}"
    );
    assert!(
        other.contains(r#"namespaceId(bytes=11)="Other.Child""#),
        "{other}"
    );
}

#[test]
fn class_parser_namespace_direction_controls_relationless_siblings() {
    let render = |direction| {
        render_class(
            &format!(
                "classDiagram\ndirection {direction}\nnamespace Domain {{\n  class A\n  class B\n}}"
            ),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("namespace direction {direction} should render: {error}"))
    };

    let tb = render("TB");
    let bt = render("BT");
    let lr = render("LR");
    let rl = render("RL");

    assert!(
        first_line_index_containing(&tb, "| A |") < first_line_index_containing(&tb, "| B |"),
        "TB should preserve namespace declaration order:\n{tb}"
    );
    assert!(
        first_line_index_containing(&bt, "| B |") < first_line_index_containing(&bt, "| A |"),
        "BT should reverse namespace declaration order:\n{bt}"
    );

    let (lr_a_line, lr_a_column) = line_and_column_containing(&lr, "| A |");
    let (lr_b_line, lr_b_column) = line_and_column_containing(&lr, "| B |");
    assert_eq!(
        lr_a_line, lr_b_line,
        "LR siblings should share a row:\n{lr}"
    );
    assert!(
        lr_a_column < lr_b_column,
        "LR should preserve namespace declaration order:\n{lr}"
    );

    let (rl_a_line, rl_a_column) = line_and_column_containing(&rl, "| A |");
    let (rl_b_line, rl_b_column) = line_and_column_containing(&rl, "| B |");
    assert_eq!(
        rl_a_line, rl_b_line,
        "RL siblings should share a row:\n{rl}"
    );
    assert!(
        rl_b_column < rl_a_column,
        "RL should reverse namespace declaration order:\n{rl}"
    );
}

#[test]
fn class_parser_dotted_namespace_keeps_implicit_ancestor_ownership() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "namespace Company.Project.Module {\n",
            "  class A\n",
            "}",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("dotted namespace hierarchy should render");

    let namespace_positions = ["Company", "Project", "Module"].map(|namespace| {
        assert_eq!(
            rendered.matches(namespace).count(),
            1,
            "namespace label {namespace:?} should have one owning container:\n{rendered}"
        );
        line_and_column_containing(&rendered, namespace)
    });
    assert!(
        namespace_positions
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0 && pair[0].1 < pair[1].1),
        "dotted namespace ancestors should form three nested containers:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("| A |").count(),
        1,
        "the class should remain inside the complete namespace hierarchy:\n{rendered}"
    );
    let class_position = line_and_column_containing(&rendered, " A ");
    assert!(
        namespace_positions[2].0 < class_position.0 && namespace_positions[2].1 < class_position.1,
        "the class should remain nested inside Module:\n{rendered}"
    );
}

#[test]
fn class_parser_bottom_up_namespace_external_relation_orders_the_target_first() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction BT\n",
            "namespace Domain {\n",
            "  class A\n",
            "}\n",
            "class B\n",
            "A --> B : leaves",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("bottom-up external namespace relation should render");

    assert!(
        first_line_index_containing(&rendered, "| B |")
            < first_line_index_containing(&rendered, "| A |"),
        "BT should place the external target before the namespace source:\n{rendered}"
    );
    assert!(!rendered.contains("relations:"), "{rendered}");
    assert!(
        rendered.contains("A") && rendered.contains("leaves"),
        "{rendered}"
    );
}

#[test]
fn class_parser_sibling_namespace_relation_routes_through_facades() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "namespace Left {\n  class Source\n}\n",
            "namespace Right {\n  class Target\n}\n",
            "Source \"source\" --> \"target\" Target : calls<br>async",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("sibling namespace relation should route through namespace facades");

    assert!(!rendered.contains("relations:"), "{rendered}");
    for framed_member in ["member(bytes=6)=\"Source\"", "member(bytes=6)=\"Target\""] {
        assert!(
            rendered.contains(framed_member),
            "missing framed namespace member {framed_member:?}:\n{rendered}"
        );
    }
    for expected in [
        "Left", "Right", "Source", "Target", "source", "target", "calls", "async",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_typed_relation_preserves_authored_facade_endpoint() {
    let mut model = parse_class_model(
        "classDiagram\nnamespace Domain {\n  class T\n}\nclass Outside\nT --> Outside",
    );
    let mut facade = model.classes["T"].clone();
    facade.id = "Domain.T".to_string();
    facade.dom_id = "classDomain.T-facade".to_string();
    facade.parent = None;
    model.classes.insert(facade.id.clone(), facade);
    model
        .namespace_facade_aliases
        .insert("Domain.T".to_string(), "T".to_string());

    let resolved = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("resolved relation endpoint should render");
    model.relations[0].id1 = "Domain.T".to_string();
    let authored = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("authored facade relation endpoint should render");

    assert_ne!(resolved, authored);
    assert!(!resolved.contains("endpointRef(bytes="), "{resolved}");
    assert!(authored.contains("endpointRef(bytes=8)="), "{authored}");
    assert!(authored.contains("Domain.T"), "{authored}");
}

#[test]
fn class_parser_nested_sibling_relation_routes_at_nearest_common_scope() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "namespace Platform {\n",
            "  namespace FFI {\n    class DartBinding\n  }\n",
            "  namespace Core {\n    class Renderer\n  }\n",
            "}\n",
            "DartBinding --> Renderer : invokes",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .expect("nested sibling namespace relation should route at Platform scope");

    assert!(!rendered.contains("relations:"), "{rendered}");
    for expected in [
        "Platform",
        "FFI",
        "Core",
        "DartBinding",
        "Renderer",
        "invokes",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    for class_name in ["DartBinding", "Renderer"] {
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.contains(&format!("│ {class_name} │")))
                .count(),
            1,
            "nested namespace relation routing should render class {class_name:?} once:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_right_left_namespace_to_root_relation_preserves_semantics() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "direction RL\n",
            "namespace Domain {\n  class Service\n}\n",
            "class Gateway\n",
            "Service \"inside\" --> \"outside\" Gateway : exposes",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("right-left namespace-to-root relation should route");

    assert!(!rendered.contains("relations:"), "{rendered}");
    for expected in [
        "Domain", "Service", "Gateway", "inside", "outside", "exposes",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    let (_, domain_column) = line_and_column_containing(&rendered, "Domain");
    let (_, gateway_column) = line_and_column_containing(&rendered, "Gateway");
    assert!(
        gateway_column < domain_column,
        "RL should place target left of source:\n{rendered}"
    );
}

#[test]
fn class_parser_cross_namespace_collision_keeps_lossless_summary() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "namespace Left {\n  class A\n  class B\n}\n",
            "namespace Right {\n  class C\n  class D\n}\n",
            "A --> C : ac\n",
            "A --> D : ad\n",
            "B --> C : bc\n",
            "B --> D : bd",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("cross-namespace collision should retain a lossless fallback");

    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in ["A", "B", "C", "D", "ac", "ad", "bc", "bd"] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_namespace_internal_relationship_routes_inside_container() {
    let rendered = render_class(
        "classDiagram
namespace Domain {
  class Service
  class Repository
}
Service --> Repository : uses",
        &AsciiRenderOptions::ascii(),
    )
    .expect("namespace-internal class relationship should render");

    for expected in ["Domain", "Service", "Repository", "uses"] {
        assert!(
            rendered.contains(expected),
            "namespace internal relationship output should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "same-namespace class relationship should route inside the namespace container:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.contains("uses")),
        "routed namespace relationship should keep its label in the container:\n{rendered}"
    );
}

#[test]
fn class_parser_namespace_note_for_routes_inside_container() {
    let rendered = render_class(
        "classDiagram
namespace Domain {
  class Service
  note for Service \"Handles<br>requests\"
}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("namespace note should render");

    for expected in ["Domain", "Service", "Handles", "requests"] {
        assert!(
            rendered.contains(expected),
            "namespace note output should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "namespace note-for should route inside the namespace container instead of falling back to a top-level summary:\n{rendered}"
    );
    assert!(
        rendered.contains(':') || rendered.contains('.'),
        "namespace note-for should keep a dotted visual connector:\n{rendered}"
    );
    assert!(
        !rendered.contains("note0"),
        "namespace note output should not leak implementation note ids:\n{rendered}"
    );
}

#[test]
fn class_typed_namespace_note_preserves_authored_facade_target() {
    let mut model =
        parse_class_model("classDiagram\nnamespace Domain {\n  class T\n  note for T \"same\"\n}");
    let mut facade = model.classes["T"].clone();
    facade.id = "Domain.T".to_string();
    facade.dom_id = "classDomain.T-facade".to_string();
    facade.parent = None;
    model.classes.insert(facade.id.clone(), facade);
    model
        .namespace_facade_aliases
        .insert("Domain.T".to_string(), "T".to_string());

    let resolved = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("resolved namespace note target should render");
    model.notes[0].class_id = Some("Domain.T".to_string());
    let authored = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("authored facade namespace note target should render");

    assert_ne!(resolved, authored);
    assert!(!resolved.contains("endpointRef(bytes="), "{resolved}");
    assert!(authored.contains("endpointRef(bytes=8)="), "{authored}");
    assert!(authored.contains("Domain.T"), "{authored}");
}

#[test]
fn class_external_namespace_note_summary_preserves_provenance_and_target_identity() {
    let render = |first_target: &str, second_target: &str| {
        let mut model = parse_class_model(
            "classDiagram\nnamespace Domain {\n  class A\n  class B\n}\nnote for A \"same\"\nnote for B \"same\"",
        );
        let mut first = model.classes.shift_remove("A").expect("A should exist");
        first.id = "\u{1b}".to_string();
        first.dom_id.clone_from(&first.id);
        first.text.clone_from(&first.id);
        let mut second = model.classes.shift_remove("B").expect("B should exist");
        second.id = r"\u{1B}".to_string();
        second.dom_id.clone_from(&second.id);
        second.text.clone_from(&second.id);
        model.classes.insert(first.id.clone(), first);
        model.classes.insert(second.id.clone(), second);

        let namespace = model
            .namespaces
            .get_mut("Domain")
            .expect("Domain namespace should exist");
        namespace.class_ids = vec!["\u{1b}".to_string(), r"\u{1B}".to_string()];
        model.notes[0].class_id = Some(first_target.to_string());
        model.notes[1].class_id = Some(second_target.to_string());

        render_class_model(&model, &AsciiRenderOptions::ascii())
            .expect("external namespace notes should render")
    };

    let authored_first = render("\u{1b}", r"\u{1B}");
    let literal_first = render(r"\u{1B}", "\u{1b}");

    assert_ne!(authored_first, literal_first);
    assert!(
        authored_first.contains(r#"note(index=1, text(bytes=4)="same")"#),
        "{authored_first}"
    );
    assert!(
        authored_first.contains(r#"id(bytes=1)="\u{1B}""#),
        "{authored_first}"
    );
    assert!(
        authored_first.contains(r#"id(bytes=6)="\\u{1B}""#),
        "{authored_first}"
    );
}

#[test]
fn class_external_namespace_note_summary_discloses_authored_facade_target() {
    let mut model =
        parse_class_model("classDiagram\nnamespace Domain {\n  class T\n}\nnote for T \"same\"");
    let mut facade = model.classes["T"].clone();
    facade.id = "Domain.T".to_string();
    facade.dom_id = "classDomain.T-facade".to_string();
    facade.parent = None;
    model.classes.insert(facade.id.clone(), facade);
    model
        .namespace_facade_aliases
        .insert("Domain.T".to_string(), "T".to_string());

    let resolved = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("resolved note target should render");
    model.notes[0].class_id = Some("Domain.T".to_string());
    let authored = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("qualified authored note target should render through its facade");

    assert_ne!(
        resolved, authored,
        "qualified and unqualified authored note targets must not collapse"
    );
    assert!(!resolved.contains("targetRef(bytes="), "{resolved}");
    assert!(
        authored.contains(r#"id(bytes=1)="T" targetRef(bytes=8)="Domain.T""#),
        "{authored}"
    );
}

#[test]
fn class_external_namespace_note_summary_keeps_interface_endpoint() {
    let mut model = parse_class_model(concat!(
        "classDiagram\n",
        "namespace Domain {\n  class Marker\n}\n",
        "IService ()-- Service\n",
        "note for Marker \"interface note\"",
    ));
    model.relations.clear();
    model.notes[0].class_id = Some("interface0".to_string());

    let rendered = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("validated interface note target should remain in the summary");

    assert!(rendered.contains("relations:"), "{rendered}");
    assert!(
        rendered.contains(r#"note(index=1, text(bytes=14)="interface note")"#),
        "{rendered}"
    );
    assert!(
        rendered.contains(r#"id(bytes=10)="interface0""#),
        "{rendered}"
    );
}

#[test]
fn class_parser_empty_namespace_does_not_force_relation_summary() {
    let rendered = render_class(
        "classDiagram
namespace Empty {
}
class A
class B
A --> B : ab",
        &AsciiRenderOptions::ascii(),
    )
    .expect("empty namespace should not affect top-level relationships");

    assert!(
        !rendered.contains("relations:"),
        "empty namespace should not force top-level relationships into summary:\n{rendered}"
    );
    assert!(
        rendered.contains("ab") && rendered.contains("+---+"),
        "top-level class relationship should keep the routed layout:\n{rendered}"
    );
    assert!(
        rendered.contains("Empty"),
        "an empty namespace should remain visible without degrading unrelated relations:\n{rendered}"
    );
}

#[test]
fn class_parser_standalone_empty_namespace_remains_visible() {
    let rendered = render_class(
        "classDiagram\nnamespace Empty {\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("standalone empty namespace should render");

    assert!(rendered.contains("Empty"), "{rendered}");
    assert!(!rendered.contains("relations:"), "{rendered}");
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
fn class_parser_independent_relationships_render_as_compact_components() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nA <|-- D\nB <|-- C",
        &AsciiRenderOptions::ascii(),
    )
    .expect("independent class relationships should render as separate components");

    assert_eq!(
        rendered,
        concat!(
            "+---+    +---+\n",
            "| A |    | B |\n",
            "+---+    +---+\n",
            "  ^        ^\n",
            "  |        |\n",
            "+---+    +---+\n",
            "| D |    | C |\n",
            "+---+    +---+\n",
        )
    );
}

#[test]
fn class_parser_even_width_layered_label_uses_exact_terminal_extent() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : evenly\nB --> C : next",
        &AsciiRenderOptions::ascii(),
    )
    .expect("an even-width layered label must not be rejected as a grid overflow");

    for expected in ["A", "B", "C", "evenly", "next"] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?} from exact-width layered output:\n{rendered}"
        );
    }
    assert!(!rendered.contains("relations:"), "{rendered}");
}

#[test]
fn class_parser_multi_parent_relationship_layout_uses_barycenter_order() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nclass E\nclass F\nA --> C : ac\nA --> D : ad\nB --> C : bc\nC --> E : ce\nC --> F : cf",
        &AsciiRenderOptions::ascii(),
    )
    .expect("multi-parent class relationship layout should render");

    assert!(
        !rendered.contains("relations:"),
        "multi-parent class topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in ["ac", "ad", "bc", "ce", "cf"] {
        assert!(
            rendered.contains(expected),
            "routed multi-parent class topology should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_child_weighted_parent_order_keeps_readable_layout_routed() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nclass E\nA --> E : ae\nB --> D : bd\nC --> E : ce",
        &AsciiRenderOptions::ascii(),
    )
    .expect("child-weighted class topology should render");

    assert!(
        !rendered.contains("relations:"),
        "child-weighted class topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in ["A", "B", "C", "D", "E", "ae", "bd", "ce"] {
        assert!(
            rendered.contains(expected),
            "routed child-weighted class topology should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_four_layer_relation_layout_uses_iterative_sweep_order() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "class A0\nclass A1\nclass A2\n",
            "class B0\nclass B1\nclass B2\n",
            "class C0\nclass C1\nclass C2\n",
            "class D0\nclass D1\nclass D2\n",
            "C0 --> D0 : c0d0\n",
            "A1 --> B1 : a1b1\n",
            "A2 --> B0 : a2b0\n",
            "A1 --> B0 : a1b0\n",
            "B2 --> C2 : b2c2\n",
            "B2 --> C0 : b2c0\n",
            "C2 --> D1 : c2d1\n",
            "C2 --> D0 : c2d0\n",
            "A0 --> B1 : a0b1\n",
            "A0 --> B2 : a0b2\n",
            "C1 --> D2 : c1d2\n",
            "B2 --> C1 : b2c1",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("four-layer class relation topology should render");

    assert!(
        !rendered.contains("relations:"),
        "four-layer class topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in [
        "a0b1", "a0b2", "a1b0", "a1b1", "a2b0", "b2c0", "b2c1", "b2c2", "c0d0", "c1d2", "c2d0",
        "c2d1",
    ] {
        assert!(
            rendered.contains(expected),
            "routed four-layer class topology should keep {expected:?} visible:\n{rendered}"
        );
    }
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
fn class_parser_self_relation_renders_single_box_with_loop() {
    let rendered = render_class(
        "classDiagram\nclass Node\nNode --> Node : next",
        &AsciiRenderOptions::ascii(),
    )
    .expect("self class relation should render");

    assert_eq!(
        rendered,
        concat!(
            "+------+\n",
            "| Node |---+\n",
            "+------+   |\n",
            "   next    |\n",
            "    v------+\n",
        )
    );
}

#[test]
fn class_parser_parallel_self_relations_share_single_box_loop() {
    let rendered = render_class(
        "classDiagram\nclass Node\nNode --> Node : next\nNode ..> Node : loads",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel self class relations should render");

    assert_eq!(
        rendered,
        concat!(
            "+------+\n",
            "| Node |---+\n",
            "+------+   |\n",
            "   next    |\n",
            "    v------+\n",
            "  loads    :\n",
            "    v......+\n",
        )
    );
}

#[test]
fn class_parser_self_relation_preserves_endpoint_labels_and_both_markers() {
    let rendered = render_class(
        "classDiagram\nclass Node\nNode \"source\" <|--|> \"target\" Node : recursive",
        &AsciiRenderOptions::ascii(),
    )
    .expect("two-sided self class relation should render");

    for expected in [
        "endpoint 1: source",
        "relation: recursive",
        "endpoint 2: target",
        "^",
        "v",
    ] {
        assert!(
            rendered.contains(expected),
            "self relation should preserve {expected:?} near its loop:\n{rendered}"
        );
    }
}

#[test]
fn class_parser_parallel_self_relations_preserve_unlabelled_source_markers() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "class Node\n",
            "Node <|--|> Node : first\n",
            "Node <|--|> Node",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel two-sided self class relations should render");

    assert_eq!(
        rendered.matches("| Node |").count(),
        1,
        "parallel self relations must share one class box:\n{rendered}"
    );
    for marker in ['^', 'v'] {
        assert!(
            rendered.matches(marker).count() >= 2,
            "each parallel self relation must preserve its {marker:?} marker:\n{rendered}"
        );
    }
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
fn class_parser_authored_none_endpoint_labels_are_not_absence_sentinels() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nA \"none\" --> \"NONE\" B",
        &AsciiRenderOptions::ascii(),
    )
    .expect("authored endpoint labels should render");

    for label in ["none", "NONE"] {
        assert!(
            rendered.contains(label),
            "authored endpoint label {label:?} should remain visible:\n{rendered}"
        );
    }

    let absent = render_class(
        "classDiagram\nclass A\nclass B\nA --> B",
        &AsciiRenderOptions::ascii(),
    )
    .expect("an absent endpoint label should render");
    let authored_empty = render_class(
        "classDiagram\nclass A\nclass B\nA \"\" --> B",
        &AsciiRenderOptions::ascii(),
    )
    .expect("an authored empty endpoint label should render");
    assert_ne!(absent, authored_empty);
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
fn class_routed_aliases_disclose_authored_endpoint_identities() {
    let rendered = render_class(
        "classDiagram\nclass A[\"X\"]\nclass B[\"X\"]\nA --> B : calls",
        &AsciiRenderOptions::ascii(),
    )
    .expect("class aliases with one relation should render diagrammatically");

    assert!(!rendered.contains("relations:"), "{rendered}");
    for identity in [r#"id(bytes=1)="A""#, r#"id(bytes=1)="B""#] {
        assert!(
            rendered.contains(identity),
            "routed class aliases must preserve {identity:?}:\n{rendered}"
        );
    }
}

#[test]
fn class_local_semantic_fixture_covers_wide_members_and_relation_labels() {
    let input = read_local_semantic_fixture("class/wide_members_and_summary_labels.mmd");
    let options = AsciiRenderOptions::ascii();

    let rendered = render_class_with_grid_limit(&input, &options, 10_000)
        .expect("class diagram with wide member and relation labels should render");

    for expected in [
        "User",
        "名称",
        "Order",
        "状态🚀",
        "Audit",
        "创建🚀",
        "记录数据",
    ] {
        assert!(
            rendered.contains(expected),
            "wide class fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered
            .lines()
            .all(|line| !line.contains("<br>") || line.contains("authored(bytes=")),
        "wide class relations should expose Mermaid break syntax only inside authored framing:\n{rendered}"
    );
}

#[test]
fn class_parser_final_relation_document_obeys_the_operation_grid_budget() {
    let error = render_class_with_grid_limit(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nA --> B : ab\nC --> D : cd",
        &AsciiRenderOptions::ascii(),
        1,
    )
    .expect_err("the final joined class document must obey the operation grid budget");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn class_parser_relation_fallback_obeys_the_operation_grid_budget() {
    let error = render_class_with_grid_limit(
        "classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes<br>through\nService --> Repo : stores",
        &AsciiRenderOptions::ascii(),
        1,
    )
    .expect_err("a class fallback that cannot fit the operation budget must be rejected");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn class_parser_diagnostic_fallback_preserves_the_operation_resource_error() {
    let error = render_class_with_grid_limit(
        "classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes\nService --> Repo : stores",
        &AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true),
        1,
    )
    .expect_err("diagnostics must not bypass the operation grid budget");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn class_parser_independent_relation_pairs_render_without_shared_summary_state() {
    let options = AsciiRenderOptions::ascii();

    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nclass D\nA --> B : ab\nC --> D : cd",
        &options,
    )
    .expect("independent relation pairs should render separately");

    for expected in ["A", "B", "C", "D", "ab", "cd"] {
        assert!(
            rendered.contains(expected),
            "independent class relation pairs should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "independent class relation pairs should remain routed without shared summary state:\n{rendered}"
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

    for expected in [
        "note",
        "Handles",
        "requests",
        "Service",
        r#"authored(bytes=19)="Handles<br>requests""#,
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    assert!(rendered.contains(':'), "{rendered}");
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
fn class_render_model_preserves_independent_relationship_markers() {
    let mut model = parse_class_model("classDiagram\nclass A\nclass B\nA <|-- B");
    let aggregation = model.constants.relation_type.aggregation;
    let composition = model.constants.relation_type.composition;
    let relation = model
        .relations
        .first_mut()
        .expect("fixture should contain one relation");
    relation.relation.type1 = aggregation;
    relation.relation.type2 = composition;

    let rendered = render_class_model(&model, &AsciiRenderOptions::ascii())
        .expect("independent class relationship markers should render");

    assert!(
        rendered.contains('o'),
        "aggregation marker should remain visible:\n{rendered}"
    );
    assert!(
        rendered.contains('*'),
        "composition marker should remain visible:\n{rendered}"
    );
}

#[test]
fn class_parser_two_sided_markers_keep_cardinalities_and_label() {
    let rendered = render_class(
        "classDiagram\nA \"1\" <|--|> \"*\" B : inherits both ways",
        &AsciiRenderOptions::ascii(),
    )
    .expect("two-sided class relationship markers should render");

    for expected in ["A", "B", "1", "*", "inherits both ways", "^", "v"] {
        assert!(
            rendered.contains(expected),
            "two-sided relationship should keep {expected:?} visible:\n{rendered}"
        );
    }

    let composition = render_class(
        "classDiagram\nWhole *--* Part",
        &AsciiRenderOptions::ascii(),
    )
    .expect("two-sided composition should render");
    assert!(
        composition.matches('*').count() >= 2,
        "both composition terminals should remain visible:\n{composition}"
    );
}

#[test]
fn class_parser_dense_crossing_relationships_fall_back_to_relation_summary() {
    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense class relationships should render through relation summary fallback");

    let ab = framed_class_summary_relation("A", "-->", "B", Some("ab"));
    let ba = framed_class_summary_relation("B", "-->", "A", Some("ba"));
    let ac = framed_class_summary_relation("A", "-->", "C", Some("ac"));
    let ca = framed_class_summary_relation("C", "-->", "A", Some("ca"));
    let bc = framed_class_summary_relation("B", "-->", "C", Some("bc"));
    let cb = framed_class_summary_relation("C", "-->", "B", Some("cb"));
    assert_eq!(
        rendered,
        format!(
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
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
            ),
            ab, ba, ac, ca, bc, cb,
        )
    );
}

#[test]
fn class_relation_summary_frames_multiline_endpoint_labels_injectively() {
    let source = |endpoint_label: &str| {
        format!(
            concat!(
                "classDiagram\n",
                "direction BT\n",
                "class A\n",
                "class B\n",
                "class C\n",
                "A \"{}\" --> B : ab\n",
                "B --> A : ba\n",
                "A --> C : ac\n",
                "C --> A : ca\n",
                "B --> C : bc\n",
                "C --> B : cb",
            ),
            endpoint_label
        )
    };
    let literal_slash = render_class(&source("a/b"), &AsciiRenderOptions::ascii())
        .expect("literal slash endpoint label should render through summary fallback");
    let authored_break = render_class(&source("a<br>b"), &AsciiRenderOptions::ascii())
        .expect("multiline endpoint label should render through summary fallback");
    let authored_empty = render_class(&source(""), &AsciiRenderOptions::ascii())
        .expect("empty endpoint label should render through summary fallback");

    assert!(literal_slash.contains("relations:"), "{literal_slash}");
    assert!(authored_break.contains("relations:"), "{authored_break}");
    assert!(authored_empty.contains("relations:"), "{authored_empty}");
    assert!(
        literal_slash.contains("endpoint1=[bytes=3 \"a/b\"] -->"),
        "literal slash identity should remain framed on the semantic source:\n{literal_slash}"
    );
    assert!(
        authored_break.contains("endpoint1=[bytes=1 \"a\", bytes=1 \"b\",")
            && authored_break.contains("authored(bytes=6)=")
            && authored_break.contains("<br>")
            && authored_break.contains("] -->"),
        "authored line boundaries and source bytes should remain framed:\n{authored_break}"
    );
    assert!(
        authored_empty.contains("endpoint1=[bytes=0 \"\",")
            && authored_empty.contains("authored(bytes=0)=")
            && authored_empty.contains("] -->"),
        "authored empty endpoint labels should remain framed:\n{authored_empty}"
    );
    assert_ne!(literal_slash, authored_break);
    assert_ne!(literal_slash, authored_empty);
}

#[test]
fn class_parser_k2_2_relationships_use_a_bounded_planar_layout() {
    let rendered = render_class(
        concat!(
            "classDiagram\n",
            "class A\n",
            "class B\n",
            "class C\n",
            "class D\n",
            "A --> C : ac\n",
            "A o-- D : ad\n",
            "B ..> C : bc\n",
            "B *-- D : bd",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("bounded K2,2 class relationships should render diagrammatically");

    assert!(
        !rendered.contains("relations:"),
        "a strict K2,2 component should use the bounded planar layout:\n{rendered}"
    );
    for expected in ["ac", "ad", "bc", "bd", "^", "v", "o", "*"] {
        assert!(
            rendered.contains(expected),
            "bounded K2,2 output should retain {expected:?}:\n{rendered}"
        );
    }
    for node in ["A", "B", "C", "D"] {
        assert_eq!(
            rendered.matches(&format!("| {node} |")).count(),
            1,
            "bounded K2,2 output should render node {node:?} exactly once:\n{rendered}"
        );
    }
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
        framed_class_summary_relation("B", "<|..", "A", Some("ab")),
        framed_class_summary_relation("A", "<|..", "B", Some("ba")),
        framed_class_summary_relation("A", "..> ", "C", Some("ac")),
        framed_class_summary_relation("B", "--> ", "C", Some("bc")),
    ] {
        assert!(
            rendered.contains(&expected),
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
    for expected in [
        framed_class_summary_relation("A", "--", "B", Some("ab")),
        framed_class_summary_relation("B", "--", "A", Some("ba")),
        framed_class_summary_relation("A", "--", "C", Some("ac")),
        framed_class_summary_relation("C", "--", "A", Some("ca")),
        framed_class_summary_relation("B", "--", "C", Some("bc")),
        framed_class_summary_relation("C", "--", "B", Some("cb")),
    ] {
        assert!(
            rendered.contains(&expected),
            "dense plain association summary should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("-->") && !rendered.contains("<--"),
        "dense plain association summary should not invent arrowheads:\n{rendered}"
    );
}

#[test]
fn class_parser_relation_layout_propagates_grid_resource_errors() {
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .expect("test resource limit should be valid");

    let error = render_class_with_resources(
        "classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes<br>through\nService --> Repo : stores",
        &AsciiRenderOptions::ascii(),
        resources,
    )
    .expect_err("grid resource errors must not become summary fallback");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxGridCells
    ));
}

#[test]
fn class_parser_relation_summary_can_show_crossing_diagnostic() {
    let options = AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true);

    let rendered = render_class(
        "classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb",
        &options,
    )
    .expect("class crossing summary diagnostic should render");

    assert!(rendered.contains("relations:"), "{rendered}");
    assert!(rendered.contains("reason: crossing"), "{rendered}");
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

    let ab = framed_class_summary_relation("A", "-->", "B", Some("ab"));
    let ba = framed_class_summary_relation("B", "-->", "A", Some("ba"));
    let ac = framed_class_summary_relation("A", "-->", "C", Some("ac"));
    let ca = framed_class_summary_relation("C", "-->", "A", Some("ca"));
    let bc = framed_class_summary_relation("B", "-->", "C", Some("bc"));
    let cb = framed_class_summary_relation("C", "-->", "B", Some("cb"));
    assert_eq!(
        strip_ansi(&rendered),
        format!(
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
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
                "{}\n",
            ),
            ab, ba, ac, ca, bc, cb,
        )
    );
    for expected_fragment in [
        "\u{1b}[38;2;16;16;16m",
        "\u{1b}[38;2;32;32;32m",
        "\u{1b}[38;2;48;48;48mrelations:",
        "\u{1b}[38;2;80;80;80mid(bytes=1)=\"A\" --> id(bytes=1)=\"B\" : ab",
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
        r#"id(bytes=7)="Gateway" --> id(bytes=7)="Service" : receives"#,
        "request",
        r#"id(bytes=7)="Service" --> id(bytes=7)="Gateway" : returns"#,
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
        rendered
            .lines()
            .all(|line| !line.contains("<br>") || line.contains("authored(bytes=")),
        "dense multiline semantic class fixture should expose Mermaid break syntax only inside authored framing:\n{rendered}"
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
