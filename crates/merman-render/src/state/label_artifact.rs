//! Operation-local measured geometry retained by one State family artifact.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::entities::decode_mermaid_entities_for_render_text;
use crate::text::{TextMeasurer, WrapMode};

use super::{
    ResolvedLabelTypography, StateLabelMeasurement, StateNativeLabelGeometry,
    measure_state_markdown_label,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelSourceKind {
    Markdown,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelOwner<'a> {
    Node(&'a str),
    NodeTitle(&'a str),
    NodeDescription(&'a str),
    ClusterTitle(&'a str),
    Edge(&'a str),
}

#[derive(Clone, Copy)]
pub(crate) struct StateLabelMetricsRequest<'a> {
    pub(crate) owner: StateLabelOwner<'a>,
    pub(crate) text: &'a str,
    pub(crate) source_kind: StateLabelSourceKind,
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) typography: &'a ResolvedLabelTypography,
    pub(crate) max_width_px: Option<f64>,
    pub(crate) wrap_mode: WrapMode,
}

#[derive(Debug)]
struct StateLabelSlots<T> {
    nodes: BTreeMap<Box<str>, T>,
    node_titles: BTreeMap<Box<str>, T>,
    node_descriptions: BTreeMap<Box<str>, T>,
    cluster_titles: BTreeMap<Box<str>, T>,
    edges: BTreeMap<Box<str>, T>,
}

impl<T> Default for StateLabelSlots<T> {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            node_titles: BTreeMap::new(),
            node_descriptions: BTreeMap::new(),
            cluster_titles: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
}

impl<T> StateLabelSlots<T> {
    fn insert(&mut self, owner: StateLabelOwner<'_>, value: T) {
        match owner {
            StateLabelOwner::Node(id) => {
                self.nodes.insert(id.into(), value);
            }
            StateLabelOwner::NodeTitle(id) => {
                self.node_titles.insert(id.into(), value);
            }
            StateLabelOwner::NodeDescription(id) => {
                self.node_descriptions.insert(id.into(), value);
            }
            StateLabelOwner::ClusterTitle(id) => {
                self.cluster_titles.insert(id.into(), value);
            }
            StateLabelOwner::Edge(id) => {
                self.edges.insert(id.into(), value);
            }
        }
    }
}

fn label_slot<'a, T>(slots: &'a StateLabelSlots<T>, owner: StateLabelOwner<'_>) -> Option<&'a T> {
    match owner {
        StateLabelOwner::Node(id) => slots.nodes.get(id),
        StateLabelOwner::NodeTitle(id) => slots.node_titles.get(id),
        StateLabelOwner::NodeDescription(id) => slots.node_descriptions.get(id),
        StateLabelOwner::ClusterTitle(id) => slots.cluster_titles.get(id),
        StateLabelOwner::Edge(id) => slots.edges.get(id),
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecarBuilder {
    measured_native_geometries: RefCell<StateLabelSlots<StateNativeLabelGeometry>>,
}

impl StateLabelSidecarBuilder {
    pub(crate) fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<StateNativeLabelGeometry> {
        let geometries = self.measured_native_geometries.borrow();
        label_slot(&geometries, owner).cloned()
    }

    pub(crate) fn measure_for_layout(
        &self,
        request: StateLabelMetricsRequest<'_>,
    ) -> StateLabelMeasurement {
        let mut measurement = measure_label(request);
        if let Some(geometry) = StateNativeLabelGeometry::from_measurement(request, &measurement) {
            measurement.metrics = geometry.layout_metrics();
            self.measured_native_geometries
                .borrow_mut()
                .insert(request.owner, geometry);
        }
        measurement
    }

    pub(crate) fn finish(self) -> StateLabelSidecar {
        StateLabelSidecar {
            measured_native_geometries: self.measured_native_geometries.into_inner(),
        }
    }
}

fn measure_label(request: StateLabelMetricsRequest<'_>) -> StateLabelMeasurement {
    match request.source_kind {
        StateLabelSourceKind::Markdown => measure_state_markdown_label(
            request.text,
            request.measurer,
            request.typography.text_style(),
            request.max_width_px,
            request.wrap_mode,
        ),
        StateLabelSourceKind::Plain => {
            let text = decode_mermaid_entities_for_render_text(request.text);
            StateLabelMeasurement {
                metrics: request.measurer.measure_wrapped(
                    text.as_ref(),
                    request.typography.text_style(),
                    request.max_width_px,
                    request.wrap_mode,
                ),
                uses_html_wrapping_table: false,
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecar {
    measured_native_geometries: StateLabelSlots<StateNativeLabelGeometry>,
}

impl StateLabelSidecar {
    pub(crate) fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<&StateNativeLabelGeometry> {
        label_slot(&self.measured_native_geometries, owner)
    }
}
