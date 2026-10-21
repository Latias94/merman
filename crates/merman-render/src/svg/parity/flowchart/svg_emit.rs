use super::defs::{FlowchartMarkerEmissionPlan, prepare_flowchart_defs};
use super::document::{FlowchartSvgDocumentRequest, prepare_flowchart_svg_document};
use super::document_ids::FlowchartDocumentIds;
use super::render_config::FlowchartRenderConfig;
use super::viewbox::{
    FlowchartRenderedBoundsRequest, FlowchartViewboxBounds, FlowchartViewboxBoundsRequest,
    prepare_flowchart_rendered_bounds, prepare_flowchart_viewbox_bounds,
};
use super::*;

pub(in crate::svg::parity) fn render_flowchart_svg_artifact(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_core::ParseMetadata,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_flowchart_svg_model(flowchart_request(artifact, metadata), options)
}

pub(super) fn flowchart_request<'a>(
    artifact: &'a crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &'a merman_core::ParseMetadata,
) -> FlowchartSvgModelRequest<'a> {
    FlowchartSvgModelRequest {
        layout: artifact.pair().layout(),
        swimlane_layout: None,
        model: artifact.pair().semantic(),
        render_context: artifact.render_context(),
        effective_config: &metadata.effective_config,
        diagram_type: metadata.diagram_type.as_str(),
        diagram_title: metadata.title.as_deref(),
        svg_label_sidecar: artifact.svg_label_sidecar(),
        theme_evidence: artifact.theme_evidence(),
        effect_evidence: artifact.effect_evidence(),
        expected_effect_applications: artifact.expected_effect_applications(),
        edge_style_plan: artifact.edge_style_plan(),
        edge_theme: artifact.edge_theme(),
        prepared_theme: artifact.prepared_theme(),
        prepared_nodes: artifact.prepared_nodes(),
        render_config: artifact.render_config(),
    }
}

pub(in crate::svg::parity) fn flowchart_edge_geometry(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_core::ParseMetadata,
    options: &SvgExecution<'_>,
) -> Result<Vec<crate::model::EdgePaintGeometry>> {
    super::paint::edge_geometry(flowchart_request(artifact, metadata), options)
}

#[derive(Clone, Copy)]
pub(super) struct FlowchartSvgModelRequest<'a> {
    pub(super) render_config: &'a FlowchartRenderConfig,
    pub(super) prepared_nodes: &'a super::node_inventory::FlowchartPreparedNodes,
    pub(super) prepared_theme: &'a crate::flowchart::FlowchartPreparedTheme,
    pub(super) layout: &'a FlowchartLayout,
    pub(super) swimlane_layout: Option<&'a crate::model::SwimlaneLayout>,
    pub(super) model: &'a crate::flowchart::FlowchartModel,
    pub(super) render_context: &'a crate::flowchart::FlowchartRenderContext,
    pub(super) effective_config: &'a merman_core::MermaidConfig,
    pub(super) diagram_type: &'a str,
    pub(super) diagram_title: Option<&'a str>,
    pub(super) svg_label_sidecar: &'a crate::flowchart::FlowchartSvgLabelSidecar,
    pub(super) effect_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) expected_effect_applications: &'a std::cell::Cell<usize>,
    pub(super) theme_evidence: &'a crate::flowchart::FlowchartThemeEvidenceRecorder,
    pub(super) edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub(super) edge_theme: &'a crate::flowchart::FlowchartEdgeThemeStyle,
}

