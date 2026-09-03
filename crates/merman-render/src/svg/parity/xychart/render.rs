use super::super::*;
use merman_core::diagrams::xychart::XyChartDiagramRenderModel;

// XYChart diagram SVG renderer implementation (split from parity.rs).

pub(crate) fn render_xychart_diagram_svg(
    layout: &XyChartDiagramLayout,
    model: &XyChartDiagramRenderModel,
    _effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    use rustc_hash::FxHashMap;
    use std::collections::hash_map::Entry;

    struct Node {
        tag: &'static str,
        attrs: Vec<(&'static str, String)>,
        text: Option<String>,
        children: Vec<usize>,
    }

    impl Node {
        fn attr(&mut self, name: &'static str, value: impl Into<String>) {
            self.attrs.push((name, value.into()));
        }
    }

    fn node(tag: &'static str) -> Node {
        Node {
            tag,
            attrs: Vec::with_capacity(6),
            text: None,
            children: Vec::with_capacity(2),
        }
    }

    fn push_child(arena: &mut Vec<Node>, parent: usize, child: Node) -> usize {
        let id = arena.len();
        arena.push(child);
        arena[parent].children.push(id);
        id
    }

    fn render_node(out: &mut String, arena: &[Node], id: usize) {
        let n = &arena[id];
        out.push('<');
        out.push_str(n.tag);
        for (k, v) in &n.attrs {
            let _ = write!(out, r#" {k}="{v}""#);
        }
        if n.children.is_empty() && n.text.as_deref().unwrap_or("").is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        if let Some(t) = n.text.as_deref() {
            out.push_str(t);
        }
        for c in &n.children {
            render_node(out, arena, *c);
        }
        let _ = write!(out, "</{}>", n.tag);
    }

    fn ensure_group_path<'a>(
        arena: &mut Vec<Node>,
        groups_by_path: &mut FxHashMap<(usize, &'a str), usize>,
        group_texts: &'a [String],
    ) -> usize {
        let mut parent = 0usize;
        for seg in group_texts {
            let class = seg.as_str();
            let gid = match groups_by_path.entry((parent, class)) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let mut g = node("g");
                    g.attr("class", class);
                    let id = push_child(arena, parent, g);
                    entry.insert(id);
                    id
                }
            };
            parent = gid;
        }
        parent
    }

    fn fmt_xy(v: f64) -> String {
        if v.is_nan() {
            return "NaN".to_string();
        }
        if !v.is_finite() {
            return "NaN".to_string();
        }
        fmt_string(v)
    }

    let diagram_id = options.diagram_id_or("xychart");
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|description| description.trim_end_matches('\n'))
        .filter(|description| !description.trim().is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "xychart");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::XyChart, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
    }

    out.push_str("<style>");
    push_xychart_css(&mut out, diagram_id);
    out.push_str("</style>");
    options.checkpoint_emit()?;

    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"<g/>"#);

    // Build the `.main` group as an ordered DOM tree, matching Mermaid's D3 `getGroup()` behavior.
    let mut arena: Vec<Node> = Vec::with_capacity(layout.drawables.len().saturating_mul(4) + 2);
    arena.push(node("g"));
    arena[0].attr("class", "main");

    // Background rectangle.
    let mut bg = node("rect");
    bg.attr("width", fmt_xy(layout.width));
    bg.attr("height", fmt_xy(layout.height));
    bg.attr("class", "background");
    bg.attr("fill", escape_xml(&layout.background_color));
    push_child(&mut arena, 0, bg);

    let mut groups_by_path: FxHashMap<(usize, &str), usize> = FxHashMap::with_capacity_and_hasher(
        layout.drawables.len().saturating_mul(2) + 4,
        Default::default(),
    );

    for shape in &layout.drawables {
        match shape {
            crate::model::XyChartDrawableElem::Rect { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);

                // Append rect elements.
                for r in data {
                    let mut n = node("rect");
                    n.attr("x", fmt_xy(r.x));
                    if !r.y.is_nan() {
                        n.attr("y", fmt_xy(r.y));
                    }
                    n.attr("width", fmt_xy(r.width));
                    n.attr("height", fmt_xy(r.height));
                    n.attr("fill", escape_xml(&r.fill));
                    n.attr("stroke", escape_xml(&r.stroke_fill));
                    n.attr("stroke-width", fmt_xy(r.stroke_width));
                    push_child(&mut arena, parent, n);
                }
            }
            crate::model::XyChartDrawableElem::Text { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);

                for t in data {
                    let mut n = node("text");
                    n.attr("x", "0");
                    n.attr("y", "0");
                    n.attr("fill", escape_xml(&t.fill));
                    n.attr("font-size", fmt_string(t.font_size));
                    n.attr(
                        "dominant-baseline",
                        crate::xychart::xychart_text_baseline(&t.vertical_pos).as_svg(),
                    );
                    n.attr(
                        "text-anchor",
                        crate::xychart::xychart_text_anchor(&t.horizontal_pos).as_svg(),
                    );
                    let rot = t.rotation;
                    n.attr(
                        "transform",
                        format!(
                            "translate({}, {}) rotate({})",
                            fmt_xy(t.x),
                            fmt_xy(t.y),
                            fmt_xy(rot)
                        ),
                    );
                    n.text = Some(escape_xml(&t.text));
                    push_child(&mut arena, parent, n);
                }
            }
            crate::model::XyChartDrawableElem::BarDataLabel { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);

                for t in data {
                    let mut n = node("text");
                    n.attr("x", fmt_xy(t.x));
                    n.attr("y", fmt_xy(t.y));
                    n.attr(
                        "text-anchor",
                        crate::xychart::xychart_text_anchor(&t.horizontal_pos).as_svg(),
                    );
                    n.attr(
                        "dominant-baseline",
                        crate::xychart::xychart_text_baseline(&t.vertical_pos).as_svg(),
                    );
                    n.attr("fill", escape_xml(&t.fill));
                    n.attr("font-size", format!("{}px", fmt_xy(t.font_size)));
                    n.text = Some(escape_xml(&t.text));
                    push_child(&mut arena, parent, n);
                }
            }
            crate::model::XyChartDrawableElem::Path { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);

                for p in data {
                    let mut n = node("path");
                    n.attr("d", escape_xml(&p.path));
                    n.attr("fill", escape_xml(p.fill.as_deref().unwrap_or("none")));
                    n.attr("stroke", escape_xml(&p.stroke_fill));
                    n.attr("stroke-width", fmt_xy(p.stroke_width));
                    push_child(&mut arena, parent, n);
                }
            }
        }
    }

    render_node(&mut out, &arena, 0);
    out.push_str(r#"<g class="mermaid-tmp-group"/>"#);
    out.push_str("</svg>\n");
    options.checkpoint_emit()?;
    root_document.complete(out)
}
