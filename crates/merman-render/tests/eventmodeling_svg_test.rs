mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::model::EventModelingDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{DiagramFamilyId, LayoutOptions};

const THEME_TEXT_SOURCE: &str = r#"eventmodeling
tf 01 ui View
tf 02 cmd Run ->> 01
tf 03 evt Done ->> 02
"#;

const CORE_PAINT_SOURCE: &str = "eventmodeling\ntf 01 ui View\ntf 02 cmd Run ->> 01\ntf 03 evt Done ->> 02\ntf 04 rmo State ->> 03\ntf 05 pcr Work ->> 04\n";

fn render_eventmodeling_with_session(
    source: &str,
    engine: MermaidEngine,
    session: RenderSession,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Event Modeling")
        .expect("detect Event Modeling")
        .into_presentation();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_eventmodeling(
    source: &str,
    engine: MermaidEngine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    render_eventmodeling_with_session(source, engine, session, diagram_id)
}

fn render_eventmodeling_with_raw_theme_css(
    environment: &RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        serde_json::json!({
            "themeCSS": ".em-box span { color: #de3163; } .em-swimlane { fill: #2468ac; }"
        }),
    ));
    render_eventmodeling_with_session(
        THEME_TEXT_SOURCE,
        engine,
        environment.begin_session().unwrap(),
        "eventmodeling-theme-css",
    )
}

fn eventmodeling_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Event Modeling SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Event Modeling stylesheet")
        .to_string()
}

fn render_authored_eventmodeling_title(
    title: &str,
    font_size_css: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    use merman_model::families::event_modeling::{EventModelingDiagramModel, EventModelingFrame};
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };

    // This public data boundary accepts caller-authored labels without claiming grammar support.
    let config = merman_model::MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "fontSize": font_size_css }
    }));
    let presentation = PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "eventmodeling".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
        },
        RenderSemanticModel::EventModeling(EventModelingDiagramModel {
            frames: vec![EventModelingFrame {
                name: "01".to_string(),
                frame_kind: "timeframe".to_string(),
                model_entity_type: "ui".to_string(),
                entity_identifier: title.to_string(),
                source_frames: Vec::new(),
                data_inline_value: None,
                data_reference: None,
            }],
            ..Default::default()
        }),
        PresentationContext::None,
        &merman_model::resources::InputResourcePolicy::default(),
        &merman_model::OperationControl::new(),
    )
    .unwrap()
    .expect("public caller-authored Event Modeling model");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(
        &presentation,
        &LayoutOptions::headless_svg_defaults(),
        session,
    )?
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn style_property<'a>(style: &'a str, expected_property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case(expected_property))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn eventmodeling_inherited_text_paint(svg: &str) -> (String, String, usize, usize) {
    let document = roxmltree::Document::parse(svg).expect("valid Event Modeling SVG");
    let id = document
        .root_element()
        .attribute("id")
        .expect("Event Modeling SVG id");
    let stylesheet = eventmodeling_stylesheet(svg);
    let root_rule = stylesheet
        .split('}')
        .filter_map(|rule| rule.split_once('{'))
        .find_map(|(selector, body)| (selector.trim() == format!("#{id}")).then_some(body))
        .expect("Event Modeling root text-paint rule");
    let fill = style_property(root_rule, "fill")
        .expect("root SVG text fill")
        .to_string();
    let color = style_property(root_rule, "color")
        .expect("root XHTML text color")
        .to_string();
    let mut swimlanes = 0;
    let mut boxes = 0;
    for node in document.descendants().filter(|node| node.is_element()) {
        let swimlane = node.has_tag_name("text")
            && node
                .parent()
                .is_some_and(|parent| parent.attribute("class") == Some("em-swimlane"));
        let box_text = node.tag_name().name() == "span"
            && node
                .ancestors()
                .any(|ancestor| ancestor.attribute("class") == Some("em-box"));
        if !swimlane && !box_text {
            continue;
        }
        for ancestor in node
            .ancestors()
            .take_while(|ancestor| *ancestor != document.root_element())
        {
            assert!(
                ancestor.attribute("fill").is_none(),
                "local fill blocks text inheritance: {svg}"
            );
            assert!(
                ancestor.attribute("color").is_none(),
                "local color blocks text inheritance: {svg}"
            );
            if let Some(style) = ancestor.attribute("style") {
                assert!(
                    style_property(style, "fill").is_none(),
                    "inline fill blocks themeCSS: {svg}"
                );
                assert!(
                    style_property(style, "color").is_none(),
                    "inline color blocks themeCSS: {svg}"
                );
            }
        }
        swimlanes += usize::from(swimlane);
        boxes += usize::from(box_text);
    }
    // This proves the emitted CSS and inheritance structure. The browser probe separately
    // checks computed values and author-CSS precedence, without approximating the cascade here.
    (fill, color, swimlanes, boxes)
}

