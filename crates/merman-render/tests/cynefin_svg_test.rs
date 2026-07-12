mod common;

use std::sync::Arc;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

fn parse_layout_and_render(input: &str, layout_options: &LayoutOptions) -> (LayoutDiagram, String) {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse cynefin")
        .expect("detect cynefin");
    let layout = layout_parsed_render_layout_only(&parsed, layout_options).expect("layout cynefin");
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("cynefin-test".to_string()),
            ..Default::default()
        },
    )
    .expect("render cynefin");

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
    let layout_options =
        LayoutOptions::default().with_text_measurer(Arc::new(FontAwareTextMeasurer));
    let (layout, svg) = parse_layout_and_render(
        r#"---
config:
  fontFamily: '"Fira Code", monospace'
---
cynefin-beta
complex
"A"
"#,
        &layout_options,
    );

    let LayoutDiagram::CynefinDiagram(layout) = layout else {
        panic!("expected cynefin layout");
    };
    assert_eq!(layout.items[0].width, 120.0);
    assert!(
        svg.contains(r#"#cynefin-test{font-family:"Fira Code",monospace;"#),
        "global font family should be emitted by the common Mermaid CSS: {svg}"
    );
}
