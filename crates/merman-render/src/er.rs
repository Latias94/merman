use crate::layout_work::OperationLayoutWorkControl;
use crate::model::ErDiagramLayout;
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use crate::{Error, Result};
use merman_render_graph::er::er_marker_id;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

#[cfg(feature = "layout-shoal")]
mod shoal;
mod swimlane;
use crate::layout_backend::GraphLayoutBackend;
#[cfg(any(test, feature = "layout-shoal"))]
pub(crate) use merman_render_graph::er::ErCssBinding;
pub(crate) use merman_render_graph::er::{
    ErBoxLabel, ErConfigView, ErEntityThemePlan, ErLayoutSettings, ErNativeStyle, ErPreparedLabels,
    ErSwimlanePaintLayout, swimlane_relationship_html_source,
};
use merman_render_graph::er::{
    ErPreparedRelationshipLabel, er_layout_bounds, er_subgraph_title_metrics,
    er_swimlane_title_metrics,
};

pub(crate) type ErEntity = merman_model::families::er::ErEntity;
#[cfg(all(test, feature = "layout-shoal"))]
type ErRelationship = merman_model::families::er::ErRelationship;
#[cfg(feature = "layout-shoal")]
pub(crate) type ErSubgraph = merman_model::families::er::ErSubgraph;

#[cfg(test)]
fn test_layout_settings(config: &Value, direction: &str, swimlane: bool) -> ErLayoutSettings {
    let view = ErConfigView::new(config);
    view.layout_settings_with_native_style(
        direction,
        None,
        None,
        view.relationship_html_labels(swimlane),
    )
}

#[cfg(test)]
pub(crate) fn prepare_er_labels(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    swimlane: bool,
) -> Arc<ErPreparedLabels> {
    let settings = test_layout_settings(effective_config, &model.direction, swimlane);
    let work_meter = Arc::new(OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    Arc::new(
        ErPreparedLabels::prepare_for_layout(
            model,
            measurer,
            &settings,
            effective_config,
            swimlane,
            &work_meter,
        )
        .expect("prepare ER labels"),
    )
}

pub(crate) struct ErPreparedLayout {
    pub(crate) layout: ErDiagramLayout,
    pub(crate) route_contract: crate::graph_layout::RouteContract,
}

/// Lays out an ER model with the family's resolved layout and typography settings.
#[allow(
    clippy::too_many_arguments,
    reason = "ER layout shares prepared typography and work accounting across compiled adapters."
)]
pub(crate) fn layout_er_diagram_typed(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    backend: GraphLayoutBackend,
    measurer: &dyn TextMeasurer,
    #[cfg(feature = "layout-elk")] operation_seed: merman_layout_elk::ElkOperationSeed,
    settings: ErLayoutSettings,
    #[cfg(feature = "layout-shoal")] execution: Option<&crate::LayoutExecution<'_>>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<ErPreparedLayout> {
    use crate::graph_layout::RouteContract;

    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&work_meter));
    let adapter_work = er_layout_adapter_work(model, &work_control)?;
    work_control.charge_adapter(adapter_work)?;
    validate_er_relationship_endpoints(model)?;
    let prepared_labels = Arc::new(ErPreparedLabels::prepare_for_layout(
        model,
        measurer,
        &settings,
        effective_config,
        backend == GraphLayoutBackend::Swimlane,
        &work_meter,
    )?);
    backend.validate_rootless_graph()?;

    match backend {
        #[cfg(feature = "layout-dagre")]
        GraphLayoutBackend::Dagre => {
            let layout = merman_render_graph::er::layout_dagre(
                model,
                measurer,
                settings,
                &prepared_labels,
                &mut work_control,
            )?;
            Ok(ErPreparedLayout {
                layout: ErDiagramLayout {
                    nodes: layout.nodes,
                    edges: layout.edges,
                    render_edges: layout.render_edges,
                    swimlane: None,
                    clusters: layout.clusters,
                    bounds: layout.bounds,
                    prepared_labels,
                },
                route_contract: RouteContract::DagreParity,
            })
        }
        #[cfg(feature = "layout-shoal")]
        GraphLayoutBackend::Shoal => shoal::layout(
            model,
            measurer,
            &settings,
            prepared_labels,
            crate::config::config_diagram_look(effective_config).is_neo(),
            execution,
            &mut work_control,
        ),
        GraphLayoutBackend::Swimlane => swimlane::layout_er_diagram_swimlane_typed(
            model,
            effective_config,
            measurer,
            &settings,
            prepared_labels,
            work_meter,
        )
        .map(|layout| ErPreparedLayout {
            layout,
            route_contract: RouteContract::Swimlane,
        }),
        #[cfg(feature = "layout-elk")]
        GraphLayoutBackend::Elk(algorithm) => {
            let layout = merman_render_graph::er::layout_elk(
                model,
                effective_config,
                algorithm.algorithm(),
                measurer,
                settings,
                &prepared_labels,
                operation_seed,
                &mut work_control,
            )?;
            Ok(ErPreparedLayout {
                layout: ErDiagramLayout {
                    nodes: layout.nodes,
                    // ELK's canonical edges already form the SVG-facing projection.
                    render_edges: Vec::new(),
                    swimlane: None,
                    edges: layout.edges,
                    clusters: layout.clusters,
                    bounds: layout.bounds,
                    prepared_labels,
                },
                route_contract: RouteContract::ElkParity,
            })
        }
        _ => Err(backend.unavailable_for("er")),
    }
}

