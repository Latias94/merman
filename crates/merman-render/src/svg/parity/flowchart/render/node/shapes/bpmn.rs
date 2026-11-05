use super::super::emission::FlowchartNodeShapeEmissionReceipt;
use crate::diagram_theme::ResolvedStyleProperty;
use crate::flowchart::bpmn::{BpmnGeometry, BpmnShape, ICON_SIZE};
use crate::svg::parity::{escape_xml_display, fmt_display};

#[derive(Default)]
struct BpmnOptions<'a> {
    icon: Option<&'a str>,
    markers: &'a [&'a str],
    data_direction: Option<&'a str>,
    collection: bool,
    annotation_right: bool,
}

/// Only declarations selected by the prepared typed lane may reach these extra terminals.
#[derive(Default)]
struct BpmnTerminalStyle<'a> {
    fill: Option<&'a str>,
    stroke: Option<&'a str>,
    stroke_width: Option<&'a str>,
    stroke_dasharray: Option<&'a str>,
}

impl<'a> BpmnTerminalStyle<'a> {
    fn resolve(shape: BpmnShape, theme_style: &'a str) -> Self {
        let event = shape.event_rings().is_some();
        let annotation = shape == BpmnShape::Annotation;
        let mut style = Self::default();
        if !event && !annotation {
            return style;
        }
        // The prepared selection also excludes direct BPMN structural classDef owners. Do not
        // recover fields from the mixed source style or reinterpret rule/palette winners.
        for declaration in theme_style
            .split(';')
            .filter_map(crate::mermaid_style::parse_style_declaration)
        {
            let value = Some(declaration.source_value());
            match declaration.property() {
                "fill" if event => style.fill = value,
                "stroke" => style.stroke = value,
                // Event ring weights belong to BPMN notation even under a typed theme.
                "stroke-width" if annotation => style.stroke_width = value,
                "stroke-dasharray" => style.stroke_dasharray = value,
                _ => {}
            }
        }
        style
    }

    fn is_empty(&self) -> bool {
        self.fill.is_none()
            && self.stroke.is_none()
            && self.stroke_width.is_none()
            && self.stroke_dasharray.is_none()
    }

    fn write_declarations(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
    ) -> crate::Result<FlowchartNodeShapeEmissionReceipt> {
        let mut receipt = FlowchartNodeShapeEmissionReceipt::unverified();
        for (name, property, value) in [
            ("fill", ResolvedStyleProperty::Fill, self.fill),
            ("stroke", ResolvedStyleProperty::Stroke, self.stroke),
            (
                "stroke-width",
                ResolvedStyleProperty::StrokeWidth,
                self.stroke_width,
            ),
            (
                "stroke-dasharray",
                ResolvedStyleProperty::StrokeDasharray,
                self.stroke_dasharray,
            ),
        ] {
            if let Some(value) = value {
                let _ = write!(out, "{name}:{};", escape_xml_display(value));
                out.checkpoint()?;
                receipt = receipt.with_typed_paint_reach(property);
            }
        }
        Ok(receipt)
    }
}

pub(in crate::svg::parity::flowchart::render::node) fn render_bpmn(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    shape: BpmnShape,
) -> crate::Result<FlowchartNodeShapeEmissionReceipt> {
    let options = BpmnOptions {
        icon: common.node_icon.filter(|icon| !icon.is_empty()),
        ..Default::default()
    };
    let metrics = common.label_metrics(ctx, label);
    let geometry = shape.geometry(
        metrics,
        !label.text.is_empty(),
        options.icon.is_some(),
        !options.markers.is_empty(),
    );
    let receipt = write_shape(
        out,
        shape,
        geometry,
        common.style,
        common.theme_style,
        &options,
        |icon, size, index| {
            let scope = ctx
                .document_ids()
                .icon_scope(common.node_id)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!(
                        "missing prepared BPMN icon scope for node `{}`",
                        common.node_id
                    ),
                })?;
            let id_scope = crate::svg::icon_registry::IconIdScopePrefix::from_parts(
                &[scope.as_str()],
                ctx.work_meter,
            )?
            .scope_parts(&["-bpmn-", &index.to_string()], ctx.work_meter)?;
            crate::svg::icon_registry::bpmn_icon_svg(
                ctx.icon_registry,
                crate::svg::icon_registry::IconRenderRequest {
                    icon_name: icon,
                    width_px: size,
                    height_px: size,
                    fallback_prefix: Some("bpmn"),
                    extra_class: None,
                    id_scope,
                    effective_config: ctx.config,
                    work_meter: ctx.work_meter,
                },
            )
        },
    )?;
    label.dy = geometry.label_center_y;
    if shape == BpmnShape::Annotation {
        label.dx = if options.annotation_right { -3.0 } else { 3.0 };
    }
    Ok(receipt)
}

