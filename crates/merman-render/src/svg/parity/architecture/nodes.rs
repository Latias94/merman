use std::fmt::Write as _;

use crate::architecture_metrics::ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX;
use crate::model::Bounds;
use crate::text::{TextMeasurer, VendoredFontMetricsTextMeasurer, WrapMode};

use super::super::{escape_xml_into, fmt};
use super::foreign_object::{
    escape_xml_ampersands_preserving_xml_entities, normalize_xhtml_fragment_for_foreign_object,
};
use super::geometry::{GroupRect, extend_bounds};
use super::icons::{
    arch_icon_needs_id_scope, write_arch_icon_svg, write_arch_icon_svg_with_registry,
};
use super::labels::{
    plain_ascii_words_single_line_width, svg_line_plain_text, wrap_svg_words_to_lines,
    write_architecture_service_title, write_svg_plain_ascii_words_text_line, write_svg_text_lines,
};
use super::model::ArchitectureModelAccess;
use super::settings::ArchitectureRenderSettings;

pub(super) struct ArchitectureNodeRenderContext<'a, M: ArchitectureModelAccess> {
    pub(super) out: &'a mut String,
    pub(super) diagram_id: &'a str,
    pub(super) model: &'a M,
    pub(super) node_xy: &'a rustc_hash::FxHashMap<&'a str, (f64, f64)>,
    pub(super) settings: &'a ArchitectureRenderSettings,
    pub(super) text_measurer: &'a VendoredFontMetricsTextMeasurer,
    pub(super) sanitize_config: &'a merman_core::MermaidConfig,
    pub(super) icon_registry: Option<&'a crate::svg::IconRegistry>,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) singleton_icon_text_service_id: Option<&'a str>,
}

fn write_diagram_service_id(out: &mut String, diagram_id: &str, service_id: &str) {
    escape_xml_into(out, diagram_id);
    out.push_str("-service-");
    escape_xml_into(out, service_id);
}

fn write_diagram_node_id(out: &mut String, diagram_id: &str, node_id: &str) {
    escape_xml_into(out, diagram_id);
    out.push_str("-node-");
    escape_xml_into(out, node_id);
}

