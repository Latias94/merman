pub mod cose_bilkent;
pub mod fcose;

#[derive(Debug, Clone)]
pub enum Algorithm {
    /// Cytoscape COSE-Bilkent (Mermaid mindmap default).
    CoseBilkent(CoseBilkentOptions),
    /// Cytoscape FCoSE (Mermaid architecture layout).
    Fcose(FcoseOptions),
}

#[derive(Debug, Clone)]
pub struct CoseBilkentOptions {
    /// Seed for deterministic randomness. The upstream JS implementation relies on `Math.random`,
    /// so the Rust port will use a reproducible RNG here.
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
    /// Override for layout-base/CoSE `DEFAULT_EDGE_LENGTH` (used for repulsion/grid range, overlap
    /// separation buffer, and convergence thresholds).
    ///
    /// In upstream Cytoscape FCoSE, `DEFAULT_EDGE_LENGTH` is derived from the `idealEdgeLength`
    /// option (before inter-graph nesting/smart adjustments), then used by layout-base constants
    /// such as `MIN_REPULSION_DIST` and the FR-grid cell size. Keeping this value aligned is
    /// important for parity with Mermaid-generated SVG baselines.
    pub default_edge_length: Option<f64>,
    pub alignment_constraint: Option<AlignmentConstraint>,
    pub relative_placement_constraint: Vec<RelativePlacementConstraint>,
}

impl Default for FcoseOptions {
    fn default() -> Self {
        Self {
            random_seed: 0,
            default_edge_length: None,
            alignment_constraint: None,
            relative_placement_constraint: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AlignmentConstraint {
    /// Nodes in each inner vec share the same y coordinate (horizontal alignment).
    pub horizontal: Vec<Vec<String>>,
    /// Nodes in each inner vec share the same x coordinate (vertical alignment).
    pub vertical: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct RelativePlacementConstraint {
    pub left: Option<String>,
    pub right: Option<String>,
    pub top: Option<String>,
    pub bottom: Option<String>,
    pub gap: f64,
}
