use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};
use regex::Regex;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn render_architecture_fixture_with_options(
    fixture_name: &str,
    options: &SvgRenderOptions,
) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("architecture")
        .join(fixture_name);
    let text = std::fs::read_to_string(&path).expect("read fixture");

    render_architecture_text_with_options(&text, options)
}

fn render_architecture_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let layout = layout_parsed_render_layout_only(&parsed, &layout_options).expect("layout ok");

    render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        options,
    )
    .expect("render SVG")
}

fn render_architecture_fixture(fixture_name: &str) -> String {
    render_architecture_fixture_with_options(
        fixture_name,
        &SvgRenderOptions {
            diagram_id: Some("architecture-crosslinks".to_string()),
            ..Default::default()
        },
    )
}

fn deep_group_chain_diagram(depth: usize) -> String {
    let mut lines = vec![
        r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%"#.to_string(),
        "architecture-beta".to_string(),
    ];
    for i in 0..depth {
        let parent = (i > 0)
            .then(|| format!(" in g{}", i - 1))
            .unwrap_or_default();
        lines.push(format!("  group g{i}(cloud)[G{i}]{parent}"));
    }
    lines.push(format!("  service leaf(server)[Leaf] in g{}", depth - 1));
    lines.join("\n")
}

fn arrow_transform_after_edge(svg: &str, edge_id: &str) -> String {
    let pattern = format!(r#"id="{}"[^>]*/><polygon([^>]*)>"#, regex::escape(edge_id));
    let re = Regex::new(&pattern).expect("valid regex");
    let attrs = re
        .captures(svg)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or_else(|| panic!("missing arrow polygon after edge {edge_id}"));
    assert!(
        attrs.contains(r#"class="arrow""#),
        "expected polygon after edge {edge_id} to be an arrow, got {attrs}"
    );

    let transform_re = Regex::new(r#"\btransform="([^"]+)""#).expect("valid regex");
    transform_re
        .captures(attrs)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| panic!("missing arrow transform after edge {edge_id}"))
}

fn service_translate(svg: &str, service_id: &str) -> (f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\btransform="translate\(([^,\s]+)[,\s]+([^)]+)\)""#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing service transform for {service_id}"));
    let x = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service x transform for {service_id}"));
    let y = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service y transform for {service_id}"));
    (x, y)
}

