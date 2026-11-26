use merman_model::families::radar::RadarDiagramModel;

use crate::Result;
use crate::model::{LayoutPoint, RadarDiagramLayout};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;

mod config;
mod css_binding;
mod native_theme;
mod theme;

pub(crate) use config::RadarConfigView;
pub(crate) use css_binding::RadarCssBinding;
pub(crate) use theme::{RadarSeriesPaintPlan, RadarTypographyThemePlan};

pub(crate) fn effective_title<'a>(
    model: &'a RadarDiagramModel,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
}

pub(crate) fn format_points(points: &[LayoutPoint]) -> String {
    let mut out = String::new();
    let mut buffer = ryu_js::Buffer::new();
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(buffer.format(point.x));
        out.push(',');
        out.push_str(buffer.format(point.y));
    }
    out
}

/// Lays out a Radar model under the cumulative work meter owned by the render operation.
pub(crate) fn layout_radar_diagram_typed_with_work_meter(
    model: &RadarDiagramModel,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
) -> Result<RadarDiagramLayout> {
    merman_render_charts::radar::layout_radar_diagram_typed_with_work_meter(
        model,
        effective_config,
        work_meter,
    )
}

#[cfg(test)]
mod tests {
    use super::layout_radar_diagram_typed_with_work_meter;
    use crate::resources::OperationWorkMeter;
    use crate::text::DeterministicTextMeasurer;
    use crate::{Error, RenderResourcePolicy};
    use merman_model::families::radar::RadarDiagramModel;
    use merman_model::{OperationControl, OperationPhase};
    use serde_json::json;

    #[test]
    fn radar_layout_observes_cancellation_before_and_during_fractional_graticules() {
        let mut model = RadarDiagramModel::default();
        model.options.ticks = 2.5;
        for checkpoints in [0, 2] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            let meter = OperationWorkMeter::new_with_control(
                RenderResourcePolicy::unbounded_for_trusted_input(),
                control,
            );
            let error = layout_radar_diagram_typed_with_work_meter(
                &model,
                &json!({}),
                &DeterministicTextMeasurer::default(),
                &meter,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                Error::Cancelled(cancelled) if cancelled.phase == OperationPhase::Layout
            ));
            assert_eq!(meter.used(), if checkpoints == 0 { 0 } else { 3 });
        }
    }
}
