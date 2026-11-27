use super::BlockTypographyThemePlan;
use crate::model::BlockDiagramLayout;
use crate::resources::OperationWorkMeter;
use merman_model::families::block::BlockDiagramRenderModel;
use std::sync::Arc;

#[derive(Debug, Default)]
pub(crate) struct BlockEdgeRoutes {
    #[cfg(feature = "layout-shoal")]
    orthogonal: Vec<Option<BlockOrthogonalRoute>>,
}

#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_structured::block::BlockOrthogonalRoute;

impl BlockEdgeRoutes {
    pub(crate) fn prepare(
        model: &BlockDiagramRenderModel,
        layout: &mut BlockDiagramLayout,
        typography: &BlockTypographyThemePlan,
        #[cfg(feature = "layout-shoal")] backend: crate::layout_backend::GraphLayoutBackend,
        work: &Arc<OperationWorkMeter>,
    ) -> crate::Result<Self> {
        #[cfg(feature = "layout-shoal")]
        if !layout.edges.is_empty() && backend == crate::layout_backend::GraphLayoutBackend::Shoal {
            return Ok(Self {
                orthogonal: super::orthogonal::prepare(model, layout, typography, work)?,
            });
        }
        #[cfg(not(feature = "layout-shoal"))]
        let _ = (model, layout, typography, work);
        Ok(Self::default())
    }

    pub(crate) fn as_slice(
        &self,
    ) -> &[Option<merman_render_structured::block::BlockOrthogonalRoute>] {
        #[cfg(feature = "layout-shoal")]
        {
            &self.orthogonal
        }
        #[cfg(not(feature = "layout-shoal"))]
        {
            &[]
        }
    }
}