pub(super) fn render_flowchart_svg_model(
    request: FlowchartSvgModelRequest<'_>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let FlowchartSvgModelRequest {
        render_config,
        prepared_nodes: _,
        prepared_theme,
        layout,
        swimlane_layout,
        model,
        render_context: _,
        effective_config,
        diagram_type,
        diagram_title,
        svg_label_sidecar,
        theme_evidence,
        effect_evidence: _,
        expected_effect_applications,
        edge_style_plan: _,
        edge_theme: _,
    } = request;
    super::paint::with_flowchart_paint_context(
        request,
        options,
        |ctx, render_edges, hierarchy_plan| {
            let render_timing = options.timing();
            let mut timings = timing::RenderTimings::default();
            let total_timer = render_timing.start();
            let effective_config_value = effective_config.as_value();
            let document_ids = ctx.document_ids();
            let diagram_id = ctx.diagram_id;
            let font_family = &render_config.font_family;
            let font_size = render_config.font_size;
            let diagram_padding = render_config.diagram_padding;
            let use_max_width = render_config.use_max_width;
            let title_top_margin = render_config.title_top_margin;
            let text_surface_paint = &prepared_theme.text_surface;
            text_surface_paint.begin_terminal_emission(ctx.work_meter)?;
            let marker_plan = FlowchartMarkerEmissionPlan::prepare(ctx, hierarchy_plan)?;
            let theme_resource_policy = options.theme_resource_policy();
            let node_effects = node_effect::FlowchartNodeEffects::prepare(
                ctx,
                hierarchy_plan,
                &theme_resource_policy,
            )?;
            expected_effect_applications.set(node_effects.expected_applications());
            ctx.node_effects
                .set(node_effects)
                .expect("node effects are prepared once");

            let mut edge_path_cache: FxHashMap<
                crate::flowchart::FlowchartEdgeKey,
                FlowchartEdgePathCacheEntry,
            > = FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());

            let subgraph_title_y_shift =
                crate::flowchart::FlowchartConfigView::new(effective_config_value)
                    .render_subgraph_title_y_shift();

            let mut detail = FlowchartRenderDetails::default();
            let mut viewbox_edge_curve_bounds = std::time::Duration::ZERO;
            let _g_viewbox = render_timing.section(&mut timings.viewbox);

            let bounds = prepare_flowchart_rendered_bounds(
                FlowchartRenderedBoundsRequest {
                    ctx,
                    layout,
                    subgraph_title_y_shift,
                },
                hierarchy_plan,
            )?;
            let FlowchartViewboxBounds {
                diagram_title,
                title_anchor_x,
                source_paint_bounds,
                bbox_min_x,
                bbox_min_y,
                bbox_max_x,
                bbox_max_y,
            } = prepare_flowchart_viewbox_bounds(
                FlowchartViewboxBoundsRequest {
                    ctx,
                    render_edges,
                    base_bounds: bounds,
                    diagram_title,
                    font_family,
                    title_top_margin,
                    timing: render_timing,
                    theme_resource_policy: &theme_resource_policy,
                    marker_plan: &marker_plan,
                    viewbox_edge_curve_bounds: &mut viewbox_edge_curve_bounds,
                    detail: &mut detail,
                    edge_path_cache: &mut edge_path_cache,
                },
                hierarchy_plan,
            )?;

            expected_effect_applications.set(
                expected_effect_applications.get()
                    + ctx
                        .label_effects
                        .get()
                        .map_or(0, |effects| effects.expected_applications())
                    + ctx
                        .edge_effects
                        .get()
                        .map_or(0, |effects| effects.expected_applications()),
            );

            if !ctx.hand_drawn_seed.seed().may_use_math_random() {
                super::paint::prepare_rough_edge_paths(
                    ctx,
                    hierarchy_plan.ordered_edges(),
                    &mut edge_path_cache,
                )?;
            }
            let mut edge_labels =
                super::paint::FlowchartEdgeLabels::prepare(ctx, hierarchy_plan, &edge_path_cache)?;
            let document = prepare_flowchart_svg_document(FlowchartSvgDocumentRequest {
                family_id: if swimlane_layout.is_some() {
                    crate::DiagramFamilyId::SWIMLANE
                } else if diagram_type == "agentflow" {
                    crate::DiagramFamilyId::AGENTFLOW
                } else {
                    crate::DiagramFamilyId::FLOWCHART
                },
                diagram_id,
                diagram_type,
                model,
                document_ids,
                use_max_width,
                diagram_padding,
                source_paint_bounds,
                bbox_min_x,
                bbox_min_y,
                bbox_max_x,
                bbox_max_y,
            });

            drop(_g_viewbox);
            let _g_render_svg = render_timing.section(&mut timings.render_svg);

            let mut out = BoundedSvgOutput::new(options.work_meter());

            if let Some(base_typography) = svg_label_sidecar
                .base_typography()
                .filter(|plan| plan.requires_terminal_evidence())
            {
                base_typography.begin_terminal_emission();
            }

            let root_document = document.push_root_open(&mut out)?;
            document.push_accessibility_metadata(&mut out);
            out.push_str("<style>");
            out.checkpoint()?;
            {
                let mut css_out = super::super::output::XmlTextOutput::new(&mut out);
                write_flowchart_css(
                    &mut css_out,
                    diagram_id,
                    diagram_type,
                    document_ids.drop_shadow(),
                    document_ids.drop_shadow_small(),
                    document_ids.root_gradient(),
                    &prepared_theme.compatibility,
                    font_family,
                    font_size,
                    &model.class_defs,
                    Some(text_surface_paint),
                )?;
                if diagram_type == "agentflow" {
                    super::agentflow::write_css(
                        &mut css_out,
                        diagram_id,
                        &prepared_theme.compatibility,
                    )?;
                }
                text_surface_paint.generic_text.record_stylesheet_emission();
                text_surface_paint.background.record_stylesheet();
                if swimlane_layout.is_some() {
                    super::swimlane::write_swimlane_css(
                        &mut css_out,
                        diagram_id,
                        &prepared_theme.compatibility.cluster_border,
                    );
                }
            }
            out.push_str("</style>");
            out.checkpoint()?;
            if let Some(base_typography) = svg_label_sidecar
                .base_typography()
                .filter(|plan| plan.requires_terminal_evidence())
            {
                base_typography.record_stylesheet_emission(font_family, font_size);
            }

            let defs = prepare_flowchart_defs(
                document_ids.marker_scope(),
                diagram_type,
                ctx,
                &marker_plan,
            );

            let mut root_session = FlowchartRootRenderSession {
                timing: render_timing,
                details: &mut detail,
                edge_cache: &mut edge_path_cache,
                hierarchy_plan,
                marker_plan: &marker_plan,
                edge_labels: &mut edge_labels,
            };
            if layout.uses_elk_adapter_dom {
                out.push_str("<g>");
                defs.push_base_markers(&mut out)?;
                out.checkpoint()?;
                defs.push_extra_markers(&mut out)?;
                out.checkpoint()?;
                render_flowchart_elk_root_groups(&mut out, ctx, &mut root_session)?;
                out.push_str("</g>");
                // The shadow filters are siblings of Mermaid's marker/root wrapper,
                // rather than children of the wrapper that owns the painted graph.
                push_flowchart_shadow_defs(
                    &mut out,
                    document_ids,
                    &prepared_theme.compatibility.look_defs,
                );
                out.checkpoint()?;
            } else {
                push_flowchart_shadow_defs(
                    &mut out,
                    document_ids,
                    &prepared_theme.compatibility.look_defs,
                );
                out.checkpoint()?;
                out.push_str("<g>");
                defs.push_base_markers(&mut out)?;
                render_flowchart_root(&mut out, ctx, None, 0.0, 0.0, &mut root_session)?;

                defs.push_extra_markers(&mut out)?;
                out.push_str("</g>");
                out.checkpoint()?;
            }
            push_flowchart_gradient(
                &mut out,
                document_ids,
                &prepared_theme.compatibility.look_defs,
            );
            out.checkpoint()?;
            if let Some(title) = diagram_title.as_deref() {
                let title_x = title_anchor_x;
                let title_y = -title_top_margin;
                let _ = write!(
                    &mut out,
                    r#"<text text-anchor="middle" x="{}" y="{}" class="{}">{}</text>"#,
                    fmt(title_x),
                    fmt(title_y),
                    title_css_class(diagram_type),
                    escape_xml(title)
                );
                text_surface_paint.generic_text.record_label(
                    crate::flowchart::FlowchartTextPaintChannel::DiagramTitle,
                    crate::flowchart::FlowchartTextPaintFacts::from_visible(
                        &crate::text::VisibleTextStyleFacts::plain_text(title),
                    ),
                    crate::flowchart::FlowchartSourceFacetStatus::Absent,
                    ctx.work_meter,
                )?;
                if !title.trim().is_empty() {
                    ctx.record_base_typography_label_emission(
                        crate::flowchart::FlowchartBaseTypographyLabelEmission::diagram_title(),
                    );
                }
            }
            out.push_str("</svg>\n");
            out.checkpoint()?;

            drop(_g_render_svg);
            timings.total = total_timer
                .map(merman_core::runtime::OperationTimer::elapsed)
                .unwrap_or_default();
            if render_timing.is_enabled() {
                eprintln!(
                    "[render-timing] diagram=flowchart-v2 total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} viewbox_edge_curve_bounds={:?} viewbox_edge_root_lookup={:?} viewbox_edge_curve_offsets={:?} viewbox_edge_curve_geom={:?} viewbox_edge_curve_bbox_union={:?} viewbox_edge_curve_geom_calls={} viewbox_edge_curve_geom_skipped_bounds={} render_svg={:?} finalize={:?} root_calls={} clusters={:?} edges_select={:?} edge_paths={:?} edge_labels={:?} dom_order={:?} nodes={:?} node_style_compile={:?} node_roughjs={:?} node_roughjs_calls={} node_label_html={:?} node_label_html_calls={} nested_roots={:?}",
                    timings.total,
                    timings.deserialize_model,
                    timings.build_ctx,
                    timings.viewbox,
                    viewbox_edge_curve_bounds,
                    detail.viewbox_edge_root_lookup,
                    detail.viewbox_edge_curve_offsets,
                    detail.viewbox_edge_curve_geom,
                    detail.viewbox_edge_curve_bbox_union,
                    detail.viewbox_edge_curve_geom_calls,
                    detail.viewbox_edge_curve_geom_skipped_bounds,
                    timings.render_svg,
                    timings.finalize_svg,
                    detail.root_calls,
                    detail.clusters,
                    detail.edges_select,
                    detail.edge_paths,
                    detail.edge_labels,
                    detail.dom_order,
                    detail.nodes,
                    detail.node_style_compile,
                    detail.node_roughjs,
                    detail.node_roughjs_calls,
                    detail.node_label_html,
                    detail.node_label_html_calls,
                    detail.nested_roots,
                );
            }
            let rooted = root_document.complete(out.finish()?)?;
            if let Some(evidence) =
                marker_plan.finish_theme_evidence(ctx.resolved_theme, ctx.work_meter)?
            {
                theme_evidence.record_marker_evidence(evidence);
            }
            Ok(rooted)
        },
    )
}

