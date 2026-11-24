use crate::model::Bounds;
use crate::svg::icon_registry::{IconIdScope, IconIdScopePrefix};
use crate::text::TextMeasurer;
use merman_model::families::architecture::{
    ArchitectureDiagramModel, ArchitectureNodeKind, ArchitectureNodeModel,
};

use super::super::{SvgDiagramId, SvgOutput, escape_attr_into, escape_xml_into, fmt};
use super::ArchitectureEmitCheckpoints;
use super::geometry::{GroupRect, extend_bounds};
use super::icons::{
    arch_icon_needs_id_scope, write_arch_icon_svg, write_arch_icon_svg_with_registry,
};
#[cfg(feature = "layout-shoal")]
use super::labels::write_architecture_centered_text;
use super::labels::{write_architecture_service_title, write_svg_text_lines};
use super::settings::ArchitectureRenderSettings;
use crate::architecture::labels::{architecture_svg_text_terminal_bounds, wrap_svg_words_to_lines};

pub(super) struct ArchitectureNodeRenderContext<'a, O: SvgOutput> {
    pub(super) out: &'a mut O,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) model: &'a ArchitectureDiagramModel,
    #[cfg(feature = "layout-shoal")]
    pub(super) shoal: Option<&'a crate::architecture::ArchitectureShoalGeometry>,
    pub(super) node_xy: &'a rustc_hash::FxHashMap<&'a str, (f64, f64)>,
    pub(super) settings: &'a ArchitectureRenderSettings,
    pub(super) native: Option<&'a crate::architecture::ArchitectureNativeStyle>,
    pub(super) text_measurer: &'a dyn TextMeasurer,
    pub(super) sanitize_config: &'a merman_model::presentation::PresentationConfig,
    pub(super) icon_registry: Option<&'a crate::svg::IconRegistry>,
    pub(super) work_meter: &'a crate::resources::OperationWorkMeter,
    pub(super) icon_scope_root: Option<IconIdScopePrefix>,
    pub(super) service_icon_scope_prefix: Option<IconIdScopePrefix>,
    pub(super) group_icon_scope_prefix: Option<IconIdScopePrefix>,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) checkpoints: ArchitectureEmitCheckpoints<'a>,
}

fn architecture_service_has_icon_text(service: &ArchitectureNodeModel) -> bool {
    // Mermaid@12.3.0 svgDraw.ts uses string truthiness before sanitizing iconText:
    // an empty string falls back to the background, while whitespace selects the icon branch.
    service.icon.is_none()
        && service
            .icon_text
            .as_deref()
            .is_some_and(|text| !text.is_empty())
}

fn architecture_icon_scope(
    root: &mut Option<IconIdScopePrefix>,
    branch: &mut Option<IconIdScopePrefix>,
    diagram_id: SvgDiagramId<'_>,
    branch_name: &'static str,
    local_id: &str,
    work_meter: &crate::resources::OperationWorkMeter,
) -> crate::Result<IconIdScope> {
    let root_prefix = match *root {
        Some(prefix) => prefix,
        None => {
            let prefix = IconIdScopePrefix::from_parts(&[diagram_id.semantic_str()], work_meter)?;
            *root = Some(prefix);
            prefix
        }
    };
    let branch_prefix = match *branch {
        Some(prefix) => prefix,
        None => {
            let prefix = root_prefix.extend_parts(&[branch_name], work_meter)?;
            *branch = Some(prefix);
            prefix
        }
    };
    branch_prefix.scope_parts(&[local_id, "-icon"], work_meter)
}

fn write_diagram_service_id(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    service_id: &str,
) {
    let _ = write!(out, "{diagram_id}");
    out.push_str("-service-");
    escape_xml_into(out, service_id);
}

fn write_diagram_node_id(out: &mut impl SvgOutput, diagram_id: SvgDiagramId<'_>, node_id: &str) {
    let _ = write!(out, "{diagram_id}");
    out.push_str("-node-");
    escape_xml_into(out, node_id);
}

