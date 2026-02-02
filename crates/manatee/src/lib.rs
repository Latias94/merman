#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub positions: std::collections::BTreeMap<String, Point>,
}

#[derive(Debug, Clone)]
pub enum Algorithm {
    /// Cytoscape COSE-Bilkent (Mermaid mindmap default).
    CoseBilkent(CoseBilkentOptions),
    /// Cytoscape FCoSE (Mermaid architecture layout).
    Fcose(FcoseOptions),
}

#[derive(Debug, Clone)]
pub struct CoseBilkentOptions {
    pub random_seed: u64,
}

impl Default for CoseBilkentOptions {
    fn default() -> Self {
        Self { random_seed: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct FcoseOptions {
    pub random_seed: u64,
}

impl Default for FcoseOptions {
    fn default() -> Self {
        Self { random_seed: 0 }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("graph contains an edge with a missing endpoint: {edge_id}")]
    MissingEndpoint { edge_id: String },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Headless layout entry point.
///
/// This is currently a scaffold. Implementations will be added incrementally as the port
/// progresses.
pub fn layout(graph: &Graph, _algorithm: Algorithm) -> Result<LayoutResult> {
    let mut node_exists: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for n in &graph.nodes {
        node_exists.insert(n.id.as_str());
    }
    for e in &graph.edges {
        if !node_exists.contains(e.source.as_str()) || !node_exists.contains(e.target.as_str()) {
            return Err(Error::MissingEndpoint {
                edge_id: e.id.clone(),
            });
        }
    }

    // Temporary deterministic fallback: preserve sorted-by-id order and place nodes on a line.
    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();
    let mut ids = graph
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect::<Vec<_>>();
    ids.sort();
    for (idx, id) in ids.iter().enumerate() {
        positions.insert(
            (*id).to_string(),
            Point {
                x: idx as f64 * 100.0,
                y: 0.0,
            },
        );
    }

    Ok(LayoutResult { positions })
}