pub(super) fn flowchart_node_theme_ordinals<'a>(
    nodes: &'a [crate::flowchart::FlowNode],
    subgraphs: &'a [crate::flowchart::FlowSubgraph],
    uses_elk_adapter_dom: bool,
) -> FxHashMap<&'a str, usize> {
    let subgraph_ids: FxHashSet<&str> = subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let mut ordinals = FxHashMap::default();
    let mut next_ordinal = 1usize;

    // Theme ordinals describe surfaces that reach the Node emitter, not Graphlib/ELK vertices.
    // ELK materializes every subgraph as a node in the semantic model, but renders it as a
    // cluster. Dagre renders only empty subgraphs through the ordinary Node surface.
    for node in nodes {
        if subgraph_ids.contains(node.id.as_str()) {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = ordinals.entry(node.id.as_str()) {
            entry.insert(next_ordinal);
            next_ordinal += 1;
        }
    }

    if !uses_elk_adapter_dom {
        for subgraph in subgraphs
            .iter()
            .filter(|subgraph| subgraph.nodes.is_empty())
        {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                ordinals.entry(subgraph.id.as_str())
            {
                entry.insert(next_ordinal);
                next_ordinal += 1;
            }
        }
    }

    ordinals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> crate::flowchart::FlowNode {
        crate::flowchart::FlowNode {
            id: id.to_string(),
            provenance: Default::default(),
            label: None,
            label_type: None,
            layout_shape: None,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    fn subgraph(id: &str, nodes: &[&str]) -> crate::flowchart::FlowSubgraph {
        crate::flowchart::FlowSubgraph {
            id: id.to_string(),
            title: id.to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: nodes.iter().map(|node| (*node).to_string()).collect(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn node_theme_ordinals_follow_actual_node_emitter_surfaces() {
        let nodes = vec![node("A"), node("group"), node("A"), node("empty")];
        let subgraphs = vec![subgraph("group", &["A"]), subgraph("empty", &[])];

        let dagre = flowchart_node_theme_ordinals(&nodes, &subgraphs, false);
        assert_eq!(dagre.len(), 2);
        assert_eq!(dagre.get("A"), Some(&1));
        assert_eq!(dagre.get("empty"), Some(&2));
        assert!(!dagre.contains_key("group"));

        let elk = flowchart_node_theme_ordinals(&nodes, &subgraphs, true);
        assert_eq!(elk.len(), 1);
        assert_eq!(elk.get("A"), Some(&1));
        assert!(!elk.contains_key("group"));
        assert!(!elk.contains_key("empty"));
    }
}

fn push_flowchart_shadow_defs(
    out: &mut impl crate::svg::parity::SvgOutput,
    document_ids: &FlowchartDocumentIds,
    prepared: &crate::svg::PreparedLookDefs,
) {
    let flood_color = prepared.flood_color();
    let offset = super::super::look_defs::NEO_SHADOW_OFFSET_PX;
    let _ = write!(
        out,
        r#"<defs><filter id="{}" height="130%" width="130%"><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        document_ids.drop_shadow(),
        flood_color,
        document_ids.drop_shadow_small(),
        flood_color
    );
}

fn push_flowchart_gradient(
    out: &mut impl crate::svg::parity::SvgOutput,
    document_ids: &FlowchartDocumentIds,
    prepared: &crate::svg::PreparedLookDefs,
) {
    let Some((gradient_start, gradient_stop)) = prepared.gradient() else {
        return;
    };

    let gradient_start = escape_xml(gradient_start);
    let gradient_stop = escape_xml(gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        document_ids.root_gradient(),
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}
#[cfg(test)]
mod integration_tests {
    use super::super::render_input::prepare_flowchart_render_inputs;
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::model::{FlowchartLayout, LayoutNode};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};

    #[test]
    fn unmerged_self_loop_occurrences_keep_source_owner_label_and_first_slot() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA -->|first| A\nA -->|regular| B\nA -->|last| A\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let render_context = parsed.flowchart_render_context().unwrap().clone();
        let (_, semantic) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(mut model) = semantic else {
            panic!("expected Flowchart model");
        };
        model.keyword = "agentflow-beta".to_owned();
        // Authored ids may overlap generated helper spellings without sharing an owner.
        model.edges[1].id = "A-cyclic-special-1".to_owned();
        let owners = crate::flowchart::FlowchartEdgeOwners::new(vec![
            crate::flowchart::FlowchartEdgeKey::self_loop_segment(2, 0),
            crate::flowchart::FlowchartEdgeKey::self_loop_segment(2, 1),
            crate::flowchart::FlowchartEdgeKey::self_loop_segment(2, 2),
            crate::flowchart::FlowchartEdgeKey::new(1),
        ]);
        let inputs = prepare_flowchart_render_inputs(&model, &render_context, &owners, false);
        let render_model = crate::flowchart::FlowchartRenderModelRef::new(&model, &render_context);
        assert_eq!(
            inputs
                .render_edges
                .iter()
                .map(|edge| edge.key)
                .collect::<Vec<_>>(),
            owners.iter().collect::<Vec<_>>()
        );
        assert_eq!(inputs.extra_nodes.len(), 2);
        let labels = inputs
            .render_edges
            .iter()
            .map(|edge| render_model.edge_label_for_occurrence(edge.key, &edge.edge))
            .collect::<Vec<_>>();
        assert_eq!(labels, [None, Some("last"), None, Some("regular")]);
        assert_eq!(
            inputs.render_edges[0].edge.id,
            inputs.render_edges[3].edge.id
        );
        assert_ne!(inputs.render_edges[0].key, inputs.render_edges[3].key);
    }

    #[test]
    fn diagram_id_terminal_precedes_later_flowchart_node_emission_error() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA@{ img: \"https://example.invalid/a.svg\", label: \"A\" }\n",
                ParseOptions::strict(),
            )
            .expect("parse succeeds")
            .expect("detects Flowchart");
        let render_context = parsed
            .flowchart_render_context()
            .expect("Flowchart render context")
            .clone();
        let (metadata, semantic) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(mut model) = semantic else {
            panic!("expected Flowchart model");
        };
        let node = model.nodes.first_mut().expect("fixture node");
        assert_eq!(node.layout_shape.as_deref(), Some("imageSquare"));
        node.img = None;

        let layout = FlowchartLayout {
            nodes: vec![LayoutNode {
                id: node.id.clone(),
                x: 40.0,
                y: 30.0,
                width: 80.0,
                height: 60.0,
                is_cluster: false,
                label_width: Some(10.0),
                label_height: Some(10.0),
            }],
            edges: Vec::new(),
            edge_owners: Default::default(),
            clusters: Vec::new(),
            bounds: None,
            dom_node_order_by_root: std::collections::HashMap::from([(
                String::new(),
                vec![node.id.clone()],
            )]),
            uses_elk_adapter_dom: false,
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .expect("valid SVG byte limit");
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions {
            diagram_id: Some("terminal".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::FLOWCHART,
        )
        .expect("SVG execution");
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &metadata.effective_config,
            super::render_config::flowchart_node_label_fill_config_override(
                &metadata.effective_config,
            ),
            execution.work_meter(),
        )
        .expect("prepared theme");
        let render_config = super::render_config::prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &prepared_theme.compatibility,
            layout.uses_elk_adapter_dom,
            sidecar.base_typography(),
            sidecar.edge_label_padding(),
        );
        let edge_style_plan = FlowchartEdgeStylePlan::prepare_for_model(
            &model,
            &metadata.effective_config,
            false,
            execution.work_meter(),
        )
        .expect("edge style plan")
        .with_resolved_stroke_widths(
            &model,
            &Default::default(),
            &metadata.effective_config,
            &prepared_theme.compatibility,
            execution.work_meter(),
        )
        .expect("prepared stroke widths")
        .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
        edge_style_plan
            .bind_terminal_typography(&render_config.text_style, false)
            .expect("terminal edge typography");
        let prepared_nodes = super::node_inventory::FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            super::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &metadata.effective_config,
            &prepared_theme,
            &render_config,
            session.work_meter(),
        )
        .expect("prepared nodes");

        let error = render_flowchart_svg_model(
            FlowchartSvgModelRequest {
                render_config: &render_config,
                prepared_theme: &prepared_theme,
                prepared_nodes: &prepared_nodes,
                layout: &layout,
                swimlane_layout: None,
                model: &model,
                render_context: &render_context,
                effective_config: &metadata.effective_config,
                diagram_type: metadata.diagram_type.as_str(),
                diagram_title: metadata.title.as_deref(),
                theme_evidence: &Default::default(),
                effect_evidence: &Default::default(),
                expected_effect_applications: &Default::default(),
                edge_theme: &Default::default(),
                edge_style_plan: &edge_style_plan,
                svg_label_sidecar: &sidecar,
            },
            &execution,
        )
        .expect_err("diagram-ID rejection must stop before the invalid image node is emitted");

        let crate::Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte rejection, got {error}");
        };
        assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
    }

    #[test]
    fn elk_terminal_straightening_reaches_svg_and_preserves_ports() {
        use crate::model::{LayoutEdge, LayoutPoint};
        use base64::Engine as _;

        // Pinned Mermaid geometry.spec.ts terminal-jog case, with measured rectangles whose
        // boundaries coincide with its ports. The layout is fixed to isolate SVG postprocessing.
        let render = |enabled: bool, trace: bool| {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(
                    &format!("---\nconfig:\n  elk:\n    straightenEdges: {enabled}\n---\nflowchart LR\nA --> B"),
                    ParseOptions::strict(),
                ).unwrap().unwrap();
            let render_context = parsed.flowchart_render_context().unwrap().clone();
            let (metadata, semantic) = parsed.into_parts();
            let RenderSemanticModel::Flowchart(model) = semantic else {
                panic!("Flowchart");
            };
            let edge_id = model.edges[0].id.clone();
            let layout = FlowchartLayout {
                nodes: [("A", 153.0, 116.25), ("B", 400.0, 320.0)]
                    .into_iter()
                    .map(|(id, x, y)| LayoutNode {
                        id: id.into(),
                        x,
                        y,
                        width: 80.0,
                        height: 40.0,
                        is_cluster: false,
                        label_width: Some(10.0),
                        label_height: Some(10.0),
                    })
                    .collect(),
                edges: vec![LayoutEdge {
                    id: edge_id.clone(),
                    from: "A".into(),
                    to: "B".into(),
                    from_cluster: None,
                    to_cluster: None,
                    points: [
                        (193.0, 116.25),
                        (218.0, 116.25),
                        (218.0, 119.5),
                        (400.0, 119.5),
                        (400.0, 300.0),
                    ]
                    .into_iter()
                    .map(|(x, y)| LayoutPoint { x, y })
                    .collect(),
                    label: None,
                    start_label_left: None,
                    start_label_right: None,
                    end_label_left: None,
                    end_label_right: None,
                    start_marker: None,
                    end_marker: None,
                    stroke_dasharray: None,
                }],
                edge_owners: crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]),
                clusters: Vec::new(),
                bounds: None,
                dom_node_order_by_root: std::collections::HashMap::from([(
                    String::new(),
                    vec!["A".into(), "B".into()],
                )]),
                uses_elk_adapter_dom: true,
            };
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let request = SvgRenderOptions {
                diagram_id: Some("terminal-jog".into()),
                ..SvgRenderOptions::default()
            };
            let debug = if trace {
                SvgDebugOptions::default().with_flowchart_edge_trace(
                    edge_id,
                    crate::svg::FlowchartEdgeTraceCollector::default(),
                )
            } else {
                SvgDebugOptions::default()
            };
            let execution = SvgExecution::unthemed_for_test(
                &request,
                &debug,
                &session,
                crate::DiagramFamilyId::FLOWCHART,
            )
            .unwrap();
            let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
            let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
                None,
                &metadata.effective_config,
                super::render_config::flowchart_node_label_fill_config_override(
                    &metadata.effective_config,
                ),
                execution.work_meter(),
            )
            .expect("prepared theme");
            let render_config = super::render_config::prepare_flowchart_render_config(
                &model,
                &metadata.effective_config,
                &prepared_theme.compatibility,
                layout.uses_elk_adapter_dom,
                sidecar.base_typography(),
                sidecar.edge_label_padding(),
            );
            let edge_style_plan = FlowchartEdgeStylePlan::prepare_for_model(
                &model,
                &metadata.effective_config,
                false,
                execution.work_meter(),
            )
            .expect("edge style plan")
            .with_resolved_stroke_widths(
                &model,
                &Default::default(),
                &metadata.effective_config,
                &prepared_theme.compatibility,
                execution.work_meter(),
            )
            .expect("prepared stroke widths")
            .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
            edge_style_plan
                .bind_terminal_typography(&render_config.text_style, false)
                .expect("terminal edge typography");
            let prepared_nodes = super::node_inventory::FlowchartPreparedNodes::prepare(
                &model,
                &render_context,
                super::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
                &sidecar,
                None,
                &metadata.effective_config,
                &prepared_theme,
                &render_config,
                session.work_meter(),
            )
            .expect("prepared nodes");
            render_flowchart_svg_model(
                FlowchartSvgModelRequest {
                    render_config: &render_config,
                    prepared_theme: &prepared_theme,
                    prepared_nodes: &prepared_nodes,
                    layout: &layout,
                    swimlane_layout: None,
                    model: &model,
                    render_context: &render_context,
                    effective_config: &metadata.effective_config,
                    diagram_type: metadata.diagram_type.as_str(),
                    diagram_title: None,
                    theme_evidence: &Default::default(),
                    effect_evidence: &Default::default(),
                    expected_effect_applications: &Default::default(),
                    edge_theme: &Default::default(),
                    edge_style_plan: &edge_style_plan,
                    svg_label_sidecar: &sidecar,
                },
                &execution,
            )
            .unwrap()
            .to_string()
        };
        let route = |svg: &str| {
            let doc = crate::xml_dom::Document::parse(svg).unwrap();
            let path = doc
                .descendants()
                .find(|n| n.attribute("data-points").is_some())
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            (
                path.attribute("d").unwrap().to_owned(),
                points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            )
        };
        let off = render(false, false);
        let on = render(true, false);
        let (old_path, old_points) = route(&off);
        let (new_path, new_points) = route(&on);
        assert_eq!(old_points.len(), 5);
        assert_eq!(new_points.len(), 3);
        assert_eq!(old_points.first(), new_points.first());
        assert_eq!(old_points.last(), new_points.last());
        assert_eq!(new_points[0].1, new_points[1].1);
        assert_ne!(old_path, new_path);
        assert_eq!(
            on,
            render(true, true),
            "diagnostics must preserve processed geometry"
        );
    }
}
