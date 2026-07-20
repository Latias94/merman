mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{
    RenderEnvironment, RenderSession, TextMeasurementPhase, TextMeasurementPolicy,
};
use merman_render::family;
use merman_render::model::{LayoutEdge, SequenceDiagramLayout};
use merman_render::sequence::layout_sequence_diagram_typed_with_title;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use std::path::PathBuf;
#[cfg(feature = "ratex-math")]
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn parse_sequence_for_render(engine: &Engine, text: &str) -> ParsedDiagramRender {
    engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
}

fn layout_sequence_from_parsed(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> SequenceDiagramLayout {
    let RenderSemanticModel::Sequence(model) = parsed.model() else {
        panic!("expected Sequence render model");
    };
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    layout_sequence_diagram_typed_with_title(
        model,
        parsed.metadata().title.as_deref(),
        parsed.metadata().effective_config.as_value(),
        &measurer,
        session.math_renderer(),
    )
    .expect("typed Sequence layout")
}

fn extract_self_closing_tags<'a>(s: &'a str, tag_name: &str) -> Vec<&'a str> {
    let needle = format!("<{tag_name}");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = s[i..].find(&needle) {
        let start = i + pos;
        let Some(end_rel) = s[start..].find("/>") else {
            break;
        };
        let end = start + end_rel + 2;
        out.push(&s[start..end]);
        i = end;
    }
    out
}

fn extract_paired_tags<'a>(s: &'a str, tag_name: &str) -> Vec<&'a str> {
    let needle = format!("<{tag_name}");
    let closing = format!("</{tag_name}>");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = s[i..].find(&needle) {
        let start = i + pos;
        let Some(end_rel) = s[start..].find(&closing) else {
            break;
        };
        let end = start + end_rel + closing.len();
        out.push(&s[start..end]);
        i = end;
    }
    out
}

fn text_rows_by_class(svg: &str, class_name: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
    document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "text"
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == class_name)
                })
        })
        .map(|node| {
            node.descendants()
                .filter(|descendant| descendant.is_text())
                .filter_map(|descendant| descendant.text())
                .collect::<String>()
        })
        .collect()
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn root_view_box_and_max_width(svg: &str) -> ([f64; 4], f64) {
    let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "svg", "expected SVG root element");

    let values = root
        .attribute("viewBox")
        .expect("Sequence root viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    let view_box: [f64; 4] = values
        .try_into()
        .unwrap_or_else(|values: Vec<f64>| panic!("expected four viewBox values: {values:?}"));

    let max_width = root
        .attribute("style")
        .expect("Sequence root style")
        .split(';')
        .map(str::trim)
        .find_map(|declaration| declaration.strip_prefix("max-width:"))
        .map(str::trim)
        .and_then(|value| value.strip_suffix("px"))
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric Sequence root max-width");

    (view_box, max_width)
}

fn sequence_number_x(svg: &str, number: &str) -> f64 {
    extract_paired_tags(svg, "text")
        .into_iter()
        .find(|tag| {
            tag.contains(r#"class="sequenceNumber""#) && tag.ends_with(&format!(">{number}</text>"))
        })
        .and_then(|tag| attr_f64(tag, "x"))
        .unwrap_or_else(|| panic!("missing sequence number {number}: {svg}"))
}

fn render_sequence_svg_from_fixture(fixture: &str) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_sequence_svg_from_text(&text)
}

fn render_sequence_svg_from_fixture_with_options(
    fixture: &str,
    options: &SvgRenderOptions,
) -> String {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

fn sequence_layout_json_from_fixture(fixture: &str) -> serde_json::Value {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    let parsed = parse_sequence_for_render(&Engine::new(), &text);

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Sequence artifact")
        .layout_json()
        .expect("project Sequence layout JSON")
}

fn render_sequence_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    render_sequence_svg_from_text_with_engine(engine, text)
}

fn render_sequence_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let parsed = parse_sequence_for_render(&engine, text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

fn render_sequence_svg_with_theme_variables(
    text: &str,
    theme_variables: serde_json::Value,
) -> String {
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": theme_variables,
    })));
    let parsed = parse_sequence_for_render(&engine, text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

fn layout_sequence_from_text(text: &str) -> SequenceDiagramLayout {
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), text);
    layout_sequence_from_parsed(&parsed, &session)
}

