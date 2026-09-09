use super::super::*;
use crate::sankey::{SANKEY_LABEL_FONT_SIZE_PX, SankeyVisualLinkPaint, build_sankey_visual_plan};

#[derive(Clone)]
struct SankeyNodeUid<'a> {
    diagram_id: Option<SvgDiagramId<'a>>,
    local_id: String,
}

impl std::fmt::Display for SankeyNodeUid<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.diagram_id {
            Some(diagram_id) => write!(formatter, "{diagram_id}-{}", self.local_id),
            None => formatter.write_str(&self.local_id),
        }
    }
}

pub(crate) fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let plan = build_sankey_visual_plan(layout, effective_config)?;
    let diagram_id = options.diagram_id_or("sankey");
    let scope_generated_ids = options.has_explicit_diagram_id();
    let vb_w = plan.bounds.max_x - plan.bounds.min_x;
    let vb_h = plan.bounds.max_y - plan.bounds.min_y;

    let root_spec = root_svg::RootViewportSpec::mermaid(
        root_svg::DiagramBounds::from_view_box(plan.bounds.min_x, plan.bounds.min_y, vb_w, vb_h),
        plan.use_max_width,
    )
    .with_max_width(root_svg::RootMaxWidth::SvgNumber(vb_w));

    let mut out = String::new();
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Sankey, diagram_id)
            .write_open(
                &mut out,
                root_spec,
                root_svg::RootChrome {
                    dom: root_svg::RootDomProfile {
                        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
                        fixed_style_placement: root_svg::RootStylePlacement::Tail,
                        trailing_newline: false,
                        ..Default::default()
                    },
                    ..root_svg::RootChrome::new(diagram_id, "sankey")
                },
            )?;
    let _ = write!(
        &mut out,
        "<style>{}</style>",
        sankey_css(diagram_id, effective_config)
    );
    options.checkpoint_emit()?;
    out.push_str("<g/>");

    let mut uid_count: usize = 0;
    let mut next_generated_id = |prefix: &str| -> SankeyNodeUid<'_> {
        uid_count += 1;
        let local_id = format!("{prefix}{uid_count}");
        SankeyNodeUid {
            diagram_id: scope_generated_ids.then_some(diagram_id),
            local_id,
        }
    };

    let mut node_uids = Vec::with_capacity(plan.nodes.len());
    for _ in &plan.nodes {
        options.checkpoint_emit()?;
        node_uids.push(next_generated_id("node-"));
    }

    out.push_str(r#"<g class="nodes">"#);
    for (node, node_uid) in plan.nodes.iter().zip(&node_uids) {
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="node" id="{id}" transform="translate({x},{y})" x="{x}" y="{y}"><rect height="{h}" width="{w}" fill="{fill}"/></g>"#,
            id = node_uid,
            x = fmt(node.x),
            y = fmt(node.y),
            h = fmt(node.height),
            w = fmt(node.width),
            fill = escape_attr(&node.fill),
        );
    }
    out.push_str("</g>");

    let _ = write!(
        &mut out,
        r#"<g class="node-labels" font-size="{font_size}">"#,
        font_size = fmt(SANKEY_LABEL_FONT_SIZE_PX)
    );

    let append_labels = |out: &mut String, class_name: Option<&str>| -> Result<()> {
        for label in &plan.labels {
            options.checkpoint_emit()?;
            let class_attr = class_name
                .map(|class_name| format!(r#" class="{}""#, escape_attr(class_name)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<text{class_attr} x="{x}" y="{y}" dy="{dy}em" text-anchor="{anchor}">{text}</text>"#,
                class_attr = class_attr,
                x = fmt(label.x),
                y = fmt(label.y),
                dy = fmt(label.dy_em),
                anchor = label.anchor.as_svg(),
                text = escape_xml(&label.text),
            );
        }
        Ok(())
    };
    if plan.outlined_labels {
        append_labels(&mut out, Some("sankey-label-bg"))?;
        append_labels(&mut out, Some("sankey-label-fg"))?;
    } else {
        append_labels(&mut out, None)?;
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="links" fill="none" stroke-opacity="0.5">"#);

    for link in &plan.links {
        options.checkpoint_emit()?;
        let path_d = format!(
            "M{sx},{y0}C{mx},{y0},{mx},{y1},{tx},{y1}",
            sx = fmt(link.start_x),
            y0 = fmt(link.start_y),
            mx = fmt(link.control_x),
            y1 = fmt(link.end_y),
            tx = fmt(link.end_x),
        );

        out.push_str(r#"<g class="link" style="mix-blend-mode: multiply;">"#);

        let stroke = match &link.paint {
            SankeyVisualLinkPaint::Solid(color) => color.clone(),
            SankeyVisualLinkPaint::LinearGradient {
                start_color,
                end_color,
            } => {
                let gradient_id = next_generated_id("linearGradient-");
                let _ = write!(
                    &mut out,
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{x1}" x2="{x2}"><stop offset="0%" stop-color="{c1}"/><stop offset="100%" stop-color="{c2}"/></linearGradient>"#,
                    id = &gradient_id,
                    x1 = fmt(link.start_x),
                    x2 = fmt(link.end_x),
                    c1 = escape_attr(start_color),
                    c2 = escape_attr(end_color),
                );
                format!("url(#{})", gradient_id)
            }
        };

        let _ = write!(
            &mut out,
            r#"<path d="{d}" stroke="{stroke}" stroke-width="{sw}"/></g>"#,
            d = escape_xml(&path_d),
            stroke = escape_attr(&stroke),
            sw = fmt(link.width),
        );
    }

    out.push_str("</g>");
    out.push_str("</svg>");
    root_document.complete(out)
}
