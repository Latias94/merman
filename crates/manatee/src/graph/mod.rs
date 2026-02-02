use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn validate(&self) -> Result<()> {
        let mut node_exists: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for n in &self.nodes {
            node_exists.insert(n.id.as_str());
        }
        for e in &self.edges {
            if !node_exists.contains(e.source.as_str()) || !node_exists.contains(e.target.as_str())
            {
                return Err(Error::MissingEndpoint {
                    edge_id: e.id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub width: f64,
    pub height: f64,
    /// Optional initial position (center), mirroring Cytoscape's `position` field.
    pub x: f64,
    pub y: f64,
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
