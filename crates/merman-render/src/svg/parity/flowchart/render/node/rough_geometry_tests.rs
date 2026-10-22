use crate::svg::parity::SvgExecution;
use crate::svg::parity::flowchart as flow;
use crate::svg::parity::flowchart::types::FlowchartRenderCtx;
use crate::svg::{SvgDebugOptions, SvgRenderOptions};

fn declaration(shape: &str) -> String {
    match shape {
        "iconCircle" => "icon: \"fa:fa-user\", form: circle".to_string(),
        "iconSquare" => "icon: \"fa:fa-user\", form: square".to_string(),
        "iconRounded" => "icon: \"fa:fa-user\", form: rounded".to_string(),
        "icon" => "icon: \"fa:fa-user\"".to_string(),
        "imageSquare" => "img: \"https://example.com/a.png\"".to_string(),
        "roundedRect" => "shape: rounded".to_string(),
        _ => format!("shape: {shape}"),
    }
}

fn with_context<T>(
    look: &str,
    seed: i64,
    shape: &str,
    tiny_svg_budget: bool,
    consume: impl FnOnce(&FlowchartRenderCtx<'_>) -> crate::Result<T>,
) -> T {
    let source = format!(
        "flowchart LR\nsubgraph S[Outer]\n A@{{ {}, label: \"Test\" }}\nend\n",
        declaration(shape)
    );
    let parsed = merman_core::MermaidEngine::new()
        .with_site_config(merman_core::presentation::PresentationConfig::from_value(
            serde_json::json!({"look":look,"handDrawnSeed":seed,"flowchart":{"htmlLabels":false}}),
        ))
        .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
        .unwrap()
        .unwrap();
    let render_context = parsed.flowchart_render_context().unwrap().clone();
    let (metadata, semantic) = parsed.into_parts();
    let merman_core::RenderSemanticModel::Flowchart(model) = semantic else {
        panic!("Flowchart")
    };
    let layout = crate::model::FlowchartLayout {
        nodes: vec![crate::model::LayoutNode {
            id: "A".into(),
            x: 100.0,
            y: 100.0,
            width: 120.0,
            height: 80.0,
            is_cluster: false,
            label_width: Some(32.0),
            label_height: Some(16.0),
        }],
        clusters: vec![crate::model::LayoutCluster {
            id: "S".into(),
            x: 100.0,
            y: 100.0,
            width: 180.0,
            height: 150.0,
            diff: 0.0,
            offset_y: 0.0,
            title: "Outer".into(),
            title_label: crate::model::LayoutLabel {
                x: 100.0,
                y: 40.0,
                width: 50.0,
                height: 16.0,
            },
            requested_dir: None,
            effective_dir: "LR".into(),
            padding: 15.0,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        }],
        edges: vec![],
        edge_owners: Default::default(),
        bounds: None,
        dom_node_order_by_root: std::collections::HashMap::from([("S".into(), vec!["A".into()])]),
        uses_elk_adapter_dom: false,
    };
    let resources = if tiny_svg_budget {
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
            .unwrap()
    } else {
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
    };
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic().with_fixed_seed(0),
        )
        .with_resource_policy(resources)
        .begin_session()
        .unwrap();
    let options = SvgRenderOptions {
        diagram_id: Some("rough-prefix".into()),
        ..Default::default()
    };
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::unthemed_for_test(
        &options,
        &debug,
        &session,
        crate::DiagramFamilyId::FLOWCHART,
    )
    .unwrap();
    let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
    let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
        None,
        &metadata.effective_config,
        flow::render_config::flowchart_node_label_fill_config_override(&metadata.effective_config),
        execution.work_meter(),
    )
    .unwrap();
    let render_config = flow::render_config::prepare_flowchart_render_config(
        &model,
        &metadata.effective_config,
        &theme.compatibility,
        false,
        sidecar.base_typography(),
        sidecar.edge_label_padding(),
    );
    let edge_style_plan = flow::FlowchartEdgeStylePlan::prepare_for_model(
        &model,
        &metadata.effective_config,
        false,
        execution.work_meter(),
    )
    .unwrap()
    .with_resolved_stroke_widths(
        &model,
        &Default::default(),
        &metadata.effective_config,
        &theme.compatibility,
        execution.work_meter(),
    )
    .unwrap()
    .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
    edge_style_plan
        .bind_terminal_typography(&render_config.text_style, false)
        .unwrap();
    let prepared_nodes = flow::node_inventory::FlowchartPreparedNodes::prepare(
        &model,
        &render_context,
        flow::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
        &sidecar,
        None,
        &metadata.effective_config,
        &theme,
        &render_config,
        session.work_meter(),
    )
    .unwrap();
    flow::paint::with_flowchart_paint_context(
        flow::svg_emit::FlowchartSvgModelRequest {
            render_config: &render_config,
            prepared_theme: &theme,
            prepared_nodes: &prepared_nodes,
            layout: &layout,
            swimlane_layout: None,
            model: &model,
            render_context: &render_context,
            effective_config: &metadata.effective_config,
            diagram_type: &metadata.diagram_type,
            diagram_title: None,
            svg_label_sidecar: &sidecar,
            theme_evidence: &Default::default(),
            effect_evidence: &Default::default(),
            expected_effect_applications: &Default::default(),
            edge_style_plan: &edge_style_plan,
            edge_theme: &Default::default(),
        },
        &execution,
        |ctx, _, _| consume(ctx),
    )
    .unwrap()
}

