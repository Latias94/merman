//! Operation-local measured geometry retained by one State family artifact.

use std::cell::RefCell;
use std::collections::BTreeMap;

use merman_render_core::{
    entities::decode_mermaid_entities_for_render_text,
    text::{
        TextMeasurer, WrapMode,
        inline_html::{MermaidMarkdownWordType, svg_markdown_word_style},
    },
};

use super::{
    label::{StateLabelMeasurement, measure_state_markdown_label, measure_state_svg_plain_label},
    label_geometry::StateNativeLabelGeometry,
    style_plan::ResolvedLabelTypography,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateLabelSourceKind {
    Markdown,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateLabelOwner<'a> {
    Node(&'a str),
    NodeTitle(&'a str),
    NodeDescription(&'a str),
    ClusterTitle(&'a str),
    Edge(&'a str),
}

/// Borrows the resolved typography and the operation's measurer for one label.
#[derive(Clone, Copy)]
pub struct StateLabelMetricsRequest<'a> {
    pub owner: StateLabelOwner<'a>,
    pub text: &'a str,
    pub source_kind: StateLabelSourceKind,
    pub measurer: &'a dyn TextMeasurer,
    pub typography: &'a ResolvedLabelTypography,
    pub max_width_px: Option<f64>,
    pub wrap_mode: WrapMode,
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
pub struct StateLabelSidecarBuilder {
    measured_native_geometries: RefCell<StateLabelSlots<StateNativeLabelGeometry>>,
    #[cfg(feature = "layout-grid")]
    grid_labels: RefCell<Option<super::grid::StateGridLabels>>,
}

impl StateLabelSidecarBuilder {
    #[cfg(feature = "layout-grid")]
    pub(super) fn set_grid_labels(&self, labels: super::grid::StateGridLabels) {
        self.grid_labels.replace(Some(labels));
    }

    pub fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<StateNativeLabelGeometry> {
        let geometries = self.measured_native_geometries.borrow();
        label_slot(&geometries, owner).cloned()
    }

    pub fn measure_for_layout(
        &self,
        request: StateLabelMetricsRequest<'_>,
    ) -> StateLabelMeasurement {
        let measurer = request
            .typography
            .svg_measurer(request.measurer, request.wrap_mode);
        let request = StateLabelMetricsRequest {
            measurer: measurer.as_measurer(),
            ..request
        };
        let mut measurement = measure_label(request);
        if let Some(geometry) = StateNativeLabelGeometry::from_measurement(request, &measurement) {
            measurement.metrics = geometry.layout_metrics();
            self.measured_native_geometries
                .borrow_mut()
                .insert(request.owner, geometry);
        }
        measurement
    }

    pub fn finish(self) -> StateLabelSidecar {
        StateLabelSidecar {
            measured_native_geometries: self.measured_native_geometries.into_inner(),
            #[cfg(feature = "layout-grid")]
            grid_labels: self.grid_labels.into_inner(),
        }
    }
}

#[cfg(feature = "layout-layered")]
pub(super) fn measure_label_for_layout(
    request: StateLabelMetricsRequest<'_>,
) -> StateLabelMeasurement {
    let measurer = request
        .typography
        .svg_measurer(request.measurer, request.wrap_mode);
    measure_label(StateLabelMetricsRequest {
        measurer: measurer.as_measurer(),
        ..request
    })
}

fn measure_label(request: StateLabelMetricsRequest<'_>) -> StateLabelMeasurement {
    let style = if request.wrap_mode == WrapMode::HtmlLike {
        std::borrow::Cow::Borrowed(request.typography.text_style())
    } else {
        svg_markdown_word_style(
            request.typography.text_style(),
            MermaidMarkdownWordType::Normal,
        )
    };
    match request.source_kind {
        StateLabelSourceKind::Markdown => measure_state_markdown_label(
            request.text,
            request.measurer,
            &style,
            request.max_width_px,
            request.wrap_mode,
        ),
        StateLabelSourceKind::Plain if request.wrap_mode != WrapMode::HtmlLike => {
            StateLabelMeasurement {
                metrics: measure_state_svg_plain_label(request.text, request.measurer, &style),
                uses_html_wrapping_table: false,
            }
        }
        StateLabelSourceKind::Plain => {
            let text = decode_mermaid_entities_for_render_text(request.text);
            StateLabelMeasurement {
                metrics: request.measurer.measure_wrapped(
                    text.as_ref(),
                    &style,
                    request.max_width_px,
                    request.wrap_mode,
                ),
                uses_html_wrapping_table: false,
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct StateLabelSidecar {
    measured_native_geometries: StateLabelSlots<StateNativeLabelGeometry>,
    #[cfg(feature = "layout-grid")]
    grid_labels: Option<super::grid::StateGridLabels>,
}

impl StateLabelSidecar {
    #[cfg(feature = "layout-grid")]
    pub(super) fn grid_labels(&self) -> &[super::grid::StateGridLabel] {
        self.grid_labels
            .as_ref()
            .map_or(&[], |grid| grid.labels.as_slice())
    }

    pub fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<&StateNativeLabelGeometry> {
        label_slot(&self.measured_native_geometries, owner)
    }
}
