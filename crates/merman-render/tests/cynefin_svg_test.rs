mod common;

use std::sync::Arc;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::CynefinDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

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
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse cynefin")
        .expect("detect cynefin");
    let artifact = family::prepare(parsed, layout_options, session).expect("prepare cynefin");
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

#[derive(Debug)]
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
