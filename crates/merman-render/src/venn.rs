//! Venn geometry and solver helpers.

pub use merman_render_charts::venn::{
    VennArc, VennArea, VennCircle, VennLayoutArea, VennLayoutError, VennLayoutOptions,
    VennLayoutResult, VennPoint, VennSolution, VennTextPoint, circle_area,
    circle_circle_intersection, circle_overlap, compute_text_centre, compute_venn_layout,
    disjoint_cluster, distance, distance_from_intersect_area, greedy_layout, intersection_area,
    intersection_area_path, loss_function, normalize_solution, scale_solution, venn,
};