fn validate_er_relationship_endpoints(
    model: &merman_model::families::er::ErDiagramModel,
) -> Result<()> {
    let mut node_ids: HashSet<&str> = model
        .entities
        .values()
        .map(|entity| entity.id.as_str())
        .collect();
    node_ids.extend(model.subgraphs.iter().map(|subgraph| subgraph.id.as_str()));
    for relationship in &model.relationships {
        // The render model indexes entities by source name, while relationships store the
        // renderer-facing generated `entity-*` ids.  Validate against the entity values rather
        // than the map keys; checking the keys would reject every ordinary parsed relationship
        // before Dagre/ELK gets a chance to lay it out.
        if !node_ids.contains(relationship.entity_a.as_str())
            || !node_ids.contains(relationship.entity_b.as_str())
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        }
    }
    Ok(())
}

fn er_layout_adapter_work(
    model: &merman_model::families::er::ErDiagramModel,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let attribute_count = model.entities.values().try_fold(0usize, |count, entity| {
        work_control.checked_add(count, entity.attributes.len())
    })?;
    let entity_work = work_control.checked_mul(model.entities.len(), 12)?;
    let attribute_work = work_control.checked_mul(attribute_count, 6)?;
    let relationship_work = work_control.checked_mul(model.relationships.len(), 10)?;
    let class_work = work_control.checked_mul(model.classes.len(), 3)?;
    let subgraph_membership = model.subgraphs.iter().try_fold(0usize, |total, subgraph| {
        work_control.checked_add(total, subgraph.nodes.len())
    })?;
    let subgraph_work = work_control.checked_add(
        work_control.checked_mul(model.subgraphs.len(), 12)?,
        subgraph_membership,
    )?;
    work_control.checked_add(
        work_control.checked_add(entity_work, attribute_work)?,
        work_control.checked_add(
            work_control.checked_add(relationship_work, class_work)?,
            subgraph_work,
        )?,
    )
}

