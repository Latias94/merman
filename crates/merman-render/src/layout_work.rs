pub(crate) use merman_render_core::layout_work::OperationLayoutWorkControl;

/// Attributes the solver call, excluding the adapter's input and output projections.
#[cfg(any(
    feature = "layout-cytoscape",
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
#[allow(
    unused_macros,
    reason = "Solver calls depend on the selected diagram features."
)]
macro_rules! measure_layout_solve {
    ($solve:expr) => {{
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        $solve
    }};
}
#[cfg(any(
    feature = "layout-cytoscape",
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
pub(crate) use measure_layout_solve;
