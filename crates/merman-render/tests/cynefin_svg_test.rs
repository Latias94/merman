mod common;

use std::sync::Arc;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementPolicy,
    TextMeasurementProfile, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::CynefinDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};

fn parse_layout_and_render(
    input: &str,
    layout_options: &LayoutOptions,
) -> (CynefinDiagramLayout, String) {
    parse_layout_and_render_with_environment(
        input,
        layout_options,
        &RenderEnvironment::deterministic(),
    )
}

fn parse_layout_and_render_with_environment(
    input: &str,
    layout_options: &LayoutOptions,
    environment: &RenderEnvironment,
) -> (CynefinDiagramLayout, String) {
    let session = environment.begin_session().unwrap();
    parse_layout_and_render_with_session(input, layout_options, session)
}

fn parse_layout_and_render_with_session(
    input: &str,
    layout_options: &LayoutOptions,
    session: RenderSession,
) -> (CynefinDiagramLayout, String) {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse cynefin")
        .expect("detect cynefin")
        .into_presentation();
    let artifact = family::prepare(&parsed, layout_options, session).expect("prepare cynefin");
    let projection = artifact.layout_json().expect("serialize Cynefin layout");
    let layout: CynefinDiagramLayout =
        serde_json::from_value(projection["layout"]["CynefinDiagram"].clone())
            .expect("Cynefin layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("cynefin-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render cynefin")
        .svg()
        .to_owned();

    (layout, svg)
}

fn try_render_cynefin_svg_with_resource_policy(
    input: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse Cynefin resource-bound fixture")
        .expect("detect Cynefin resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("cynefin-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

fn cynefin_stylesheet(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .expect("valid Cynefin SVG")
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Cynefin stylesheet")
        .to_owned()
}

fn cynefin_rule_body<'a>(stylesheet: &'a str, diagram_id: &str, selector: &str) -> &'a str {
    let rule = format!("#{diagram_id} {selector}{{");
    let body = stylesheet
        .split_once(&rule)
        .unwrap_or_else(|| panic!("missing Cynefin rule {rule:?}"))
        .1;
    let end = body.find('}').expect("Cynefin CSS rule closes");
    &body[..end]
}

const CYNEFIN_NATIVE_SOURCE: &str = r#"---
config:
  cynefin:
    showDomainDescriptions: true
---
cynefin-beta
clear
"Runbook"
complex
"Retrospective"
clear --> complex : "Probe"
"#;

#[derive(Debug)]
struct CynefinNativeFontProbe;

impl TextMeasurer for CynefinNativeFontProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        assert_eq!(
            style.font_family.as_deref(),
            Some("\"Domain Sans\", sans-serif")
        );
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.font_weight.as_deref(), Some("550"));
        TextMetrics {
            width: 10.0 * text.len() as f64,
            height: 20.0,
            line_count: 1,
        }
    }
}

