use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_flowchart_v2_svg};
use merman_render::text::VendoredFontMetricsTextMeasurer;
use merman_render::{LayoutOptions, layout_parsed};

fn attr_value<'a>(s: &'a str, name: &str) -> &'a str {
    let needle = format!(r#"{name}=""#);
    let start = s.find(&needle).expect("attribute") + needle.len();
    let end = s[start..].find('"').expect("attribute end") + start;
    &s[start..end]
}

fn parse_translate(value: &str) -> (f64, f64) {
    let inner = value
        .strip_prefix("translate(")
        .and_then(|v| v.strip_suffix(')'))
        .expect("translate(...)");
    let mut parts = inner.split(',').map(str::trim);
    let x = parts.next().expect("translate x").parse().expect("x");
    let y = parts.next().expect("translate y").parse().expect("y");
    (x, y)
}

fn path_numbers(path: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut start = None;
    for (idx, ch) in path.char_indices() {
        let is_number_char = ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E');
        if is_number_char {
            start.get_or_insert(idx);
        } else if let Some(s) = start.take() {
            if let Ok(v) = path[s..idx].parse() {
                out.push(v);
            }
        }
    }
    if let Some(s) = start {
        if let Ok(v) = path[s..].parse() {
            out.push(v);
        }
    }
    out
}

fn shape_path_bbox(shape_chunk: &str) -> (f64, f64, f64, f64) {
    let mut rest = shape_chunk;
    let mut bbox: Option<(f64, f64, f64, f64)> = None;
    while let Some(idx) = rest.find(r#"<path d=""#) {
        rest = &rest[idx + r#"<path d=""#.len()..];
        let end = rest.find('"').expect("path d end");
        let nums = path_numbers(&rest[..end]);
        for pair in nums.chunks_exact(2) {
            let point = (pair[0], pair[1], pair[0], pair[1]);
            bbox = Some(match bbox {
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(point.0),
                    min_y.min(point.1),
                    max_x.max(point.2),
                    max_y.max(point.3),
                ),
                None => point,
            });
        }
        rest = &rest[end..];
    }
    bbox.expect("shape path bbox")
}

fn assert_close(actual: f64, expected: f64, name: &str) {
    assert!(
        (actual - expected).abs() <= 1e-6,
        "{name}: expected {expected}, got {actual}"
    );
}

#[test]
fn flowchart_stacked_rectangle_svg_uses_layout_bbox_once() {
    let text = r#"flowchart
 n0@{ shape: procs, label: "procs" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };
    let node = layout.nodes.iter().find(|n| n.id == "n0").expect("node n0");

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            apply_root_overrides: false,
            ..Default::default()
        },
    )
    .expect("render svg");

    let node_start = svg.find(r#"<g class="node default""#).expect("node group");
    let node_chunk = &svg[node_start..];
    let label_start = node_chunk.find(r#"<g class="label""#).expect("label group");
    let shape_chunk = &node_chunk[..label_start];
    let label_chunk = &node_chunk[label_start..];

    let (min_x, min_y, max_x, max_y) = shape_path_bbox(shape_chunk);
    assert_close(
        max_x - min_x,
        node.width,
        "stacked rectangle rendered width",
    );
    assert_close(
        max_y - min_y,
        node.height,
        "stacked rectangle rendered height",
    );

    let label_open_end = label_chunk.find('>').expect("label open end");
    let label_open = &label_chunk[..label_open_end];
    let (label_x, label_y) = parse_translate(attr_value(label_open, "transform"));
    let foreign_object_start = label_chunk
        .find("<foreignObject ")
        .expect("foreignObject start");
    let foreign_object_open_end = label_chunk[foreign_object_start..]
        .find('>')
        .expect("foreignObject open end")
        + foreign_object_start;
    let foreign_object_open = &label_chunk[foreign_object_start..foreign_object_open_end];
    let label_w: f64 = attr_value(foreign_object_open, "width")
        .parse()
        .expect("label width");
    let label_h: f64 = attr_value(foreign_object_open, "height")
        .parse()
        .expect("label height");

    assert_close(label_x, -label_w / 2.0 - 5.0, "stacked rectangle label x");
    assert_close(label_y, -label_h / 2.0 + 5.0, "stacked rectangle label y");
}

#[test]
fn flowchart_stacked_rectangle_classic_merges_each_layer_path() {
    let text = r#"flowchart
 n0@{ shape: procs, label: "procs" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            apply_root_overrides: false,
            ..Default::default()
        },
    )
    .expect("render svg");

    let node_start = svg.find(r#"<g class="node default""#).expect("node group");
    let node_chunk = &svg[node_start..];
    let label_start = node_chunk.find(r#"<g class="label""#).expect("label group");
    let shape_chunk = &node_chunk[..label_start];

    assert!(
        shape_chunk.matches(r#"<g><path "#).count() >= 2,
        "expected Mermaid 11.15 classic procs to merge each RoughJS layer into a grouped path: {svg}"
    );
    assert!(
        shape_chunk.contains(r#"fill-opacity="1""#)
            && shape_chunk.contains(r#"stroke-opacity="1""#),
        "expected merged paths to carry Mermaid 11.15 mergePaths fill/stroke opacity attrs: {svg}"
    );
    assert!(
        !shape_chunk.contains(r#"stroke="none" stroke-width="0""#),
        "expected classic procs not to expose separate fill/stroke RoughJS paths after mergePaths: {svg}"
    );
}
