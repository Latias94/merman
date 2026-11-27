//! Measured Flowchart projection for Mermaid Layered.

use std::collections::HashMap;
use std::fmt;

use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::graph_layout::{
    BorderStitch, RoutingStyle, SolvedEdge,
    mermaid_layered::{self, LayeredError, Options},
};
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutNode,
    math::MathRenderer,
    resources::{OperationWorkError, OperationWorkMeter},
    text::TextMeasurer,
};

use crate::class_style::FlowchartClassDefinitions;
use crate::measured::FlowchartMeasurement;
use crate::projection::{FlowchartEdgeOwners, FlowchartRenderModelRef};
use crate::style::FlowchartEdgeStylePlan;
use crate::svg_label::FlowchartSvgLabelSidecarBuilder;

mod input;
mod projection;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowLayeredRouting {
    Rounded,
    Curved,
    Stepped,
    Straight,
}

impl FlowLayeredRouting {
    fn from_config(config: &PresentationConfig) -> Self {
        match config
            .get("layered")
            .and_then(|layered| layered.get("edgeRouting"))
            .and_then(serde_json::Value::as_str)
        {
            Some("curved") => Self::Curved,
            Some("stepped") => Self::Stepped,
            Some("straight") => Self::Straight,
            _ => Self::Rounded,
        }
    }

    const fn geometry(self) -> RoutingStyle {
        match self {
            Self::Rounded | Self::Stepped => RoutingStyle::Orthogonal,
            Self::Curved => RoutingStyle::Curved,
            Self::Straight => RoutingStyle::Polyline,
        }
    }
}

#[derive(Debug)]
pub struct FlowLayeredRoutes {
    pub routing: FlowLayeredRouting,
    /// Group slots index `FlowLayeredLayout::clusters`.
    pub group_parents: Vec<Option<u32>>,
    /// Aligned with the materialized `edges` and their owners. Node endpoints index `nodes`.
    pub edges: Vec<SolvedEdge>,
    pub border_stitches: Vec<BorderStitch>,
}

#[derive(Debug)]
pub struct FlowLayeredLayout {
    /// Leaf slots come first, in FlowDB order, followed by expanded cluster nodes.
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub edge_owners: FlowchartEdgeOwners,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    pub dom_node_order_by_root: HashMap<String, Vec<String>>,
    pub routes: FlowLayeredRoutes,
}

#[derive(Debug)]
pub enum FlowLayeredError {
    Provider(LayeredError),
    DuplicateEdgeId { id: String },
    InvalidNesting { id: String },
    Host(Error),
}

impl FlowLayeredError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Provider(error) => error.is_recoverable(),
            Self::DuplicateEdgeId { .. } | Self::InvalidNesting { .. } => true,
            Self::Host(_) => false,
        }
    }
}

impl fmt::Display for FlowLayeredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => fmt::Display::fmt(error, f),
            Self::DuplicateEdgeId { id } => write!(f, "layout: duplicate edge id '{id}'"),
            Self::InvalidNesting { id } => {
                write!(f, "layered: subgraph '{id}' is nested inside itself")
            }
            Self::Host(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for FlowLayeredError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(error) => Some(error),
            Self::Host(error) => Some(error),
            _ => None,
        }
    }
}

impl From<Error> for FlowLayeredError {
    fn from(error: Error) -> Self {
        Self::Host(error)
    }
}

impl From<OperationWorkError> for FlowLayeredError {
    fn from(error: OperationWorkError) -> Self {
        Self::Host(error.into())
    }
}

/// Runs a single measured Layered attempt on the caller's operation meter.
#[allow(
    clippy::too_many_arguments,
    reason = "The Flow layout adapter borrows the prepared configuration and operation services."
)]
pub fn layout(
    view: FlowchartRenderModelRef<'_>,
    class_defs: &FlowchartClassDefinitions<'_>,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style: &FlowchartEdgeStylePlan,
    work: &OperationWorkMeter,
) -> Result<FlowLayeredLayout, FlowLayeredError> {
    let routing = FlowLayeredRouting::from_config(config);
    let measurement = FlowchartMeasurement::new(
        view,
        class_defs,
        config,
        measurer,
        math_renderer,
        labels,
        edge_style,
    );
    let output = (|| {
        let input = input::FlowLayeredInput::measure(view, config, &measurement, routing, work)?;
        let solved =
            mermaid_layered::layout(&input.graph, Options::default(), work).map_err(|error| {
                match error {
                    LayeredError::Work(error) => error.into(),
                    LayeredError::ArithmeticOverflow => work.arithmetic_overflow().into(),
                    LayeredError::AllocationFailed => allocation_error().into(),
                    error => FlowLayeredError::Provider(error),
                }
            })?;
        input.project(
            solved.graph,
            routing,
            measurement.settings.cluster_padding,
            work,
        )
    })();
    work.checkpoint(OperationPhase::Layout)?;
    output
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

fn reserve<T>(amount: usize, work: &OperationWorkMeter) -> Result<Vec<T>, FlowLayeredError> {
    work.charge(amount)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(amount)
        .map_err(|_| allocation_error())?;
    Ok(values)
}

fn owned(text: &str, work: &OperationWorkMeter) -> Result<String, FlowLayeredError> {
    work.charge(text.len().max(1))?;
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| allocation_error())?;
    result.push_str(text);
    Ok(result)
}
