use super::super::timing::RenderTimings;
use super::groups::ClassSplitEdgeGroupsRenderContext;
use super::nodes::{
    ClassNodesRenderContext, ClassNodesRenderState, render_class_elk_adapter_dom,
    render_class_render_tree,
};
use super::root::{CLASS_GRAPH_MARGIN_PX, begin_class_svg_document};
use super::settings::ClassRenderSettings;
use super::viewbox::{ClassViewBoxContext, class_viewbox};
use super::*;

pub(in crate::svg::parity) fn render_class_diagram_svg_model_with_config(
    layout: &ClassDiagramLayout,
    model: &ClassSvgModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_class_diagram_svg_model_inner(
        layout,
        model,
        effective_config.as_value(),
        Some(effective_config),
        diagram_title,
        measurer,
        options,
    )
}

fn render_class_diagram_svg_model_inner(
    layout: &ClassDiagramLayout,
    model: &ClassSvgModel,
    effective_config: &serde_json::Value,
    borrowed_sanitize_config: Option<&merman_core::MermaidConfig>,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let timing = options.timing();
    let total_timer = timing.start();
    let mut timings = RenderTimings::default();

    let mut detail = ClassRenderDetails::default();

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let aria_roledescription = model.diagram_type.as_str();
    let mut sanitize_config: Option<merman_core::MermaidConfig> = None;

    let build_ctx_guard = timing.section(&mut timings.build_ctx);
    let hand_drawn_seed = options.rough_randomness(
        effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(options.seed() as f64),
        "render.class.roughjs",
    );
    let settings = ClassRenderSettings::from_config(effective_config, hand_drawn_seed);

    // Mermaid's Dagre renderer applies fixed 8px graph margins. Its registered ELK renderer emits
    // the layout coordinates directly and keeps the viewport padding as the only outer margin.
    let content_tx = if layout.uses_elk_adapter_dom {
        0.0
    } else {
        CLASS_GRAPH_MARGIN_PX
    };
    let content_ty = content_tx;

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). We don't have a
    // browser DOM, so approximate the effective bbox by accumulating bounds for the elements we
    // emit (using the exact same `d` strings we output for paths).
    let mut content_bounds: Option<Bounds> = None;

    let render_guard = timing.section(&mut timings.render_svg);
    let estimated_svg_bytes = 2048usize
        + model.classes.len().saturating_mul(512)
        + model.relations.len().saturating_mul(384)
        + model.notes.len().saturating_mul(256)
        + model.namespaces.len().saturating_mul(128);
    let mut out = String::with_capacity(estimated_svg_bytes);
    let root_context =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Class, diagram_id);
    let document = begin_class_svg_document(
        &mut out,
        model,
        diagram_id,
        aria_roledescription,
        &root_context,
    )?;

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    let css = class_css(
        diagram_id,
        effective_config,
        settings
            .text_style
            .font_family
            .as_deref()
            .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif"),
        settings.font_size_css.as_str(),
    );
    out.push_str("<style>");
    out.push_str(&css);
    out.push_str("</style>");

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    // Mermaid 11.16 inserts both the ordinary and margin-aware marker variants for every look.
    class_markers(&mut out, diagram_id, aria_roledescription, true);
    if layout.uses_elk_adapter_dom {
        out.push_str("</g>");
        push_class_shadow_defs(&mut out, diagram_id, effective_config);
        push_class_gradient(&mut out, diagram_id, effective_config);
    }

    let ClassRenderLookups {
        class_nodes_by_id,
        relations_by_id,
        relation_index_by_id,
        note_by_id,
        iface_by_id,
    } = ClassRenderLookups::new(model);

    drop(build_ctx_guard);

    let marker_url_prefix = {
        let mut out = String::new();
        let _ = write!(&mut out, "{}", escape_attr_display(diagram_id));
        out.push('_');
        let _ = write!(&mut out, "{}", escape_attr_display(aria_roledescription));
        out.push('-');
        out
    };

    let terminal_text_style = TextStyle {
        font_family: settings.text_style.font_family.clone(),
        font_size: 11.0,
        font_weight: None,
        font_style: None,
    };
    let group_ctx = ClassSplitEdgeGroupsRenderContext {
        edges: &layout.edges,
        relations_by_id: &relations_by_id,
        relation_index_by_id: &relation_index_by_id,
        marker_url_prefix: &marker_url_prefix,
        diagram_id,
        content_tx,
        content_ty,
        edge_use_html_labels: settings.edge_use_html_labels,
        text_measurer: measurer,
        terminal_text_style: &terminal_text_style,
        mermaid_config: borrowed_sanitize_config,
        math_renderer: options.math_renderer(),
        look: settings.look.as_str(),
        hand_drawn_seed: settings.hand_drawn_seed.clone(),
        timing,
        edge_paths_class: if layout.uses_elk_adapter_dom {
            "edges edgePaths"
        } else {
            "edgePaths"
        },
    };

    // The layout-owned render tree preserves the exact recursive Dagre graph that produced these
    // coordinates. Rendering consumes that tree directly instead of inferring namespace extraction
    // and edge ownership a second time from flattened compatibility data.
    let nodes_start = timing.start();

    let nodes_ctx = ClassNodesRenderContext {
        layout,
        class_nodes_by_id: &class_nodes_by_id,
        note_by_id: &note_by_id,
        iface_by_id: &iface_by_id,
        settings: &settings,
        effective_config,
        diagram_id,
        measurer,
        mermaid_config: borrowed_sanitize_config,
        math_renderer: options.math_renderer(),
        content_tx,
        content_ty,
        timing,
    };
    if layout.uses_elk_adapter_dom {
        render_class_elk_adapter_dom(
            ClassNodesRenderState {
                out: &mut out,
                content_bounds: &mut content_bounds,
                detail: &mut detail,
                sanitize_config: &mut sanitize_config,
                borrowed_sanitize_config,
            },
            &nodes_ctx,
            &group_ctx,
        )?;
    } else {
        out.push_str(r#"<g class="root">"#);
        render_class_render_tree(
            ClassNodesRenderState {
                out: &mut out,
                content_bounds: &mut content_bounds,
                detail: &mut detail,
                sanitize_config: &mut sanitize_config,
                borrowed_sanitize_config,
            },
            &nodes_ctx,
            &group_ctx,
        )?;
        out.push_str("</g>"); // root
        out.push_str("</g>"); // wrapper
    }
    if let Some(s) = nodes_start {
        detail.nodes += s.elapsed();
    }

    // The Dagre renderer completes its graph wrapper before the shared resources are appended.
    // The registered ELK renderer receives those resources before it inserts its top-level groups.
    if !layout.uses_elk_adapter_dom {
        push_class_shadow_defs(&mut out, diagram_id, effective_config);
        push_class_gradient(&mut out, diagram_id, effective_config);
    }

    drop(render_guard);
    let viewbox_guard = timing.section(&mut timings.viewbox);

    let view_box = class_viewbox(ClassViewBoxContext {
        content_bounds,
        viewport_padding: settings.viewport_padding,
        diagram_title,
        diagram_title_bbox_x: diagram_title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(|title| {
                let title_style = TextStyle {
                    font_family: settings.text_style.font_family.clone(),
                    // Mermaid emits `classDiagramTitleText`, while the Class stylesheet's 18px
                    // rule targets `classTitleText`; the diagram title therefore inherits the
                    // root SVG font size.
                    font_size: settings.text_style.font_size,
                    font_weight: None,
                    font_style: None,
                };
                measurer.measure_svg_title_bbox_x(title, &title_style)
            }),
    });

    // Mermaid renders the diagram title as a direct child of `<svg>` (outside the wrapper `<g>`),
    // centered in the root viewport.
    if let Some(title) = view_box.title.as_ref() {
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="classDiagramTitleText">{}</text>"#,
            fmt(title.x),
            fmt(title.y),
            escape_xml_display(title.text)
        );
    }

    drop(viewbox_guard);
    let finalize_guard = timing.section(&mut timings.finalize_svg);

    let final_root_spec =
        root_svg::RootViewportSpec::responsive(root_svg::DiagramBounds::from_view_box(
            view_box.min_x,
            view_box.min_y,
            view_box.width,
            view_box.height,
        ))
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(view_box.width));
    let root_document = root_context.finish_document(&mut out, document.root, final_root_spec)?;

    out.push_str("</svg>");
    drop(finalize_guard);

    if let Some(s) = total_timer {
        timings.total = s.elapsed();
        emit_class_render_timing(&timings, &detail, layout);
    }
    root_document.complete(out)
}