#[test]
fn sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    autonumber
    participant C as Client
    participant S as Server
    participant D as Database
    participant Q as Message Queue

    C->>+S: Submit Order
    S->>D: Save Order
    D-->>S: Confirm
    S->>Q: Send Notification
    S-->>-C: Return Order ID

    Note over Q: Async Processing
    Q->>S: Consume Message
    S->>C: Push Notification"#,
    );

    let activation = extract_self_closing_tags(&svg, "rect")
        .into_iter()
        .find(|tag| tag.contains(r#"class="activation0""#))
        .unwrap_or_else(|| panic!("missing activation rect: {svg}"));
    let activation_left = attr_f64(activation, "x").expect("activation x");
    let activation_width = attr_f64(activation, "width").expect("activation width");
    let activation_right = activation_left + activation_width;

    let n2 = sequence_number_x(&svg, "2");
    let n4 = sequence_number_x(&svg, "4");
    let n5 = sequence_number_x(&svg, "5");

    assert!(
        (n2 - (activation_left + 1.0)).abs() <= 0.0001,
        "expected message 2 number to sit inside the left activation bound, got {n2} for activation {activation}"
    );
    assert!(
        (n4 - (activation_left + 1.0)).abs() <= 0.0001,
        "expected message 4 number to sit inside the left activation bound, got {n4} for activation {activation}"
    );
    assert!(
        (n5 - (activation_right - 1.0)).abs() <= 0.0001,
        "expected message 5 number to sit inside the right activation bound, got {n5} for activation {activation}"
    );
}

#[test]
fn sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15() {
    let layout = layout_sequence_from_text(
        r#"sequenceDiagram
    participant C as Caller
    participant A as Active

    C->>+A: Open outer
    A->>+A: Open inner
    C->>A: Call nested
    A-->>-A: Close inner
    C->>A: Call outer"#,
    );

    let a_center = layout
        .nodes
        .iter()
        .find(|node| node.id == "actor-top-A")
        .map(|node| node.x)
        .expect("actor A center");
    let c_to_a_edges: Vec<&LayoutEdge> = layout
        .edges
        .iter()
        .filter(|edge| edge.from == "C" && edge.to == "A")
        .collect();
    assert_eq!(c_to_a_edges.len(), 3, "expected three C->A messages");

    let nested_call = c_to_a_edges[1];
    let outer_call = c_to_a_edges[2];
    let expected_left_target = a_center - 5.0 - 3.0;

    assert!(
        (nested_call.points[1].x - expected_left_target).abs() <= 0.0001,
        "expected nested activation target to use the full activation stack left bound, got {} with A center {a_center}",
        nested_call.points[1].x
    );
    assert!(
        (outer_call.points[1].x - expected_left_target).abs() <= 0.0001,
        "expected remaining outer activation target to keep the same left bound, got {} with A center {a_center}",
        outer_call.points[1].x
    );
}

#[test]
fn sequence_representative_roots_are_finite_and_scale_with_fixture_complexity() {
    let cases = [
        "activation_explicit.mmd",
        "stress_sequence_batch5_many_participants_spacing_050.mmd",
        "zed_pr_57644_sequence.mmd",
    ];
    let mut roots = Vec::new();

    for fixture in cases {
        let svg =
            render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
        let (view_box, max_width) = root_view_box_and_max_width(&svg);

        assert!(
            view_box.into_iter().all(f64::is_finite),
            "expected finite root geometry for {fixture}: {view_box:?}"
        );
        assert!(
            view_box[2] > 0.0 && view_box[3] > 0.0 && max_width.is_finite(),
            "expected positive root extent for {fixture}: viewBox={view_box:?}, max-width={max_width}"
        );
        assert!(
            (max_width - view_box[2]).abs() <= 1e-6,
            "root max-width must track viewBox width for {fixture}: viewBox={view_box:?}, max-width={max_width}"
        );

        roots.push((view_box[2], view_box[3]));
    }

    let activation = roots[0];
    let many_participants = roots[1];
    let long_conversation = roots[2];
    assert!(
        many_participants.0 > long_conversation.0 && long_conversation.0 > activation.0,
        "participant count should drive representative root widths: {roots:?}"
    );
    assert!(
        long_conversation.1 > many_participants.1 && many_participants.1 > activation.1,
        "message depth should drive representative root heights: {roots:?}"
    );
}

#[test]
fn sequence_block_root_width_replays_upstream_bounds_insert_lifecycle() {
    for (fixture, expected_min_x, expected_width) in [
        ("stress_create_destroy_inside_alt_030.mmd", -50.0, 734.0),
        ("stress_critical_break_007.mmd", -50.0, 650.0),
    ] {
        let svg = render_sequence_svg_from_fixture(fixture);
        let (view_box, max_width) = root_view_box_and_max_width(&svg);
        assert_eq!(
            view_box[0], expected_min_x,
            "unexpected root x for {fixture}"
        );
        assert_eq!(
            view_box[2], expected_width,
            "unexpected width for {fixture}"
        );
        assert_eq!(max_width, expected_width);
    }
}

#[test]
fn sequence_actor_lifecycle_adjustment_survives_block_close() {
    let fixture = "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010.mmd";
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(path).expect("fixture");
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let layout = layout_sequence_from_parsed(&parsed, &session);
    let actor = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing lifecycle actor {id}"))
    };

    let alice_top = actor("actor-top-Alice");
    let bob_top = actor("actor-top-Bob");
    let john_top = actor("actor-top-John");
    let alice_bottom = actor("actor-bottom-Alice");
    let bob_bottom = actor("actor-bottom-Bob");
    let john_bottom = actor("actor-bottom-John");

    assert!(
        john_top.y > alice_top.y.max(bob_top.y),
        "created actor must begin below the initially declared actors"
    );
    assert!(
        john_bottom.y < alice_bottom.y.min(bob_bottom.y),
        "destroyed actor must end before ordinary footer actors"
    );

    let lifeline = layout
        .edges
        .iter()
        .find(|edge| edge.id == "lifeline-John")
        .expect("John lifecycle edge");
    assert_eq!(lifeline.from, john_top.id);
    assert_eq!(lifeline.to, john_bottom.id);
    let lifeline_start = lifeline.points.first().expect("lifeline start").y;
    let lifeline_end = lifeline.points.last().expect("lifeline end").y;
    let creation_boundary = john_top.y + john_top.height / 2.0;
    let destruction_boundary = john_bottom.y - john_bottom.height / 2.0;

    assert!(
        (lifeline_start - creation_boundary).abs() <= 1e-6
            && (lifeline_end - destruction_boundary).abs() <= 1e-6
            && lifeline_start < lifeline_end,
        "John's lifeline must remain bounded by its create/destroy actors after block closure"
    );
}

