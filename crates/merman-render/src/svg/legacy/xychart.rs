use super::*;

// XYChart diagram SVG renderer implementation (split from legacy.rs).

pub(super) fn render_xychart_diagram_svg(
    layout: &XyChartDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    use std::collections::{BTreeMap, HashMap};

    #[derive(Debug, Clone)]
    struct Node {
        tag: String,
        attrs: BTreeMap<String, String>,
        text: Option<String>,
        children: Vec<usize>,
    }

    fn node(tag: &str) -> Node {
        Node {
            tag: tag.to_string(),
            attrs: BTreeMap::new(),
            text: None,
            children: Vec::new(),
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
        out.push_str(&n.tag);
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

    fn text_anchor(horizontal_pos: &str) -> &'static str {
        match horizontal_pos {
            "left" => "start",
            "right" => "end",
            _ => "middle",
        }
    }

    fn dominant_baseline(vertical_pos: &str) -> &'static str {
        if vertical_pos == "top" {
            "text-before-edge"
        } else {
            "middle"
        }
    }

    fn fmt_xy(v: f64) -> String {
        if v.is_nan() {
            return "NaN".to_string();
        }
        if !v.is_finite() {
            return "NaN".to_string();
        }
        fmt(v)
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("xychart");
    let diagram_id_esc = escape_xml(diagram_id);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {w} {h}" style="max-width: {w}px; background-color: white;" role="graphics-document document" aria-roledescription="xychart">"#,
        w = fmt(layout.width.max(1.0)),
        h = fmt(layout.height.max(1.0)),
    );

    let css = xychart_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);

    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"<g/>"#);

    // Build the `.main` group as an ordered DOM tree, matching Mermaid's D3 `getGroup()` behavior.
    let mut arena: Vec<Node> = Vec::new();
    arena.push(node("g"));
    arena[0]
        .attrs
        .insert("class".to_string(), "main".to_string());

    // Background rectangle.
    let mut bg = node("rect");
    bg.attrs.insert("width".to_string(), fmt_xy(layout.width));
    bg.attrs.insert("height".to_string(), fmt_xy(layout.height));
    bg.attrs
        .insert("class".to_string(), "background".to_string());
    bg.attrs
        .insert("fill".to_string(), escape_xml(&layout.background_color));
    push_child(&mut arena, 0, bg);

    let mut groups_by_prefix: HashMap<String, usize> = HashMap::new();

    for shape in &layout.drawables {
        match shape {
            crate::model::XyChartDrawableElem::Rect { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let mut prefix = String::new();
                let mut parent = 0usize;
                for (i, seg) in group_texts.iter().enumerate() {
                    let cur_parent = if i > 0 {
                        groups_by_prefix.get(&prefix).copied().unwrap_or(0)
                    } else {
                        0
                    };
                    parent = cur_parent;
                    prefix.push_str(seg);
                    let gid = if let Some(existing) = groups_by_prefix.get(&prefix).copied() {
                        existing
                    } else {
                        let mut g = node("g");
                        g.attrs.insert("class".to_string(), seg.clone());
                        let id = push_child(&mut arena, parent, g);
                        groups_by_prefix.insert(prefix.clone(), id);
                        id
                    };
                    parent = gid;
                }

                // Append rect elements.
                for r in data {
                    let mut n = node("rect");
                    n.attrs.insert("x".to_string(), fmt_xy(r.x));
                    if !r.y.is_nan() {
                        n.attrs.insert("y".to_string(), fmt_xy(r.y));
                    }
                    n.attrs.insert("width".to_string(), fmt_xy(r.width));
                    n.attrs.insert("height".to_string(), fmt_xy(r.height));
                    n.attrs.insert("fill".to_string(), escape_xml(&r.fill));
                    n.attrs
                        .insert("stroke".to_string(), escape_xml(&r.stroke_fill));
                    n.attrs
                        .insert("stroke-width".to_string(), fmt_xy(r.stroke_width));
                    push_child(&mut arena, parent, n);
                }

                // Optional bar data labels (Mermaid emits these in the renderer, not the DB).
                if layout.show_data_label {
                    let char_width_factor = 0.7;

                    #[derive(Clone)]
                    struct BarItem<'a> {
                        rect: &'a crate::model::XyChartRectData,
                        label: &'a str,
                    }

                    let mut valid_items: Vec<BarItem<'_>> = Vec::new();
                    for (idx, r) in data.iter().enumerate() {
                        let Some(label) = layout.label_data.get(idx) else {
                            continue;
                        };
                        if r.width > 0.0 && r.height > 0.0 {
                            valid_items.push(BarItem { rect: r, label });
                        }
                    }

                    if !valid_items.is_empty() {
                        if layout.chart_orientation == "horizontal" {
                            fn fits(
                                item: &BarItem<'_>,
                                font_size: f64,
                                char_width_factor: f64,
                            ) -> bool {
                                let text_w = font_size
                                    * (item.label.chars().count() as f64)
                                    * char_width_factor;
                                text_w <= item.rect.width - 10.0
                            }

                            let mut min_font = f64::INFINITY;
                            for item in &valid_items {
                                let mut fs = item.rect.height * 0.7;
                                while !fits(item, fs, char_width_factor) && fs > 0.0 {
                                    fs -= 1.0;
                                }
                                min_font = min_font.min(fs);
                            }
                            let uniform = min_font.floor().max(0.0);
                            for item in &valid_items {
                                let mut t = node("text");
                                t.attrs.insert(
                                    "x".to_string(),
                                    fmt_xy(item.rect.x + item.rect.width - 10.0),
                                );
                                t.attrs.insert(
                                    "y".to_string(),
                                    fmt_xy(item.rect.y + item.rect.height / 2.0),
                                );
                                t.attrs.insert("text-anchor".to_string(), "end".to_string());
                                t.attrs
                                    .insert("dominant-baseline".to_string(), "middle".to_string());
                                t.attrs.insert("fill".to_string(), "black".to_string());
                                t.attrs.insert(
                                    "font-size".to_string(),
                                    format!("{}px", fmt_xy(uniform)),
                                );
                                t.text = Some(escape_xml(item.label));
                                push_child(&mut arena, parent, t);
                            }
                        } else {
                            let y_offset = 10.0;
                            fn fits(
                                item: &BarItem<'_>,
                                font_size: f64,
                                char_width_factor: f64,
                                y_offset: f64,
                            ) -> bool {
                                let text_w = font_size
                                    * (item.label.chars().count() as f64)
                                    * char_width_factor;
                                let center_x = item.rect.x + item.rect.width / 2.0;
                                let left = center_x - text_w / 2.0;
                                let right = center_x + text_w / 2.0;
                                let horizontal =
                                    left >= item.rect.x && right <= item.rect.x + item.rect.width;
                                let vertical = item.rect.y + y_offset + font_size
                                    <= item.rect.y + item.rect.height;
                                horizontal && vertical
                            }

                            let mut min_font = f64::INFINITY;
                            for item in &valid_items {
                                let denom = (item.label.chars().count() as f64) * char_width_factor;
                                let mut fs = if denom <= 0.0 {
                                    0.0
                                } else {
                                    item.rect.width / denom
                                };
                                while !fits(item, fs, char_width_factor, y_offset) && fs > 0.0 {
                                    fs -= 1.0;
                                }
                                min_font = min_font.min(fs);
                            }
                            let uniform = min_font.floor().max(0.0);
                            for item in &valid_items {
                                let mut t = node("text");
                                t.attrs.insert(
                                    "x".to_string(),
                                    fmt_xy(item.rect.x + item.rect.width / 2.0),
                                );
                                t.attrs
                                    .insert("y".to_string(), fmt_xy(item.rect.y + y_offset));
                                t.attrs
                                    .insert("text-anchor".to_string(), "middle".to_string());
                                t.attrs
                                    .insert("dominant-baseline".to_string(), "hanging".to_string());
                                t.attrs.insert("fill".to_string(), "black".to_string());
                                t.attrs.insert(
                                    "font-size".to_string(),
                                    format!("{}px", fmt_xy(uniform)),
                                );
                                t.text = Some(escape_xml(item.label));
                                push_child(&mut arena, parent, t);
                            }
                        }
                    }
                }
            }
            crate::model::XyChartDrawableElem::Text { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let mut prefix = String::new();
                let mut parent = 0usize;
                for (i, seg) in group_texts.iter().enumerate() {
                    let cur_parent = if i > 0 {
                        groups_by_prefix.get(&prefix).copied().unwrap_or(0)
                    } else {
                        0
                    };
                    parent = cur_parent;
                    prefix.push_str(seg);
                    let gid = if let Some(existing) = groups_by_prefix.get(&prefix).copied() {
                        existing
                    } else {
                        let mut g = node("g");
                        g.attrs.insert("class".to_string(), seg.clone());
                        let id = push_child(&mut arena, parent, g);
                        groups_by_prefix.insert(prefix.clone(), id);
                        id
                    };
                    parent = gid;
                }

                for t in data {
                    let mut n = node("text");
                    n.attrs.insert("x".to_string(), "0".to_string());
                    n.attrs.insert("y".to_string(), "0".to_string());
                    n.attrs.insert("fill".to_string(), escape_xml(&t.fill));
                    n.attrs.insert("font-size".to_string(), fmt(t.font_size));
                    n.attrs.insert(
                        "dominant-baseline".to_string(),
                        dominant_baseline(&t.vertical_pos).to_string(),
                    );
                    n.attrs.insert(
                        "text-anchor".to_string(),
                        text_anchor(&t.horizontal_pos).to_string(),
                    );
                    let rot = t.rotation;
                    n.attrs.insert(
                        "transform".to_string(),
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
            crate::model::XyChartDrawableElem::Path { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let mut prefix = String::new();
                let mut parent = 0usize;
                for (i, seg) in group_texts.iter().enumerate() {
                    let cur_parent = if i > 0 {
                        groups_by_prefix.get(&prefix).copied().unwrap_or(0)
                    } else {
                        0
                    };
                    parent = cur_parent;
                    prefix.push_str(seg);
                    let gid = if let Some(existing) = groups_by_prefix.get(&prefix).copied() {
                        existing
                    } else {
                        let mut g = node("g");
                        g.attrs.insert("class".to_string(), seg.clone());
                        let id = push_child(&mut arena, parent, g);
                        groups_by_prefix.insert(prefix.clone(), id);
                        id
                    };
                    parent = gid;
                }

                for p in data {
                    let mut n = node("path");
                    n.attrs.insert("d".to_string(), escape_xml(&p.path));
                    n.attrs.insert(
                        "fill".to_string(),
                        escape_xml(p.fill.as_deref().unwrap_or("none")),
                    );
                    n.attrs
                        .insert("stroke".to_string(), escape_xml(&p.stroke_fill));
                    n.attrs
                        .insert("stroke-width".to_string(), fmt_xy(p.stroke_width));
                    push_child(&mut arena, parent, n);
                }
            }
        }
    }

    render_node(&mut out, &arena, 0);
    out.push_str(r#"<g class="mermaid-tmp-group"/>"#);
    out.push_str("</svg>\n");
    Ok(out)
}