pub(super) fn push_architecture_services_and_junctions<M: ArchitectureModelAccess>(
    ctx: &mut ArchitectureNodeRenderContext<'_, M>,
) {
    let out = &mut *ctx.out;
    let diagram_id = ctx.diagram_id;
    let model = ctx.model;
    let node_xy = ctx.node_xy;
    let settings = ctx.settings;
    let text_measurer = ctx.text_measurer;
    let sanitize_config = ctx.sanitize_config;
    let singleton_icon_text_service_id = ctx.singleton_icon_text_service_id;

    let service_count = model.services().count();
    let junction_count = model.junctions().count();

    if service_count == 0 && junction_count == 0 {
        out.push_str(r#"<g class="architecture-services"/>"#);
    } else {
        out.push_str(r#"<g class="architecture-services">"#);
        for svc in model.services() {
            let (x, y) = node_xy.get(svc.id).copied().unwrap_or((0.0, 0.0));
            let y = y + singleton_icon_text_offset_y(singleton_icon_text_service_id, svc.id);

            out.push_str(r#"<g id=""#);
            write_diagram_service_id(out, diagram_id, svc.id);
            let _ = write!(
                out,
                r#"" class="architecture-service" transform="translate({x},{y})">"#,
                x = fmt(x),
                y = fmt(y)
            );

            if let Some(title) = svc.title.map(str::trim).filter(|t| !t.is_empty()) {
                // Mermaid uses `width = iconSize * 1.5` for service titles.
                write_architecture_service_title(
                    out,
                    title,
                    settings.icon_size_px,
                    settings.icon_size_px * 1.5,
                    text_measurer,
                    &settings.text_style,
                );
            }

            out.push_str("<g>");
            match (svc.icon, svc.icon_text) {
                (Some(icon), _) => {
                    out.push_str("<g>");
                    if arch_icon_needs_id_scope(icon, ctx.icon_registry.is_some()) {
                        let id_scope = format!("{diagram_id}-service-{}-icon", svc.id);
                        write_arch_icon_svg_with_registry(
                            out,
                            icon,
                            settings.icon_size_px,
                            ctx.icon_registry,
                            &id_scope,
                        );
                    } else {
                        write_arch_icon_svg(out, icon, settings.icon_size_px, "");
                    }
                    out.push_str("</g>");
                }
                (None, Some(icon_text)) => {
                    out.push_str("<g>");
                    write_arch_icon_svg(out, "blank", settings.icon_size_px, "");
                    out.push_str("</g>");

                    // Mermaid computes `iconText` clamp from the DOM `font-size` applied to the
                    // foreignObject content. For Architecture this tracks `architecture.fontSize`,
                    // not the separate SVG text measurement/font-size path used for service/group
                    // labels.
                    let line_clamp = ((settings.icon_size_px - 2.0) / settings.arch_font_size_px)
                        .floor()
                        .max(1.0) as i64;
                    let sanitized =
                        merman_core::sanitize::sanitize_text(icon_text.trim(), sanitize_config);
                    let sanitized = normalize_xhtml_fragment_for_foreign_object(&sanitized);
                    let sanitized = escape_xml_ampersands_preserving_xml_entities(&sanitized);
                    let _ = write!(
                        out,
                        r#"<g><foreignObject width="{w}" height="{h}"><div class="node-icon-text" style="height: {h}px;" xmlns="http://www.w3.org/1999/xhtml"><div style="-webkit-line-clamp: {clamp};">{text}</div></div></foreignObject></g>"#,
                        w = fmt(settings.icon_size_px),
                        h = fmt(settings.icon_size_px),
                        clamp = line_clamp,
                        text = sanitized
                    );
                }
                (None, None) => {
                    out.push_str(r#"<path class="node-bkg" id=""#);
                    write_diagram_node_id(out, diagram_id, svc.id);
                    let _ = write!(
                        out,
                        r#"" d="M0,{s} V5 Q0,0 5,0 H{inner_s} Q{s},0 {s},5 V{s} Z"/>"#,
                        s = fmt(settings.icon_size_px),
                        inner_s = fmt(settings.icon_size_px - 5.0)
                    );
                }
            }
            out.push_str("</g>");

            out.push_str("</g>");
        }

        for junction in model.junctions() {
            let (x, y) = node_xy.get(junction.id).copied().unwrap_or((0.0, 0.0));

            let _ = write!(
                out,
                r#"<g class="architecture-junction" transform="translate({x},{y})"><g><rect id=""#,
                x = fmt(x),
                y = fmt(y),
            );
            write_diagram_node_id(out, diagram_id, junction.id);
            let _ = write!(
                out,
                r#"" fill-opacity="0" width="{s}" height="{s}"/></g></g>"#,
                s = fmt(settings.icon_size_px)
            );
        }
        out.push_str("</g>");
    }
}

pub(super) fn push_architecture_groups<'a, M: ArchitectureModelAccess>(
    ctx: &mut ArchitectureNodeRenderContext<'a, M>,
    group_rects: &[GroupRect<'a>],
) {
    let out = &mut *ctx.out;
    let settings = ctx.settings;
    let text_measurer = ctx.text_measurer;
    let content_bounds = &mut *ctx.content_bounds;

    if ctx.model.groups_len() == 0 {
        out.push_str(r#"<g class="architecture-groups"/>"#);
    } else {
        out.push_str(r#"<g class="architecture-groups">"#);

        for grp in group_rects {
            let x = grp.x;
            let y = grp.y;
            let w = grp.w;
            let h = grp.h;
            let group_icon_size_px = settings.padding_px * 0.75;
            let x1 = x - settings.half_icon;
            let y1 = y - settings.half_icon;

            out.push_str(r#"<rect id=""#);
            escape_xml_into(out, ctx.diagram_id);
            out.push_str("-group-");
            escape_xml_into(out, grp.id);
            let _ = write!(
                out,
                r#"" x="{x}" y="{y}" width="{w}" height="{h}" class="node-bkg"/>"#,
                x = fmt(x),
                y = fmt(y),
                w = fmt(w.max(1.0)),
                h = fmt(h.max(1.0))
            );

            out.push_str("<g>");

            let mut shifted_x1 = x1;
            let mut shifted_y1 = y1;
            if let Some(icon) = grp.icon.map(str::trim).filter(|t| !t.is_empty()) {
                let _ = write!(
                    out,
                    r#"<g transform="translate({x}, {y})"><g>"#,
                    x = fmt(shifted_x1 + settings.half_icon + 1.0),
                    y = fmt(shifted_y1 + settings.half_icon + 1.0)
                );
                if arch_icon_needs_id_scope(icon, ctx.icon_registry.is_some()) {
                    let id_scope = format!("{}-group-{}-icon", ctx.diagram_id, grp.id);
                    write_arch_icon_svg_with_registry(
                        out,
                        icon,
                        group_icon_size_px,
                        ctx.icon_registry,
                        &id_scope,
                    );
                } else {
                    write_arch_icon_svg(out, icon, group_icon_size_px, "");
                }
                out.push_str("</g></g>");
                shifted_x1 += group_icon_size_px;
                // Mermaid uses `architecture.fontSize` for this alignment tweak (not the global SVG
                // font size used for label rendering).
                shifted_y1 += settings.arch_font_size_px / 2.0 - 3.0;
            }

            if let Some(title) = grp.title.map(str::trim).filter(|t| !t.is_empty()) {
                let plain_title_width = plain_ascii_words_single_line_width(
                    title,
                    w,
                    text_measurer,
                    &settings.text_style,
                );
                let lines = if plain_title_width.is_some() {
                    None
                } else {
                    Some(wrap_svg_words_to_lines(
                        title,
                        w,
                        text_measurer,
                        &settings.text_style,
                    ))
                };

                // Group titles are SVG `<text>` (no explicit bbox geometry), so our SVG bbox pass
                // cannot "see" their extents. Union a conservative horizontal bbox so
                // `setupGraphViewbox(svg.getBBox() + padding)` matches upstream in parity-root.
                let title_bbox_w = if let Some(width) = plain_title_width {
                    width
                } else {
                    let lines = lines.as_ref().expect("group title lines");
                    let mut title_bbox_w = 0.0f64;
                    let has_multiline_title = lines.len() > 1;
                    for line in lines {
                        let s = svg_line_plain_text(line);
                        let m = text_measurer.measure_wrapped(
                            s.as_str(),
                            &settings.text_style,
                            None,
                            WrapMode::SvgLike,
                        );
                        // Chromium's SVG `getBBox()` reports multi-`tspan` title rows on an integer
                        // pixel boundary in upstream Architecture roots; keep this limited to wrapped
                        // group titles so ordinary service labels and one-line group titles retain the
                        // existing fractional text metric model.
                        let width = if has_multiline_title {
                            m.width.ceil()
                        } else {
                            m.width
                        };
                        title_bbox_w = title_bbox_w.max(width);
                    }
                    title_bbox_w
                };
                if title_bbox_w.is_finite() && title_bbox_w > 0.0 {
                    let title_x = shifted_x1 + settings.half_icon + 4.0;
                    // Keep Y extents within the group rect; we only need this to expand X.
                    let title_bounds = Bounds {
                        min_x: title_x,
                        min_y: y,
                        max_x: title_x + title_bbox_w,
                        max_y: y + h,
                    };
                    extend_bounds(content_bounds, title_bounds);
                }
                let _ = write!(
                    out,
                    r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="start" text-anchor="start" transform="translate({x}, {y})"><g><rect class="background" style="stroke: none"/>"#,
                    x = fmt(shifted_x1 + settings.half_icon + 4.0),
                    y = fmt(shifted_y1 + settings.half_icon + 2.0)
                );
                if plain_title_width.is_some() {
                    write_svg_plain_ascii_words_text_line(out, title);
                } else if let Some(lines) = lines.as_ref() {
                    write_svg_text_lines(out, lines);
                }
                out.push_str("</g></g>");
            }

            out.push_str("</g>");
        }

        out.push_str("</g>");
    }
}

fn singleton_icon_text_offset_y(
    singleton_icon_text_service_id: Option<&str>,
    service_id: &str,
) -> f64 {
    if singleton_icon_text_service_id == Some(service_id) {
        ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX
    } else {
        0.0
    }
}
