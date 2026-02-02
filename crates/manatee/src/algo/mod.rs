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
}

impl Default for FcoseOptions {
    fn default() -> Self {
        Self { random_seed: 0 }
    }
}