#[test]
fn cynefin_native_domains_and_items_keep_the_canvas_visible() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
              "schema_version": 2,
              "extends": "studio",
              "roles": {
                "canvas": "#F8FAFC", "surface.container": "#DEE5EA", "surface.node": "#EEF1F2",
                "stroke.connector": "#36485A", "text.primary": "#223344", "text.title": "#19364D"
              },
              "appearances": {"light": {"data": {"fill": ["#BAD4E9", "#E9CDBA", "#D1E9BA", "#E9BACD", "#DCBAE9"]}}},
              "typography": {"sans": ["Domain Sans", "sans-serif"], "base_size_px": 20, "label_weight": 550}
            }"##,
            None,
        )
        .unwrap();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.cynefin-native-font").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(CynefinNativeFontProbe),
        )),
    );
    let session = environment
        .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
        .unwrap();
    let (layout, svg) = parse_layout_and_render_with_session(
        CYNEFIN_NATIVE_SOURCE,
        &LayoutOptions::default(),
        session,
    );
    assert_eq!(
        layout
            .items
            .iter()
            .find(|item| item.label == "Runbook")
            .unwrap()
            .width,
        90.0
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let canvas = doc.root_element().first_element_child().unwrap();
    assert_eq!(canvas.attribute("class"), Some("merman-theme-canvas"));
    assert_eq!(
        canvas.first_element_child().unwrap().attribute("fill"),
        Some("#F8FAFC")
    );
    let by_class = |class| {
        doc.descendants()
            .filter(move |node| node.attribute("class") == Some(class))
    };
    let domains: Vec<_> = by_class("cynefinDomain").collect();
    assert_eq!(domains.len(), 4);
    assert!(
        domains
            .iter()
            .all(|domain| domain.attribute("fill") == Some("#DEE5EA")
                && domain.attribute("fill-opacity") == Some("0.4"))
    );
    let confusion = by_class("cynefinConfusion").next().unwrap();
    assert_eq!(confusion.attribute("fill"), Some("#DEE5EA"));
    assert_eq!(confusion.attribute("fill-opacity"), Some("0.5"));
    let accents: Vec<_> = by_class("cynefinDomainAccent").collect();
    assert_eq!(
        accents
            .iter()
            .filter_map(|node| node.attribute("fill"))
            .collect::<Vec<_>>(),
        ["#D1E9BA", "#E9CDBA", "#E9BACD", "#BAD4E9", "#DCBAE9"]
    );
    let clip = doc
        .descendants()
        .find(|node| node.has_tag_name("clipPath"))
        .unwrap();
    assert_eq!(
        clip.first_element_child().unwrap().attribute("d"),
        confusion.attribute("d")
    );
    assert_eq!(
        accents[4].attribute("clip-path"),
        Some("url(#cynefin-confusion-clip-cynefin-test)")
    );
    assert!(by_class("cynefinItem").all(|node| node.attribute("fill") == Some("#EEF1F2")));
    let css = cynefin_stylesheet(&svg);
    assert!(
        cynefin_rule_body(&css, "cynefin-test", ".cynefinDomainLabel").contains("fill:#19364D;")
    );
    assert_eq!(
        cynefin_rule_body(&css, "cynefin-test", ".cynefinItemText"),
        "font-size:20px;fill:#223344;"
    );
    assert!(
        cynefin_rule_body(&css, "cynefin-test", ".cynefinArrowLine").contains("stroke:#36485A;")
    );
    assert!(cynefin_rule_body(&css, "cynefin-test", ".cynefinArrowHead").contains("fill:#36485A;"));

    let source = r#"---
config:
  themeVariables:
    fontFamily: 'SourceCynefinFont, monospace'
    cynefin:
      complexBg: '#AECFEF'
      textColor: '#2C3D50'
      arrowColor: '#B04225'
---
cynefin-beta
complex
"Retrospective"
complex --> clear : "Practice"
"#;
    let native = NativeTheme::new(theme).with_background(Color::TRANSPARENT);
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(native)
        .unwrap();
    let (_, svg) = parse_layout_and_render_with_session(source, &LayoutOptions::default(), session);
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let canvas = doc.root_element().first_element_child().unwrap();
    assert_eq!(canvas.attribute("class"), Some("merman-theme-canvas"));
    assert!(canvas.first_element_child().is_none());
    for class in ["cynefinDomain", "cynefinDomainAccent", "cynefinItem"] {
        assert!(
            doc.descendants()
                .any(|node| node.attribute("class") == Some(class)
                    && node.attribute("fill") == Some("#AECFEF"))
        );
    }
    let css = cynefin_stylesheet(&svg);
    assert!(css.contains("#cynefin-test{font-family:SourceCynefinFont,monospace;"));
    assert!(cynefin_rule_body(&css, "cynefin-test", ".cynefinItemText").contains("fill:#2C3D50;"));
    assert!(
        cynefin_rule_body(&css, "cynefin-test", ".cynefinArrowLine").contains("stroke:#B04225;")
    );
    let (_, classic) = parse_layout_and_render(source, &LayoutOptions::default());
    assert!(!classic.contains("cynefinDomainAccent"));
    assert!(!classic.contains("merman-theme-canvas"));
}

#[test]
fn cynefin_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let input = r#"---
title: Bounded Cynefin
config:
  cynefin:
    seed: 1
---
cynefin-beta
accTitle: Bounded Cynefin accessibility title
accDescr: A bounded Cynefin diagram with items and a transition
clear
  "Runbook"
complex
  "Retrospective"
