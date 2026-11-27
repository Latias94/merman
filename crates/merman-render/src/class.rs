//! Class layout providers and compatibility exports.

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
use merman_render_graph::class::ClassDiagramModel;
pub(crate) use merman_render_graph::class::{
    ClassNodeVisualPlan, ClassPreparedLabels, ClassRelationThemePlan, ClassRenderConfig,
    ClassTextThemePlan, class_layout_work_units, class_requires_math,
};
#[cfg(feature = "layout-shoal")]
use merman_render_graph::class::{class_namespace_label, normalize_dir};

#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk_layout;
#[cfg(feature = "layout-shoal")]
mod measured;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;

#[doc(hidden)]
#[cfg(feature = "layout-dagre")]
pub use dagre::debug_build_class_diagram_dagre_graph;
#[cfg(feature = "layout-dagre")]
pub(crate) use dagre::layout as layout_class_diagram_typed_with_config;
#[cfg(feature = "layout-elk")]
pub(crate) use elk_layout::layout_class_diagram_elk_typed_with_config_and_operation_seed;

#[cfg(all(test, feature = "layout-dagre"))]
mod native_theme;
#[cfg(all(test, feature = "layout-dagre"))]
mod tests;