fn group_rect(svg: &str, group_id: &str) -> (f64, f64, f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\bx="([^"]+)"[^>]*\by="([^"]+)"[^>]*\bwidth="([^"]+)"[^>]*\bheight="([^"]+)""#,
        regex::escape(group_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing group rect for {group_id}"));
    let parse = |idx: usize, label: &str| {
        caps.get(idx)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or_else(|| panic!("invalid {label} for {group_id}"))
    };
    (
        parse(1, "x"),
        parse(2, "y"),
        parse(3, "width"),
        parse(4, "height"),
    )
}

fn svg_max_width(svg: &str) -> f64 {
    let re = Regex::new(r#"style="max-width:\s*([^;]+)px;"#).expect("valid regex");
    re.captures(svg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing max-width in root svg style"))
}

fn icon_text_line_clamp(svg: &str, service_id: &str) -> i64 {
    let pattern = format!(
        r#"id="{}"[\s\S]*?-webkit-line-clamp:\s*([0-9]+);"#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    re.captures(svg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or_else(|| panic!("missing iconText line clamp for {service_id}"))
}

fn assert_close(actual: f64, expected: f64, message: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= 1e-6,
        "{message}: expected {expected}, got {actual}, delta {delta}"
    );
}

#[test]
fn architecture_svg_handles_deep_group_chain() {
    const DEPTH: usize = 64;
    let source = deep_group_chain_diagram(DEPTH);
    let handle = std::thread::Builder::new()
        .name("architecture-deep-group-svg".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            render_architecture_text_with_options(
                &source,
                &SvgRenderOptions {
                    diagram_id: Some("architecture-deep-groups".to_string()),
                    ..Default::default()
                },
            )
        })
        .expect("spawn architecture deep group SVG test");
    let svg = handle
        .join()
        .expect("architecture deep group SVG should finish without stack overflow");

    assert!(
        svg.contains(r#"id="architecture-deep-groups-service-leaf""#),
        "expected deepest service to render"
    );
    assert!(
        svg.contains(&format!(
            r#"id="architecture-deep-groups-group-g{}""#,
            DEPTH - 1
        )),
        "expected deepest group to render"
    );
}

#[test]
fn architecture_svg_honors_mermaid_11_15_style_theme_variables() {
    let text = r##"%%{init: {"themeVariables": {"lineColor": "#445566", "primaryBorderColor": "#778899", "archEdgeColor": "#010203", "archEdgeArrowColor": "#040506", "archEdgeWidth": 7, "archGroupBorderColor": "#070809", "archGroupBorderWidth": "6px"}}}%%
architecture-beta
  group core(cloud)[Core]
  service api(server)[API] in core
  service db(database)[DB] in core
  api:R --> L:db
"##;

    let svg = render_architecture_text_with_options(
        text,
        &SvgRenderOptions {
            diagram_id: Some("architecture-theme".to_string()),
            ..Default::default()
        },
    );

    assert!(svg.contains(r#"#architecture-theme .edge{stroke-width:7;stroke:#010203;fill:none;}"#));
    assert!(svg.contains(r#"#architecture-theme .arrow{fill:#040506;}"#));
    assert!(svg.contains(
        r#"#architecture-theme .node-bkg{fill:none;stroke:#070809;stroke-width:6px;stroke-dasharray:8;}"#
    ));
    assert!(
        !svg.contains(r#"#architecture-theme .edge{stroke-width:3;stroke:#445566;fill:none;}"#)
    );
    assert!(!svg.contains(r#"#architecture-theme .arrow{fill:#445566;}"#));
    assert!(!svg.contains(
        r#"#architecture-theme .node-bkg{fill:none;stroke:#778899;stroke-width:2px;stroke-dasharray:8;}"#
    ));
}

#[test]
fn architecture_diagonal_arrows_follow_the_actual_edge_segment() {
    let svg = render_architecture_fixture(
        "stress_architecture_batch5_services_outside_groups_crosslinks_078.mmd",
    );

    let diagonal = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_east_api_0");
    assert!(
        diagonal.contains("rotate("),
        "expected diagonal Architecture edge arrow to rotate with the edge segment, got {diagonal}"
    );

    let vertical = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_west_api_0");
    assert!(
        !vertical.contains("rotate("),
        "axis-aligned Architecture arrows should keep the Mermaid-compatible translate-only DOM, got {vertical}"
    );
}

#[test]
fn architecture_group_alignment_follows_source_endpoint_traversal_order() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_deep_nesting_013.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-deep".to_string()),
            ..Default::default()
        },
    );

    let lb = service_translate(&svg, "architecture-deep-service-lb");
    let api = service_translate(&svg, "architecture-deep-service-api");
    let cache = service_translate(&svg, "architecture-deep-service-cache");
    let ext = service_translate(&svg, "architecture-deep-service-ext");

    assert_close(
        lb.1,
        api.1,
        "lb/api should share Mermaid's horizontal alignment",
    );
    assert_close(
        lb.0,
        ext.0,
        "lb/ext should share Mermaid's vertical alignment",
    );
    assert_close(
        api.0,
        cache.0,
        "api/cache should share the final core/data vertical alignment",
    );
}

#[test]
fn architecture_group_rect_uses_configured_padding_for_small_icons() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch6_init_fontsize_icon_size_wrap_093.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-padding".to_string()),
            ..Default::default()
        },
    );

    let left = group_rect(&svg, "architecture-padding-group-left");
    assert!(
        left.2 >= 158.0,
        "custom architecture.padding should expand the group rect beyond the legacy iconSize/2 sizing, got width {}",
        left.2
    );
}

#[test]
fn architecture_vertical_edge_label_bounds_use_create_text_y_offsets() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch4_init_small_icons_061.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-small-icons".to_string()),
            ..Default::default()
        },
    );

    let group = group_rect(&svg, "architecture-small-icons-group-g");
    assert!(
        group.2 > 158.5 && group.2 < 158.6,
        "small-icon service/group sizing should remain icon-floor dominated, got group width {}",
        group.2
    );
    assert!(
        group.3 > 171.5 && group.3 < 171.6,
        "compound label bottom should follow architecture.fontSize + 1px for custom font sizes, got group height {}",
        group.3
    );

    let max_width = svg_max_width(&svg);
    assert!(
        (max_width - 187.85890197753906).abs() < 0.001,
        "vertical edge label createText bbox should contribute to the root width, got {max_width}"
    );
}

#[test]
fn architecture_long_title_group_rect_uses_narrower_long_label_canvas_approximation() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch5_long_titles_and_punct_076.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-batch5-long".to_string()),
            ..Default::default()
        },
    );

    let pipeline = group_rect(&svg, "architecture-batch5-long-group-pipeline");
    assert!(
        pipeline.2 > 460.0 && pipeline.2 < 473.5,
        "unexpected pipeline group width regression for long-title architecture fixture after the narrower long-label canvas approximation: {}",
        pipeline.2
    );
}

#[test]
fn architecture_icon_text_clamp_uses_architecture_font_size() {
    let svg = render_architecture_fixture_with_options(
        "upstream_architecture_docs_service_icon_text.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-icontext".to_string()),
            ..Default::default()
        },
    );

    let clamp = icon_text_line_clamp(&svg, "architecture-icontext-service-with_icon_text");
    assert_eq!(
        clamp, 4,
        "iconText clamp should follow default architecture.fontSize=16 with iconSize=80"
    );
}

#[test]
#[ignore = "diagnostic matrix for Architecture root-width experiments"]
fn architecture_root_width_diagnostic_matrix() {
    let fixtures = [
        "stress_architecture_batch5_long_titles_and_punct_076.mmd",
        "stress_architecture_batch4_init_small_icons_061.mmd",
        "stress_architecture_html_titles_and_escapes_041.mmd",
        "stress_architecture_unicode_and_xml_escapes_019.mmd",
        "stress_architecture_long_group_titles_018.mmd",
        "stress_architecture_batch6_long_group_titles_wrapping_extreme_095.mmd",
    ];

    for fixture in fixtures {
        let svg = render_architecture_fixture_with_options(
            fixture,
            &SvgRenderOptions {
                diagram_id: Some("architecture-diagnostic".to_string()),
                ..Default::default()
            },
        );
        let max_width = svg_max_width(&svg);
        println!("{fixture}: max-width={max_width}");
    }
}
