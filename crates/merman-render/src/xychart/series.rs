use merman_model::families::xy_chart::{XyChartDiagramRenderModel, XyChartPlotType};

use crate::chart_palette::plot_color_from_palette;

use super::css_binding::XyChartCssBinding;

#[derive(Debug)]
struct SeriesPaint {
    fill: String,
    stroke: String,
    point_label_fill: String,
    stroke_width: f64,
}

#[derive(Debug)]
pub(crate) struct XyChartSeriesPaintPlan {
    css: XyChartCssBinding,
    chart_config: super::ChartConfig,
    paints: Vec<SeriesPaint>,
}

impl XyChartSeriesPaintPlan {
    pub(super) fn css_binding(&self) -> &XyChartCssBinding {
        &self.css
    }

    pub(super) fn chart_config(&self) -> &super::ChartConfig {
        &self.chart_config
    }

    pub(crate) fn into_data_label_color(self) -> String {
        self.css.data_label_color
    }

    pub(crate) fn new(
        config: &serde_json::Value,
        model: &XyChartDiagramRenderModel,
        native: Option<&crate::theme::FamilyTheme>,
    ) -> Self {
        let css = match native {
            Some(theme) => XyChartCssBinding::native(&theme.roles),
            None => XyChartCssBinding::resolve(config),
        };
        let palette = &css.plot_color_palette;
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
                            let fill = if bar {
                                let fill = theme.data.fill(index);
                                if wireframe {
                                    fill.composite(crate::theme::Color::TRANSPARENT, 0.2)
                                        .to_string()
                                } else {
                                    fill.to_string()
                                }
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
                            let color = plot_color_from_palette(palette, index);
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
            chart_config: super::parse_chart_config(config, model),
            css,
        }
    }

    pub(crate) fn plot_count(&self) -> usize {
        self.paints.len()
    }
    pub(crate) fn fill_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }
    pub(crate) fn stroke_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.stroke.as_str())
    }
    pub(crate) fn point_label_fill_css(&self, index: usize) -> Option<&str> {
        self.paints
            .get(index)
            .map(|paint| paint.point_label_fill.as_str())
    }
    pub(crate) fn stroke_width(&self, index: usize) -> Option<f64> {
        self.paints.get(index).map(|paint| paint.stroke_width)
    }
}
