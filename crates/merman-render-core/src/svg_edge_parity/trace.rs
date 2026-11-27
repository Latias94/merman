//! Caller-owned diagnostics for Flow edge projection.

use std::sync::{Arc, Mutex, PoisonError};

/// A point captured while diagnosing one flowchart edge route.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FlowchartEdgeTracePoint {
    pub x: f64,
    pub y: f64,
}

/// An in-memory diagnostic record for one flowchart edge route.
///
/// The renderer never serializes this value or writes it to a host filesystem. Callers that need
/// a file own that I/O boundary and can serialize a drained record themselves.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FlowchartEdgeTrace {
    pub fixture_diagram_id: String,
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub layout_from: String,
    pub layout_to: String,
    pub from_cluster: Option<String>,
    pub to_cluster: Option<String>,
    pub origin_x: f64,
    pub origin_y: f64,
    pub tx: f64,
    pub ty: f64,
    pub base_points: Vec<FlowchartEdgeTracePoint>,
    pub points_after_intersect: Vec<FlowchartEdgeTracePoint>,
    pub points_for_render: Vec<FlowchartEdgeTracePoint>,
    pub points_for_data_points: Vec<FlowchartEdgeTracePoint>,
}

/// Explicit, caller-owned storage for flowchart route diagnostics.
///
/// Clones share one collection so a caller can retain a handle while a render owns a clone. A
/// poisoned lock is recovered because trace collection is diagnostic-only and must not turn a
/// completed render into an ambient host failure.
#[derive(Debug, Clone, Default)]
pub struct FlowchartEdgeTraceCollector(Arc<Mutex<Vec<FlowchartEdgeTrace>>>);

impl FlowchartEdgeTraceCollector {
    #[doc(hidden)]
    pub fn record(&self, trace: FlowchartEdgeTrace) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(trace);
    }

    /// Returns a snapshot without consuming the collected records.
    pub fn snapshot(&self) -> Vec<FlowchartEdgeTrace> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Drains all records collected so far.
    pub fn drain(&self) -> Vec<FlowchartEdgeTrace> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
