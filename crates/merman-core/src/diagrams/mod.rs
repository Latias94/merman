//! Private family grammars, source construction and source projection callbacks.

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn source_metadata_for_test(
    metadata: &crate::PresentationMetadata,
) -> crate::SourcePresentationMetadata {
    let effective_config = crate::presentation::SourcePresentationConfig::from_frozen_controlled(
        metadata.effective_config.clone(),
        &crate::OperationControl::new(),
    )
    .expect("a private fixture control cannot be cancelled");
    crate::SourcePresentationMetadata {
        diagram_type: metadata.diagram_type.clone(),
        config: metadata.config.clone(),
        effective_config,
        title: metadata.title.clone(),
    }
}

#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]
macro_rules! include_checked_in_lalrpop_parser {
    ($(#[$attr:meta])* $name:ident, $file:literal) => {
        #[rustfmt::skip]
        #[allow(clippy::extra_unused_lifetimes)]
        #[allow(clippy::needless_lifetimes)]
        #[allow(clippy::let_unit_value)]
        #[allow(clippy::just_underscores_and_digits)]
        $(#[$attr])*
        mod $name {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/generated/lalrpop/",
                $file
            ));
        }
    };
}

#[cfg(feature = "diagram-agentflow")]
pub(crate) mod agentflow;
#[cfg(feature = "diagram-architecture")]
pub(crate) mod architecture;
#[cfg(feature = "diagram-block")]
pub(crate) mod block;
#[cfg(feature = "diagram-c4")]
pub(crate) mod c4;
#[cfg(feature = "diagram-class")]
pub(crate) mod class;
#[cfg(feature = "diagram-cynefin")]
pub(crate) mod cynefin;
#[cfg(feature = "diagram-er")]
pub(crate) mod er;
pub(crate) mod error_diagram;
#[cfg(feature = "diagram-event-modeling")]
pub(crate) mod eventmodeling;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) mod flowchart;
#[cfg(feature = "diagram-gantt")]
pub(crate) mod gantt;
#[cfg(feature = "diagram-git-graph")]
pub(crate) mod git_graph;
#[cfg(feature = "diagram-info")]
pub(crate) mod info;
#[cfg(feature = "diagram-ishikawa")]
pub(crate) mod ishikawa;
#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
mod jison_unicode;
#[cfg(feature = "diagram-journey")]
pub(crate) mod journey;
#[cfg(feature = "diagram-kanban")]
pub(crate) mod kanban;
#[cfg(any(
    test,
    feature = "diagram-architecture",
    feature = "diagram-cynefin",
    feature = "diagram-event-modeling",
    feature = "diagram-git-graph",
    feature = "diagram-info",
    feature = "diagram-packet",
    feature = "diagram-pie",
    feature = "diagram-radar",
    feature = "diagram-venn",
    feature = "diagram-treemap",
    feature = "diagram-tree-view",
    feature = "diagram-wardley",
    feature = "diagram-zenuml"
))]
#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) mod langium_common;
#[cfg(feature = "diagram-mindmap")]
pub(crate) mod mindmap;
#[cfg(feature = "diagram-packet")]
pub(crate) mod packet;
#[cfg(feature = "diagram-pie")]
pub(crate) mod pie;
#[cfg(feature = "diagram-quadrant-chart")]
pub(crate) mod quadrant_chart;
#[cfg(feature = "diagram-radar")]
pub(crate) mod radar;
#[cfg(feature = "diagram-railroad")]
pub(crate) mod railroad;
#[cfg(feature = "diagram-requirement")]
pub(crate) mod requirement;
#[cfg(feature = "diagram-sankey")]
pub(crate) mod sankey;
// Common metadata normalization also uses ECMAScript scanning without diagram features.
#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) mod scan;
#[cfg(feature = "diagram-sequence")]
pub(crate) mod sequence;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
use merman_model::families::flowchart::shapes;
#[cfg(feature = "diagram-state")]
pub(crate) mod state;
#[cfg(feature = "diagram-timeline")]
pub(crate) mod timeline;
#[cfg(feature = "diagram-tree-view")]
pub(crate) mod tree_view;
#[cfg(feature = "diagram-treemap")]
pub(crate) mod treemap;
#[cfg(feature = "diagram-usecase")]
pub(crate) mod usecase;
#[cfg(feature = "diagram-venn")]
pub(crate) mod venn;
#[cfg(feature = "diagram-wardley")]
pub(crate) mod wardley;
#[cfg(feature = "diagram-xychart")]
pub(crate) mod xychart;
#[cfg(feature = "diagram-zenuml")]
pub(crate) mod zenuml;