fn write_bounds(out: &mut impl crate::svg::parity::SvgOutput, geometry: BpmnGeometry) {
    let _ = write!(
        out,
        r#"<rect class="bpmn-bounds" x="{}" y="{}" width="{}" height="{}" opacity="0" aria-hidden="true"/>"#,
        fmt_display(-geometry.width / 2.0),
        fmt_display(-geometry.height / 2.0),
        fmt_display(geometry.width),
        fmt_display(geometry.height),
    );
}

fn write_shape(
    out: &mut impl crate::svg::parity::SvgOutput,
    shape: BpmnShape,
    geometry: BpmnGeometry,
    style: &str,
    theme_style: &str,
    options: &BpmnOptions<'_>,
    mut glyph: impl FnMut(&str, f64, usize) -> crate::Result<String>,
) -> crate::Result<FlowchartNodeShapeEmissionReceipt> {
    let terminal_style = BpmnTerminalStyle::resolve(shape, theme_style);
    let mut receipt = FlowchartNodeShapeEmissionReceipt::unverified();
    if shape != BpmnShape::Activity {
        write_bounds(out, geometry);
    }
    let style = escape_xml_display(style);
    if let Some((rings, stroke_width)) = shape.event_rings() {
        let _ = write!(out, r#"<g class="bpmn-event-body" style="{style}">"#);
        for ring in 0..rings {
            let _ = write!(
                out,
                r#"<circle class="bpmn-event-ring{}" r="{}" style="stroke-width: {}px;"#,
                if ring == 0 {
                    ""
                } else {
                    " bpmn-event-ring-inner"
                },
                fmt_display(18.0 - ring as f64 * 3.0),
                fmt_display(stroke_width),
            );
            receipt = terminal_style.write_declarations(out)?;
            out.push_str(r#""/>"#);
            out.checkpoint()?;
        }
        if let Some(icon) = options.icon {
            let svg = glyph(icon, ICON_SIZE, 0)?;
            let _ = write!(
                out,
                r#"<g class="bpmn-glyph" transform="translate(-8, -8)"><g>{svg}</g></g>"#
            );
        }
        out.push_str("</g>");
        out.checkpoint()?;
        return Ok(receipt);
    }
    match shape {
        BpmnShape::Gateway => {
            let _ = write!(
                out,
                r#"<g class="bpmn-gateway-body"><polygon class="bpmn-gateway-diamond" points="0,-25 25,0 0,25 -25,0" style="{style}"/>"#,
            );
            if let Some(icon) = options.icon {
                let svg = glyph(icon, ICON_SIZE + 2.0, 0)?;
                let _ = write!(
                    out,
                    r#"<g class="bpmn-glyph" transform="translate(-9, -9)"><g>{svg}</g></g>"#
                );
            }
            out.push_str("</g>");
        }
        BpmnShape::Activity => {
            let width = geometry.drawn_width;
            let height = geometry.drawn_height;
            let _ = write!(
                out,
                r#"<g class="bpmn-activity-body"><rect class="bpmn-activity-rect" x="{}" y="{}" width="{}" height="{}" rx="10" ry="10" style="{style}"/>"#,
                fmt_display(-width / 2.0),
                fmt_display(-height / 2.0),
                fmt_display(width),
                fmt_display(height),
            );
            if let Some(icon) = options.icon {
                let svg = glyph(icon, ICON_SIZE, 0)?;
                let _ = write!(
                    out,
                    r#"<g class="bpmn-activity-icon" transform="translate({}, {})"><g>{svg}</g></g>"#,
                    fmt_display(-width / 2.0 + 6.0),
                    fmt_display(-height / 2.0 + 6.0),
                );
            }
            if !options.markers.is_empty() {
                let step = ICON_SIZE - 2.0;
                let row_width = options.markers.len() as f64 * step;
                let _ = write!(
                    out,
                    r#"<g class="bpmn-activity-markers" transform="translate(0, {})">"#,
                    fmt_display(height / 2.0 - step - 3.0)
                );
                for (index, marker) in options.markers.iter().enumerate() {
                    let svg = glyph(marker, step, index + 1)?;
                    let _ = write!(
                        out,
                        r#"<g transform="translate({}, 0)"><g>{svg}</g></g>"#,
                        fmt_display(-row_width / 2.0 + index as f64 * step)
                    );
                    out.checkpoint()?;
                }
                out.push_str("</g>");
            }
            out.push_str("</g>");
        }
        BpmnShape::Data => {
            let _ = write!(
                out,
                r#"<g class="bpmn-data-body"><path class="bpmn-data-page" d="M-18,-25 H8 L18,-15 V25 H-18 Z" style="{style}"/><path class="bpmn-data-fold" d="M8,-25 V-15 H18"/>"#,
            );
            if let Some(direction @ ("input" | "output")) = options.data_direction {
                let _ = write!(
                    out,
                    r#"<path class="bpmn-data-arrow bpmn-data-arrow-{direction}" d="M-14,-16 h7 v-3 l5,4.5 -5,4.5 v-3 h-7 Z"/>"#,
                );
            }
            if options.collection {
                out.push_str(
                    r#"<path class="bpmn-data-collection" d="M-4,14 v9 M0,14 v9 M4,14 v9"/>"#,
                );
            }
            out.push_str("</g>");
        }
        BpmnShape::DataStore => {
            let _ = write!(
                out,
                r#"<g class="bpmn-data-body"><path class="bpmn-store-body" d="M-25,-19 a25,6 0 0 1 50,0 V19 a25,6 0 0 1 -50,0 Z" style="{style}"/>"#,
            );
            for offset in [0.0, 5.0, 10.0] {
                let _ = write!(
                    out,
                    r#"<path class="bpmn-store-rings" d="M-25,{} a25,6 0 0 0 50,0"/>"#,
                    fmt_display(-19.0 + offset)
                );
            }
            out.push_str("</g>");
        }
        BpmnShape::Annotation => {
            let half_width = geometry.width / 2.0;
            let half_height = geometry.height / 2.0;
            let (start, across, back) = if options.annotation_right {
                (half_width - 6.0, 6, -6)
            } else {
                (-half_width + 6.0, -6, 6)
            };
            let _ = write!(
                out,
                r#"<g class="bpmn-annotation-body"><path class="bpmn-annotation-bracket" d="M{},{} h{across} V{} h{back}""#,
                fmt_display(start),
                fmt_display(-half_height),
                fmt_display(half_height),
            );
            if !terminal_style.is_empty() {
                out.push_str(r#" style=""#);
                receipt = terminal_style.write_declarations(out)?;
                out.push('"');
            }
            out.push_str("/></g>");
        }
        BpmnShape::Start | BpmnShape::Intermediate | BpmnShape::Boundary | BpmnShape::End => {
            unreachable!()
        }
    }
    out.checkpoint()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(shape: BpmnShape, options: BpmnOptions<'_>) -> (String, Vec<(String, f64, usize)>) {
        let metrics = crate::text::TextMetrics {
            width: 32.921875,
            height: 24.0,
            line_count: 1,
        };
        let geometry = shape.geometry(
            metrics,
            true,
            options.icon.is_some(),
            !options.markers.is_empty(),
        );
        let mut out = String::from("<svg>");
        let mut glyphs = Vec::new();
        write_shape(
            &mut out,
            shape,
            geometry,
            "fill:#aabbcc;stroke:#112233",
            "",
            &options,
            |icon, size, index| {
                glyphs.push((icon.to_owned(), size, index));
                Ok("<svg/>".to_owned())
            },
        )
        .unwrap();
        out.push_str("</svg>");
        (out, glyphs)
    }

    #[test]
    fn bpmn_event_weights_and_glyph_placements_follow_the_source_dom() {
        for (shape, rings, weight) in [
            (BpmnShape::Start, 1, "1.6"),
            (BpmnShape::Intermediate, 2, "1.4"),
            (BpmnShape::Boundary, 2, "1.4"),
            (BpmnShape::End, 1, "3.4"),
        ] {
            let (svg, glyphs) = render(
                shape,
                BpmnOptions {
                    icon: Some("bpmn:message"),
                    ..Default::default()
                },
            );
            let document = crate::xml_dom::Document::parse(&svg).unwrap();
            let circles: Vec<_> = document
                .descendants()
                .filter(|node| node.has_tag_name("circle"))
                .collect();
            assert_eq!(circles.len(), rings);
            assert_eq!(circles[0].attribute("r"), Some("18"));
            assert!(circles.iter().all(|node| node.attribute("style")
                == Some(format!("stroke-width: {weight}px;").as_str())));
            assert_eq!(glyphs, [("bpmn:message".to_owned(), 16.0, 0)]);
            assert!(svg.contains(r#"transform="translate(-8, -8)""#));
            assert!(svg.contains(r#"opacity="0" aria-hidden="true""#));
        }
        let (gateway, glyphs) = render(
            BpmnShape::Gateway,
            BpmnOptions {
                icon: Some("bpmn:exclusive"),
                ..Default::default()
            },
        );
        assert!(gateway.contains(r#"points="0,-25 25,0 0,25 -25,0""#));
        assert_eq!(glyphs, [("bpmn:exclusive".to_owned(), 18.0, 0)]);
    }

    #[test]
    fn bpmn_internal_activity_markers_data_marks_and_attachment_face_are_composed() {
        let (activity, glyphs) = render(
            BpmnShape::Activity,
            BpmnOptions {
                icon: Some("bpmn:service"),
                markers: &["bpmn:loop", "bpmn:subprocess"],
                ..Default::default()
            },
        );
        assert!(activity.contains(r#"class="bpmn-activity-icon" transform="translate(-44, -34)""#));
        assert!(activity.contains(r#"class="bpmn-activity-markers" transform="translate(0, 23)""#));
        assert_eq!(
            glyphs,
            [
                ("bpmn:service".to_owned(), 16.0, 0),
                ("bpmn:loop".to_owned(), 14.0, 1),
                ("bpmn:subprocess".to_owned(), 14.0, 2),
            ]
        );
        for direction in ["input", "output"] {
            let (data, glyphs) = render(
                BpmnShape::Data,
                BpmnOptions {
                    data_direction: Some(direction),
                    collection: true,
                    ..Default::default()
                },
            );
            assert!(data.contains(&format!("bpmn-data-arrow-{direction}")));
            assert!(data.contains(r#"d="M-4,14 v9 M0,14 v9 M4,14 v9""#));
            assert!(glyphs.is_empty());
        }
        let (annotation, _) = render(
            BpmnShape::Annotation,
            BpmnOptions {
                annotation_right: true,
                ..Default::default()
            },
        );
        assert!(annotation.contains("h6 V20 h-6"));
        let (store, _) = render(BpmnShape::DataStore, BpmnOptions::default());
        assert_eq!(store.matches("bpmn-store-rings").count(), 3);
    }
}