pub(super) fn push_architecture_services_and_junctions<O: SvgOutput>(
    ctx: &mut ArchitectureNodeRenderContext<'_, O>,
) -> crate::Result<()> {
    let checkpoints = ctx.checkpoints;
    checkpoints.checkpoint()?;
    let out = &mut *ctx.out;
    let diagram_id = ctx.diagram_id;
    let model = ctx.model;
    let node_xy = ctx.node_xy;
    let settings = ctx.settings;
    let native = ctx.native;
    let text_measurer = ctx.text_measurer;
    let sanitize_config = ctx.sanitize_config;
    let icon_registry = ctx.icon_registry;
    let work_meter = ctx.work_meter;
    let icon_scope_root = &mut ctx.icon_scope_root;
    let service_icon_scope_prefix = &mut ctx.service_icon_scope_prefix;
    if model.nodes.is_empty() {
        out.push_str(r#"<g class="architecture-services"/>"#);
    } else {
        out.push_str(r#"<g class="architecture-services">"#);
        for (_node_index, svc) in model
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.node_type == ArchitectureNodeKind::Service)
        {
            checkpoints.checkpoint()?;
            let (x, y) = node_xy.get(svc.id.as_str()).copied().unwrap_or((0.0, 0.0));

            out.push_str(r#"<g id=""#);
            write_diagram_service_id(out, diagram_id, &svc.id);
            checkpoints.checkpoint()?;
            let _ = write!(
                out,
                r#"" class="architecture-service" transform="translate({x},{y})""#,
                x = fmt(x),
                y = fmt(y)
            );
            #[cfg(feature = "layout-shoal")]
            if ctx.shoal.is_some() {
                super::write_geometry_attrs(out, "node", &svc.id);
            }
            out.push('>');

            if let Some(native) = native {
                let _ = write!(
                    out,
                    r#"<rect class="architecture-native-service-card" width="{size}" height="{size}" rx="{radius}" ry="{radius}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                    super::super::escape_attr_display(&native.node_fill),
                    super::super::escape_attr_display(&native.node_stroke),
                    fmt(native.stroke_width),
                    size = fmt(settings.icon_size_px),
                    radius = fmt(native.radius),
                );
                checkpoints.checkpoint()?;
            }

            let has_icon_text = architecture_service_has_icon_text(svc);
            let legacy_title = |out: &mut O| {
                if let Some(title) = svc
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                {
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
            };
            #[cfg(feature = "layout-shoal")]
            if let Some(geometry) = ctx.shoal {
                if let Some(title) = &geometry.service_titles[_node_index] {
                    write_architecture_centered_text(
                        out,
                        &title.text.lines,
                        title.anchor.x - x,
                        title.anchor.y - y,
                    );
                }
            } else {
                legacy_title(out);
            }
            #[cfg(not(feature = "layout-shoal"))]
            legacy_title(out);

            out.push_str("<g>");
            match (svc.icon.as_deref(), svc.icon_text.as_deref()) {
                (Some(icon), _) => {
                    out.push_str("<g>");
                    if arch_icon_needs_id_scope(icon, icon_registry.is_some()) {
                        let id_scope = architecture_icon_scope(
                            icon_scope_root,
                            service_icon_scope_prefix,
                            diagram_id,
                            "-service-",
                            &svc.id,
                            work_meter,
                        )?;
                        write_arch_icon_svg_with_registry(
                            out,
                            icon,
                            settings.icon_size_px,
                            icon_registry,
                            id_scope,
                            sanitize_config,
                            work_meter,
                            native,
                        )?;
                    } else {
                        write_arch_icon_svg(out, icon, settings.icon_size_px, None, native)?;
                    }
                    out.push_str("</g>");
                }
                (None, Some(icon_text)) if has_icon_text => {
                    out.push_str("<g>");
                    write_arch_icon_svg(out, "blank", settings.icon_size_px, None, native)?;
                    out.push_str("</g>");

                    // Mermaid computes `iconText` clamp from the DOM `font-size` applied to the
                    // foreignObject content. That content inherits the diagram root CSS size;
                    // `architecture.fontSize` remains a separate Cytoscape-layout setting.
                    // svgDraw.ts strips every nondigit before parseInt: 0.5px becomes 5.
                    // Apply that source quirk only to the clamp divisor, retaining actual
                    // fractional sizes for CSS and glyph measurement and zero-sized boxes.
                    let clamp_font_size = fmt(settings.text_style.font_size)
                        .to_string()
                        .chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<f64>()
                        .unwrap_or(f64::NAN);
                    let line_clamp = ((settings.icon_size_px - 2.0) / clamp_font_size).floor();
                    let line_clamp = if line_clamp == f64::INFINITY {
                        "Infinity".to_owned()
                    } else if line_clamp == f64::NEG_INFINITY {
                        "-Infinity".to_owned()
                    } else if line_clamp == 0.0 {
                        "0".to_owned()
                    } else {
                        line_clamp.to_string()
                    };
                    let parse_work = std::cell::Cell::new(0);
                    let sink = crate::html::HtmlSanitizeSink::new(
                        work_meter
                            .policy()
                            .value(crate::resources::ResourceLimitId::MaxSvgBytes)
                            .unwrap_or(usize::MAX),
                        work_meter,
                        &parse_work,
                        merman_model::OperationPhase::Emit,
                    );
                    let sanitized = crate::html::resolve_svg_fragment(
                        merman_markup::sanitize::sanitize_xhtml_in_svg_context_with_sink(
                            icon_text.trim(),
                            sanitize_config.as_config(),
                            &sink,
                        ),
                        work_meter,
                    )?;
                    let _ = write!(
                        out,
                        r#"<g><foreignObject width="{w}" height="{h}"><div class="node-icon-text" style="height: {h}px;" xmlns="http://www.w3.org/1999/xhtml"><div style="-webkit-line-clamp: {clamp};">"#,
                        w = fmt(settings.icon_size_px),
                        h = fmt(settings.icon_size_px),
                        clamp = line_clamp,
                    );
                    out.push_str(&sanitized);
                    out.push_str("</div></div></foreignObject></g>");
                }
                (None, _) if native.is_none() => {
                    out.push_str(r#"<path class="node-bkg" id=""#);
                    write_diagram_node_id(out, diagram_id, &svc.id);
                    checkpoints.checkpoint()?;
                    out.push('"');
                    let _ = write!(
                        out,
                        r#" d="M0,{s} V5 Q0,0 5,0 H{inner_s} Q{s},0 {s},5 V{s} Z"/>"#,
                        s = fmt(settings.icon_size_px),
                        inner_s = fmt(settings.icon_size_px - 5.0)
                    );
                }
                _ => {}
            }
            out.push_str("</g>");

            out.push_str("</g>");
            out.checkpoint()?;
        }

        for junction in model
            .nodes
            .iter()
            .filter(|node| node.node_type == ArchitectureNodeKind::Junction)
        {
            checkpoints.checkpoint()?;
            let (x, y) = node_xy
                .get(junction.id.as_str())
                .copied()
                .unwrap_or((0.0, 0.0));

            let _ = write!(
                out,
                r#"<g class="architecture-junction" transform="translate({x},{y})""#,
                x = fmt(x),
                y = fmt(y),
            );
            #[cfg(feature = "layout-shoal")]
            if ctx.shoal.is_some() {
                super::write_geometry_attrs(out, "node", &junction.id);
            }
            out.push_str(r#"><g><rect id=""#);
            write_diagram_node_id(out, diagram_id, &junction.id);
            checkpoints.checkpoint()?;
            let _ = write!(
                out,
                r#"" fill-opacity="0" width="{s}" height="{s}"/></g></g>"#,
                s = fmt(settings.icon_size_px)
            );
            out.checkpoint()?;
        }
        out.push_str("</g>");
    }
    out.checkpoint()?;
    Ok(())
}

fn push_architecture_group_surface(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    group: &GroupRect<'_>,
    group_inline_style: Option<&str>,
    radius: Option<f64>,
    #[cfg(feature = "layout-shoal")] shoal: bool,
) -> crate::Result<()> {
    let _ = write!(out, r#"<rect id="{diagram_id}-group-"#);
    escape_xml_into(out, group.id);
    let _ = write!(
        out,
        r#"" x="{x}" y="{y}" width="{w}" height="{h}" class="node-bkg""#,
        x = fmt(group.x),
        y = fmt(group.y),
        w = fmt(group.w.max(1.0)),
        h = fmt(group.h.max(1.0)),
    );
    if let Some(radius) = radius {
        let _ = write!(out, r#" rx="{radius}" ry="{radius}""#, radius = fmt(radius));
    }
    if let Some(style) = group_inline_style {
        out.push_str(r#" style=""#);
        escape_attr_into(out, style);
        out.push('"');
    }
    #[cfg(feature = "layout-shoal")]
    if shoal {
        super::write_geometry_attrs(out, "cluster", group.id);
    }
    out.push_str("/>");
    out.checkpoint()
}

pub(super) fn push_architecture_group_surfaces(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    group_rects: &[GroupRect<'_>],
    group_inline_style: Option<&str>,
    radius: Option<f64>,
    #[cfg(feature = "layout-shoal")] shoal: bool,
    checkpoints: ArchitectureEmitCheckpoints<'_>,
) -> crate::Result<()> {
    checkpoints.checkpoint()?;
    if group_rects.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="architecture-group-surfaces">"#);
    out.checkpoint()?;
    let mut pending: rustc_hash::FxHashMap<_, _> =
        group_rects.iter().map(|group| (group.id, *group)).collect();
    let mut ancestors = Vec::new();
    for group in group_rects {
        let mut next = Some(group.id);
        while let Some(group) = next.and_then(|id| pending.remove(id)) {
            checkpoints.checkpoint()?;
            next = group.parent;
            ancestors.push(group);
        }
        for group in ancestors.drain(..).rev() {
            push_architecture_group_surface(
                out,
                diagram_id,
                &group,
                group_inline_style,
                radius,
                #[cfg(feature = "layout-shoal")]
                shoal,
            )?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

pub(super) fn push_architecture_groups<'a, O: SvgOutput>(
    ctx: &mut ArchitectureNodeRenderContext<'a, O>,
    group_rects: &[GroupRect<'a>],
    emit_surfaces: bool,
) -> crate::Result<()> {
    let checkpoints = ctx.checkpoints;
    checkpoints.checkpoint()?;
    let out = &mut *ctx.out;
    let settings = ctx.settings;
    let text_measurer = ctx.text_measurer;
    let content_bounds = &mut *ctx.content_bounds;
    let icon_registry = ctx.icon_registry;
    let work_meter = ctx.work_meter;
    let icon_scope_root = &mut ctx.icon_scope_root;
    let group_icon_scope_prefix = &mut ctx.group_icon_scope_prefix;

    if ctx.model.groups.is_empty() {
        out.push_str(r#"<g class="architecture-groups"/>"#);
    } else {
        out.push_str(r#"<g class="architecture-groups">"#);

        for (_group_index, grp) in group_rects.iter().enumerate() {
            checkpoints.checkpoint()?;
            let x = grp.x;
            let y = grp.y;
            let w = grp.w;
            let h = grp.h;
            let group_icon_size_px = settings.padding_px * 0.75;
            let x1 = x - settings.half_icon;
            let y1 = y - settings.half_icon;
            #[cfg(feature = "layout-shoal")]
            let canonical = ctx.shoal.map(|geometry| &geometry.groups[_group_index]);

            if emit_surfaces {
                push_architecture_group_surface(
                    out,
                    ctx.diagram_id,
                    grp,
                    None,
                    None,
                    #[cfg(feature = "layout-shoal")]
                    false,
                )?;
            }
            out.push_str("<g>");

            let mut shifted_x1 = x1;
            let mut shifted_y1 = y1;
            let icon = grp.icon.map(str::trim).filter(|t| !t.is_empty());
            let legacy_icon_at = (x1 + settings.half_icon + 1.0, y1 + settings.half_icon + 1.0);
            #[cfg(feature = "layout-shoal")]
            let icon_at = canonical.map_or(Some(legacy_icon_at), |group| {
                group.icon_origin.map(|p| (p.x, p.y))
            });
            #[cfg(not(feature = "layout-shoal"))]
            let icon_at = Some(legacy_icon_at);
            if let Some((icon, (icon_x, icon_y))) = icon.zip(icon_at) {
                let _ = write!(
                    out,
                    r#"<g transform="translate({x}, {y})"><g>"#,
                    x = fmt(icon_x),
                    y = fmt(icon_y)
                );
                if arch_icon_needs_id_scope(icon, icon_registry.is_some()) {
                    let id_scope = architecture_icon_scope(
                        icon_scope_root,
                        group_icon_scope_prefix,
                        ctx.diagram_id,
                        "-group-",
                        grp.id,
                        work_meter,
                    )?;
                    write_arch_icon_svg_with_registry(
                        out,
                        icon,
                        group_icon_size_px,
                        icon_registry,
                        id_scope,
                        ctx.sanitize_config,
                        work_meter,
                        ctx.native,
                    )?;
                } else {
                    write_arch_icon_svg(out, icon, group_icon_size_px, None, ctx.native)?;
                }
                out.push_str("</g></g>");
                shifted_x1 += group_icon_size_px;
                // Mermaid uses `architecture.fontSize` for this alignment tweak (not the global SVG
                // font size used for label rendering).
                shifted_y1 += settings.arch_font_size_px / 2.0 - 3.0;
            }

            let legacy_lines;
            #[cfg(feature = "layout-shoal")]
            let placed_title = canonical.map(|group| {
                group.title.as_ref().map(|title| {
                    (
                        title.text.lines.as_slice(),
                        title.anchor.x,
                        title.anchor.y,
                        title.text.bounds_at(title.anchor),
                    )
                })
            });
            #[cfg(not(feature = "layout-shoal"))]
            let placed_title = None;
            let title_plan = if let Some(title) = placed_title {
                title
            } else if let Some(title) = grp.title.map(str::trim).filter(|title| !title.is_empty()) {
                legacy_lines =
                    wrap_svg_words_to_lines(title, w, text_measurer, &settings.text_style);
                let title_x = shifted_x1 + settings.half_icon + 4.0;
                let title_y = shifted_y1 + settings.half_icon + 2.0;
                let terminal_bounds = architecture_svg_text_terminal_bounds(
                    &legacy_lines,
                    title_x,
                    title_y,
                    text_measurer,
                    &settings.text_style,
                );
                Some((legacy_lines.as_slice(), title_x, title_y, terminal_bounds))
            } else {
                None
            };
            if let Some((lines, title_x, title_y, terminal_bounds)) = title_plan {
                // Group titles are SVG `<text>` (no explicit bbox geometry), so our SVG bbox pass
                // cannot "see" their extents. Measure the emitted outer tspan DOM primitive so
                // `setupGraphViewbox(svg.getBBox() + padding)` matches upstream in parity-root.
                if terminal_bounds.max_x > terminal_bounds.min_x {
                    // Keep Y extents within the group rect; we only need this to expand X.
                    let title_bounds = Bounds {
                        min_x: terminal_bounds.min_x,
                        min_y: y,
                        max_x: terminal_bounds.max_x,
                        max_y: y + h,
                    };
                    extend_bounds(content_bounds, title_bounds);
                }
                let _ = write!(
                    out,
                    r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="start" text-anchor="start" transform="translate({x}, {y})""#,
                    x = fmt(title_x),
                    y = fmt(title_y)
                );
                #[cfg(feature = "layout-shoal")]
                if canonical.is_some() {
                    super::write_geometry_attrs(out, "cluster-label", grp.id);
                }
                out.push_str(r#"><g><rect class="background" style="stroke: none"/>"#);
                write_svg_text_lines(out, lines);
                out.push_str("</g></g>");
            }

            out.push_str("</g>");
            out.checkpoint()?;
        }

        out.push_str("</g>");
    }
    out.checkpoint()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::parity::SvgExecution;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};

    #[test]
    fn architecture_group_surfaces_order_forward_parents_before_descendants() {
        let group = |id, parent| GroupRect {
            id,
            parent,
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
            icon: None,
            title: None,
        };
        let groups = [
            group("leaf", Some("inner")),
            group("outer", None),
            group("inner", Some("outer")),
        ];
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::ARCHITECTURE,
        )
        .unwrap();
        let mut svg = String::new();
        push_architecture_group_surfaces(
            &mut svg,
            execution.diagram_id_or("architecture"),
            &groups,
            Some("fill:#123456;"),
            None,
            #[cfg(feature = "layout-shoal")]
            false,
            ArchitectureEmitCheckpoints::new(execution.work_meter()),
        )
        .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let ids = document
            .root_element()
            .children()
            .filter_map(|node| node.attribute("id"))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "architecture-group-outer",
                "architecture-group-inner",
                "architecture-group-leaf",
            ]
        );
    }
}
