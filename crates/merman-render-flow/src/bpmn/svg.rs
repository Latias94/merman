//! Balanced SVG bodies and glyphs for Flowchart BPMN shapes.

use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Error, Result,
    icon_registry::{
        IconIdScopePrefix, IconPack, IconRegistry, IconRenderRequest, mermaid_unknown_icon_svg,
        parse_icon_reference, render_icon,
    },
    number_format::canonical_number as fmt_display,
    resources::OperationWorkMeter,
    svg_output::SvgOutput,
    xml::escape_attr_display,
};

use crate::document::FlowchartDocumentIds;

use super::{BpmnGeometry, BpmnShape, ICON_SIZE};

const BUILTIN_GLYPHS: &[u8] = include_bytes!("glyphs.json");

/// Measured label and admitted SVG context for one BPMN body.
pub struct BpmnSvgRequest<'a> {
    pub shape: BpmnShape,
    pub label_size: kurbo::Size,
    pub has_label: bool,
    pub style: &'a str,
    pub theme_style: &'a str,
    pub icon: Option<&'a str>,
    pub node_id: &'a str,
    pub document_ids: &'a FlowchartDocumentIds,
    pub icon_registry: Option<&'a IconRegistry>,
    pub config: &'a PresentationConfig,
    pub work_meter: &'a OperationWorkMeter,
}

impl BpmnSvgRequest<'_> {
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) -> Result<()> {
        let options = BpmnOptions {
            icon: self.icon.filter(|icon| !icon.is_empty()),
            ..Default::default()
        };
        let geometry = self.shape.geometry(
            self.label_size,
            self.has_label,
            options.icon.is_some(),
            !options.markers.is_empty(),
        );
        write_shape(
            out,
            self.shape,
            geometry,
            self.style,
            self.theme_style,
            &options,
            |icon, size, index| self.glyph(icon, size, index),
        )?;
        *label_dy = geometry.label_center_y;
        if self.shape == BpmnShape::Annotation {
            *label_dx = if options.annotation_right { -3.0 } else { 3.0 };
        }
        Ok(())
    }

    fn glyph(&self, icon: &str, size: f64, index: usize) -> Result<String> {
        let scope =
            self.document_ids
                .icon_scope(self.node_id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "missing prepared BPMN icon scope for node `{}`",
                        self.node_id
                    ),
                })?;
        let id_scope = IconIdScopePrefix::from_parts(&[scope.as_str()], self.work_meter)?
            .scope_parts(&["-bpmn-", &index.to_string()], self.work_meter)?;
        bpmn_icon_svg(
            self.icon_registry,
            IconRenderRequest {
                icon_name: icon,
                width_px: size,
                height_px: size,
                fallback_prefix: Some("bpmn"),
                id_scope,
                effective_config: self.config,
                work_meter: self.work_meter,
            },
        )
    }
}

#[derive(Default)]
struct BpmnOptions<'a> {
    icon: Option<&'a str>,
    markers: &'a [&'a str],
    data_direction: Option<&'a str>,
    collection: bool,
    annotation_right: bool,
}

fn write_bounds(out: &mut impl SvgOutput, geometry: BpmnGeometry) {
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
    out: &mut impl SvgOutput,
    shape: BpmnShape,
    geometry: BpmnGeometry,
    style: &str,
    theme_style: &str,
    options: &BpmnOptions<'_>,
    mut glyph: impl FnMut(&str, f64, usize) -> Result<String>,
) -> Result<()> {
    if shape != BpmnShape::Activity {
        write_bounds(out, geometry);
    }
    let style = escape_attr_display(style);
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
            // The event ring owns its notation width; a native palette can supply its fill.
            if !theme_style.is_empty() {
                let _ = write!(out, "{};", escape_attr_display(theme_style));
            }
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
        return Ok(());
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
            out.push_str("/></g>");
        }
        BpmnShape::Start | BpmnShape::Intermediate | BpmnShape::Boundary | BpmnShape::End => {
            unreachable!()
        }
    }
    out.checkpoint()?;
    Ok(())
}

