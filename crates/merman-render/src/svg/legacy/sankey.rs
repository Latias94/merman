use super::*;

pub(super) fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    fn config_bool(cfg: &serde_json::Value, path: &[&str]) -> Option<bool> {
        let mut cur = cfg;
        for key in path {
            cur = cur.get(*key)?;
        }
        cur.as_bool()
    }

    fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
        let mut cur = cfg;
        for key in path {
            cur = cur.get(*key)?;
        }
        cur.as_str().map(|s| s.to_string())
    }

    let sankey_cfg = effective_config.get("sankey");
    let sankey_cfg_missing = sankey_cfg.is_none()
        || sankey_cfg.is_some_and(|v| v.as_object().is_some_and(|m| m.contains_key("$ref")));
    let show_values = if sankey_cfg_missing {
        true
    } else {
        config_bool(effective_config, &["sankey", "showValues"]).unwrap_or(true)
    };
    let prefix = if sankey_cfg_missing {
        "".to_string()
    } else {
        config_string(effective_config, &["sankey", "prefix"]).unwrap_or_default()
    };
    let suffix = if sankey_cfg_missing {
        "".to_string()
    } else {
        config_string(effective_config, &["sankey", "suffix"]).unwrap_or_default()
    };
    let link_color = if sankey_cfg_missing {
        "gradient".to_string()
    } else {
        config_string(effective_config, &["sankey", "linkColor"])
            .unwrap_or_else(|| "gradient".to_string())
    };

    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    let diagram_id = options.diagram_id.as_deref().unwrap_or("sankey");
    let diagram_id_esc = escape_xml(diagram_id);

    const LABEL_FONT_SIZE_PX: f64 = 14.0;
    const DEFAULT_ASCENT_EM: f64 = 0.9285714286;
    const DEFAULT_DESCENT_EM: f64 = 0.262;

    let mut min_x: f64 = 0.0;
    let mut min_y: f64 = 0.0;
    let mut max_x = layout_width;
    let mut max_y = layout_height;

    for n in &layout.nodes {
        min_x = min_x.min(n.x0);
        min_y = min_y.min(n.y0);
        max_x = max_x.max(n.x1);
        max_y = max_y.max(n.y1);

        let dy_em = if show_values { 0.0 } else { 0.35 };
        let baseline_y = (n.y0 + n.y1) / 2.0 + dy_em * LABEL_FONT_SIZE_PX;
        let ascent = LABEL_FONT_SIZE_PX * DEFAULT_ASCENT_EM;
        let descent = LABEL_FONT_SIZE_PX * DEFAULT_DESCENT_EM;
        min_y = min_y.min(baseline_y - ascent);
        max_y = max_y.max(baseline_y + descent);
    }

    for l in &layout.links {
        let sw = l.width.max(1.0);
        let half = sw / 2.0;
        let y0 = l.y0.min(l.y1);
        let y1 = l.y0.max(l.y1);
        min_y = min_y.min(y0 - half);
        max_y = max_y.max(y1 + half);
    }

    let vb_w = (max_x - min_x).max(1.0);
    let vb_h = (max_y - min_y).max(1.0);

    let mut max_w_attr = fmt(vb_w);
    let mut viewbox_attr = format!("{} {} {} {}", fmt(min_x), fmt(min_y), fmt(vb_w), fmt(vb_h));
    if let Some((viewbox, max_w)) =
        crate::generated::sankey_root_overrides_11_12_2::lookup_sankey_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{id}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {w}px; background-color: white;" viewBox="{viewbox}" role="graphics-document document" aria-roledescription="sankey">"#,
        id = diagram_id_esc,
        w = max_w_attr,
        viewbox = viewbox_attr,
    );
    let _ = write!(&mut out, "<style>{}</style>", sankey_css(diagram_id));
    out.push_str("<g/>");

    let scheme_tableau10: [&str; 10] = [
        "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
        "#9c755f", "#bab0ab",
    ];

    let mut color_domain: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut color_for = |id: &str| -> String {
        if let Some(&idx) = color_domain.get(id) {
            return scheme_tableau10[idx % scheme_tableau10.len()].to_string();
        }
        let idx = color_domain.len();
        color_domain.insert(id.to_string(), idx);
        scheme_tableau10[idx % scheme_tableau10.len()].to_string()
    };

    let mut uid_count: usize = 0;
    let mut next_uid = |prefix: &str| -> String {
        uid_count += 1;
        format!("{prefix}{uid_count}")
    };

    let mut node_uid_by_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for n in &layout.nodes {
        node_uid_by_id.insert(n.id.clone(), next_uid("node-"));
        let _ = color_for(&n.id);
    }

    out.push_str(r#"<g class="nodes">"#);
    for n in &layout.nodes {
        let node_uid = node_uid_by_id
            .get(&n.id)
            .cloned()
            .unwrap_or_else(|| "node-0".to_string());
        let x = n.x0;
        let y = n.y0;
        let w = n.x1 - n.x0;
        let h = n.y1 - n.y0;
        let fill = color_for(&n.id);
        let _ = write!(
            &mut out,
            r#"<g class="node" id="{id}" transform="translate({x},{y})" x="{x}" y="{y}"><rect height="{h}" width="{w}" fill="{fill}"/></g>"#,
            id = escape_xml(&node_uid),
            x = fmt(x),
            y = fmt(y),
            h = fmt(h),
            w = fmt(w),
            fill = fill,
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="node-labels" font-size="14">"#);
    for n in &layout.nodes {
        let y = (n.y0 + n.y1) / 2.0;
        let (x, anchor) = if n.x0 < layout_width / 2.0 {
            (n.x1 + 6.0, "start")
        } else {
            (n.x0 - 6.0, "end")
        };
        let dy = if show_values { "0em" } else { "0.35em" };
        let v = (n.value * 100.0).round() / 100.0;
        let text = if show_values {
            format!("{}\n{}{}{}", n.id, prefix, v, suffix)
        } else {
            n.id.clone()
        };
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" dy="{dy}" text-anchor="{anchor}">{text}</text>"#,
            x = fmt(x),
            y = fmt(y),
            dy = dy,
            anchor = anchor,
            text = escape_xml(&text),
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="links" fill="none" stroke-opacity="0.5">"#);

    for l in &layout.links {
        let source = layout
            .nodes
            .iter()
            .find(|n| n.id == l.source)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing source node {}", l.source),
            })?;
        let target = layout
            .nodes
            .iter()
            .find(|n| n.id == l.target)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing target node {}", l.target),
            })?;

        let sx = source.x1;
        let tx = target.x0;
        let mx = (sx + tx) / 2.0;
        let path_d = format!(
            "M{sx},{y0}C{mx},{y0},{mx},{y1},{tx},{y1}",
            sx = fmt(sx),
            y0 = fmt(l.y0),
            mx = fmt(mx),
            y1 = fmt(l.y1),
            tx = fmt(tx),
        );

        out.push_str(r#"<g class="link" style="mix-blend-mode: multiply;">"#);

        let stroke = match link_color.as_str() {
            "source" => color_for(&source.id),
            "target" => color_for(&target.id),
            "gradient" => {
                let gradient_id = next_uid("linearGradient-");
                let source_color = color_for(&source.id);
                let target_color = color_for(&target.id);
                let _ = write!(
                    &mut out,
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{x1}" x2="{x2}"><stop offset="0%" stop-color="{c1}"/><stop offset="100%" stop-color="{c2}"/></linearGradient>"#,
                    id = escape_xml(&gradient_id),
                    x1 = fmt(sx),
                    x2 = fmt(tx),
                    c1 = source_color,
                    c2 = target_color,
                );
                format!("url(#{})", gradient_id)
            }
            other => other.to_string(),
        };

        let stroke_width = l.width.max(1.0);
        let _ = write!(
            &mut out,
            r#"<path d="{d}" stroke="{stroke}" stroke-width="{sw}"/></g>"#,
            d = escape_xml(&path_d),
            stroke = escape_xml(&stroke),
            sw = fmt(stroke_width),
        );
    }

    out.push_str("</g>");
    out.push_str("</svg>");
    Ok(out)
}
