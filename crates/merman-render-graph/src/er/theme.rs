use std::collections::BTreeMap;
use std::sync::Arc;

use merman_render_core::{
    resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation},
    text::mermaid_markdown_to_xhtml_label_fragment,
    xml::escape_xml,
};
use merman_theme_contract::v2::FamilyTheme;

use super::{
    ErCssBinding, ErEntitySourceStyle, ErNativeStyle, ErPreparedLabels, ErPreparedSvgLayout,
    ErSubgraphSourceStyles, prepare_er_source_visuals, prepare_er_subgraph_source_styles,
};

/// Prepared HTML title and its retained text reservation.
#[derive(Debug)]
pub(super) struct ErPreparedHtmlSubgraphLabel {
    fragment: String,
    _retained: PreparedTextRetainedReservation,
}

impl ErPreparedHtmlSubgraphLabel {
    fn prepare(
        id: &str,
        work_meter: &Arc<OperationWorkMeter>,
        project: impl FnOnce() -> String,
    ) -> Result<(String, Self), OperationWorkError> {
        // Match Class's map entry/control allowance; both String heaps are counted separately.
        let owner_bytes =
            std::mem::size_of::<(String, Self)>().saturating_add(std::mem::size_of::<usize>());
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes.saturating_add(id.len()))?;
        let fragment = project();
        // The shared projector is opaque. Admit its actual capacity before copying the key
        // or allocating the map record, then retain one lease for that complete owner.
        work_meter.preflight_prepared_text_retained_bytes(
            owner_bytes
                .saturating_add(id.len())
                .saturating_add(fragment.capacity()),
        )?;
        let key = id.to_owned();
        let bytes = owner_bytes
            .saturating_add(key.capacity())
            .saturating_add(fragment.capacity());
        let retained = work_meter.reserve_prepared_text_retained_bytes(bytes)?;
        Ok((
            key,
            Self {
                fragment,
                _retained: retained,
            },
        ))
    }

    pub(super) fn fragment(&self) -> &str {
        &self.fragment
    }
}

/// Prepared styles for ER entities and subgraphs, shared by the SVG paths.
#[derive(Debug)]
pub struct ErEntityThemePlan {
    css_binding: ErCssBinding,
    entity_indices: BTreeMap<String, usize>,
    entity_source_styles: Vec<ErEntitySourceStyle>,
    subgraph_source_styles: BTreeMap<String, ErEntitySourceStyle>,
    html_subgraph_labels: BTreeMap<String, ErPreparedHtmlSubgraphLabel>,
    relationship_html_labels: bool,
    // Drop shared source views before their counted prepared-label owner.
    _prepared_labels: Arc<ErPreparedLabels>,
}

