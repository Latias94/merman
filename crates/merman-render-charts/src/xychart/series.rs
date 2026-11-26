use merman_model::families::xy_chart::{XyChartDiagramRenderModel, XyChartPlotType};
use merman_render_core::config::ConfigLookup;
use merman_theme_contract::v2::FamilyTheme;

use super::css_binding::XyChartCssBinding;
use super::layout::{ChartConfig, parse_chart_config};

#[derive(Debug)]
struct SeriesPaint {
    fill: String,
    stroke: String,
    point_label_fill: String,
    stroke_width: f64,
}

#[derive(Debug)]
pub struct XyChartSeriesPaintPlan {
    css: XyChartCssBinding,
    chart_config: ChartConfig,
    paints: Vec<SeriesPaint>,
}

impl XyChartSeriesPaintPlan {
    pub(super) fn css_binding(&self) -> &XyChartCssBinding {
        &self.css
    }

    pub(super) fn chart_config(&self) -> &ChartConfig {
        &self.chart_config
    }

    pub fn into_data_label_color(self) -> String {
        self.css.data_label_color
    }

    /// `mermaid_plot_color` selects the host's classic palette color for a series index.
    pub fn new(
        config: &impl ConfigLookup,
        model: &XyChartDiagramRenderModel,
        native: Option<&FamilyTheme>,
        mermaid_plot_color: impl Fn(usize) -> String,
    ) -> Self {
        let css = match native {
            Some(theme) => XyChartCssBinding::native(&theme.roles),
            None => XyChartCssBinding::resolve(config),
        };
        Self {
            paints: model
                .plots
                .iter()
                .enumerate()
                .map(|(index, plot)| {
                    let bar = plot.plot_type == XyChartPlotType::Bar;
                    let (fill, stroke, point_label_fill, stroke_width) = match native {
                        Some(theme) => {
                            let ink = theme.data.ink(index).to_string();
                            let wireframe = bar && theme.form.chart.bar_wireframe;
                            let fill = if wireframe {
                                ink.clone()
                            } else if bar {
                                theme.data.fill(index).to_string()
                            } else {
                                "none".to_owned()
                            };
                            let width = if wireframe {
                                1.5
                            } else if bar {
                                0.0
                            } else {
                                2.0
                            };
                            (fill, ink.clone(), ink, width)
                        }
                        None => {
                            let color = mermaid_plot_color(index);
                            let fill = if bar {
                                color.clone()
                            } else {
                                "none".to_owned()
                            };
                            (fill, color.clone(), color, if bar { 0.0 } else { 2.0 })
                        }
                    };
                    SeriesPaint {
                        fill,
                        stroke,
                        point_label_fill,
                        stroke_width,
                    }
                })
                .collect(),
            chart_config: parse_chart_config(config, model),
            css,
        }
    }

    pub(super) fn plot_count(&self) -> usize {
        self.paints.len()
    }
    pub(super) fn fill_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }
    pub(super) fn stroke_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.stroke.as_str())
    }
    pub(super) fn point_label_fill_css(&self, index: usize) -> Option<&str> {
        self.paints
            .get(index)
            .map(|paint| paint.point_label_fill.as_str())
    }
    pub(super) fn stroke_width(&self, index: usize) -> Option<f64> {
        self.paints.get(index).map(|paint| paint.stroke_width)
    }
}