#[test]
fn eventmodeling_native_palette_reaches_frames_lanes_and_relations() {
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let family = theme.family(DiagramFamilyId::EVENT_MODELING.as_str());
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
        .unwrap()
        .with_canvas_background(Color::rgb(19, 37, 61));
    let rendered = render_eventmodeling_with_session(
        CORE_PAINT_SOURCE,
        MermaidEngine::new(),
        session,
        "eventmodeling-native",
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let canvas = document.root_element().first_element_child().unwrap();
    assert_eq!(canvas.attribute("class"), Some("merman-theme-canvas"));
    assert_eq!(
        canvas.first_element_child().unwrap().attribute("fill"),
        Some("#13253D")
    );

    let boxes: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("em-box"))
        .collect();
    assert_eq!(boxes.len(), 5);
    let border = family.roles.stroke_border.first_color().to_string();
    for (node, slot) in boxes[..4].iter().zip([3, 0, 1, 2]) {
        let rect = node.first_element_child().unwrap();
        let fill = family.data.fill(slot).to_string();
        let ink = family.data.fill_ink(slot).to_string();
        assert_eq!(rect.attribute("fill"), Some(fill.as_str()));
        assert_eq!(rect.attribute("stroke"), Some(border.as_str()));
        let label = node
            .descendants()
            .find(|node| node.has_tag_name("span"))
            .unwrap();
        assert_eq!(
            style_property(label.attribute("style").unwrap(), "color"),
            Some(ink.as_str())
        );
    }
    let processor = boxes[4].first_element_child().unwrap();
    let node_fill = family.roles.surface_node.first_color().to_string();
    let processor_stroke = family.roles.status_info.to_string();
    assert_eq!(processor.attribute("fill"), Some(node_fill.as_str()));
    assert_eq!(
        processor.attribute("stroke"),
        Some(processor_stroke.as_str())
    );

    let lane = document
        .descendants()
        .find(|node| node.attribute("class") == Some("em-swimlane"))
        .unwrap()
        .first_element_child()
        .unwrap();
    let lane_fill = family.roles.surface_container.first_color().to_string();
    assert_eq!(lane.attribute("fill"), Some(lane_fill.as_str()));
    assert_eq!(lane.attribute("stroke"), Some(border.as_str()));
    let relation = document
        .descendants()
        .find(|node| node.attribute("class") == Some("em-relation"))
        .unwrap();
    let connector = family.roles.stroke_connector.first_color().to_string();
    assert_eq!(relation.attribute("stroke"), Some(connector.as_str()));
    let arrowhead = document
        .descendants()
        .find(|node| node.has_tag_name("polygon"))
        .unwrap();
    assert_eq!(arrowhead.attribute("fill"), Some(connector.as_str()));
}

#[test]
fn eventmodeling_svg_preserves_raw_metadata_tokens() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            "accTitle: Accessible ",
            " accDescr {\nFirst\n}",
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            "accTitle: \u{0085}",
            " accDescr {\n\u{0085}\n}",
        ),
        ("", "", "accTitle: ", " accDescr {\n}"),
    ] {
        let source = format!(
            "eventmodeling accDescr {{\n{description}\n}} accTitle: {title}\ntf 01 event Start\n"
        );
        let rendered =
            render_eventmodeling(&source, MermaidEngine::new(), "eventmodeling-metadata")
                .expect("selected same-line metadata syntax");
        common::assert_svg_accessibility(
            rendered.svg(),
            Some(expected_title),
            Some(expected_description),
        );
    }
}

#[test]
fn eventmodeling_config_colors_override_only_the_matching_role_and_property() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({ "themeVariables": {
                "emCommandFill": "#de3163", "emEventStroke": "#fedcba",
                "emSwimlaneBackgroundOdd": "#112233", "emRelationStroke": "#bada55"
            }}),
        ),
    );
    let rendered = render_eventmodeling(CORE_PAINT_SOURCE, engine, "eventmodeling-config").unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let boxes = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("em-box"))
        .map(|node| {
            node.children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(boxes[1].attribute("fill"), Some("#de3163"));
    assert_eq!(boxes[1].attribute("stroke"), Some("#679ac3"));
    assert_eq!(boxes[2].attribute("fill"), Some("#ffb778"));
    assert_eq!(boxes[2].attribute("stroke"), Some("#fedcba"));
    for lane in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("em-swimlane"))
    {
        let rect = lane
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(rect.attribute("fill"), Some("#112233"));
        assert_eq!(rect.attribute("stroke"), Some("rgb(240,240,240)"));
    }
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("em-relation"))
            .all(|node| node.attribute("stroke") == Some("#bada55"))
    );
}