impl ErEntityThemePlan {
    pub fn resolve(
        config: &merman_model::presentation::PresentationConfig,
        native: Option<(&FamilyTheme, ErNativeStyle)>,
        relationship_html_labels: bool,
        model: &merman_model::families::er::ErDiagramModel,
        layout: ErPreparedSvgLayout<'_>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let entities = model.entities.values().filter_map(|entity| {
            layout
                .prepared_labels
                .entity(&entity.id)
                .map(|measurement| (entity, measurement))
        });
        let mut entity_indices = BTreeMap::new();
        let mut entity_source_styles = Vec::with_capacity(model.entities.len());
        for (index, (entity, measurement)) in entities.clone().enumerate() {
            entity_indices.insert(entity.id.clone(), index);
            entity_source_styles.push(measurement.source_style.clone());
        }
        let css_binding = match native {
            Some((theme, native)) => ErCssBinding::from_native(theme, native, config),
            None => ErCssBinding::resolve(config),
        };
        let ErSubgraphSourceStyles {
            subgraphs,
            styles: subgraph_source_styles,
        } = prepare_er_subgraph_source_styles(model, layout.geometry.clusters, work_meter)?;
        let config = super::ErConfigView::new(config);
        // Only top-level lanes use flowchart.htmlLabels and unconditional Markdown titles.
        // Nested groups retain the ordinary cluster writer's root mode and label type.
        let swimlane_lanes = layout
            .swimlane
            .iter()
            .flat_map(|paint| &paint.lanes)
            .filter(|lane| lane.parent_id.is_none())
            .map(|lane| (lane.id.as_str(), lane))
            .collect::<BTreeMap<_, _>>();
        let mut html_subgraph_labels = BTreeMap::new();
        for cluster in layout.geometry.clusters {
            let lane = swimlane_lanes.get(cluster.id.as_str());
            if !config.subgraph_html_labels(lane.is_some(), relationship_html_labels)
                || html_subgraph_labels.contains_key(&cluster.id)
            {
                continue;
            }
            let (title, markdown) = if let Some(lane) = lane {
                (lane.title.as_str(), true)
            } else if let Some(subgraph) = subgraphs.get(cluster.id.as_str()) {
                (
                    subgraph.title.as_str(),
                    !matches!(subgraph.label_type.as_str(), "string" | "text"),
                )
            } else {
                continue;
            };
            let retained_meter = layout
                .prepared_labels
                .retained_work_meter
                .as_ref()
                .expect("ER prepared layout labels retain their operation ledger");
            let (key, label) =
                ErPreparedHtmlSubgraphLabel::prepare(&cluster.id, retained_meter, || {
                    if markdown {
                        mermaid_markdown_to_xhtml_label_fragment(title, true)
                    } else {
                        escape_xml(title)
                    }
                })?;
            work_meter.charge(
                1usize
                    .saturating_add(key.len())
                    .saturating_add(label.fragment.len()),
            )?;
            html_subgraph_labels.insert(key, label);
        }
        let mut subgraph_source_styles = subgraph_source_styles;
        prepare_er_source_visuals(
            &css_binding,
            entities
                .map(|(entity, _)| entity)
                .zip(&mut entity_source_styles),
            &mut subgraph_source_styles,
            work_meter,
        )?;
        Ok(Self {
            css_binding,
            entity_indices,
            entity_source_styles,
            subgraph_source_styles,
            html_subgraph_labels,
            relationship_html_labels,
            _prepared_labels: Arc::clone(layout.prepared_labels),
        })
    }

    pub(super) fn index_for_entity_id(&self, entity_id: &str) -> Option<usize> {
        self.entity_indices.get(entity_id).copied()
    }

    pub fn css_binding(&self) -> &ErCssBinding {
        &self.css_binding
    }

    pub(super) const fn relationship_html_labels(&self) -> bool {
        self.relationship_html_labels
    }

    pub(super) fn source_style(&self, entity_index: usize) -> Option<&ErEntitySourceStyle> {
        self.entity_source_styles.get(entity_index)
    }

    pub(super) fn subgraph_source_style(&self, id: &str) -> Option<&ErEntitySourceStyle> {
        self.subgraph_source_styles.get(id)
    }

    pub(super) fn html_subgraph_label(&self, id: &str) -> Option<&ErPreparedHtmlSubgraphLabel> {
        self.html_subgraph_labels.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_subgraph_retained_capacity_is_shared_until_the_last_owner_drops() {
        let meter = Arc::new(OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let (key, label) = ErPreparedHtmlSubgraphLabel::prepare("Lane", &meter, || {
            let mut fragment = String::with_capacity(4096);
            fragment.push_str("<p>Visible title</p>");
            fragment
        })
        .unwrap();
        let bytes = std::mem::size_of::<(String, ErPreparedHtmlSubgraphLabel)>()
            + std::mem::size_of::<usize>()
            + key.capacity()
            + label.fragment.capacity();
        assert!(label.fragment.capacity() > label.fragment.len());
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        let owner = Arc::new(BTreeMap::from([(key, label)]));
        let replay = Arc::clone(&owner);
        drop(owner);
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        drop(replay);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn html_subgraph_retained_preflight_and_exact_capacity_limit() {
        use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};
        let prepare = |meter: &Arc<OperationWorkMeter>, projected: &std::cell::Cell<bool>| {
            ErPreparedHtmlSubgraphLabel::prepare("Lane", meter, || {
                projected.set(true);
                let mut fragment = String::with_capacity(4096);
                fragment.push_str("<p>Visible title</p>");
                fragment
            })
        };
        let measuring = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let label = prepare(&measuring, &std::cell::Cell::new(false)).unwrap();
        let bytes = measuring.prepared_text_retained_bytes();
        drop(label);
        assert_eq!(measuring.prepared_text_retained_bytes(), 0);
        for ceiling in [1, bytes - 1, bytes] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, ceiling)
                    .unwrap(),
            ));
            let projected = std::cell::Cell::new(false);
            let result = prepare(&meter, &projected);
            if ceiling == bytes {
                let label = result.expect("exact retained capacity is admitted");
                assert_eq!(meter.prepared_text_retained_bytes(), bytes);
                drop(label);
            } else {
                let OperationWorkError::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected the retained HTML-title byte ceiling");
                };
                assert_eq!(error.limit, "max_prepared_text_retained_bytes");
                assert_eq!(error.max, ceiling);
                assert!(error.actual > ceiling);
                assert_eq!(projected.get(), ceiling != 1);
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }
}