fn next_rough_path(ctx: &FlowchartRenderCtx<'_>) -> String {
    crate::svg::parity::roughjs_common::paths::roughjs_hand_drawn_stroke_path_for_svg_path(
        "M0 0 L31 17 L60 4",
        0.7,
        &ctx.hand_drawn_seed,
    )
    .unwrap()
}

const SHAPES: &[&str] = &[
    "anchor",
    "bang",
    "browser",
    "bow-rect",
    "brace",
    "brace-r",
    "braces",
    "bucket",
    "choice",
    "circle",
    "cloud",
    "console",
    "cross-circ",
    "curv-trap",
    "cylinder",
    "datastore",
    "delay",
    "diamond",
    "folder",
    "div-rect",
    "doc",
    "doublecircle",
    "f-circ",
    "fork",
    "fr-circ",
    "hexagon",
    "h-cyl",
    "hourglass",
    "icon",
    "iconCircle",
    "iconRounded",
    "iconSquare",
    "imageSquare",
    "inv-trapezoid",
    "lean-l",
    "lean-r",
    "bolt",
    "lin-cyl",
    "lin-doc",
    "manual-file",
    "manual-input",
    "notch-pent",
    "notch-rect",
    "note",
    "odd",
    "paper-tape",
    "person",
    "rect",
    "roundedRect",
    "lin-rect",
    "sm-circ",
    "docs",
    "st-rect",
    "stadium",
    "subroutine",
    "tag-doc",
    "tag-rect",
    "text",
    "trapezoid",
    "tri",
    "win-pane",
];

#[test]
fn math_only_shape_prefix_keeps_the_next_actual_rough_path() {
    for look in ["classic", "handDrawn"] {
        for seed in [-1, 0, 4_294_967_296, 7] {
            for shape in SHAPES {
                let actual = with_context(look, seed, shape, false, |ctx| {
                    let mut svg = String::new();
                    super::render_flowchart_node(
                        &mut svg,
                        ctx,
                        "A",
                        0.0,
                        0.0,
                        crate::svg::parity::timing::RenderTiming::disabled(),
                        &mut Default::default(),
                    )?;
                    assert!(svg.contains("data-id=\"A\""));
                    assert!(!svg.contains("NaN"), "{look} {seed} {shape}");
                    assert!(!svg.contains("Infinity"), "{look} {seed} {shape}");
                    Ok(next_rough_path(ctx))
                });
                let query = with_context(look, seed, shape, true, |ctx| {
                    super::advance_rough_geometry(ctx, "A")?;
                    assert_eq!(
                        ctx.work_meter.projected_svg_bytes(),
                        0,
                        "{look} {seed} {shape}"
                    );
                    assert!(
                        ctx.document_ids.get().is_none(),
                        "query must not project document IDs"
                    );
                    Ok(next_rough_path(ctx))
                });
                assert_eq!(query, actual, "{look} {seed} {shape}");
            }
        }
    }
}

#[test]
fn math_only_cluster_prefix_keeps_the_next_actual_rough_path() {
    for look in ["classic", "handDrawn"] {
        for seed in [-1, 0, 4_294_967_296, 7] {
            let actual = with_context(look, seed, "rect", false, |ctx| {
                let mut svg = String::new();
                super::super::cluster::render_flowchart_cluster(
                    &mut svg,
                    ctx,
                    &ctx.layout.clusters[0],
                    17.0,
                    23.0,
                )?;
                assert!(svg.contains("data-et=\"cluster\""));
                Ok(next_rough_path(ctx))
            });
            let query = with_context(look, seed, "rect", true, |ctx| {
                super::super::cluster::advance_rough_geometry(
                    ctx,
                    &ctx.layout.clusters[0],
                    17.0,
                    23.0,
                )?;
                assert_eq!(ctx.work_meter.projected_svg_bytes(), 0);
                assert!(ctx.document_ids.get().is_none());
                Ok(next_rough_path(ctx))
            });
            assert_eq!(query, actual, "{look} {seed}");
        }
    }
}

#[test]
fn classic_no_label_prefix_really_advances_the_shared_math_random_stream() {
    let initial = with_context("classic", 4_294_967_296, "anchor", true, |ctx| {
        Ok(next_rough_path(ctx))
    });
    let advanced = with_context("classic", 4_294_967_296, "anchor", true, |ctx| {
        super::advance_rough_geometry(ctx, "A")?;
        Ok(next_rough_path(ctx))
    });
    assert_ne!(
        initial, advanced,
        "the sentinel observes the operation-owned stream"
    );
}
