use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::config_f64,
    text::{WrapMode, literal_label::FlowchartSvgWidthMode},
};

use crate::shapes::{FlowchartShape, is_flowchart_process_shape};

/// The FlowDB minimum only reaches ordinary nodes that use `labelHelper`.
/// Asset shapes set `node.width` before calling it; fixed-size symbols and label-derived
/// circles opt out of it.
pub fn flowchart_node_label_min_width(
    raw_label: &str,
    layout_shape: Option<&str>,
    config: &PresentationConfig,
) -> f64 {
    let shape = FlowchartShape::resolve(layout_shape.unwrap_or("squareRect"));
    if raw_label.is_empty()
        || matches!(
            shape,
            Some(
                FlowchartShape::Anchor
                    | FlowchartShape::Circle
                    | FlowchartShape::Choice
                    | FlowchartShape::CrossedCircle
                    | FlowchartShape::DoubleCircle
                    | FlowchartShape::FilledCircle
                    | FlowchartShape::ForkJoin
                    | FlowchartShape::FramedCircle
                    | FlowchartShape::SmallCircle
                    | FlowchartShape::LightningBolt
                    | FlowchartShape::Icon
                    | FlowchartShape::IconCircle
                    | FlowchartShape::IconRounded
                    | FlowchartShape::IconSquare
                    | FlowchartShape::ImageSquare
                    | FlowchartShape::CollapsedGroup
            )
        )
    {
        return 0.0;
    }
    config_f64(config, &["flowchart", "minNodeWidth"])
        .unwrap_or(0.0)
        .max(0.0)
}

pub fn flowchart_node_svg_width_mode(
    raw_label: &str,
    label_type: &str,
    wrap_mode: WrapMode,
    layout_shape: &str,
) -> FlowchartSvgWidthMode {
    if wrap_mode == WrapMode::SvgLike
        && label_type != "markdown"
        && !raw_label.contains('<')
        && !raw_label.contains('>')
        && is_flowchart_process_shape(layout_shape)
    {
        FlowchartSvgWidthMode::ComputedLength
    } else {
        FlowchartSvgWidthMode::Bbox
    }
}