#[test]
fn sequence_font_size_precedence_matches_fresh_mermaid_11_16_root() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_sequence_font_size_precedence_090.mmd",
        &SvgRenderOptions::default(),
    );
    let root = svg.split_once('>').expect("SVG root").0;
    let note = extract_self_closing_tags(&svg, "rect")
        .into_iter()
        .find(|tag| tag.contains(r#"class="note""#))
        .expect("expected note rectangle");

    assert_eq!(attr_f64(note, "height"), Some(31.0));
    assert!(
        root.contains(r#"style="max-width: 550px; background-color: white;""#)
            && root.contains(r#"viewBox="-50 -10 550 244""#),
        "expected fresh Mermaid 11.16 font-size root geometry: {root}"
    );
}

#[test]
fn sequence_parity_wraps_message_candidates_with_calculate_text_width_bbox() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_br_in_messages_notes_011.mmd",
        &SvgRenderOptions::default(),
    );
    let wrapped_message =
        "This is a longer message that should be wrapped by Mermaid&#39;s default behavior";
    let message_lines = extract_paired_tags(&svg, "text")
        .into_iter()
        .filter(|tag| tag.contains(r#"class="messageText""#))
        .collect::<Vec<_>>();

    assert_eq!(
        message_lines.len(),
        5,
        "Mermaid 11.16 keeps the wrapped message on one line: {message_lines:#?}"
    );
    assert!(
        message_lines
            .iter()
            .any(|line| line.contains(wrapped_message)),
        "expected the complete wrapped message in one SVG text node: {message_lines:#?}"
    );
}

#[test]
fn sequence_calculate_text_dimensions_wraps_long_notes_to_six_rows() {
    let expected_rows = [
        "Extremely utterly long",
        "line of longness which",
        "had previously",
        "overflown the actor box",
        "as it is much longer",
        "than what it should be",
    ];
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let svg =
            render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
        let note_rows = text_rows_by_class(&svg, "noteText");
        let note_rect = extract_self_closing_tags(&svg, "rect")
            .into_iter()
            .find(|tag| tag.contains(r#"class="note""#))
            .expect("wrapped note rectangle");

        assert_eq!(note_rows, expected_rows, "unexpected rows for {fixture}");
        assert_eq!(
            attr_f64(note_rect, "width"),
            Some(173.0),
            "unexpected note width for {fixture}"
        );
    }
}

#[test]
fn sequence_calculate_text_dimensions_keeps_first_wrapped_message_on_two_rows() {
    let fixture =
        "upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048.mmd";
    let svg = render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
    let message_rows = text_rows_by_class(&svg, "messageText");

    assert_eq!(
        message_rows.len(),
        10,
        "unexpected message rows: {message_rows:#?}"
    );
    assert_eq!(
        &message_rows[..2],
        [
            "Hello John, how are you today?",
            "I'm feeling quite verbose today."
        ],
        "the first wrapped message must stay on two rows"
    );
}

#[test]
fn sequence_fallback_wraps_block_candidates_without_losing_text() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "upstream_critical_without_options_spec.mmd",
        &SvgRenderOptions::default(),
    );
    let loop_lines = text_rows_by_class(&svg, "loopText");

    assert_eq!(
        loop_lines.len(),
        2,
        "the configured critical-title cap must wrap into two rows: {loop_lines:#?}"
    );
    assert_eq!(loop_lines.join(" "), "[Establish a connection to the DB]");
}

#[test]
fn sequence_nested_opt_wraps_from_source_block_width_like_mermaid_11_16() {
    let fixture = "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037.mmd";
    let svg = render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
    let group_start = svg
        .find(r#"<g data-et="control-structure" data-id="i17">"#)
        .unwrap_or_else(|| panic!("missing nested opt control group: {svg}"));
    let group_tail = &svg[group_start..];
    let group_end = group_tail
        .find("</g>")
        .unwrap_or_else(|| panic!("unterminated nested opt control group: {group_tail}"));
    let loop_lines: Vec<&str> = extract_paired_tags(&group_tail[..group_end], "text")
        .into_iter()
        .filter(|tag| tag.contains(r#"class="loopText""#))
        .collect();

    assert_eq!(
        loop_lines.len(),
        3,
        "nested opt title should use three Mermaid 11.16 lines"
    );
    for (line, expected) in loop_lines.iter().zip([
        "[this is a nested opt",
        "with a long title that",
        "will overflow]",
    ]) {
        assert!(
            line.contains(&format!(">{expected}</tspan>")),
            "unexpected nested opt title line: {line}"
        );
    }
}

#[test]
fn sequence_layout_json_preserves_family_wire_shape() {
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037.mmd",
        "upstream_alt_multiple_elses_spec.mmd",
        "upstream_par_multiple_ands_spec.mmd",
        "upstream_critical_with_options_spec.mmd",
    ] {
        let layout_json = sequence_layout_json_from_fixture(fixture);
        assert_eq!(
            layout_json.pointer("/meta/diagram_type"),
            Some(&serde_json::Value::String("sequence".to_string())),
            "unexpected Sequence metadata projection for {fixture}"
        );
        assert_eq!(
            layout_json.pointer("/semantic/type"),
            Some(&serde_json::Value::String("sequence".to_string())),
            "unexpected Sequence semantic projection for {fixture}"
        );
        assert!(
            layout_json
                .pointer("/semantic/messages")
                .is_some_and(serde_json::Value::is_array),
            "Sequence semantic messages must remain an array for {fixture}: {layout_json}"
        );
        let layout = layout_json
            .pointer("/layout/SequenceDiagram")
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| {
                panic!("missing SequenceDiagram layout projection for {fixture}: {layout_json}")
            });
        assert!(
            ["nodes", "edges", "clusters"]
                .into_iter()
                .all(|key| layout.get(key).is_some_and(serde_json::Value::is_array)),
            "Sequence layout collections must remain arrays for {fixture}: {layout_json}"
        );
        assert!(
            layout
                .get("bounds")
                .is_some_and(serde_json::Value::is_object),
            "Sequence layout bounds must remain an object for {fixture}: {layout_json}"
        );
    }
}

#[test]
fn sequence_bracketed_block_titles_receive_the_renderer_bracket_pair() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    par [Action 1]
        Alice->>Bob: First
    and [Action 2]
        Bob-->>Alice: Second
    end"#,
    );

    assert!(
        svg.contains(">[[Action 1]]</tspan>"),
        "expected the par title to retain its source brackets and receive renderer brackets: {svg}"
    );
    assert!(
        svg.contains(">[[Action 2]]</text>"),
        "expected the and title to retain its source brackets and receive renderer brackets: {svg}"
    );
}

#[test]
fn sequence_autonumber_renders_decimal_sequence_numbers() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
autonumber 10.01 .01
Alice->>Bob:Hello
Bob-->>Alice:Back
Bob->>Alice:Again"#,
    );

    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.01</text>"#),
        "expected first decimal sequence number in SVG"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.02</text>"#),
        "expected second decimal sequence number rounded to hundredths"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.03</text>"#),
        "expected third decimal sequence number rounded to hundredths"
    );
    assert!(
        !svg.contains("10.019999"),
        "expected decimal sequence numbers to avoid floating point artifacts"
    );
}