#[test]
fn eventmodeling_classic_text_color_is_inherited_by_svg_and_xhtml() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "textColor": "#fedcba" }
        })),
    );
    let rendered = render_eventmodeling(THEME_TEXT_SOURCE, engine, "eventmodeling-text-color")
        .expect("render site-owned Event Modeling text fill");
    let (fill, color, swimlanes, boxes) = eventmodeling_inherited_text_paint(rendered.svg());

    assert_eq!(fill, "#fedcba");
    assert_eq!(color, "#fedcba");
    assert_eq!((swimlanes, boxes), (3, 3));
}

#[test]
fn eventmodeling_root_text_paint_allows_author_css_on_both_roles() {
    let rendered = render_eventmodeling_with_raw_theme_css(
        &RenderEnvironment::deterministic().with_trusted_theme_css(true),
    )
    .expect("render Event Modeling author CSS in a trusted host");
    let (fill, color, swimlanes, boxes) = eventmodeling_inherited_text_paint(rendered.svg());
    assert_eq!(fill, color);
    assert_eq!((swimlanes, boxes), (3, 3));
    let stylesheet = eventmodeling_stylesheet(rendered.svg());
    let compact = stylesheet
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        compact.contains("#eventmodeling-theme-css.em-swimlane{fill:#2468ac;}")
            || compact.contains("#eventmodeling-theme-css.em-swimlane{fill:#2468ac}"),
        "{stylesheet}"
    );
    assert!(
        compact.contains("#eventmodeling-theme-css.em-boxspan{color:#de3163;}")
            || compact.contains("#eventmodeling-theme-css.em-boxspan{color:#de3163}"),
        "{stylesheet}"
    );
    assert!(
        !compact.contains(".em-swimlanetext{fill:"),
        "local defaults must not block group inheritance"
    );
}

#[test]
fn eventmodeling_author_css_requires_host_trust() {
    let result = render_eventmodeling_with_raw_theme_css(&RenderEnvironment::deterministic());
    assert!(matches!(
        result,
        Err(merman_render::Error::UntrustedThemeCss)
    ));
}

#[test]
#[ignore = "requires Node, tools/mermaid-cli dependencies, and Puppeteer's Chrome headless shell"]
fn eventmodeling_browser_text_cascade() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let baseline = render_eventmodeling(
        THEME_TEXT_SOURCE,
        MermaidEngine::new(),
        "eventmodeling-classic",
    )
    .expect("render browser Event Modeling baseline");
    let author_css = render_eventmodeling_with_raw_theme_css(
        &RenderEnvironment::deterministic().with_trusted_theme_css(true),
    )
    .unwrap();
    let fixtures = serde_json::json!([
        { "name": "baseline", "svg": baseline.svg(), "fill": "rgb(51, 51, 51)", "color": "rgb(51, 51, 51)" },
        { "name": "author-css", "svg": author_css.svg(), "fill": "rgb(36, 104, 172)", "color": "rgb(222, 49, 99)" },
    ]);
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/debug/eventmodeling_text_cascade.mjs");
    let mut child = Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Event Modeling browser regression");
    child
        .stdin
        .take()
        .expect("browser fixture stdin")
        .write_all(serde_json::to_string(&fixtures).unwrap().as_bytes())
        .expect("send rendered SVG fixtures to browser");
    let output = child
        .wait_with_output()
        .expect("wait for Event Modeling browser regression");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}

fn try_render_eventmodeling_svg_with_resource_policy(
    source: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Event Modeling resource-bound fixture")
        .expect("detect Event Modeling diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    Ok(artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-bounded".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned())
}

#[test]
fn eventmodeling_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "eventmodeling\ntf 01 event Start\ntf 02 cmd Continue ->> 01 { id: 42 }\n";
    let baseline = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render unbounded Event Modeling SVG");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1);

    let exact = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Event Modeling SVG ceiling"),
    )
    .expect("exact Event Modeling SVG ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = try_render_eventmodeling_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Event Modeling SVG ceiling"),
    )
    .expect_err("one byte below the Event Modeling SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Event Modeling MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
}

