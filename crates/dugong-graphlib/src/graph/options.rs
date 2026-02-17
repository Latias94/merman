//! Graph configuration options.

#[derive(Debug, Clone, Copy)]
pub struct GraphOptions {
    pub multigraph: bool,
    pub compound: bool,
    pub directed: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            multigraph: false,
            compound: false,
            directed: true,
        }
    }
}
