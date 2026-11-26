//! Quadrant Chart presentation bindings.

use crate::Result;
use crate::model::QuadrantChartDiagramLayout;
use crate::text::TextMeasurer;
use merman_model::families::quadrant_chart::QuadrantChartDiagramModel;
use serde_json::Value;

mod config;
mod native_theme;
mod theme;

pub(crate) use config::QuadrantChartConfigView;
pub(crate) use theme::QuadrantChartPointThemePlan;

pub(crate) fn layout_quadrantchart_diagram_typed(
    model: &QuadrantChartDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    point_theme: &QuadrantChartPointThemePlan,
    _text_measurer: &dyn TextMeasurer,
) -> Result<QuadrantChartDiagramLayout> {
    if point_theme.point_count() != model.points.len() {
        return Err(crate::Error::InvalidModel {
            message: "Quadrant Chart point theme plan does not match the semantic model"
                .to_string(),
        });
    }
    let mut cfg = config::QuadrantChartLayoutSettings::from_effective_config(effective_config);
    point_theme.apply_native_layout(&mut cfg);
    Ok(
        merman_render_charts::quadrantchart::layout_quadrantchart_diagram_typed(
            model,
            diagram_title,
            &cfg,
            point_theme.css(),
            point_theme.native_point_inks(),
        ),
    )
}
