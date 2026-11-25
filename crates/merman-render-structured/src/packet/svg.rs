use merman_model::families::packet::PacketDiagramModel;
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    svg_css::{
        MermaidBaseCss, SvgDiagramIdValue, escape_css_identifier, write_mermaid_base_css_prefix,
    },
    svg_output::{SvgOutput, XmlTextOutput},
    xml::escape_xml,
};

use super::{PacketDiagramLayout, PacketStyle, config::PacketBitOrder, terminal_title};

fn write_packet_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    packet_style: &PacketStyle,
) -> Result<()> {
    // Keep `:root` last (matches upstream Mermaid packet SVG baselines).
    let id = escape_css_identifier(diagram_id);
    let font = &packet_style.font_family_css;
    let style = &packet_style.css_binding;
    let start_byte_fill = style.start_byte_color.as_ref();
    let end_byte_fill = style.end_byte_color.as_ref();
    let label_fill = style.label_color.as_ref();
    let title_fill = style.title_color.as_ref();
    let font_size_css = format!("{}px", packet_style.root_font_size_px);
    let native_typography = packet_style.native_typography.as_deref();
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

/// Writes Packet accessibility text, styles, and rows into an open SVG document.
///
/// The renderer supplies an admitted diagram ID for attributes and CSS selectors.
pub fn write_svg_contents(
    out: &mut impl SvgOutput,
    layout: &PacketDiagramLayout,
    model: &PacketDiagramModel,
    style: &PacketStyle,
    diagram_title: Option<&str>,
    diagram_id: impl SvgDiagramIdValue,
) -> Result<()> {
    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(t)
        );
        out.checkpoint()?;
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(d)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    write_packet_css(
        &mut XmlTextOutput::new(out),
        diagram_id.semantic_value(),
        style,
    )?;
    out.push_str("</style>");
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;

    for word in &layout.words {
        out.push_str("<g>");
        out.checkpoint()?;
        for b in &word.blocks {
            let _ = write!(
                out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="packetBlock"/>"#,
                x = fmt(b.x),
                y = fmt(b.y),
                w = fmt(b.width),
                h = fmt(b.height)
            );
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" class="packetLabel" dominant-baseline="middle" text-anchor="middle">{text}</text>"#,
                x = fmt(b.x + b.width / 2.0),
                y = fmt(b.y + b.height / 2.0),
                text = escape_xml(&b.label)
            );

            if !layout.show_bits {
                out.checkpoint()?;
                continue;
            }
            let (leading_bit, trailing_bit) = match style.bit_order {
                PacketBitOrder::Ascending => (b.start, b.end),
                PacketBitOrder::Descending => (b.end, b.start),
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
                out,
                r#"<text x="{x}" y="{y}" class="packetByte start" dominant-baseline="auto" text-anchor="{anchor}">{text}</text>"#,
                x = fmt(start_x),
                y = fmt(bit_number_y),
                anchor = start_anchor,
                text = ryu_js::Buffer::new().format(leading_bit)
            );
            if !is_single_block {
                let _ = write!(
                    out,
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

    let terminal_title = terminal_title(model, diagram_title);
    let total_row_height = layout.row_height + layout.padding_y;
    let title_y = layout.height - total_row_height / 2.0;
    match terminal_title {
        Some(title) => {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle">{text}</text>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
                text = escape_xml(title)
            );
        }
        None => {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="middle" text-anchor="middle" class="packetTitle"/>"#,
                x = fmt(layout.width / 2.0),
                y = fmt(title_y),
            );
        }
    }
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::presentation::PresentationConfig;
    use merman_model::resources::{InputResourcePolicy, ResourceProfile};
    use merman_model::{MermaidConfig, OperationControl};
    use serde_json::{Value, json};

    fn presentation_config(value: Value) -> PresentationConfig {
        PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(value),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn packet_css_honors_mermaid_packet_theme_variables() {
        let config = presentation_config(json!({
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
        let typography_theme = PacketStyle::resolve(None, &config);
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
