mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
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

fn try_render_radar_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Radar resource-bound fixture")
        .expect("detect Radar resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Radar resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn radar_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"radar-beta
title Bounded capability
axis Reliability,Performance,Clarity
curve Current{3,4,2}
curve Target{5,5,5}
"#;
    let baseline = try_render_radar_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Radar baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Radar fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Radar SVG byte ceiling");
    let exact = try_render_radar_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Radar family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Radar SVG byte ceiling");
    let error = try_render_radar_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Radar family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Radar MaxSvgBytes rejection, got {error}");
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
