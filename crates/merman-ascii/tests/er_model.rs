mod support;

use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiRgb, TerminalWidthProfile,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelSpecRenderModel,
    ErRelationshipRenderModel,
};
use merman_core::{Engine, OperationControl, ParseOptions};
use std::path::Path;
use support::{render_controlled_model, render_model, render_model_with_resources};

fn parse_er_render_model(input: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("ER diagram should parse")
        .expect("ER diagram should be detected")
        .into_parts()
        .1
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

fn render_er_with_resources(
    input: &str,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let model = parse_er_render_model(input);

    render_model_with_resources(&model, options, resources)
}

fn render_er_model(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    render_model(&RenderSemanticModel::Er(model.clone()), options)
}

fn render_er_with_grid_limit(
    input: &str,
    options: &AsciiRenderOptions,
    max_grid_cells: usize,
) -> merman_ascii::Result<String> {
    let model = parse_er_render_model(input);
    let control = OperationControl::new();
    let context = Engine::new()
        .begin_operation()
        .expect("deterministic operation context should be available");
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, max_grid_cells)
        .expect("valid operation grid limit");

    render_controlled_model(&model, options, &control, &context, resources)
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

fn line_and_column_containing(rendered: &str, needle: &str) -> (usize, usize) {
    rendered
        .lines()
        .enumerate()
        .find_map(|(line, text)| text.find(needle).map(|column| (line, column)))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn framed_er_attribute(ty: &str, name: &str, keys: &[&str], comment: &str) -> String {
    let keys = keys
        .iter()
        .map(|key| format!(r#"bytes={} "{key}""#, key.len()))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"type(bytes={})="{ty}" name(bytes={})="{name}" keys=[{keys}] comment(bytes={})="{comment}""#,
        ty.len(),
        name.len(),
        comment.len(),
    )
}

fn framed_er_summary_endpoint(id: &str) -> String {
    format!(r#"id(bytes={})="{id}""#, id.len())
}

fn framed_er_summary_relation(source: &str, connector: &str, target: &str, label: &str) -> String {
    format!(
        "{} {connector} {} : {label}",
        framed_er_summary_endpoint(source),
        framed_er_summary_endpoint(target),
    )
}

fn assert_unsupported_er_model(model: &ErDiagramRenderModel, feature: &'static str) {
    let err = render_er_model(model, &AsciiRenderOptions::ascii())
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
fn er_local_semantic_fixture_covers_wide_attributes_and_relation_labels() {
    let input = read_local_semantic_fixture("er/wide_attributes_and_summary_labels.mmd");
    let options = AsciiRenderOptions::ascii();

    let rendered = render_er_with_grid_limit(&input, &options, 10_000)
        .expect("ER diagram with wide attributes and relation labels should render");

    for expected in [
        "CUSTOMER",
        "名称",
        "ORDER",
        "状态🚀",
        "AUDIT",
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
        "wide ER relations should not leak Mermaid break syntax:\n{rendered}"
    );
}

#[test]
fn er_parser_final_relation_document_obeys_the_operation_grid_budget() {
    let error = render_er_with_grid_limit(
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\nINVOICE ||--|| PAYMENT : captures",
        &AsciiRenderOptions::ascii(),
        1,
    )
    .expect_err("the final joined ER document must obey the operation grid budget");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn er_parser_relation_fallback_obeys_the_operation_grid_budget() {
    let error = render_er_with_grid_limit(
        "erDiagram\nCUSTOMER\nORDER\nINVOICE\nCUSTOMER ||--o{ ORDER : \"places<br>orders\"\nORDER ||--|| INVOICE : bills",
        &AsciiRenderOptions::ascii(),
        1,
    )
    .expect_err("an ER fallback that cannot fit the operation budget must be rejected");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
}

#[test]
fn er_parser_diagnostic_fallback_preserves_the_operation_resource_error() {
    let error = render_er_with_grid_limit(
        "erDiagram\nCUSTOMER\nORDER\nINVOICE\nCUSTOMER ||--o{ ORDER : places\nORDER ||--|| INVOICE : bills",
        &AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true),
        1,
    )
    .expect_err("diagnostics must not bypass the operation grid budget");

    assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
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
fn er_render_model_rejects_relationships_without_endpoint_entities() {
    let mut model = parse_er_model("erDiagram\nA ||--|| B : owns");
    model.entities.clear();

    assert_unsupported_er_model(&model, "relationships with missing endpoint entities");
}

#[test]
fn er_render_model_rejects_duplicate_rendered_entity_ids() {
    let mut model = parse_er_model("erDiagram\nA ||--|| B : owns");
    model.relationships.clear();
    let duplicate_id = model
        .entities
        .get("A")
        .expect("entity A should exist")
        .id
        .clone();
    model
        .entities
        .get_mut("B")
        .expect("entity B should exist")
        .id = duplicate_id.clone();
    assert_eq!(
        model
            .entities
            .values()
            .filter(|entity| entity.id == duplicate_id)
            .count(),
        2,
        "test model should contain two map entries with the same rendered id",
    );

    assert_unsupported_er_model(&model, "duplicate rendered ER entity ids");
}

#[test]
fn er_terminal_width_profile_preserves_complex_graphemes_and_ambiguous_width() {
    let mut model = parse_er_model("erDiagram\nA");
    let mut entity = model
        .entities
        .shift_remove("A")
        .expect("entity A should exist");
    entity.id = "👩‍💻·".to_string();
    entity.label = "👩‍💻·".to_string();
    entity.alias.clear();
    model.entities.insert("👩‍💻·".to_string(), entity);

    let unicode = render_er_model(
        &model,
        &AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Unicode),
    )
    .expect("ER entity should render with Unicode terminal widths");
    let cjk = render_er_model(
        &model,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    )
    .expect("ER entity should render with CJK terminal widths");

    assert_eq!(unicode, "+-----+\n| 👩‍💻· |\n+-----+\n");
    assert_eq!(cjk, "+------+\n| 👩‍💻· |\n+------+\n");
    assert!(
        !cjk.contains(['┌', '─', '┐', '│', '└', '┘']),
        "CJK width profiles must use single-cell structural glyphs: {cjk}"
    );
}

#[test]
fn er_relationship_labels_preserve_complex_graphemes() {
    let mut model = parse_er_model("erDiagram\nA ||--|| B : owns");
    model
        .entities
        .get_mut("A")
        .expect("entity A should exist")
        .alias = "Client 👩‍💻".to_string();
    model.relationships[0].role_a = "owns 👩‍💻".to_string();
    let rendered = render_er_model(&model, &AsciiRenderOptions::ascii())
        .expect("ER relationship should render");

    assert!(rendered.contains("Client 👩‍💻"), "{rendered}");
    assert!(rendered.contains("owns 👩‍💻"), "{rendered}");
    assert!(!rendered.contains("relations:"), "{rendered}");
}

#[test]
fn er_parser_attributes_render_in_entity_section() {
    let rendered = render_er(
        "erDiagram\nCUSTOMER {\n  string id PK\n  string name\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER should render");

    let id = framed_er_attribute("string", "id", &["PK"], "");
    let name = framed_er_attribute("string", "name", &[], "");
    let content_width = ["CUSTOMER".len(), id.len(), name.len()]
        .into_iter()
        .max()
        .expect("the entity fixture should have content");
    let border = format!("+{}+", "-".repeat(content_width + 2));
    assert_eq!(
        rendered,
        format!(
            "{border}\n| {:<content_width$} |\n{border}\n| {id:<content_width$} |\n| {name:<content_width$} |\n{border}\n",
            "CUSTOMER",
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
        framed_er_attribute("int", "id", &["PK"], ""),
        framed_er_attribute("int", "customer_id", &["FK"], "owner id"),
        framed_er_attribute("string", "email", &["UK"], ""),
    ] {
        assert!(
            rendered.contains(&expected),
            "ER attribute details should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn er_attribute_keys_and_comments_have_distinct_terminal_roles() {
    let key = render_er(
        "erDiagram\nA {\n  string email UK\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER key attribute should render");
    let comment = render_er(
        "erDiagram\nA {\n  string email \"UK\"\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER comment attribute should render");

    assert!(
        key.contains(&framed_er_attribute("string", "email", &["UK"], "")),
        "{key}"
    );
    assert!(
        comment.contains(&framed_er_attribute("string", "email", &[], "UK")),
        "{comment}"
    );
    assert_ne!(key, comment, "key and comment semantics must not collide");
}

#[test]
fn er_direct_attribute_fields_cannot_forge_renderer_owned_delimiters() {
    let render = |attribute: ErAttributeRenderModel| {
        let mut model = parse_er_model("erDiagram\nA");
        model
            .entities
            .get_mut("A")
            .expect("entity A should exist")
            .attributes = vec![attribute];
        render_er_model(&model, &AsciiRenderOptions::ascii())
            .expect("direct ER attribute should render")
    };

    let authored_key_syntax = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner [keys: PK]".to_string(),
        keys: Vec::new(),
        comment: String::new(),
    });
    let typed_key = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner".to_string(),
        keys: vec!["PK".to_string()],
        comment: String::new(),
    });
    assert_ne!(authored_key_syntax, typed_key);
    assert!(
        authored_key_syntax.contains(&framed_er_attribute("string", "owner [keys: PK]", &[], "")),
        "{authored_key_syntax}"
    );
    assert!(
        typed_key.contains(&framed_er_attribute("string", "owner", &["PK"], "")),
        "{typed_key}"
    );

    let authored_type_separator = render(ErAttributeRenderModel {
        ty: "string owner".to_string(),
        name: "id".to_string(),
        keys: Vec::new(),
        comment: String::new(),
    });
    let typed_name_separator = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner id".to_string(),
        keys: Vec::new(),
        comment: String::new(),
    });
    assert_ne!(authored_type_separator, typed_name_separator);
    assert!(
        authored_type_separator.contains(&framed_er_attribute("string owner", "id", &[], "")),
        "{authored_type_separator}"
    );
    assert!(
        typed_name_separator.contains(&framed_er_attribute("string", "owner id", &[], "")),
        "{typed_name_separator}"
    );

    let authored_comment_syntax = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner [comment: note]".to_string(),
        keys: Vec::new(),
        comment: String::new(),
    });
    let typed_comment = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner".to_string(),
        keys: Vec::new(),
        comment: "note".to_string(),
    });
    assert_ne!(authored_comment_syntax, typed_comment);
    assert!(
        authored_comment_syntax.contains(&framed_er_attribute(
            "string",
            "owner [comment: note]",
            &[],
            ""
        )),
        "{authored_comment_syntax}"
    );
    assert!(
        typed_comment.contains(&framed_er_attribute("string", "owner", &[], "note")),
        "{typed_comment}"
    );

    let authored_key_separator = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner".to_string(),
        keys: vec!["PK,FK".to_string()],
        comment: String::new(),
    });
    let two_typed_keys = render(ErAttributeRenderModel {
        ty: "string".to_string(),
        name: "owner".to_string(),
        keys: vec!["PK".to_string(), "FK".to_string()],
        comment: String::new(),
    });
    assert_ne!(authored_key_separator, two_typed_keys);
    assert!(
        authored_key_separator.contains(r#"keys=[bytes=5 "PK,FK"]"#),
        "{authored_key_separator}"
    );
    assert!(
        two_typed_keys.contains(r#"keys=[bytes=2 "PK", bytes=2 "FK"]"#),
        "{two_typed_keys}"
    );
}

#[test]
fn er_local_semantic_fixture_covers_attributes_with_relationship() {
    let input = read_local_semantic_fixture("er/attributes_with_relationship.mmd");
    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("ER attribute and relationship fixture should render");

    for expected in [
        "CUSTOMER".to_string(),
        "ORDER".to_string(),
        framed_er_attribute("string", "name", &["PK"], ""),
        framed_er_attribute("string", "email", &["UK"], ""),
        framed_er_attribute("int", "age", &[], ""),
        framed_er_attribute("int", "id", &["PK"], ""),
        framed_er_attribute("string", "status", &[], ""),
        "places".to_string(),
        "||".to_string(),
        "o{".to_string(),
    ] {
        assert!(
            rendered.contains(&expected),
            "ER attribute fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    assert!(
        first_line_index_containing(&rendered, "CUSTOMER")
            < first_line_index_containing(&rendered, "ORDER"),
        "identifying relationship should keep CUSTOMER before ORDER in the routed terminal layout:\n{rendered}"
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
fn er_parser_self_relationship_renders_single_box_with_loop() {
    let rendered = render_er(
        "erDiagram\nNODE ||--o{ NODE : \"leads to\"",
        &AsciiRenderOptions::ascii(),
    )
    .expect("self ER relationship should render");

    assert_eq!(
        rendered,
        concat!(
            "+------+\n",
            "| NODE |---||\n",
            "+------+   |\n",
            " leads to  |\n",
            "    o{-----+\n",
        )
    );
}

#[test]
fn er_parser_parallel_self_relationships_share_single_box_loop() {
    let rendered = render_er(
        "erDiagram\nNODE ||--o{ NODE : \"leads to\"\nNODE o|--|| NODE : mirrors",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel self ER relationships should render");

    assert_eq!(
        rendered,
        concat!(
            "+------+\n",
            "| NODE |----||\n",
            "+------+    |\n",
            " leads to   |\n",
            "    o{------+\n",
            "o| mirrors  |\n",
            "    ||------+\n",
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
fn er_parser_md_parent_cardinality_renders_explicit_marker() {
    let rendered = render_er(
        "erDiagram\nPROJECT u--o{ TEAM_MEMBER : parent",
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER parent cardinality should render");

    for expected in ["PROJECT", "TEAM_MEMBER", "<>", "o{", "parent"] {
        assert!(
            rendered.contains(expected),
            "ER parent relationship should keep {expected:?} visible:\n{rendered}"
        );
    }

    let unicode = render_er(
        "erDiagram\nPROJECT u--o{ TEAM_MEMBER : parent",
        &AsciiRenderOptions::unicode(),
    )
    .expect("Unicode ER parent cardinality should render");
    assert!(
        unicode.contains('◆'),
        "Unicode ER parent cardinality should use a diamond marker:\n{unicode}"
    );
}

#[test]
fn er_parser_direction_controls_terminal_layout() {
    let render = |direction| {
        render_er(
            &format!("erDiagram\ndirection {direction}\nA ||--o{{ B : owns"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|err| panic!("ER direction {direction} should render: {err}"))
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

    let mut lowercase_model = parse_er_model("erDiagram\ndirection LR\nA ||--o{ B : owns");
    lowercase_model.direction = "lr".to_string();
    assert_eq!(
        render_er_model(&lowercase_model, &AsciiRenderOptions::ascii())
            .expect("lowercase direct-model ER direction should render"),
        lr
    );
}

#[test]
fn empty_er_model_validates_direction_before_returning_empty() {
    let mut model = ErDiagramRenderModel {
        direction: "lr".to_string(),
        ..ErDiagramRenderModel::default()
    };
    assert_eq!(
        render_er_model(&model, &AsciiRenderOptions::ascii())
            .expect("a valid lowercase direction should permit an empty ER model"),
        ""
    );

    model.direction = "sideways".to_string();
    assert_eq!(
        render_er_model(&model, &AsciiRenderOptions::ascii())
            .expect_err("an empty ER model must not bypass direction validation"),
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER diagram directions",
        }
    );
}

#[test]
fn empty_er_direction_bytes_are_admitted_before_parsing() {
    let model = ErDiagramRenderModel {
        direction: format!("{}sideways", " ".repeat(1_024)),
        ..ErDiagramRenderModel::default()
    };
    let direction_bytes = model.direction.len();
    let model = RenderSemanticModel::Er(model);
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("ER direction work limit should be valid");

    let error = render_model_with_resources(&model, &AsciiRenderOptions::ascii(), resources)
        .expect_err("direction bytes must be admitted before empty-model validation");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == direction_bytes
                && details.max == 1
    ));
}

#[test]
fn er_parser_presentation_metadata_is_intentionally_omitted() {
    let baseline = render_er(
        "erDiagram\ndirection LR\nCUSTOMER ||--o{ ORDER : places",
        &AsciiRenderOptions::ascii(),
    )
    .expect("baseline ER relationship should render");
    let decorated = render_er(
        r#"erDiagram
accTitle: My access title
accDescr {
  A long multi-line description block
}
direction LR
%% this comment is intentionally omitted
classDef important fill:#f9f
CUSTOMER:::important ||--o{ ORDER : places
style CUSTOMER color:red"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("ER presentation metadata should not block terminal rendering");

    assert_eq!(
        decorated, baseline,
        "presentation-only ER metadata should not change the terminal semantic projection"
    );
    for omitted in [
        "My access title",
        "description block",
        "intentionally omitted",
        "important",
        "#f9f",
        "red",
    ] {
        assert!(
            !decorated.contains(omitted),
            "presentation metadata {omitted:?} leaked into ER output:\n{decorated}"
        );
    }
}

#[test]
fn er_parser_horizontal_component_draws_each_entity_once() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "direction LR\n",
            "A ||--o{ B : owns\n",
            "A |o..|{ B : \"may own\"\n",
            "B ||--|| C : joins",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal ER component should render");

    for title in ["| A |", "| B |", "| C |"] {
        assert_eq!(
            rendered.matches(title).count(),
            1,
            "a horizontal ER component must place {title:?} exactly once:\n{rendered}"
        );
    }
    for label in ["owns", "may own", "joins"] {
        assert!(
            rendered.contains(label),
            "horizontal ER routing must preserve {label:?}:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_unrelated_edge_crossings_use_lossless_summary() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "direction LR\n",
            "A {\n  string id\n}\n",
            "B {\n  string id\n}\n",
            "C {\n  string id\n}\n",
            "D {\n  string id\n}\n",
            "A ||--|| C : first\n",
            "B }o..|| D : second\n",
            "A ||--|| B : bridge",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("unrelated horizontal crossings should remain recoverable");

    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in [
        framed_er_summary_relation("A", "||--||", "C", "first"),
        framed_er_summary_relation("B", "}o..||", "D", "second"),
        framed_er_summary_relation("A", "||--||", "B", "bridge"),
    ] {
        assert!(
            rendered.contains(&expected),
            "summary must preserve {expected:?} after owner crossing fallback:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_shared_source_crossings_use_lossless_summary() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "direction LR\n",
            "A ||--|| B : short\n",
            "A ||--|| C : long",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("shared-source horizontal crossings should remain recoverable");

    assert!(rendered.contains("relations:"), "{rendered}");
    for expected in [
        framed_er_summary_relation("A", "||--||", "B", "short"),
        framed_er_summary_relation("A", "||--||", "C", "long"),
    ] {
        assert!(
            rendered.contains(&expected),
            "summary must preserve {expected:?} after shared-source crossing fallback:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_cardinalities_mirror_at_physical_ports() {
    for direction in ["LR", "RL"] {
        let rendered = render_er(
            &format!(
                "erDiagram\ndirection {direction}\nA }}o--o{{ B : \"relationship label that forces a wide lane\""
            ),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|err| panic!("horizontal ER direction {direction} should render: {err}"));

        let (left_title, right_title) = if direction == "LR" {
            ("| A |", "| B |")
        } else {
            ("| B |", "| A |")
        };
        let (_, left_column) = line_and_column_containing(&rendered, left_title);
        let (_, right_column) = line_and_column_containing(&rendered, right_title);
        let connector = rendered
            .lines()
            .find(|line| line.contains("}o") && line.contains("o{"))
            .unwrap_or_else(|| {
                panic!("ER cardinalities must mirror at physical ports:\n{rendered}")
            });
        let between_boxes = &connector[left_column + left_title.len()..right_column];

        assert!(
            !between_boxes.contains(' '),
            "the ER connector must span the complete port-to-port gap:\n{rendered}"
        );
        assert!(
            rendered.contains("relationship label that forces a wide lane"),
            "the ER relationship label must remain attached to its routed lane:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_parallel_self_relations_share_one_box() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "direction LR\n",
            "NODE ||--o{ NODE : children\n",
            "NODE |o..|{ NODE : optional",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal parallel ER self relations should render");

    assert_eq!(
        rendered.matches("| NODE |").count(),
        1,
        "parallel ER self relations must share one entity box:\n{rendered}"
    );
    for expected in ["children", "optional", "||", "o{", "|o", "|{"] {
        assert!(
            rendered.contains(expected),
            "parallel ER self relation must preserve {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_self_relation_mirrors_source_cardinality() {
    let rendered = render_er(
        "erDiagram\ndirection LR\nNODE }o--|| NODE : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal ER self relation should render");

    assert_eq!(rendered.matches("| NODE |").count(), 1, "{rendered}");
    assert!(rendered.contains("}o"), "{rendered}");
    assert!(rendered.contains("||"), "{rendered}");
    assert!(rendered.contains("owns"), "{rendered}");
}

#[test]
fn er_parser_horizontal_mixed_self_and_normal_relations_use_lossless_summary() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "direction LR\n",
            "A ||--o{ A : self\n",
            "A }o--|| B : next",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed horizontal ER relations should remain recoverable");

    assert_eq!(rendered.matches("| A |").count(), 1, "{rendered}");
    assert_eq!(rendered.matches("| B |").count(), 1, "{rendered}");
    for expected in [
        "relations:".to_string(),
        framed_er_summary_relation("A", "||--o{", "A", "self"),
        framed_er_summary_relation("A", "}o--||", "B", "next"),
    ] {
        assert!(
            rendered.contains(&expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_horizontal_direction_propagates_resource_errors() {
    let input = "erDiagram\ndirection RL\nA ||--o{ B : owns";

    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
    ] {
        let resources = AsciiResourcePolicy::default()
            .with_limit(limit, 1)
            .expect("horizontal ER resource limit should be valid");
        let error = render_er_with_resources(input, &AsciiRenderOptions::ascii(), resources)
            .expect_err("horizontal ER rendering must propagate resource errors");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details) if details.limit == limit
        ));
    }
}

#[test]
fn er_parser_preserves_entity_declaration_order() {
    let input = "erDiagram\nZETA\nALPHA\nMIDDLE";
    let model = parse_er_model(input);
    assert!(
        model
            .entities
            .values()
            .map(|entity| entity.label.as_str())
            .eq(["ZETA", "ALPHA", "MIDDLE"])
    );

    let rendered = render_er(input, &AsciiRenderOptions::ascii())
        .expect("declaration-ordered ER entities should render");
    assert!(
        first_line_index_containing(&rendered, "| ZETA |")
            < first_line_index_containing(&rendered, "| ALPHA |")
    );
    assert!(
        first_line_index_containing(&rendered, "| ALPHA |")
            < first_line_index_containing(&rendered, "| MIDDLE |")
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
fn er_parser_multi_parent_relationship_layout_uses_barycenter_order() {
    let rendered = render_er(
        "erDiagram\nA ||--|| C : ac\nA ||--|| D : ad\nB ||--|| C : bc\nC ||--|| E : ce\nC ||--|| F : cf",
        &AsciiRenderOptions::ascii(),
    )
    .expect("multi-parent ER relationship layout should render");

    assert!(
        !rendered.contains("relations:"),
        "multi-parent ER topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in ["ac", "ad", "bc", "ce", "cf"] {
        assert!(
            rendered.contains(expected),
            "routed multi-parent ER topology should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_child_weighted_parent_order_keeps_readable_layout_routed() {
    let rendered = render_er(
        "erDiagram\nA ||--|| E : ae\nB ||--|| D : bd\nC ||--|| E : ce",
        &AsciiRenderOptions::ascii(),
    )
    .expect("child-weighted ER topology should render");

    assert!(
        !rendered.contains("relations:"),
        "child-weighted ER topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in ["A", "B", "C", "D", "E", "ae", "bd", "ce"] {
        assert!(
            rendered.contains(expected),
            "routed child-weighted ER topology should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_four_layer_relationship_layout_uses_iterative_sweep_order() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "C0 ||--|| D0 : c0d0\n",
            "A1 ||--|| B1 : a1b1\n",
            "A2 ||--|| B0 : a2b0\n",
            "A1 ||--|| B0 : a1b0\n",
            "B2 ||--|| C2 : b2c2\n",
            "B2 ||--|| C0 : b2c0\n",
            "C2 ||--|| D1 : c2d1\n",
            "C2 ||--|| D0 : c2d0\n",
            "A0 ||--|| B1 : a0b1\n",
            "A0 ||--|| B2 : a0b2\n",
            "C1 ||--|| D2 : c1d2\n",
            "B2 ||--|| C1 : b2c1",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("four-layer ER relationship topology should render");

    assert!(
        !rendered.contains("relations:"),
        "four-layer ER topology should use routed layout, not summary:\n{rendered}"
    );
    for expected in [
        "a0b1", "a0b2", "a1b0", "a1b1", "a2b0", "b2c0", "b2c1", "b2c2", "c0d0", "c1d2", "c2d0",
        "c2d1",
    ] {
        assert!(
            rendered.contains(expected),
            "routed four-layer ER topology should keep {expected:?} visible:\n{rendered}"
        );
    }
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
fn er_parser_parallel_relationship_layout_uses_lossless_summary_when_ports_do_not_fit() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nA ||..o{ B : contains",
        &AsciiRenderOptions::ascii(),
    )
    .expect("parallel ER relationships should preserve every relationship");

    let owns = framed_er_summary_relation("A", "||--||", "B", "owns");
    let contains = framed_er_summary_relation("A", "||..o{", "B", "contains");
    assert_eq!(
        rendered,
        format!("+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\nrelations:\n{owns}\n{contains}\n")
    );
}

#[test]
fn er_parser_bidirectional_relationship_layout_preserves_both_directions_in_summary() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba",
        &AsciiRenderOptions::ascii(),
    )
    .expect("bidirectional ER relationships should remain recoverable");

    let ab = framed_er_summary_relation("A", "||--||", "B", "ab");
    let ba = framed_er_summary_relation("B", "||--||", "A", "ba");
    assert_eq!(
        rendered,
        format!("+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\nrelations:\n{ab}\n{ba}\n")
    );
}

#[test]
fn er_parser_mixed_parallel_relationship_layout_preserves_all_facts_in_summary() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : a\nA ||..o{ B : b\nA ||--|| C : c",
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed parallel ER relationships should preserve every relationship");

    let a = framed_er_summary_relation("A", "||--||", "B", "a");
    let b = framed_er_summary_relation("A", "||..o{", "B", "b");
    let c = framed_er_summary_relation("A", "||--||", "C", "c");
    assert_eq!(
        rendered,
        format!(
            "+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\n+---+\n| C |\n+---+\n\nrelations:\n{a}\n{b}\n{c}\n"
        )
    );
}

#[test]
fn er_aliases_preserve_authored_entity_identities_when_display_labels_collide() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "A[\"X\"]\n",
            "B[\"X\"]\n",
            "C[\"X\"]\n",
            "A ||--|| B : first\n",
            "A ||..o{ B : second\n",
            "A ||--|| C : third\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("colliding ER aliases should keep lossless relationship identities");

    for identity in ["A", "B", "C"].map(framed_er_summary_endpoint) {
        assert!(
            rendered.contains(&identity),
            "ER boxes must preserve authored endpoint identity {identity:?}:\n{rendered}"
        );
    }
    for label in ["first", "second", "third"] {
        assert!(rendered.contains(label), "missing {label:?}:\n{rendered}");
    }
    for generated_id in ["entity-A-0", "entity-B-1", "entity-C-2"] {
        assert!(
            !rendered.contains(generated_id),
            "generated entity ids must not leak into summaries:\n{rendered}"
        );
    }
}

#[test]
fn er_summary_frames_direct_authored_identities_that_share_one_display_label() {
    let control_identity = "\u{1b}".to_string();
    let authored_escape_identity = r"\u{1B}".to_string();
    let control_rendered_id = "entity-control-0".to_string();
    let escape_rendered_id = "entity-escape-1".to_string();
    let bridge_rendered_id = "entity-bridge-2".to_string();
    let mut model = ErDiagramRenderModel::default();
    model.entities.insert(
        control_identity.clone(),
        ErEntityRenderModel {
            id: control_rendered_id.clone(),
            label: "Same".to_string(),
            ..ErEntityRenderModel::default()
        },
    );
    model.entities.insert(
        authored_escape_identity.clone(),
        ErEntityRenderModel {
            id: escape_rendered_id.clone(),
            label: "Same".to_string(),
            ..ErEntityRenderModel::default()
        },
    );
    model.entities.insert(
        "bridge".to_string(),
        ErEntityRenderModel {
            id: bridge_rendered_id.clone(),
            label: "Bridge".to_string(),
            ..ErEntityRenderModel::default()
        },
    );
    model.relationships = vec![
        ErRelationshipRenderModel {
            entity_a: control_rendered_id.clone(),
            role_a: "first".to_string(),
            entity_b: bridge_rendered_id.clone(),
            rel_spec: ErRelSpecRenderModel {
                card_a: "ONLY_ONE".to_string(),
                card_b: "ONLY_ONE".to_string(),
                rel_type: "IDENTIFYING".to_string(),
            },
        },
        ErRelationshipRenderModel {
            entity_a: bridge_rendered_id.clone(),
            role_a: "second".to_string(),
            entity_b: escape_rendered_id.clone(),
            rel_spec: ErRelSpecRenderModel {
                card_a: "ONLY_ONE".to_string(),
                card_b: "ONLY_ONE".to_string(),
                rel_type: "IDENTIFYING".to_string(),
            },
        },
        ErRelationshipRenderModel {
            entity_a: control_rendered_id.clone(),
            role_a: "spanning".to_string(),
            entity_b: escape_rendered_id.clone(),
            rel_spec: ErRelSpecRenderModel {
                card_a: "ONLY_ONE".to_string(),
                card_b: "ONLY_ONE".to_string(),
                rel_type: "IDENTIFYING".to_string(),
            },
        },
    ];

    let rendered = render_er_model(&model, &AsciiRenderOptions::ascii())
        .expect("direct ER identities should remain recoverable");

    assert!(rendered.contains("relations:"), "{rendered}");
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.contains("| Same"))
            .count(),
        2,
        "{rendered}"
    );
    assert!(
        rendered.contains(r#"id(bytes=1)="\u{1B}""#),
        "the normalized control id must retain its authored byte identity:\n{rendered}"
    );
    assert!(
        rendered.contains(r#"id(bytes=6)="\\u{1B}""#),
        "the authored escape must remain distinct from the raw control id:\n{rendered}"
    );
    for rendered_id in [control_rendered_id, escape_rendered_id, bridge_rendered_id] {
        assert!(
            !rendered.contains(&rendered_id),
            "renderer-owned entity ids must not leak into summaries:\n{rendered}"
        );
    }
}

#[test]
fn er_terminal_normalization_discloses_the_authored_box_identity() {
    let render_identity = |identity: &str| {
        let mut model = ErDiagramRenderModel::default();
        model.entities.insert(
            identity.to_string(),
            ErEntityRenderModel {
                id: "entity-0".to_string(),
                label: identity.to_string(),
                ..ErEntityRenderModel::default()
            },
        );
        render_er_model(&model, &AsciiRenderOptions::ascii())
            .expect("direct ER identity should render")
    };

    let control = render_identity("\u{1b}");
    let authored_escape = render_identity(r"\u{1B}");

    assert!(control.contains(r#"id(bytes=1)="\u{1B}""#), "{control}");
    assert_ne!(control, authored_escape);
}

#[test]
fn er_single_line_display_projection_discloses_the_authored_owner() {
    let render_display = |display: &str, as_alias: bool| {
        let mut model = ErDiagramRenderModel::default();
        model.entities.insert(
            "AUTHORED_ID".to_string(),
            ErEntityRenderModel {
                id: "entity-0".to_string(),
                label: if as_alias {
                    "fallback".to_string()
                } else {
                    display.to_string()
                },
                alias: if as_alias {
                    display.to_string()
                } else {
                    String::new()
                },
                ..ErEntityRenderModel::default()
            },
        );
        render_er_model(&model, &AsciiRenderOptions::ascii())
            .expect("direct ER display text should render")
    };

    for (authored, projected_literal, disclosure) in [
        ("\u{1b}", r"\u{1B}", r#"label(bytes=1)="\u{1B}""#),
        ("\n", r"\u{A}", r#"label(bytes=1)="\n""#),
    ] {
        let transformed = render_display(authored, false);
        let literal = render_display(projected_literal, false);

        assert!(
            transformed.contains(disclosure),
            "missing authored display disclosure {disclosure:?}:\n{transformed}"
        );
        assert!(
            transformed.contains(r#"id(bytes=11)="AUTHORED_ID""#),
            "the fixed ER identity should remain disclosed:\n{transformed}"
        );
        assert_ne!(transformed, literal);
    }

    let alias = render_display("\u{1b}", true);
    assert!(
        alias.contains(r#"alias(bytes=1)="\u{1B}""#),
        "the authored alias must own its disclosure row:\n{alias}"
    );
}

#[test]
fn er_parser_spanning_level_relationship_layout_summarizes_invalid_outer_port() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : a\nB ||--|| C : b\nA ||--|| C : c",
        &AsciiRenderOptions::ascii(),
    )
    .expect("spanning-level ER relationships should remain recoverable");

    let a = framed_er_summary_relation("A", "||--||", "B", "a");
    let b = framed_er_summary_relation("B", "||--||", "C", "b");
    let c = framed_er_summary_relation("A", "||--||", "C", "c");
    assert_eq!(
        rendered,
        format!(
            "+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\n+---+\n| C |\n+---+\n\nrelations:\n{a}\n{b}\n{c}\n"
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
    assert!(rendered.contains(&framed_er_attribute("date", "created_at", &[], "")));
    assert!(rendered.contains(&framed_er_attribute("string", "status", &[], "")));
    assert!(!rendered.contains("created_│at"));
    assert!(!rendered.contains("sta│us"));
}

#[test]
fn er_parser_cyclic_relationship_layout_summarizes_disconnected_back_edge() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : owns\nB ||--|| C : owns\nC ||--|| A : owns",
        &AsciiRenderOptions::ascii(),
    )
    .expect("cyclic ER relationships should render");

    let ab = framed_er_summary_relation("A", "||--||", "B", "owns");
    let bc = framed_er_summary_relation("B", "||--||", "C", "owns");
    let ca = framed_er_summary_relation("C", "||--||", "A", "owns");
    assert_eq!(
        rendered,
        format!(
            "+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\n+---+\n| C |\n+---+\n\nrelations:\n{ab}\n{bc}\n{ca}\n"
        )
    );
}

#[test]
fn er_parser_parallel_relationship_layout_keeps_diagram_when_ports_fit() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "VeryWideParent ||--|| VeryWideChild : p\n",
            "VeryWideParent ||..o{ VeryWideChild : b",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("wide parallel ER relationships should keep distinct lanes");

    assert!(!rendered.contains("relations:"), "{rendered}");
    for marker in ["||", "o{"] {
        assert!(rendered.contains(marker), "missing {marker:?}:\n{rendered}");
    }
    for label in ["p", "b"] {
        assert!(rendered.contains(label), "missing {label:?}:\n{rendered}");
    }
}

#[test]
fn er_parser_dense_crossing_relationships_fall_back_to_relation_summary() {
    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba\nA ||--|| C : ac\nC ||--|| A : ca\nB ||--|| C : bc\nC ||--|| B : cb",
        &AsciiRenderOptions::ascii(),
    )
    .expect("dense ER relationships should render through relation summary fallback");

    let ab = framed_er_summary_relation("A", "||--||", "B", "ab");
    let ba = framed_er_summary_relation("B", "||--||", "A", "ba");
    let ac = framed_er_summary_relation("A", "||--||", "C", "ac");
    let ca = framed_er_summary_relation("C", "||--||", "A", "ca");
    let bc = framed_er_summary_relation("B", "||--||", "C", "bc");
    let cb = framed_er_summary_relation("C", "||--||", "B", "cb");
    assert_eq!(
        rendered,
        format!(
            "+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\n+---+\n| C |\n+---+\n\nrelations:\n{ab}\n{ba}\n{ac}\n{ca}\n{bc}\n{cb}\n"
        )
    );
}

#[test]
fn er_parser_k2_2_relationships_use_a_bounded_planar_layout() {
    let rendered = render_er(
        concat!(
            "erDiagram\n",
            "A ||--o{ C : ac\n",
            "A |{--|| D : ad\n",
            "B o|..|{ C : bc\n",
            "B ||--|| D : bd",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("bounded K2,2 ER relationships should render diagrammatically");

    assert!(
        !rendered.contains("relations:"),
        "a strict K2,2 component should use the bounded planar layout:\n{rendered}"
    );
    for expected in ["A", "B", "C", "D", "ac", "ad", "bc", "bd", "||", "o{", "|{"] {
        assert!(
            rendered.contains(expected),
            "bounded K2,2 output should retain {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn er_parser_relationship_layout_propagates_grid_resource_errors() {
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .expect("test resource limit should be valid");

    let error = render_er_with_resources(
        "erDiagram\nCUSTOMER\nORDER\nINVOICE\nCUSTOMER ||--o{ ORDER : \"places<br>orders\"\nORDER ||--|| INVOICE : bills",
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
fn er_parser_relation_summary_can_show_crossing_diagnostic() {
    let options = AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true);

    let rendered = render_er(
        "erDiagram\nA ||--|| B : ab\nB ||--|| A : ba\nA ||--|| C : ac\nC ||--|| A : ca\nB ||--|| C : bc\nC ||--|| B : cb",
        &options,
    )
    .expect("ER crossing summary diagnostic should render");

    assert!(rendered.contains("relations:"), "{rendered}");
    assert!(rendered.contains("reason: crossing"), "{rendered}");
}

#[test]
fn er_parser_independent_relationship_pairs_render_without_shared_summary_state() {
    let options = AsciiRenderOptions::ascii();

    let rendered = render_er(
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\nINVOICE ||--|| PAYMENT : captures",
        &options,
    )
    .expect("independent ER relationship pairs should render separately");

    for expected in [
        "CUSTOMER", "ORDER", "INVOICE", "PAYMENT", "places", "captures",
    ] {
        assert!(
            rendered.contains(expected),
            "independent ER relationship pairs should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("relations:"),
        "independent ER relationship pairs should remain routed without shared summary state:\n{rendered}"
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

    let ab = framed_er_summary_relation("A", "||--||", "B", "ab");
    let ba = framed_er_summary_relation("B", "||--||", "A", "ba");
    let ac = framed_er_summary_relation("A", "||--||", "C", "ac");
    let ca = framed_er_summary_relation("C", "||--||", "A", "ca");
    let bc = framed_er_summary_relation("B", "||--||", "C", "bc");
    let cb = framed_er_summary_relation("C", "||--||", "B", "cb");
    let html_ab = ab.replace('"', "&quot;");
    let html_ba = ba.replace('"', "&quot;");
    let html_ac = ac.replace('"', "&quot;");
    let html_ca = ca.replace('"', "&quot;");
    let html_bc = bc.replace('"', "&quot;");
    let html_cb = cb.replace('"', "&quot;");
    assert_eq!(
        strip_html_spans(&rendered),
        format!(
            "+---+\n| A |\n+---+\n\n+---+\n| B |\n+---+\n\n+---+\n| C |\n+---+\n\nrelations:\n{html_ab}\n{html_ba}\n{html_ac}\n{html_ca}\n{html_bc}\n{html_cb}\n"
        )
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+---+</span>".to_string(),
        "<span style=\"color:#202020\">A</span>".to_string(),
        "<span style=\"color:#303030\">relations:</span>".to_string(),
        format!("<span style=\"color:#505050\">{html_ab}</span>"),
    ] {
        assert!(
            rendered.contains(&expected_fragment),
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
        "CUSTOMER".to_string(),
        "ORDER".to_string(),
        "INVOICE".to_string(),
        "PAYMENT".to_string(),
        "relations:".to_string(),
        format!(
            "{} ||--o{{ {}",
            framed_er_summary_endpoint("CUSTOMER"),
            framed_er_summary_endpoint("ORDER")
        ),
        "places".to_string(),
        "orders".to_string(),
        "belongs".to_string(),
        "to".to_string(),
        "reconciles".to_string(),
        "payment".to_string(),
        "captures".to_string(),
        "funds".to_string(),
    ] {
        assert!(
            rendered.contains(&expected),
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
fn er_parser_complex_styled_example_limits_summary_to_unroutable_component() {
    let rendered = render_er(
        r#"erDiagram
    CAR ||--o{ DRIVER : "insured for"
    CAR }o--|| PERSON : "owned by"
    NODE ||--o{ NODE : "leads to"
    BOOK["Book"]:::core {
      string *title PK "Title"
      string[] author-ref[name](1) FK "Author ref"
    }
    BOOK ||--o{ PAGE : has
    PAGE {
      int number PK
    }
    classDef core fill:#f96,stroke:#333,stroke-width:2px,color:#fff
    class BOOK core"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("complex styled ER example should render");

    for expected in [
        "Book".to_string(),
        framed_er_attribute("string", "*title", &["PK"], "Title"),
        framed_er_attribute("string[]", "author-ref[name](1)", &["FK"], "Author ref"),
        "PAGE".to_string(),
        framed_er_attribute("int", "number", &["PK"], ""),
        "CAR".to_string(),
        "DRIVER".to_string(),
        "PERSON".to_string(),
        "NODE".to_string(),
        "relations:".to_string(),
        format!(
            "{} ||--o{{ {}",
            framed_er_summary_endpoint("CAR"),
            framed_er_summary_endpoint("DRIVER")
        ),
        format!(
            "{} }}o--|| {}",
            framed_er_summary_endpoint("CAR"),
            framed_er_summary_endpoint("PERSON")
        ),
        "insured for".to_string(),
        "owned by".to_string(),
        "leads to".to_string(),
        "has".to_string(),
    ] {
        assert!(
            rendered.contains(&expected),
            "complex styled ER example should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert_eq!(
        rendered.matches("| NODE |").count(),
        1,
        "the routed self relation should keep one entity box:\n{rendered}"
    );
}

#[test]
fn er_local_semantic_fixture_covers_routed_schema_with_attributes() {
    let input = read_local_semantic_fixture("er/routed_schema_with_attributes.mmd");

    let rendered = render_er(&input, &AsciiRenderOptions::ascii())
        .expect("routed schema ER fixture should render");

    for expected in [
        "CUSTOMER".to_string(),
        "ORDER".to_string(),
        "LINE_ITEM".to_string(),
        "PRODUCT".to_string(),
        framed_er_attribute("string", "id", &["PK"], ""),
        framed_er_attribute("string", "email", &["UK"], ""),
        framed_er_attribute("int", "quantity", &[], ""),
        "places".to_string(),
        "contains".to_string(),
        "supplies".to_string(),
    ] {
        assert!(
            rendered.contains(&expected),
            "routed schema fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("relations:"),
        "conflicting LINE_ITEM cardinalities should use the lossless summary:\n{rendered}"
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