#[cfg(all(test, feature = "layout-shoal"))]
mod tests {
    use crate::resources::OperationWorkMeter;
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};

    fn er_resource_model() -> merman_model::families::er::ErDiagramModel {
        let mut model = merman_model::families::er::ErDiagramModel::default();
        model.entities.insert(
            "A".into(),
            super::ErEntity {
                id: "A".into(),
                label: "AB".into(),
                css_styles: vec!["font-size:32px".into(), "font-style:italic".into()],
                ..Default::default()
            },
        );
        model
    }

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn er_shoal_uses_the_cumulative_budget_and_releases_labels_on_failure() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        use std::sync::Arc;

        let mut model = er_resource_model();
        model.subgraphs.push(super::ErSubgraph {
            id: "g".into(),
            title: "Group title".into(),
            label_type: "string".into(),
            nodes: vec!["A".into()],
            ..Default::default()
        });
        model.relationships.push(super::ErRelationship {
            entity_a: "g".into(),
            entity_b: "A".into(),
            role_a: "relationship".into(),
            ..Default::default()
        });
        let config = serde_json::json!({"layout": "shoal", "htmlLabels": false});
        let prepare = |measurer: &dyn TextMeasurer, meter| {
            let settings = super::test_layout_settings(&config, &model.direction, false);
            super::layout_er_diagram_typed(
                &model,
                &config,
                super::GraphLayoutBackend::Shoal,
                measurer,
                #[cfg(feature = "layout-elk")]
                merman_layout_elk::ElkOperationSeed::from_operation_seed(std::num::NonZeroU64::MIN),
                settings,
                None,
                meter,
            )
        };
        let standard = DeterministicTextMeasurer::default();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let meter = Arc::new(OperationWorkMeter::new(policy));
        let layout = prepare(&standard, Arc::clone(&meter)).expect("Shoal ER layout");
        assert!(layout.route_contract.canonical_routes().is_some());
        let required = meter.used();
        drop(layout);

        let meter = Arc::new(OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, required)
                .unwrap(),
        ));
        meter.charge(1).unwrap();
        let error = prepare(&standard, Arc::clone(&meter))
            .err()
            .expect("layout also counts prior work");
        assert!(matches!(
            error,
            crate::Error::ResourceLimitExceeded(ref error)
                if error.limit == "max_layout_work_units"
        ));
        assert_eq!(meter.prepared_text_retained_bytes(), 0);

        struct CancellingTitle(merman_model::OperationControl);
        impl TextMeasurer for CancellingTitle {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                if text.contains("Group") {
                    self.0.cancel();
                }
                DeterministicTextMeasurer::default().measure(text, style)
            }
        }
        let control = merman_model::OperationControl::new();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            policy,
            control.clone(),
        ));
        let error = prepare(&CancellingTitle(control), Arc::clone(&meter))
            .err()
            .expect("cancelling during title measurement aborts the layout");
        assert!(matches!(error, crate::Error::Cancelled(_)));
        assert!(meter.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}

#[cfg(all(test, feature = "layout-dagre"))]
mod native_theme_tests {
    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_er_uses_table_roles_and_relationship_background() {
        let theme = ThemeCompiler::new().compile_json(br##"{
            "schema_version":2,"extends":"paper",
            "roles":{"surface.node":"#EEEEEE","surface.emphasis":"#345678","surface.container":"#DFEBFC","stroke.connector":"#123456"},
            "typography":{"mono":["Attribute Mono","monospace"]},
            "form":{"radius":6,"edge_label_bg":"canvas","edge_style":"straight"}
        }"##, None).unwrap();
        let environment = RenderEnvironment::deterministic();
        for html in [false, true] {
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nerDiagram\nORDER ||--o{{ LINE : contains\nLINE {{\n  string sku\n  int quantity\n}}\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let prepared = family::prepare(
                parsed.presentation(),
                &LayoutOptions::default(),
                environment
                    .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                    .unwrap()
                    .with_canvas_background(crate::theme::Color::rgb(250, 251, 252)),
            )
            .unwrap();
            let render = prepared
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let dom = roxmltree::Document::parse(render.svg()).unwrap();
            for (class, fill) in [
                ("er-native-header", "#345678"),
                ("row-rect-odd", "#EEEEEE"),
                ("row-rect-even", "#DFEBFC"),
            ] {
                assert!(
                    dom.descendants()
                        .any(|node| node.attribute("class") == Some(class)
                            && node
                                .attribute("style")
                                .is_some_and(|style| style.contains(fill))),
                    "missing {class} fill"
                );
            }
            let css = dom
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(
                css.contains(".label.attribute-type{font-family:\"Attribute Mono\", monospace;")
            );
            assert!(css.contains(".labelBkg{background-color:#FAFBFC;}"));
            assert!(css.contains(".edgeLabel .label rect{fill:#FAFBFC;}"));
            assert!(dom.descendants().any(|node| {
                node.attribute("marker-end")
                    .is_some_and(|marker| marker.contains("zeroOrMore"))
            }));
        }
    }
}