#[test]
fn eventmodeling_prepared_source_data_retains_spans_without_reactivating_attributes() {
    let source = "eventmodeling\ntf 01 ui View { <br/><span>Data</span><b style=\"font-size:20px\"><span>Nested</span></b> }\n";
    for security in ["strict", "loose"] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": security,
                "themeVariables": { "fontSize": "0px" }
            })),
        );
        let rendered = render_eventmodeling(source, engine, "eventmodeling-source-data").unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let code = document
            .descendants()
            .find(|node| node.has_tag_name("code"))
            .unwrap();
        for text in ["Data", "Nested"] {
            assert!(
                code.descendants()
                    .any(|node| node.has_tag_name("span") && node.text() == Some(text)),
                "{security}: {text}"
            );
        }
        assert!(!code.descendants().any(|node| {
            node.attribute("style")
                .is_some_and(|style| style.contains("font-size"))
        }));
    }
}

#[test]
fn eventmodeling_authored_inline_descendants_retain_local_styles() {
    for (html, tag, css) in [
        (
            r#"<b style="font-size:20px"><span>View</span></b><br/>"#,
            "b",
            "font-size:20px",
        ),
        (
            r#"<span style="font-size:20px">View</span><br/>"#,
            "span",
            "font-size:20px",
        ),
        (
            r#"<code style="font-size:20px">View</code><br/>"#,
            "code",
            "font-size:20px",
        ),
        (
            r#"<b style="font-size:var(--host-size)">View</b><br/>"#,
            "b",
            "font-size:var(--host-size)",
        ),
    ] {
        let rendered = render_authored_eventmodeling_title(html, "0px").unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name(tag) && node.attribute("style") == Some(css)),
            "prepared inline style: {html}"
        );
        assert!(
            document
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("View")),
            "prepared DOM content: {html}"
        );
    }
}

#[test]
fn eventmodeling_theme_font_family_keeps_precedence_over_root_font_family() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily": "EventRootFont",
            "themeVariables": { "fontFamily": "EventThemeFont" }
        })),
    );
    let rendered = render_eventmodeling(THEME_TEXT_SOURCE, engine, "eventmodeling-font-family")
        .expect("render config-owned Event Modeling font stack");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-family: EventThemeFont;"));
    assert!(!stylesheet.contains("EventRootFont"));
}

#[test]
fn eventmodeling_styles_are_scoped_per_inline_svg() {
    let source = "eventmodeling\ntf 01 ui View\n";
    let render = |diagram_id, font_size| {
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({"themeVariables": {"fontSize": font_size}}),
        ));
        render_eventmodeling(source, engine, diagram_id).expect("render scoped Event Modeling SVG")
    };
    let rendered_a = render("event-a", "12px");
    let rendered_b = render("event-b", "24px");

    let stylesheet_a = eventmodeling_stylesheet(rendered_a.svg());
    let stylesheet_b = eventmodeling_stylesheet(rendered_b.svg());
    assert!(
        stylesheet_a.contains("#event-a .em-swimlane text,#event-a .em-box span { font-family:")
    );
    assert!(stylesheet_a.contains("font-size: 12px;"));
    assert!(stylesheet_a.contains("#event-a .em-relation { fill: none; }"));
    assert!(!stylesheet_a.contains("#event-b "));
    assert!(
        stylesheet_b.contains("#event-b .em-swimlane text,#event-b .em-box span { font-family:")
    );
    assert!(stylesheet_b.contains("font-size: 24px;"));
    assert!(stylesheet_b.contains("#event-b .em-relation { fill: none; }"));
    assert!(!stylesheet_b.contains("#event-a "));
}

#[test]
fn eventmodeling_css_font_size_uses_theme_variable_not_root_layout_size() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontSize": "31px",
            "themeVariables": { "fontSize": "22px" }
        })),
    );
    let rendered = render_eventmodeling(THEME_TEXT_SOURCE, engine, "eventmodeling-font-size")
        .expect("render Event Modeling split font-size configuration");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-size: 22px;"), "{stylesheet}");
    assert!(!stylesheet.contains("font-size: 31px;"), "{stylesheet}");
}

#[test]
fn eventmodeling_materialized_theme_font_size_reaches_css_without_explicit_ownership() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "theme": "redux"
        })),
    );
    let rendered = render_eventmodeling(THEME_TEXT_SOURCE, engine, "eventmodeling-font-size")
        .expect("render Redux Event Modeling font size");
    let stylesheet = eventmodeling_stylesheet(rendered.svg());

    assert!(stylesheet.contains("font-size: 14px;"), "{stylesheet}");
}

