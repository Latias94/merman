#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! This crate owns the ELK-specific dependency surface so higher-level crates can keep the layout
//! engine optional. The current package boundary is intentionally small; renderer integration will
//! grow from this API as the first `flowchart-elk` admission slice lands.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Mermaid's default ELK layout, equivalent to upstream `elk.layered`.
    Layered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    Left,
    Right,
    Up,
    #[default]
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeKind {
    #[default]
    Leaf,
    Group,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub id: String,
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<Direction>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<Label>,
    pub minlen: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutResult {
    pub nodes: Vec<NodeLayout>,
    pub edges: Vec<EdgeLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    pub id: String,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ELK layout algorithm is not implemented yet: {algorithm:?}")]
    UnsupportedAlgorithm { algorithm: Algorithm },
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    let _ = graph;
    Err(Error::UnsupportedAlgorithm { algorithm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_reports_unsupported_until_backend_is_admitted() {
        let err = layout(&Graph::default(), Algorithm::Layered).unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedAlgorithm {
                algorithm: Algorithm::Layered
            }
        ));
    }
}
