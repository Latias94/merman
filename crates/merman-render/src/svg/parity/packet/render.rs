use super::super::css::{MermaidBaseCss, write_mermaid_base_css_prefix};
use super::super::*;
use merman_model::families::packet::PacketDiagramModel;

fn write_packet_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    typography_theme: &crate::packet::PacketTypographyThemePlan,
) -> Result<()> {
    // Keep `:root` last (matches upstream Mermaid packet SVG baselines).
    let id = crate::svg::escape_css_identifier(diagram_id);
    let font = typography_theme.font_family_css();
    let style = typography_theme.css_binding();
    let start_byte_fill = style.start_byte_color.as_ref();
    let end_byte_fill = style.end_byte_color.as_ref();
    let label_fill = style.label_color.as_ref();
    let title_fill = style.title_color.as_ref();
    let font_size_css = format!("{}px", typography_theme.root_font_size_px());
    let native_typography = typography_theme.native_typography();
    let block_id = if native_typography.is_some() {
        id.as_ref()
    } else {
        diagram_id
    };
    write_mermaid_base_css_prefix(
        out,
        &id,
        MermaidBaseCss {
            font_family: font,
            font_size_css: &font_size_css,
            normal_edge_stroke_width_css: "1px",
            text_color: "#333",
            line_color: "#333333",
            error_bkg: "#552222",
            error_text: "#552222",
        },
    )?;
    let _ = write!(
        out,
        r#"#{} .packetByte{{font-size:{};}}#{} .packetByte.start{{fill:{};}}#{} .packetByte.end{{fill:{};}}#{} .packetLabel{{fill:{};font-size:{};}}#{} .packetTitle{{fill:{};font-size:{};}}#{} .packetBlock{{stroke:{};stroke-width:{};fill:{};}}"#,
        id,
        style.byte_font_size,
        id,
        start_byte_fill,
        id,
        end_byte_fill,
        id,
        label_fill,
        style.label_font_size,
        id,
        title_fill,
        style.title_font_size,
        block_id,
        style.block_stroke_color,
        style.block_stroke_width,
        style.block_fill_color
    );
    out.checkpoint()?;
    if let Some(native) = native_typography {
        let byte_font = native.byte_font_family.as_deref().unwrap_or(font);
        let title_font = native.title_font_family.as_deref().unwrap_or(font);
        let _ = write!(
            out,
            "#{id} .packetByte{{font-family:{byte_font};}}\
             #{id} .packetLabel{{font-weight:{};}}\
             #{id} .packetTitle{{font-family:{title_font};font-weight:{};}}",
            native.label_weight, native.title_weight,
        );
        out.checkpoint()?;
    }
    let _ = write!(out, r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    out.checkpoint()?;

    Ok(())
}

pub(crate) fn render_packet_diagram_svg_model(
    layout: &PacketDiagramLayout,
    model: &PacketDiagramModel,
    typography_theme: &crate::packet::PacketTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let title_from_semantic = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let title_from_meta = diagram_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let terminal_title = title_from_semantic.or(title_from_meta);
    let config = crate::packet::PacketConfigView::new(effective_config);
    let bit_order = config.bit_order();

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: layout.width.max(1.0),
        max_y: layout.height.max(1.0),
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, config.use_max_width());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "packet");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::PACKET, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(t)
        );
        out.checkpoint()?;
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(d)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    write_packet_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id.semantic_str(),
        typography_theme,
    )?;
    out.push_str("</style>");
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;

    for word in &layout.words {
        out.push_str("<g>");
        out.checkpoint()?;
        for b in &word.blocks {
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="packetBlock"/>"#,
                x = fmt(b.x),
                y = fmt(b.y),
                w = fmt(b.width),
                h = fmt(b.height)
            );
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="packetLabel" dominant-baseline="middle" text-anchor="middle">{text}</text>"#,
                x = fmt(b.x + b.width / 2.0),
                y = fmt(b.y + b.height / 2.0),
                text = escape_xml(&b.label)
            );

            if !layout.show_bits {
                out.checkpoint()?;
                continue;
            }
            let (leading_bit, trailing_bit) = match bit_order {
                crate::packet::PacketBitOrder::Ascending => (b.start, b.end),
                crate::packet::PacketBitOrder::Descending => (b.end, b.start),
            };
            let is_single_block = b.end - b.start + 1.0 == 1.0;
            let bit_number_y = b.y - 2.0;
            let start_x = if is_single_block {
                b.x + b.width / 2.0
            } else {
                b.x
            };
            let start_anchor = if is_single_block { "middle" } else { "start" };
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="packetByte start" dominant-baseline="auto" text-anchor="{anchor}">{text}</text>"#,
                x = fmt(start_x),
                y = fmt(bit_number_y),
                anchor = start_anchor,
                text = ryu_js::Buffer::new().format(leading_bit)
            );
            if !is_single_block {
                let _ = write!(
                    &mut out,
                    r#"<text x="{x}" y="{y}" class="packetByte end" dominant-baseline="auto" text-anchor="end">{text}</text>"#,
                    x = fmt(b.x + b.width),
                    y = fmt(bit_number_y),
                    text = ryu_js::Buffer::new().format(trailing_bit)
                );
            }
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    let total_row_height = layout.row_height + layout.padding_y;
    let title_y = layout.height - total_row_height / 2.0;
    match terminal_title {
        Some(title) => {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle">{text}</text>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
                text = escape_xml(title)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle"/>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
            );
        }
    }
    out.checkpoint()?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn packet_css_honors_mermaid_packet_theme_variables() {
        let config = crate::test_support::presentation_config(json!({
            "themeVariables": { "packet": {
                "byteFontSize": "11px",
                "startByteColor": "#111111",
                "endByteColor": "#222222",
                "labelColor": "#333333",
                "labelFontSize": "13px",
                "titleColor": "#444444",
                "titleFontSize": "15px",
                "blockStrokeColor": "#555555",
                "blockStrokeWidth": 2,
                "blockFillColor": "#666666"
            } }
        }));
        let typography_theme = crate::packet::PacketTypographyThemePlan::resolve(None, &config);
        let mut css = String::new();
        write_packet_css(&mut css, "pkt", &typography_theme).expect("write Packet CSS");

        assert!(css.contains("#pkt .packetByte{font-size:11px;}"));
        assert!(css.contains("#pkt .packetByte.start{fill:#111111;}"));
        assert!(css.contains("#pkt .packetByte.end{fill:#222222;}"));
        assert!(css.contains("#pkt .packetLabel{fill:#333333;font-size:13px;}"));
        assert!(css.contains("#pkt .packetTitle{fill:#444444;font-size:15px;}"));
        assert!(css.contains("#pkt .packetBlock{stroke:#555555;stroke-width:2;fill:#666666;}"));
    }
}
