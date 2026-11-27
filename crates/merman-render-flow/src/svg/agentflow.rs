//! The facade supplies container ordinals from the prepared Flowchart projection.

use super::FlowchartRenderCtx;

pub(super) use crate::stylesheet::ContainerPalette;

pub(super) fn container_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    let ordinal = ctx.model.subgraph_color_ordinal(id)?;
    ctx.palette.slot_for_ordinal(ctx.diagram_type, ordinal)
}
