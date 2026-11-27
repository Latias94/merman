//! Measurement and layout geometry for single-contour outer-path bodies.

use crate::node_body::{OuterPathBody, OuterPathTranslation};

use crate::node_geometry::DelayGeometry;
use crate::node_geometry::DisplayGeometry;
use crate::svg::types::FlowchartRenderCtx;

use super::super::{FlowchartNodeGeometry, FlowchartNodeLabelState};
use super::shape_label_metrics;

pub(super) enum OuterPathShape {
    BowTieRectangle,
    CurvedTrapezoid,
    Delay,
    NotchedPentagon,
}

impl OuterPathShape {
    pub(super) fn prepare(
        self,
        ctx: &FlowchartRenderCtx<'_>,
        common: &FlowchartNodeGeometry<'_>,
        label: &FlowchartNodeLabelState<'_>,
    ) -> OuterPathBody {
        let layout_node = match self {
            Self::BowTieRectangle => None,
            _ => Some(common.layout_node),
        };
        let metrics = shape_label_metrics(ctx, common, label, layout_node);
        let padding = ctx.node_padding;

        match self {
            Self::BowTieRectangle => {
                OuterPathBody::bow_tie(metrics.width, metrics.height, padding, common.look_is_neo())
            }
            Self::CurvedTrapezoid => {
                let geometry = DisplayGeometry::from_label(
                    metrics.width,
                    metrics.height,
                    padding,
                    merman_render_core::config::mermaid_config_diagram_look(ctx.config).is_neo(),
                );
                OuterPathBody::from_points(
                    &geometry.points,
                    OuterPathTranslation::Xy(geometry.tx, geometry.ty),
                )
            }
            Self::Delay => {
                let geometry = DelayGeometry::from_label(
                    metrics.width,
                    metrics.height,
                    padding,
                    merman_render_core::config::mermaid_config_diagram_look(ctx.config).is_neo(),
                );
                OuterPathBody::from_points(&geometry.points, OuterPathTranslation::None)
            }
            Self::NotchedPentagon => OuterPathBody::notched_pentagon(
                metrics.width,
                metrics.height,
                padding,
                common.look_is_neo(),
            ),
        }
    }
}
