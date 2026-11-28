use super::*;
use crate::graph_layout::RouteContract;
use merman_model::{OperationPhase, families::er::ErDiagramModel};
use merman_render_graph::er::{ErLayeredError, ErLayeredLabels, layout_layered};

/// `None` requests a fresh Dagre attempt. All first-attempt owners have already dropped.
pub(super) fn prepare(
    model: &ErDiagramModel,
    config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    work: &Arc<OperationWorkMeter>,
    allow_dagre_retry: bool,
) -> Result<Option<ErPreparedLayout>> {
    let attempt = (|| {
        let labels = ErLayeredLabels::prepare(model, config, measurer, settings, work)?;
        let layout = layout_layered(model, config, settings, &labels, work)?;
        Ok::<_, ErLayeredError>(ErPreparedLayout {
            layout: ErDiagramLayout {
                nodes: layout.nodes,
                edges: layout.edges,
                render_edges: Vec::new(),
                swimlane: None,
                clusters: layout.clusters,
                bounds: layout.bounds,
                prepared_labels: labels.into_labels(),
            },
            route_contract: RouteContract::ErLayered(Arc::new(layout.paint)),
        })
    })();
    work.checkpoint(OperationPhase::Layout)?;
    match attempt {
        Ok(layout) => Ok(Some(layout)),
        Err(ErLayeredError::Host(error)) => Err(error),
        Err(error) if error.is_recoverable() => {
            if !allow_dagre_retry || !cfg!(feature = "layout-dagre") {
                return Err(GraphLayoutBackend::Dagre.unavailable_for("er"));
            }
            Ok(None)
        }
        Err(error) => Err(Error::InvalidModel {
            message: format!("ER Layered layout failed: {error}"),
        }),
    }
}

#[cfg(all(test, feature = "layout-dagre"))]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::text::DeterministicTextMeasurer;
    use merman_model::families::er::{ErRelationship, ErSubgraph};

    #[test]
    fn er_layered_retry_releases_first_labels_and_charges_both_attempts() {
        let mut model = ErDiagramModel {
            direction: "TB".to_owned(),
            subgraphs: vec![ErSubgraph {
                id: "Accounts".to_owned(),
                title: "Accounts department".to_owned(),
                label_type: "string".to_owned(),
                nodes: vec!["A".to_owned()],
                ..Default::default()
            }],
            relationships: vec![ErRelationship {
                entity_a: "A".to_owned(),
                entity_b: "B".to_owned(),
                role_a: "owns".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        };
        for id in ["A", "B"] {
            model.entities.insert(
                id.to_owned(),
                ErEntity {
                    id: id.to_owned(),
                    label: id.to_owned(),
                    ..Default::default()
                },
            );
        }
        let config = serde_json::json!({"htmlLabels":false,
            "flowchart":{"subGraphTitleMargin":{"top":1.7e308,"bottom":1.7e308}}});
        let text = DeterministicTextMeasurer::default();
        let execute = |backend, meter: &Arc<OperationWorkMeter>, allow_retry| {
            let settings = test_layout_settings(&config, &model.direction, false);
            layout_er_diagram_typed(
                &model,
                &config,
                backend,
                &text,
                #[cfg(feature = "layout-elk")]
                merman_layout_elk::ElkOperationSeed::from_operation_seed(std::num::NonZeroU64::MIN),
                settings,
                #[cfg(feature = "layout-shoal")]
                None,
                Arc::clone(meter),
                allow_retry,
            )
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let first = Arc::new(OperationWorkMeter::new(policy));
        assert!(matches!(
            execute(GraphLayoutBackend::Layered, &first, false),
            Err(Error::MissingCapability { .. })
        ));
        assert!(first.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(first.prepared_text_retained_bytes(), 0);

        let dagre = Arc::new(OperationWorkMeter::new(policy));
        let direct = execute(GraphLayoutBackend::Dagre, &dagre, false).unwrap();
        let retry_work = first.used() + dagre.used();
        let caps = |limit| {
            Arc::new(OperationWorkMeter::new(
                policy
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            ))
        };
        let retry = caps(retry_work);
        let output = execute(GraphLayoutBackend::Layered, &retry, true).unwrap();
        assert!(matches!(output.route_contract, RouteContract::ErLayered(_)));
        assert_eq!(retry.used(), retry_work);
        assert_eq!(
            retry.prepared_text_retained_bytes(),
            dagre.prepared_text_retained_bytes()
        );
        assert_eq!(
            serde_json::to_value(&output.layout).unwrap(),
            serde_json::to_value(&direct.layout).unwrap()
        );
        drop(output);
        assert_eq!(retry.prepared_text_retained_bytes(), 0);

        let denied = caps(retry_work - 1);
        assert!(
            matches!(execute(GraphLayoutBackend::Layered, &denied, true), Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_layout_work_units")
        );
        assert_eq!(denied.prepared_text_retained_bytes(), 0);
    }
}
