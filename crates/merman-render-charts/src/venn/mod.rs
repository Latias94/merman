//! Typed Venn layout and the source-backed geometry kernel.

mod config;
mod layout;
mod model;
mod solver;

pub use layout::{VennFontSizes, layout_venn_diagram_typed_with_work_meter};
pub use model::{
    VennAreaLayout, VennCircleLayout, VennDiagramLayout, VennTextAreaLayout,
    VennTextDebugCellLayout, VennTextNodeLayout,
};
pub use solver::{
    VennArc, VennArea, VennCircle, VennLayoutArea, VennLayoutError, VennLayoutOptions,
    VennLayoutResult, VennPoint, VennSolution, VennTextPoint, circle_area,
    circle_circle_intersection, circle_overlap, compute_text_centre, compute_venn_layout,
    disjoint_cluster, distance, distance_from_intersect_area, greedy_layout, intersection_area,
    intersection_area_path, loss_function, normalize_solution, scale_solution, venn,
};