#[test]
fn eventmodeling_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  eventmodeling:
    padding: 24
    useMaxWidth: true
  themeVariables:
    emRelationStroke: '#135790'
    emCommandFill: '#DDEEFF'
    emCommandStroke: '#336699'
    textColor: '#111111'
---
eventmodeling
tf 01 ui Web.ShopCart
tf 02 cmd Cart.AddItem ->> 01 { sku: "SKU-1" }
tf 03 evt Cart.ItemAdded ->> 02 [[ItemAddedData]]
rf 04 rmo Cart.Summary
tf 05 evt Cart.CheckedOut

data ItemAddedData {
  sku: "SKU-1"
  quantity: 1
}
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "eventmodeling");

    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let svg = rendered.svg();

    assert!(svg.contains(r#"aria-roledescription="eventmodeling""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"max-width:"#));
    assert!(svg.contains(r#"<g/><g class="em-swimlane">"#));
    assert!(svg.contains(r#"class="em-relation""#));
    assert!(svg.contains(r#"class="em-box""#));
    assert!(svg.contains(r#"id="em-arrowhead-eventmodeling-test""#));
    assert!(svg.contains(r#"font-family: "trebuchet ms",verdana,arial,sans-serif;"#));
    assert!(svg.contains(r#"color: #111111;"#));
    assert!(svg.contains(r##"stroke="#135790""##));
    assert!(svg.contains(r##"fill="#DDEEFF""##));
}

#[test]
fn eventmodeling_svg_wires_accessibility_metadata_to_the_root() {
    let input = r#"eventmodeling accDescr {
  Event model description
} accTitle: Accessible event model
tf 01 event Start
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse eventmodeling")
        .expect("detect eventmodeling")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-a11y".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render eventmodeling")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-labelledby="chart-title-eventmodeling-a11y""#));
    assert!(svg.contains(r#"aria-describedby="chart-desc-eventmodeling-a11y""#));
    assert!(svg.contains(
        r#"<title id="chart-title-eventmodeling-a11y">accTitle: Accessible event model</title>"#
    ));
    assert!(svg.contains(
        "<desc id=\"chart-desc-eventmodeling-a11y\"> accDescr {\nEvent model description\n}</desc>"
    ));
}

#[test]
fn eventmodeling_docs_minimum_layout_uses_bounded_deterministic_label_metrics() {
    let input =
        include_str!("../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact
        .layout_json()
        .expect("serialize EventModeling layout");
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone())
            .expect("EventModeling layout projection");

    assert_eq!(layout.boxes.len(), 5);
    assert!(!layout.swimlanes.is_empty());
    assert!(!layout.relations.is_empty());
    assert!(
        layout
            .boxes
            .iter()
            .all(|box_layout| box_layout.width.is_finite()
                && box_layout.height.is_finite()
                && (100.0..=470.0).contains(&box_layout.width)
                && (100.0..=770.0).contains(&box_layout.height)),
        "EventModeling boxes must stay inside the source-defined min/max geometry: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].width > layout.boxes[0].width,
        "the data-rich event must remain wider than the short UI frame: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].height >= layout.boxes[0].height,
        "the data-rich event must not become shorter than the short UI frame: {:?}",
        layout.boxes
    );
    let rightmost_box = layout
        .boxes
        .iter()
        .map(|box_layout| box_layout.x + box_layout.width)
        .fold(0.0_f64, f64::max);
    assert!(layout.total_width > rightmost_box);
    assert!(layout.total_height > 0.0);
}

#[test]
fn reset_frames_keep_explicit_sources_without_inventing_previous_frame_relations() {
    let source = "eventmodeling\ntf 01 ui UI\ntf 02 cmd Command\ntf 03 evt Event\nrf 04 rmo ReadModel ->> 03\nrf 05 ui OtherUI\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone()).unwrap();
    let pairs: Vec<_> = layout
        .relations
        .iter()
        .map(|relation| {
            (
                relation.source_frame.as_str(),
                relation.target_frame.as_str(),
            )
        })
        .collect();
    assert_eq!(pairs, [("01", "02"), ("02", "03"), ("03", "04")]);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("em-relation"))
            .count(),
        3
    );
}

fn assert_eventmodeling_relation_projection(
    source: &str,
    expected_sources: &[&[&str]],
    expected_pairs: &[(&str, &str)],
) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let merman_model::presentation::RenderSemanticModel::EventModeling(model) = parsed.model()
    else {
        panic!("expected Event Modeling model");
    };
    assert_eq!(model.frames.len(), expected_sources.len());
    for (frame, expected) in model.frames.iter().zip(expected_sources) {
        assert_eq!(frame.source_frames, *expected, "frame {}", frame.name);
    }
    let authored_model = serde_json::to_value(model).unwrap();
    let semantic = merman_core::semantic::SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap();
    let merman_model::semantic::SemanticDiagramModel::EventModeling(semantic_model) =
        semantic.model
    else {
        panic!("expected canonical Event Modeling model");
    };
    assert_eq!(
        authored_model,
        serde_json::to_value(semantic_model).unwrap()
    );
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone()).unwrap();
    assert_eq!(layout.boxes.len(), expected_sources.len());
    let pairs: Vec<_> = layout
        .relations
        .iter()
        .map(|relation| {
            (
                relation.source_frame.as_str(),
                relation.target_frame.as_str(),
            )
        })
        .collect();
    assert_eq!(pairs, expected_pairs, "{source}");

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("class") == Some("em-relation"))
        .collect();
    assert_eq!(paths.len(), expected_pairs.len(), "{source}");
    for (path, relation) in paths.iter().zip(&layout.relations) {
        let segments: Vec<_> = svgtypes::PathParser::from(path.attribute("d").unwrap())
            .collect::<Result<_, _>>()
            .unwrap();
        let [
            svgtypes::PathSegment::MoveTo {
                abs: true,
                x: x1,
                y: y1,
            },
            svgtypes::PathSegment::LineTo {
                abs: true,
                x: x2,
                y: y2,
            },
        ] = segments.as_slice()
        else {
            panic!("expected a straight Event Modeling relation: {segments:?}");
        };
        for (actual, expected) in [*x1, *y1, *x2, *y2].into_iter().zip([
            relation.x1,
            relation.y1,
            relation.x2,
            relation.y2,
        ]) {
            assert!((actual - expected).abs() < 1e-6, "{source}: {path:?}");
        }
    }
    assert_eq!(serde_json::to_value(model).unwrap(), authored_model);
}

#[test]
fn eventmodeling_repeated_explicit_sources_emit_one_relation_per_frame() {
    for sources in [&["01"][..], &["01", "01"][..]] {
        let references = sources
            .iter()
            .map(|name| format!(" ->> {name}"))
            .collect::<String>();
        let source = format!("eventmodeling\ntf 01 cmd Start\ntf 02 evt Done{references}\n");
        assert_eventmodeling_relation_projection(&source, &[&[], sources], &[("01", "02")]);
    }
}

#[test]
fn eventmodeling_explicit_relations_follow_frame_declaration_order() {
    for (first, second) in [("01", "02"), ("20", "10")] {
        for sources in [&[second, first][..], &[second, first, second][..]] {
            let references = sources
                .iter()
                .map(|name| format!(" ->> {name}"))
                .collect::<String>();
            let source = format!(
                "eventmodeling\ntf {first} evt First\ntf {second} evt Second\ntf 03 cmd Run{references}\n"
            );
            assert_eventmodeling_relation_projection(
                &source,
                &[&[], &[], sources],
                &[(first, "03"), (second, "03")],
            );
        }
    }
}

#[test]
fn eventmodeling_explicit_sources_skip_missing_and_future_frames_without_fallback() {
    let source = concat!(
        "eventmodeling\n",
        "tf 01 evt First ->> 02 ->> 99 ->> 02\n",
        "tf 02 cmd Run ->> 99 ->> 99\n",
        "rf 03 evt Done ->> 02 ->> 99 ->> 02\n",
        "rf 04 ui OtherUI\n",
    );
    assert_eventmodeling_relation_projection(
        source,
        &[&["02", "99", "02"], &["99", "99"], &["02", "99", "02"], &[]],
        &[("02", "03")],
    );
}

#[test]
fn eventmodeling_repeated_self_source_uses_the_already_positioned_current_frame() {
    assert_eventmodeling_relation_projection(
        "eventmodeling\ntf 01 cmd Start ->> 01 ->> 01\n",
        &[&["01", "01"]],
        &[("01", "01")],
    );
}

#[derive(Default)]
struct EventModelingLabelProbe {
    measured_lines: std::sync::Mutex<Vec<String>>,
}

impl merman_render::text::TextMeasurer for EventModelingLabelProbe {
    fn measure(
        &self,
        text: &str,
        style: &merman_render::text::TextStyle,
    ) -> merman_render::text::TextMetrics {
        merman_render::text::DeterministicTextMeasurer::default().measure(text, style)
    }

    fn measure_svg_simple_text_bbox_width_px(
        &self,
        text: &str,
        style: &merman_render::text::TextStyle,
    ) -> f64 {
        self.measured_lines.lock().unwrap().push(text.to_owned());
        merman_render::text::DeterministicTextMeasurer::default()
            .measure_svg_simple_text_bbox_width_px(text, style)
    }
}

fn render_prepared_eventmodeling_labels(
    source: &str,
    config: serde_json::Value,
) -> (serde_json::Value, String) {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use std::sync::Arc;

    let measurer = Arc::new(EventModelingLabelProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("eventmodeling-prepared-xhtml").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer.clone())),
    );
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .expect("prepare Event Modeling XHTML");
    let projection = artifact.layout_json().unwrap();
    let layout = projection["layout"]["EventModelingDiagram"].clone();
    let measured = measurer.measured_lines.lock().unwrap().clone();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("consume prepared Event Modeling XHTML");
    assert_eq!(
        *measurer.measured_lines.lock().unwrap(),
        measured,
        "emission must not wrap or measure the labels again",
    );
    roxmltree::Document::parse(rendered.svg()).expect("valid XHTML in SVG");
    let _: EventModelingDiagramLayout =
        serde_json::from_value(layout.clone()).expect("existing public layout schema");
    let mut keys = layout
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "bounds",
            "boxes",
            "padding",
            "relations",
            "swimlanes",
            "total_height",
            "total_width",
            "use_max_width",
            "viewbox_x",
            "viewbox_y",
        ]
    );
    for box_layout in layout["boxes"].as_array().unwrap() {
        let mut keys = box_layout
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "entity_identifier",
                "fill",
                "frame_kind",
                "frame_name",
                "height",
                "index",
                "model_entity_type",
                "stroke",
                "swimlane_index",
                "text",
                "width",
                "x",
                "y",
            ]
        );
    }
    (layout, rendered.svg().to_owned())
}

#[test]
fn eventmodeling_prepared_identifier_wrap_is_measured_and_emitted() {
    let identifier = "W".repeat(90);
    let source = format!("eventmodeling\ntf 01 ui {identifier}\n");
    let (layout, svg) = render_prepared_eventmodeling_labels(&source, serde_json::json!({}));
    assert_eq!(layout["boxes"][0]["text"], identifier);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("b"))
        .unwrap();
    assert!(title.children().any(|node| node.has_tag_name("br")));
    let text = title
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(
        text.contains('-'),
        "wrapLabel must retain its generated hyphens"
    );
    assert_eq!(text.replace('-', ""), identifier);
}

#[test]
fn eventmodeling_prepared_data_preserves_wrap_nbsp_entities_and_reference_break() {
    let words = "alpha beta gamma delta ".repeat(8);
    let source = format!(
        "eventmodeling\ntf 01 ui Inline {{ {words}&lt;img&gt; &amp; &quot;q&quot; }}\ntf 02 evt Referenced [[D]]\ndata D {{\n  key: &lt;img&gt; &amp; &quot;q&quot; &bogus;\n}}\n"
    );
    let (layout, svg) = render_prepared_eventmodeling_labels(&source, serde_json::json!({}));
    assert!(
        layout["boxes"][0]["text"]
            .as_str()
            .unwrap()
            .contains(&words)
    );
    assert!(
        layout["boxes"][1]["text"]
            .as_str()
            .unwrap()
            .starts_with("Referenced\n")
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let codes = document
        .descendants()
        .filter(|node| node.has_tag_name("code"))
        .collect::<Vec<_>>();
    assert_eq!(codes.len(), 2);
    assert!(
        codes[0].children().any(|node| node.has_tag_name("br")),
        "inline data wraps"
    );
    assert!(
        !codes[0].last_child().unwrap().has_tag_name("br"),
        "inline data has no reference break"
    );
    assert!(
        codes[1].last_child().unwrap().has_tag_name("br"),
        "even short references retain the final break"
    );
    for code in &codes {
        let text = code
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert!(text.contains('\u{a0}'), "wrapped data spaces become NBSP");
        assert!(text.contains("<img>"), "encoded tags remain text");
        assert!(
            text.contains('&') && text.contains("\"q\""),
            "HTML entities decode once"
        );
        assert!(!code.descendants().any(|node| node.has_tag_name("img")));
    }
    let reference = codes[1]
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(
        reference.contains("&bogus;"),
        "unknown entities stay literal XML text"
    );
}

#[test]
fn eventmodeling_prepared_authored_data_is_sanitized_before_xhtml_emission() {
    let source = "eventmodeling\ntf 01 ui Safe { <img src=x onerror=alert(1)> &lt;script&gt;literal&lt;/script&gt; &amp; }\n";
    for config in [
        serde_json::json!({}),
        serde_json::json!({"securityLevel": "strict"}),
        serde_json::json!({"securityLevel": "loose"}),
        serde_json::json!({"securityLevel": "custom"}),
    ] {
        let (_, svg) = render_prepared_eventmodeling_labels(source, config);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("script"))
        );
        assert!(document.descendants().all(|node| {
            node.attributes()
                .all(|attr| !attr.name().to_ascii_lowercase().starts_with("on"))
        }));
        let text = document
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert!(
            text.contains("script"),
            "entity-authored script remains literal text"
        );
    }
}

#[test]
fn eventmodeling_prepared_label_preserves_host_cancellation() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::Arc;

    struct CancellingMeasurer(merman_model::OperationControl);
    impl TextMeasurer for CancellingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.cancel();
            merman_render::text::DeterministicTextMeasurer::default().measure(text, style)
        }
    }
    let control = merman_model::OperationControl::new();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("eventmodeling-label-cancellation").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(CancellingMeasurer(control.clone())),
        )),
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            &format!("eventmodeling\ntf 01 ui {}\n", "W".repeat(90)),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let result = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session_with_control(control).unwrap(),
    );
    let Err(merman_render::Error::Cancelled(error)) = result else {
        panic!("Event Modeling label preparation must preserve cancellation");
    };
    assert_eq!(error.phase, merman_model::OperationPhase::Layout);
}