#[test]
fn sequence_svg_honors_mermaid_11_15_theme_css_options() {
    let svg = render_sequence_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"actorBorder": "#220000", "actorBkg": "#330000", "actorTextColor": "#fafafa", "actorLineColor": "#444444", "signalColor": "#555555", "signalTextColor": "#777777", "labelBoxBorderColor": "#888888", "labelBoxBkgColor": "#999999", "labelTextColor": "#aaaaaa", "loopTextColor": "#bbbbbb", "noteBorderColor": "#cccccc", "noteBkgColor": "#dddddd", "noteTextColor": "#eeeeee", "noteFontWeight": 600, "activationBkgColor": "#010203", "activationBorderColor": "#040506", "nodeBorder": "#070809"}}}%%
sequenceDiagram
autonumber
participant Alice
participant Bob
Alice->>Bob: Hello
activate Bob
Note over Alice,Bob: Readable note
loop Retry
Alice-->>Bob: Again
end"##,
    );

    assert!(
        svg.contains(r#".actor{stroke:#220000;fill:#330000;"#),
        "expected actor theme variables in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#"text.actor>tspan{fill:#fafafa;stroke:none;}"#),
        "expected actor text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".actor-line{stroke:#444444;}"#),
        "expected actor lifeline theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".messageLine0{stroke-width:1.5;stroke-dasharray:none;stroke:#555555;}"#),
        "expected signal color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".messageText{fill:#777777;stroke:none;}"#),
        "expected signal text color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".labelBox{stroke:#888888;fill:#999999;filter:none;}"#),
        "expected label box theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".labelText,#merman .labelText>tspan{fill:#aaaaaa;stroke:none;}"#),
        "expected label text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".loopText,#merman .loopText>tspan{fill:#bbbbbb;stroke:none;}"#),
        "expected loop text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".sectionTitle,#merman .sectionTitle>tspan{fill:#bbbbbb;stroke:none;}"#),
        "expected section title theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".note{stroke:#cccccc;fill:#dddddd;}"#),
        "expected note theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(
            r#".noteText,#merman .noteText>tspan{fill:#eeeeee;stroke:none;font-weight:600;}"#
        ),
        "expected note text theme color and weight in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".activation0{fill:#010203;stroke:#040506;}"#),
        "expected activation theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#"g rect.rect{filter:"#) && svg.contains(r#"stroke:#070809;"#),
        "expected Sequence rect node border theme color in CSS: {svg}"
    );
}

#[test]
fn sequence_note_width_expands_for_literal_br_backslash_t_with_fallback_profile() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("html_br_variants_and_wrap.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let layout = layout_sequence_from_parsed(&parsed, &session);

    let note = layout
        .nodes
        .iter()
        .find(|n| n.id == "note-7")
        .expect("expected note-7 layout node");

    // Mermaid's text-dimension probe treats the escaped `<br \t/>` as literal single-run text,
    // then adds the normal note padding. The reusable fallback profile must preserve that semantic
    // expansion without encoding the fixture's browser-specific width.
    assert!(
        note.width > 150.0 && note.width.is_finite(),
        "expected literal escaped <br> note to expand beyond the default width, got {}",
        note.width
    );
}

#[test]
fn sequence_alt_multiple_elses_separators_touch_frame_edges() {
    let svg = render_sequence_svg_from_fixture("upstream_alt_multiple_elses_spec.mmd");

    let line_tags = extract_self_closing_tags(&svg, "line");
    let loop_lines: Vec<&str> = line_tags
        .into_iter()
        .filter(|t| t.contains(r#"class="loopLine""#))
        .collect();

    let dashed_separators: Vec<&str> = loop_lines
        .iter()
        .copied()
        .filter(|t| t.contains("stroke-dasharray: 3, 3"))
        .collect();
    assert_eq!(
        dashed_separators.len(),
        2,
        "expected 2 dashed separators for 3 alt sections"
    );

    let y0 = attr_f64(dashed_separators[0], "y1").expect("sep y1");
    let y1 = attr_f64(dashed_separators[1], "y1").expect("sep y1");
    assert!(
        (y0 - y1).abs() > 0.0001,
        "expected separators to have distinct y"
    );

    let mut frame_min_x = f64::INFINITY;
    let mut frame_max_x = f64::NEG_INFINITY;
    for t in &loop_lines {
        if t.contains("style=") {
            continue;
        }
        let (Some(x1), Some(x2)) = (attr_f64(t, "x1"), attr_f64(t, "x2")) else {
            continue;
        };
        if (x1 - x2).abs() <= 0.0001 {
            frame_min_x = frame_min_x.min(x1);
            frame_max_x = frame_max_x.max(x1);
        }
    }
    assert!(frame_min_x.is_finite() && frame_max_x.is_finite());

    for sep in dashed_separators {
        let x1 = attr_f64(sep, "x1").expect("sep x1");
        let x2 = attr_f64(sep, "x2").expect("sep x2");
        assert!(
            x1 <= frame_min_x + 0.0001,
            "expected separator x1 ({x1}) to touch frame left edge ({frame_min_x})"
        );
        assert!(
            x2 >= frame_max_x - 0.0001,
            "expected separator x2 ({x2}) to touch frame right edge ({frame_max_x})"
        );
    }
}

#[test]
fn sequence_rect_block_is_root_level_before_actors() {
    let svg = render_sequence_svg_from_fixture("upstream_rect_block_spec.mmd");

    let fill_pos = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected rect fill to match directive payload");
    let rect_pos = svg[..fill_pos]
        .rfind("<rect")
        .expect("expected rect tag for fill");
    let rect_end_rel = svg[rect_pos..]
        .find("/>")
        .expect("expected self-closing rect tag");
    let rect_tag = &svg[rect_pos..(rect_pos + rect_end_rel + 2)];
    assert!(rect_tag.contains(r#"class="rect""#), "expected rect class");

    let actor_pos = svg
        .find(r#"class="actor actor-bottom""#)
        .expect("expected bottom actors");
    assert!(
        rect_pos < actor_pos,
        "expected rect blocks to be emitted before actor groups"
    );
}

#[test]
fn sequence_bare_rect_uses_resolved_theme_fill_and_explicit_override() {
    let bare_rect = r#"sequenceDiagram
participant A
participant B
rect
A->>B: Hello
end"#;

    let rect_fill = render_sequence_svg_with_theme_variables(
        bare_rect,
        serde_json::json!({
            "rectBkgColor": "#112233",
            "actorBkg": "#445566"
        }),
    );
    assert!(
        extract_self_closing_tags(&rect_fill, "rect")
            .into_iter()
            .any(|tag| tag.contains(r#"class="rect""#) && tag.contains(r##"fill="#112233""##)),
        "rectBkgColor should be the first bare rect fallback: {rect_fill}"
    );

    let explicit_fill = render_sequence_svg_with_theme_variables(
        &bare_rect.replacen("rect\n", "rect rgb(1, 2, 3)\n", 1),
        serde_json::json!({ "rectBkgColor": "#112233" }),
    );
    assert!(
        extract_self_closing_tags(&explicit_fill, "rect")
            .into_iter()
            .any(|tag| tag.contains(r#"class="rect""#) && tag.contains(r#"fill="rgb(1, 2, 3)""#)),
        "an explicit rect color should override theme fallbacks: {explicit_fill}"
    );
}

#[test]
fn sequence_nested_rect_blocks_render_in_start_order() {
    let svg = render_sequence_svg_from_fixture("upstream_nested_rect_blocks_spec.mmd");

    let outer = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected outer rect fill");
    let inner = svg
        .find(r#"fill="rgb(0, 0, 0)""#)
        .expect("expected inner rect fill");
    assert!(
        outer < inner,
        "expected nested rect blocks to be emitted in start order"
    );
}

#[test]
fn sequence_notes_render_inline_with_block_frames() {
    let svg = render_sequence_svg_from_fixture("stress_end_in_labels_025.mmd");

    let loop_pos = svg
        .find("[health(end)check]")
        .expect("expected loop frame label");
    let note_pos = svg.find(r#"class="note""#).expect("expected note group");
    let alt_pos = svg
        .find("[should continue]")
        .expect("expected alt frame label");

    assert!(
        loop_pos < note_pos,
        "expected completed loop frame to render before the later note"
    );
    assert!(
        note_pos < alt_pos,
        "expected note to render before its enclosing alt frame closes"
    );
}

#[test]
fn sequence_notes_expand_viewbox_left_for_leftof_notes() {
    let svg = render_sequence_svg_from_fixture("notes_placements.mmd");
    assert!(
        svg.contains(r#"viewBox="-150 -10"#),
        "expected viewBox min_x to expand for left-of notes"
    );
    assert!(
        svg.contains(r#"max-width: 750px"#),
        "expected max-width to reflect expanded viewBox width"
    );
}

#[test]
#[ignore = "documented Sequence root-width residual: deterministic local 570px vs Mermaid 11.16 upstream 567px"]
fn sequence_long_leftof_notes_keep_mermaid_11_16_root_width() {
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let svg = render_sequence_svg_from_fixture(fixture);
        assert!(
            svg.contains(r#"max-width: 567px"#),
            "expected long left-of note fixture {fixture} to keep Mermaid 11.16 root width"
        );
    }
}

#[test]
fn sequence_long_leftof_notes_drop_the_stale_width_slack() {
    let engine = Engine::new();
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let path = workspace_root()
            .join("fixtures")
            .join("sequence")
            .join(fixture);
        let text = std::fs::read_to_string(&path).expect("fixture");
        let parsed = parse_sequence_for_render(&engine, &text);
        let session = RenderEnvironment::parity()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .unwrap();
        let layout = layout_sequence_from_parsed(&parsed, &session);
        let note = layout
            .nodes
            .iter()
            .find(|n| n.id == "note-1")
            .expect("expected note-1 layout node");
        assert_eq!(
            note.width, 155.0,
            "expected long left-of note fixture {fixture} to use the source-backed wrapped width"
        );
    }
}

#[test]
fn sequence_frontmatter_title_expands_layout_root_y() {
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_html_demos_sequence_sequence_diagram_demos_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    assert_eq!(
        parsed.metadata().title.as_deref(),
        Some("With forced menus")
    );
    let RenderSemanticModel::Sequence(model) = parsed.model() else {
        panic!("expected Sequence render model");
    };
    assert!(
        model.title.is_none(),
        "frontmatter title should stay in parse metadata, not the sequence semantic title"
    );

    let layout = layout_sequence_from_parsed(&parsed, &session);
    let bounds = layout.bounds.as_ref().expect("sequence root bounds");
    assert_eq!(bounds.min_y, -50.0);
}

#[test]
fn sequence_message_font_size_override_matches_mermaid_cli_baselines() {
    // Mermaid CLI (mmdc) currently does not reflect `sequence.messageFontSize` overrides in the
    // emitted SVG; it sticks to the global `fontSize` defaults. Keep our Stage B output aligned
    // with the upstream baselines under `fixtures/upstream-svgs/sequence`.
    let svg = render_sequence_svg_from_fixture(
        "upstream_cypress_sequencediagram_spec_should_render_different_message_fonts_when_configured_011.mmd",
    );
    assert!(
        svg.contains("font-size: 16px"),
        "expected message/actor text to use the global fontSize (16px) like Mermaid CLI baselines"
    );
    assert!(
        !svg.contains("font-size: 18px"),
        "expected sequence.messageFontSize (18px) to not affect SVG output under the pinned upstream baselines"
    );
}

#[test]
fn sequence_central_connection_rtl_layout_matches_fixture_golden_spacing() {
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(
            "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033.mmd",
        );
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let layout = layout_sequence_from_parsed(&parsed, &session);

    let actor_center = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.x)
            .unwrap_or_else(|| panic!("missing node {id}"))
    };

    assert_eq!(actor_center("actor-top-Alice"), 75.0);
    assert_eq!(actor_center("actor-top-Bob"), 443.0);
    assert_eq!(actor_center("actor-top-Charlie"), 820.0);

    let edge = layout
        .edges
        .iter()
        .find(|edge| edge.id == "msg-1")
        .expect("expected first central-connection edge");
    assert_eq!(edge.points.len(), 2);
    assert_eq!(edge.points[0].x, 442.0);
    assert_eq!(edge.points[1].x, 83.0);
}

#[test]
fn sequence_central_connection_rtl_svg_uses_layout_actor_centers() {
    let fixture = "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033.mmd";
    let svg = render_sequence_svg_from_fixture(fixture);

    assert!(
        svg.contains(r#"<text x="443" y="32.5""#),
        "expected Bob top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        svg.contains(r#"<text x="820" y="32.5""#),
        "expected Charlie top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        extract_self_closing_tags(&svg, "line")
            .into_iter()
            .any(|tag| {
                tag.contains(r#"x1="442""#)
                    && tag.contains(r#"x2="83""#)
                    && tag.contains(r#"class="messageLine"#)
            }),
        "expected first message x positions to stay near layout/golden spacing: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn sequence_svg_renders_ratex_math_message_and_note_end_to_end() {
    let text = r#"sequenceDiagram
participant A
participant B
A->>B: $$x^2$$
Note right of B: $$x^2$$
"#;
    let environment = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
    let session = environment.begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let svg = rendered.svg();

    assert!(
        svg.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG sizing in sequence labels: {svg}"
    );
    assert!(
        svg.contains(r#"<div style="width: fit-content;""#),
        "expected Sequence math labels to use the KaTeX foreignObject shell: {svg}"
    );
    assert!(
        svg.contains("<path"),
        "expected RaTeX glyph paths in sequence SVG: {svg}"
    );
    assert!(
        !svg.contains("$$x^2$$"),
        "expected math source delimiters to be replaced by rendered SVG: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn sequence_docs_math_fixture_renders_supported_ratex_formulas() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_docs_math_sequence_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let environment = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
    let session = environment.begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let svg = rendered.svg();

    let inline_formula_count = svg
        .matches(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 "#)
        .count();
    assert!(
        inline_formula_count >= 7,
        "expected participant, message, and note math labels to render through RaTeX: {svg}"
    );
    assert!(
        !svg.contains(r#"Solve: $$\sqrt{2+2}$$"#) && !svg.contains(r#"Answer: $$2$$"#),
        "expected mixed sequence message formulas to replace source delimiters: {svg}"
    );
}