clear --> complex : "Probe"
"#;
    let baseline = try_render_cynefin_svg_with_resource_policy(
        input,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Cynefin baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Cynefin fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Cynefin SVG byte ceiling");
    let exact = try_render_cynefin_svg_with_resource_policy(input, exact_policy)
        .expect("the exact Cynefin family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Cynefin SVG byte ceiling");
    let error = try_render_cynefin_svg_with_resource_policy(input, below_policy)
        .expect_err("one byte below the Cynefin family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Cynefin MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn cynefin_svg_uses_frontmatter_title_unless_body_title_overrides_it() {
    let (_, frontmatter_svg) = parse_layout_and_render(
        r#"---
title: Frontmatter title
---
cynefin-beta
complex
"A"
"#,
        &LayoutOptions::default(),
    );
    assert!(
        frontmatter_svg.contains(r#"class="cynefinTitle""#)
            && frontmatter_svg.contains(">Frontmatter title</text>"),
        "frontmatter title should render when the body has no title: {frontmatter_svg}"
    );

    let (_, body_svg) = parse_layout_and_render(
        r#"---
title: Frontmatter title
---
cynefin-beta
title Body title
complex
"A"
"#,
        &LayoutOptions::default(),
    );
    assert!(body_svg.contains(">Body title</text>"), "{body_svg}");
    assert!(
        !body_svg.contains(">Frontmatter title</text>"),
        "body title should override frontmatter like Mermaid 11.16: {body_svg}"
    );
}

struct FontAwareTextMeasurer;

impl TextMeasurer for FontAwareTextMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        let width = if style.font_family.as_deref() == Some(r#""Fira Code",monospace"#) {
            100.0
        } else {
            10.0
        };
        TextMetrics {
            width,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn cynefin_global_font_family_drives_css_and_item_measurement() {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.cynefin-font-aware").unwrap(),
        "test",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(FontAwareTextMeasurer),
        )),
    );
    let (layout, svg) = parse_layout_and_render_with_environment(
        r#"---
config:
  fontFamily: '"Fira Code", monospace'
---
cynefin-beta
complex
"A"
"#,
        &LayoutOptions::default(),
        &environment,
    );

    assert_eq!(layout.items[0].width, 120.0);
    assert!(
        svg.contains(r#"#cynefin-test{font-family:"Fira Code",monospace;"#),
        "global font family should be emitted by the common Mermaid CSS: {svg}"
    );
}

struct CynefinTerminalSizeMeasurer(f64);

impl TextMeasurer for CynefinTerminalSizeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        assert_eq!(style.font_size, self.0);
        assert_eq!(style.font_weight, None);
        assert_eq!(style.font_style, None);
        TextMetrics {
            width: self.0 * text.len() as f64,
            height: self.0,
            line_count: 1,
        }
    }
}

#[test]
fn cynefin_item_measurement_uses_the_final_nonnegative_css_size() {
    for (size, used_size) in [(0.0, 0.0), (1.0, 1.0), (-1.0, 32.0), (0.0004, 0.0004)] {
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.cynefin-terminal-size").unwrap(),
            "1",
        )
        .unwrap();
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
                identity,
                Arc::new(CynefinTerminalSizeMeasurer(used_size)),
            )),
        );
        let config = serde_json::json!({"themeVariables": {
            "fontSize": "24pt", "cynefin": {"itemFontSize": size}
        }});
        let source = format!("---\nconfig: {config}\n---\ncynefin-beta\ncomplex\n\"Item\"\n");
        let (layout, svg) = parse_layout_and_render_with_environment(
            &source,
            &LayoutOptions::default(),
            &environment,
        );
        let measured_width = if used_size == 0.0 {
            28.0
        } else {
            used_size * 4.0
        };
        assert_eq!(layout.items[0].width, measured_width + 20.0, "{size}");
        assert!(
            cynefin_stylesheet(&svg).contains(&format!(".cynefinItemText{{font-size:{size}px;"))
        );
    }
}

const CYNEFIN_FONT_BINDING_SOURCE: &str =
    "cynefin-beta\ntitle Cynefin\ncomplex\n\"Item\"\nclear --> complex : \"Transition\"\n";

#[test]
fn cynefin_font_binding_retains_json_types_and_ignored_role_declarations() {
    for (value, direct, annotation, title, item_px) in [
        (serde_json::json!(24), "24px", "23px", "26px", 24.0),
        (serde_json::json!("24"), "24px", "23px", "242px", 24.0),
        (
            serde_json::json!("24pt"),
            "24ptpx",
            "NaNpx",
            "24pt2px",
            32.0,
        ),
        (
            serde_json::json!("potato"),
            "potatopx",
            "NaNpx",
            "potato2px",
            32.0,
        ),
    ] {
        let config = serde_json::json!({
            "themeVariables": {"fontSize": "24pt", "cynefin": {
                "domainFontSize": value, "itemFontSize": value
            }}
        });
        let source = format!("---\nconfig: {config}\n---\n{CYNEFIN_FONT_BINDING_SOURCE}");
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.cynefin-font-binding").unwrap(),
            "1",
        )
        .unwrap();
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
                identity,
                Arc::new(CynefinTerminalSizeMeasurer(item_px)),
            )),
        );
        let (layout, svg) = parse_layout_and_render_with_environment(
            &source,
            &LayoutOptions::default(),
            &environment,
        );
        assert_eq!(layout.items[0].width, item_px * 4.0 + 20.0);
        let stylesheet = cynefin_stylesheet(&svg);
        for (selector, expected) in [
            (".cynefinDomainLabel", direct),
            (".cynefinItemText", direct),
            (".cynefinSubtitle", annotation),
            (".cynefinArrowLabel", annotation),
            (".cynefinTitle", title),
        ] {
            assert!(
                cynefin_rule_body(&stylesheet, "cynefin-test", selector)
                    .contains(&format!("font-size:{expected};")),
                "{value}/{selector}"
            );
        }
    }
}