mod stage_contract {
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_render::environment::{
        MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_render::{LayoutOptions, family};
    use std::sync::{Arc, Mutex};

    #[test]
    fn transient_author_whitespace_does_not_consume_retained_terminal_budget() {
        let source = format!(
            "eventmodeling\ntf 01 ui Data {{ A{}B }}\n",
            " ".repeat(4096)
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let render = |environment: RenderEnvironment| {
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::headless_svg_defaults(),
                environment.begin_session().unwrap(),
            )
            .unwrap();
            artifact
                .render_svg(
                    &merman_render::svg::SvgRenderOptions::default(),
                    &merman_render::svg::SvgDebugOptions::default(),
                )
                .unwrap()
                .svg()
                .to_owned()
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1024)
            .unwrap();
        let bounded = render(RenderEnvironment::deterministic().with_resource_policy(policy));
        assert_eq!(bounded, render(RenderEnvironment::deterministic()));
        let document = roxmltree::Document::parse(&bounded).unwrap();
        let code = document
            .descendants()
            .find(|node| node.has_tag_name("code"))
            .unwrap();
        assert_eq!(code.text(), Some("A\u{a0}B"));
    }

    #[derive(Default)]
    struct Probe(Mutex<Vec<String>>);
    impl TextMeasurer for Probe {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            DeterministicTextMeasurer::default().measure(text, style)
        }
        fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.0.lock().unwrap().push(text.to_owned());
            DeterministicTextMeasurer::default().measure_svg_simple_text_bbox_width_px(text, style)
        }
    }

    #[test]
    fn eventmodeling_calculate_text_dimensions_keeps_pre_serialization_nbsp_spelling() {
        let source = "eventmodeling\ntf 01 ui Data { W W W W W W W W }\n";
        let probe = Arc::new(Probe::default());
        let id = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("audit-event-stage").unwrap(),
            "1",
        )
        .unwrap();
        let env = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::uniform(TextMeasurementProfile::new(id, probe.clone())),
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            env.begin_session().unwrap(),
        )
        .unwrap();
        assert_eq!(
            artifact.layout_json().unwrap()["layout"]["EventModelingDiagram"]["boxes"][0]["width"],
            333.0
        );
        let calls = probe.0.lock().unwrap();
        let code: Vec<_> = calls.iter().filter(|s| s.contains("<code")).collect();
        assert!(!code.is_empty());
        assert!(
            code.iter().all(|s| s.contains("&nbsp;")),
            "pinned db.ts measures generated content before XHTML serialization; drawSimpleText uses tspan.text, so entity spellings are literal in this probe"
        );
    }

    #[test]
    fn eventmodeling_small_prepared_budget_is_an_error() {
        let source = "eventmodeling\ntf 01 ui Data { W W W W W W W W }\n";
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 8)
            .unwrap();
        let env = RenderEnvironment::deterministic().with_resource_policy(policy);
        let result = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            env.begin_session().unwrap(),
        );
        assert!(
            matches!(result, Err(merman_render::Error::ResourceLimitExceeded(_))),
            "prepared-label exhaustion must not become a successful empty label"
        );
    }
}
