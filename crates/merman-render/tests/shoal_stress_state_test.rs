#![cfg(all(feature = "diagram-state", feature = "layout-shoal"))]

use kurbo::{BezPath, ParamCurve, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{Appearance, BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::Document;

fn render_shoal_state(source: &str, appearance: Appearance, canvas: Option<Color>) -> String {
    let source =
        format!("---\nconfig:\n  layout: shoal\n  look: neo\n  htmlLabels: false\n---\n{source}");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(appearance))
        .unwrap();
    let theme = NativeTheme::new(theme);
    let theme = match canvas {
        Some(canvas) => theme.with_background(canvas),
        None => theme,
    };
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(theme)
        .unwrap();
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap()
    .svg()
    .to_owned()
}

#[test]
fn shoal_state_renders_late_nested_choice_as_a_diamond() {
    let source = "\
stateDiagram-v2
direction BT
[*] --> Root
state Root {
  [*] --> Triage
  state Triage <<choice>>
  Triage --> FastPath : low
  Triage --> MedPath : medium
  Triage --> SlowPath : high
  Triage --> BlockPath : blocked
}
Root --> [*]
";
    let svg = render_shoal_state(source, Appearance::Dark, Some(Color::TRANSPARENT));
    let document = Document::parse(&svg).unwrap();
    let triage = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some("Triage"))
        .expect("Triage state");
    let fill = triage
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("none"))
        .expect("Triage diamond fill");
    let path = BezPath::from_svg(fill.attribute("d").unwrap()).unwrap();
    let bounds = path.bounding_box();
    let diagonal_sides = path
        .segments()
        .filter(|segment| {
            let delta = segment.end() - segment.start();
            delta.x.abs() > bounds.width() / 4.0 && delta.y.abs() > bounds.height() / 4.0
        })
        .count();
    assert_eq!(diagonal_sides, 4);
}

#[test]
fn shoal_state_neo_region_dividers_use_the_label_backplate_color() {
    let source = "\
stateDiagram-v2
direction LR
[*] --> Operational
state Operational {
  [*] --> Primary
  Primary --> Failover : quorum lost
  --
  [*] --> MetricsIdle
  MetricsIdle --> Scraping : interval tick
  --
  [*] --> CompactionIdle
  CompactionIdle --> Compacting : threshold hit
}
Operational --> [*]
";
    let svg = render_shoal_state(source, Appearance::Light, None);
    let document = Document::parse(&svg).unwrap();
    let regions: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("divider"))
        .collect();
    assert_eq!(regions.len(), 3);
    for region in regions {
        assert_eq!(region.attribute("data-look"), Some("neo"));
        assert!(region.descendants().any(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("divider")
        }));
    }

    let id = document.root_element().attribute("id").unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    let region_fill = "#EFF4FA";
    assert!(css.contains(&format!(
        r#"#{id} [data-et="divider"][data-look="neo"] rect.divider{{fill:{region_fill};}}"#
    )));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("class") == Some("merman-edge-label-background")
            && node.attribute("fill") == Some(region_fill)
    }));
}
