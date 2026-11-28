//! Projects the prepared State artifact and operation environment into the graph writer.

use crate::Result;
use crate::graph_layout::RouteContract;
use crate::model::StateDiagramLayout;
use merman_model::{families::state::StateDiagramRenderModel, family::DiagramFamilyId};
use merman_render_core::{root_svg::RootedSvg, text::TextMeasurer};
use merman_render_graph::state::{StateLabelSidecar, StateStylePlan, StateSvgContext};

use super::SvgExecution;

#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::state::state_canonical_marker;

#[cfg(feature = "layout-shoal")]
fn state_canonical_edge_painter<'a>(
    routes: &'a crate::graph_layout::canonical::CanonicalRoutes,
    edge_style: Option<merman_theme_contract::v2::EdgeStyle>,
    work_meter: &'a crate::resources::OperationWorkMeter,
) -> impl for<'points> Fn(
    &crate::model::LayoutEdge,
    kurbo::Vec2,
    f64,
    Option<&roughr::core::RoughRandomness>,
    &'points mut crate::graph_layout::canonical::CanonicalPaintScratch,
) -> Result<merman_render_graph::state::StateCanonicalEdge<'points>>
+ 'a {
    use crate::graph_layout::route_paint::RouteStyle;
    use merman_render_graph::state::StateCanonicalEdge;

    move |edge, translation, stroke_outset, randomness, scratch| {
        let geometry = routes.paint_with_borrowed_points(
            edge,
            translation,
            stroke_outset,
            edge_style.map_or(RouteStyle::Curved, RouteStyle::from_theme),
            work_meter,
            scratch,
        )?;
        let rough_d = if let Some(randomness) = randomness {
            work_meter.charge(geometry.path.d.len().max(1))?;
            Some(merman_render_core::rough_svg::constrained_rough_edge_path(
                &geometry.path.d,
                geometry.markers,
                work_meter,
                randomness,
            )?)
        } else {
            None
        };
        Ok(StateCanonicalEdge {
            d: geometry.path.d,
            bounds: geometry.path.bounds,
            data_points: geometry.data_points,
            marker_points: geometry.marker_points,
            end_marker: geometry.markers.1,
            rough_d,
        })
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "The prepared family inputs are borrowed separately."
)]
pub(super) fn render_state_diagram_svg_model(
    layout: &StateDiagramLayout,
    model: &StateDiagramRenderModel,
    label_sidecar: &StateLabelSidecar,
    route_contract: &RouteContract,
    style_plan: &StateStylePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = route_contract.canonical_routes().map(|routes| {
        state_canonical_edge_painter(
            routes,
            style_plan.compatibility().native_edge_style(),
            options.work_meter(),
        )
    });

    let diagram_id = options.diagram_id_or("merman");
    let context = StateSvgContext {
        diagram_id,
        viewport: options.root_viewport(DiagramFamilyId::STATE, diagram_id),
        elk_parity: route_contract.is_elk_parity(),
        #[cfg(feature = "layout-grid")]
        grid: match route_contract {
            RouteContract::Grid(paint) => Some(*paint),
            _ => None,
        },
        #[cfg(feature = "layout-layered")]
        layered: match route_contract {
            RouteContract::StateLayered(paint) => Some(paint.as_ref()),
            _ => None,
        },
        #[cfg(feature = "layout-shoal")]
        canonical_routes: canonical_routes
            .as_ref()
            .map(|paint| paint as &merman_render_graph::state::StateCanonicalEdgePainter<'_>),
        work_meter: options.work_meter(),
        operation: options.family.session().operation_context(),
        render_seed: options.seed(),
        timing: options.timing(),
        include_edges: options.debug.include_edges,
        include_nodes: options.debug.include_nodes,
        native_theme: options.native_family_theme(),
    };
    merman_render_graph::state::render_state_diagram_svg_model(
        layout,
        model,
        label_sidecar,
        style_plan,
        effective_config,
        diagram_title,
        measurer,
        &context,
    )
}

#[cfg(all(test, feature = "layout-elk"))]
mod tests {
    use super::*;
    use crate::svg::SvgRenderOptions;

    #[test]
    fn state_elk_neo_note_crossing_rewrites_the_complete_original_mask() {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nA --> B: move\nC --> D\nnote right of A: annotation\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::State(model) = parsed.model() else {
            panic!("expected State render model");
        };
        let note = model
            .edges
            .iter()
            .find(|edge| edge.classes.contains("note-edge"))
            .unwrap();
        // Supply controlled provider routes to the real State paint path. The note crosses an
        // unrelated transition, while the labelled transition is deliberately away from both.
        let edges: Vec<crate::model::LayoutEdge> = model
            .edges
            .iter()
            .map(|edge| {
                let points = if edge.id == note.id {
                    vec![
                        crate::model::LayoutPoint { x: 0.0, y: 50.0 },
                        crate::model::LayoutPoint { x: 100.0, y: 50.0 },
                    ]
                } else if edge.start == "C" {
                    vec![
                        crate::model::LayoutPoint { x: 50.0, y: 0.0 },
                        crate::model::LayoutPoint { x: 50.0, y: 100.0 },
                    ]
                } else {
                    vec![
                        crate::model::LayoutPoint { x: 0.0, y: 200.0 },
                        crate::model::LayoutPoint { x: 100.0, y: 200.0 },
                    ]
                };
                crate::model::LayoutEdge {
                    id: edge.id.clone(),
                    from: edge.start.clone(),
                    to: edge.end.clone(),
                    from_cluster: None,
                    to_cluster: None,
                    points,
                    label: (!edge.label.is_empty()).then_some(crate::model::LayoutLabel {
                        x: 50.0,
                        y: 200.0,
                        width: 32.0,
                        height: 24.0,
                    }),
                    start_label_left: None,
                    start_label_right: None,
                    end_label_left: None,
                    end_label_right: None,
                    start_marker: None,
                    end_marker: None,
                    stroke_dasharray: None,
                }
            })
            .collect();
        let layout = StateDiagramLayout {
            nodes: Vec::new(),
            clusters: Vec::new(),
            edges,
            bounds: None,
            uses_elk_adapter_dom: true,
            elk_edge_paths: Default::default(),
        };
        let render = |hops: serde_json::Value| {
            let config =
                serde_json::json!({"look":"neo", "htmlLabels":false, "elk":{"lineHops":hops}});
            let plan = StateStylePlan::resolve_classic(model, &config, true);
            let labels = merman_render_graph::state::StateLabelSidecarBuilder::default().finish();
            crate::svg::with_test_svg_execution(
                crate::DiagramFamilyId::STATE,
                &SvgRenderOptions::default(),
                |execution| {
                    render_state_diagram_svg_model(
                        &layout,
                        model,
                        &labels,
                        &crate::graph_layout::RouteContract::ElkParity,
                        &plan,
                        &config,
                        None,
                        &crate::text::DeterministicTextMeasurer::default(),
                        execution,
                    )
                    .unwrap()
                    .into_string_for(crate::DiagramFamilyId::STATE)
                    .unwrap()
                },
            )
        };
        let baseline = render(serde_json::json!(false));
        let baseline_doc = crate::xml_dom::Document::parse(&baseline).unwrap();
        for hops in [serde_json::json!(true), serde_json::json!("gap")] {
            let svg = render(hops);
            let doc = crate::xml_dom::Document::parse(&svg).unwrap();
            for expected in baseline_doc
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
            {
                let actual = doc
                    .descendants()
                    .find(|node| {
                        node.attribute("data-id") == expected.attribute("data-id")
                            && node.has_tag_name("path")
                    })
                    .unwrap();
                assert_eq!(
                    actual.attribute("data-points"),
                    expected.attribute("data-points")
                );
                assert_eq!(
                    actual.attribute("marker-end"),
                    expected.attribute("marker-end")
                );
                if expected.attribute("data-id") != Some(note.id.as_str()) {
                    assert_eq!(actual.attribute("d"), expected.attribute("d"));
                    assert_eq!(actual.attribute("style"), expected.attribute("style"));
                    continue;
                }
                let d = actual.attribute("d").unwrap();
                assert_ne!(Some(d), expected.attribute("d"));
                let style = actual.attribute("style").unwrap();
                let mask = style
                    .split(';')
                    .find_map(|declaration| declaration.trim().strip_prefix("stroke-dasharray:"))
                    .unwrap();
                let mask: Vec<f64> = mask
                    .split_whitespace()
                    .map(|number| number.parse().unwrap())
                    .collect();
                assert_eq!(mask.len(), 4, "{style}");
                assert_eq!((mask[0], mask[1], mask[3]), (0.0, 0.0, 2.0));
                let length =
                    merman_render_core::svg_edge_parity::svg_path_length_from_d(d).unwrap();
                assert!(
                    (mask[2] + 2.0 - length).abs() < 1e-6,
                    "{style}: length={length}"
                );
            }
            let label = |document: &crate::xml_dom::Document<'_>| {
                let group = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some("edgeLabels"))
                    .unwrap();
                group
                    .descendants()
                    .filter_map(|node| node.attribute("transform"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            assert_eq!(label(&doc), label(&baseline_doc));
        }
    }
}
