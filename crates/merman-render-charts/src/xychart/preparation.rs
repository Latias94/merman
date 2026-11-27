use merman_model::{
    families::xy_chart::XyChartDiagramRenderModel, presentation::PresentationConfig,
};
use merman_render_core::{Result, text::TextMeasurer};
use merman_theme_contract::v2::FamilyTheme;

use super::{
    XyChartDiagramLayout, XyChartSeriesPaintPlan, layout_xychart_diagram_typed,
    palette::{mermaid_plot_palette, plot_color_from_palette},
    theme::XyChartTypographyThemePlan,
};

/// The chart presentation retained alongside the typed layout.
#[derive(Debug)]
pub struct XyChartStyle {
    pub(super) typography: XyChartTypographyThemePlan,
    pub(super) bar_legend_plots: Vec<usize>,
    pub(super) data_label_color: String,
}

/// Resolves chart presentation and geometry with the host's routed text measurer.
///
/// `has_canvas_paint` suppresses the chart background when the session supplies
/// a canvas paint, including a transparent override.
pub fn prepare_xychart_diagram(
    model: &XyChartDiagramRenderModel,
    diagram_title: Option<&str>,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    has_canvas_paint: bool,
    text_measurer: &dyn TextMeasurer,
) -> Result<(XyChartDiagramLayout, XyChartStyle)> {
    let palette = if native.is_none() {
        mermaid_plot_palette(config)
    } else {
        Vec::new()
    };
    let series_paint = XyChartSeriesPaintPlan::new(config, model, native, |index| {
        plot_color_from_palette(&palette, index)
    });
    let typography = native.map_or_else(
        || XyChartTypographyThemePlan::resolve(config),
        |theme| XyChartTypographyThemePlan::native(config, &theme.typography),
    );
    let (mut layout, bar_legend_plots) = layout_xychart_diagram_typed(
        model,
        diagram_title,
        &series_paint,
        typography.layout_typography(),
        native,
        text_measurer,
    )?;
    if has_canvas_paint {
        layout.background_color = "none".to_owned();
    }
    Ok((
        layout,
        XyChartStyle {
            typography,
            bar_legend_plots,
            data_label_color: series_paint.into_data_label_color(),
        },
    ))
}