fn bpmn_icon_svg(
    registry: Option<&IconRegistry>,
    request: IconRenderRequest<'_>,
) -> Result<String> {
    let width = request.width_px;
    let height = request.height_px;
    request
        .work_meter
        .charge_emit_work(request.icon_name.len())?;
    let reference = parse_icon_reference(request.icon_name, request.fallback_prefix);
    let use_builtin = reference.is_some_and(|(prefix, _)| prefix == "bpmn")
        && registry.is_none_or(|registry| !registry.is_pack_registered("bpmn"));
    let rendered = if use_builtin {
        static BUILTIN: std::sync::OnceLock<IconRegistry> = std::sync::OnceLock::new();
        let registry = BUILTIN.get_or_init(|| {
            IconRegistry::from_packs([IconPack::new(BUILTIN_GLYPHS)])
                .expect("source-owned BPMN glyphs are valid IconifyJSON")
        });
        render_icon(registry, request)?
    } else {
        registry
            .map(|registry| render_icon(registry, request))
            .transpose()?
            .flatten()
    };
    Ok(rendered.unwrap_or_else(|| mermaid_unknown_icon_svg(width, height)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::{
        MermaidConfig, OperationControl,
        resources::{InputResourcePolicy, ResourceProfile},
    };
    use merman_render_core::{
        resources::{RenderResourcePolicy, ResourceLimitId},
        svg_css::SvgId,
        svg_output::BoundedSvgOutput,
    };
    use quick_xml::{Reader, events::Event};

    use crate::document::FlowchartDocumentIdRequest;

    fn assert_event_rings(svg: &str, expected: usize, stroke_width: &str) {
        let mut reader = Reader::from_str(svg);
        let mut rings = 0;
        let style = format!("stroke-width: {stroke_width}px;");
        loop {
            match reader.read_event().unwrap() {
                Event::Empty(tag) if tag.name().as_ref() == "circle" => {
                    if rings == 0 {
                        let radius = tag.try_get_attribute("r").unwrap().unwrap();
                        assert_eq!(radius.value.as_ref(), "18");
                    }
                    let attr = tag.try_get_attribute("style").unwrap().unwrap();
                    assert_eq!(attr.value.as_ref(), style.as_str());
                    rings += 1;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(rings, expected);
    }

    fn render(shape: BpmnShape, options: BpmnOptions<'_>) -> (String, Vec<(String, f64, usize)>) {
        let geometry = shape.geometry(
            kurbo::Size::new(32.921875, 24.0),
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
            assert_event_rings(&svg, rings, weight);
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

    fn strict_config() -> PresentationConfig {
        PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(serde_json::json!({"securityLevel":"strict"})),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }

    fn bpmn_icon(
        registry: Option<&IconRegistry>,
        name: &str,
        policy: RenderResourcePolicy,
    ) -> Result<String> {
        let config = strict_config();
        let meter = OperationWorkMeter::new(policy);
        let scope_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        bpmn_icon_svg(
            registry,
            IconRenderRequest {
                icon_name: name,
                width_px: 18.0,
                height_px: 18.0,
                fallback_prefix: Some("bpmn"),
                id_scope: IconIdScopePrefix::from_parts(&["bpmn"], &scope_meter)
                    .unwrap()
                    .scope_parts(&[], &scope_meter)
                    .unwrap(),
                effective_config: &config,
                work_meter: &meter,
            },
        )
    }

    #[test]
    fn bpmn_glyphs_use_the_validated_registry_and_preserve_an_existing_pack() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let pack: serde_json::Value = serde_json::from_slice(BUILTIN_GLYPHS).unwrap();
        assert_eq!(pack["icons"].as_object().unwrap().len(), 29);
        for name in pack["icons"].as_object().unwrap().keys() {
            let svg = bpmn_icon(None, &format!("bpmn:{name}"), policy).unwrap();
            assert!(svg.contains("currentColor"), "{name}: {svg}");
            assert!(svg.contains(r#"viewBox="0 0 24 24""#));
            assert!(!svg.contains("ArialMT"));
        }
        let custom = br#"{"prefix":"custom","icons":{"message":{"body":"<path d=\"M1 2h3\"/>"}}}"#;
        let registry =
            IconRegistry::from_packs([IconPack::new(custom).with_registration_name("bpmn")])
                .unwrap();
        assert!(registry.is_pack_registered("bpmn"));
        let svg = bpmn_icon(Some(&registry), "message", policy).unwrap();
        assert!(svg.contains("M1 2h3"));
        assert!(!svg.contains("bpmn-glyph-fold"));
        let empty =
            IconRegistry::from_packs([IconPack::new(br#"{"prefix":"bpmn","icons":{}}"#)]).unwrap();
        assert!(empty.is_pack_registered("bpmn"));
        let missing = bpmn_icon(Some(&empty), "message", policy).unwrap();
        assert!(missing.contains("ArialMT"));
        assert!(missing.contains(r#"width="18" height="18""#));
        assert!(!missing.contains("bpmn-glyph-fold"));
    }

    #[test]
    fn bpmn_glyphs_cannot_bypass_svg_or_work_limits() {
        for limit in [
            ResourceLimitId::MaxSvgBytes,
            ResourceLimitId::MaxLayoutWorkUnits,
        ] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(limit, 1)
                .unwrap();
            assert!(matches!(
                bpmn_icon(None, "bpmn:message", policy),
                Err(Error::ResourceLimitExceeded(_))
            ));
        }
    }

    #[test]
    fn bpmn_event_checks_sink_before_a_missing_glyph_scope() {
        let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("diagram").unwrap(),
            edge_order: &[],
            node_dom_index: &Default::default(),
            subgraph_index_by_id: &Default::default(),
            layout_nodes: &[],
            swimlane_nodes: None,
            swimlane_lanes: None,
        });
        let config = strict_config();
        for shape in [BpmnShape::Intermediate, BpmnShape::Gateway] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                .unwrap();
            let work_meter = OperationWorkMeter::new(policy);
            let mut out = BoundedSvgOutput::new(&work_meter);
            let mut label_dx = 1.0;
            let mut label_dy = 2.0;
            let err = BpmnSvgRequest {
                shape,
                label_size: kurbo::Size::new(32.0, 24.0),
                has_label: true,
                style: "",
                theme_style: "",
                icon: Some("bpmn:message"),
                node_id: "absent",
                document_ids: &document_ids,
                icon_registry: None,
                config: &config,
                work_meter: &work_meter,
            }
            .write(&mut out, &mut label_dx, &mut label_dy)
            .unwrap_err();
            if shape == BpmnShape::Intermediate {
                assert!(matches!(err, Error::ResourceLimitExceeded(_)), "{err:?}");
            } else {
                assert!(matches!(err, Error::InvalidModel { ref message }
                    if message == "missing prepared BPMN icon scope for node `absent`"));
            }
            assert_eq!((label_dx, label_dy), (1.0, 2.0));
        }
    }
}
