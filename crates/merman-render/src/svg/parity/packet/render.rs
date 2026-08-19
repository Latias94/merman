use super::super::*;
use merman_core::diagrams::packet::PacketDiagramRenderModel;

fn write_packet_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    typography_theme: &crate::packet::PacketTypographyThemePlan,
    typography_receipt: &mut crate::packet::PacketSurfaceReceipt,
) -> Result<()> {
    // Keep `:root` last (matches upstream Mermaid packet SVG baselines).
    let id = crate::svg::escape_css_identifier(diagram_id);
    let font = typography_theme.font_family_css();
    let style = crate::packet::PacketConfigView::new(effective_config).style_settings();
    let byte_start_role = crate::packet::PacketTextRole::ByteStart;
    let byte_end_role = crate::packet::PacketTextRole::ByteEnd;
    let label_role = crate::packet::PacketTextRole::Label;
    let title_role = crate::packet::PacketTextRole::Title;
    let start_byte_fill = typography_theme.fill_css(byte_start_role, &style.start_byte_color);
    let end_byte_fill = typography_theme.fill_css(byte_end_role, &style.end_byte_color);
    let label_fill = typography_theme.fill_css(label_role, &style.label_color);
    let title_fill = typography_theme.fill_css(title_role, &style.title_color);
    write_mermaid_default_base_css_prefix(out, &id, font)?;
    let _ = write!(
        out,
        r#"#{} .packetByte{{font-size:{};}}#{} {}{{fill:{};}}#{} {}{{fill:{};}}#{} {}{{fill:{};font-size:{};}}#{} {}{{fill:{};font-size:{};}}#{} .packetBlock{{stroke:{};stroke-width:{};fill:{};}}"#,
        id,
        style.byte_font_size,
        id,
        byte_start_role.css_selector(),
        start_byte_fill,
        id,
        byte_end_role.css_selector(),
        end_byte_fill,
        id,
        label_role.css_selector(),
        label_fill,
        style.label_font_size,
        id,
        title_role.css_selector(),
        title_fill,
        style.title_font_size,
        id,
        style.block_stroke_color,
        style.block_stroke_width,
        style.block_fill_color
    );
    out.checkpoint()?;
    let _ = write!(out, r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    out.checkpoint()?;

    typography_receipt.record_successful_css_emission(
        font,
        [
            (byte_start_role, start_byte_fill),
            (byte_end_role, end_byte_fill),
            (label_role, label_fill),
            (title_role, title_fill),
        ],
    );
    Ok(())
}

pub(crate) fn render_packet_diagram_svg_model(
    layout: &PacketDiagramLayout,
    model: &PacketDiagramRenderModel,
    typography_theme: &crate::packet::PacketTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);
    let title_from_semantic = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let title_from_meta = diagram_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let terminal_title = title_from_semantic.or(title_from_meta);
    let label_role = crate::packet::PacketTextRole::Label;
    let byte_start_role = crate::packet::PacketTextRole::ByteStart;
    let byte_end_role = crate::packet::PacketTextRole::ByteEnd;
    let title_role = crate::packet::PacketTextRole::Title;
    let expected_label_count = layout
        .words
        .iter()
        .flat_map(|word| word.blocks.iter())
        .filter(|block| !block.label.trim().is_empty())
        .count();
    let expected_byte_start_count = if layout.show_bits {
        layout.words.iter().map(|word| word.blocks.len()).sum()
    } else {
        0
    };
    let expected_byte_end_count = if layout.show_bits {
        layout
            .words
            .iter()
            .flat_map(|word| word.blocks.iter())
            .filter(|block| block.start != block.end)
            .count()
    } else {
        0
    };

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
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "packet");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = root_svg::RootViewportContext::new(
        crate::DiagramFamilyId::PACKET,
        diagram_id,
    )
    .write_open(&mut out, root_spec, root_chrome)?;
    let mut surface_receipt: crate::packet::PacketSurfaceReceipt = typography_theme
        .begin_terminal_receipt([
            (label_role, expected_label_count),
            (byte_start_role, expected_byte_start_count),
            (byte_end_role, expected_byte_end_count),
            (title_role, usize::from(terminal_title.is_some())),
        ]);

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(t)
        );
        out.checkpoint()?;
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(d)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    write_packet_css(
        &mut out,
        diagram_id,
        effective_config,
        typography_theme,
        &mut surface_receipt,
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
                r#"<text x="{x}" y="{y}" class="{class}" dominant-baseline="middle" text-anchor="middle">{text}</text>"#,
                x = fmt(b.x + b.width / 2.0),
                y = fmt(b.y + b.height / 2.0),
                class = label_role.class_attribute(),
                text = escape_xml(&b.label)
            );

            if !layout.show_bits {
                out.checkpoint()?;
                surface_receipt.record_non_empty_text(
                    label_role,
                    label_role.class_attribute(),
                    &b.label,
                );
                continue;
            }
            let is_single_block = b.start == b.end;
            let bit_number_y = b.y - 2.0;
            let start_x = if is_single_block {
                b.x + b.width / 2.0
            } else {
                b.x
            };
            let start_anchor = if is_single_block { "middle" } else { "start" };
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="{class}" dominant-baseline="auto" text-anchor="{anchor}">{text}</text>"#,
                x = fmt(start_x),
                y = fmt(bit_number_y),
                class = byte_start_role.class_attribute(),
                anchor = start_anchor,
                text = b.start
            );
            if !is_single_block {
                let _ = write!(
                    &mut out,
                    r#"<text x="{x}" y="{y}" class="{class}" dominant-baseline="auto" text-anchor="end">{text}</text>"#,
                    x = fmt(b.x + b.width),
                    y = fmt(bit_number_y),
                    class = byte_end_role.class_attribute(),
                    text = b.end
                );
            }
            out.checkpoint()?;
            surface_receipt.record_non_empty_text(
                label_role,
                label_role.class_attribute(),
                &b.label,
            );
            surface_receipt
                .record_text_occurrence(byte_start_role, byte_start_role.class_attribute());
            if !is_single_block {
                surface_receipt
                    .record_text_occurrence(byte_end_role, byte_end_role.class_attribute());
            }
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
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="{class}">{text}</text>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
                class = title_role.class_attribute(),
                text = escape_xml(title)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="{class}"/>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
                class = title_role.class_attribute(),
            );
        }
    }
    out.checkpoint()?;
    if terminal_title.is_some() {
        surface_receipt.record_text_occurrence(title_role, title_role.class_attribute());
    }

    out.push_str("</svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if !typography_theme.record_terminal(surface_receipt, options.work_meter())? {
        return Err(crate::Error::InvalidModel {
            message: "Packet typography receipt was recorded more than once".to_string(),
        });
    }
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn packet_css_honors_mermaid_11_15_packet_style_options() {
        let config = merman_core::MermaidConfig::from_value(json!({
            "packet": {
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
            }
        }));
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let typography_theme =
            crate::packet::PacketTypographyThemePlan::resolve(None, &config, &work_meter)
                .expect("resolve baseline Packet typography");
        let mut typography_receipt = typography_theme.begin_terminal_receipt([
            (crate::packet::PacketTextRole::Label, 0),
            (crate::packet::PacketTextRole::ByteStart, 0),
            (crate::packet::PacketTextRole::ByteEnd, 0),
            (crate::packet::PacketTextRole::Title, 0),
        ]);
        let mut css = String::new();
        write_packet_css(
            &mut css,
            "pkt",
            config.as_value(),
            &typography_theme,
            &mut typography_receipt,
        )
        .expect("write Packet CSS");

        assert!(css.contains("#pkt .packetByte{font-size:11px;}"));
        assert!(css.contains("#pkt .packetByte.start{fill:#111111;}"));
        assert!(css.contains("#pkt .packetByte.end{fill:#222222;}"));
        assert!(css.contains("#pkt .packetLabel{fill:#333333;font-size:13px;}"));
        assert!(css.contains("#pkt .packetTitle{fill:#444444;font-size:15px;}"));
        assert!(css.contains("#pkt .packetBlock{stroke:#555555;stroke-width:2;fill:#666666;}"));
        assert!(typography_receipt.typography_stylesheet_verified());
    }
}
