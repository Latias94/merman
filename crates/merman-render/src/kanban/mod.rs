//! Kanban layout types remain available through [`crate::model`].

pub(crate) use merman_render_structured::kanban::{
    KanbanPreparedArtifact, KanbanSvgContext, prepare_kanban_diagram, render_kanban_diagram_svg,
};

// The prepared-text budget uses the artifact's inline size. The provider stores the same
// two widths without depending on the host observation type.
#[cfg(test)]
const _: () = {
    use std::mem::{align_of, size_of};

    type Observation = Option<crate::LayoutAdaptationObservation>;
    type Widths = Option<(f64, f64)>;
    assert!(size_of::<Observation>() == size_of::<Widths>());
    assert!(align_of::<Observation>() == align_of::<Widths>());
};
