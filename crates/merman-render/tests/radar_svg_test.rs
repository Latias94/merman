mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_radar_svg_from_text(text: &str) -> String {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse radar")
        .expect("detect radar");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("layout radar");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render radar")
        .svg()
        .to_owned()
}

#[test]
fn radar_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="radarTitle""#)
            && frontmatter_svg.contains(">Frontmatter radar</text>"),
        "frontmatter title should render when the Radar body has none: {frontmatter_svg}"
    );

    let body_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
title Body radar
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(body_svg.contains(">Body radar</text>"));
    assert!(!body_svg.contains(">Frontmatter radar</text>"));
}
