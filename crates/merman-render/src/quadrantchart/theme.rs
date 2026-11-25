use merman_model::families::quadrant_chart::{
    QuadrantChartDiagramModel, QuadrantChartPointModel, QuadrantChartStyles,
};
use merman_model::presentation::PresentationConfig;

use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::theme::FamilyTheme;

mod css_binding;
pub(super) use css_binding::QuadrantChartCssBinding;

#[derive(Debug)]
pub(crate) struct QuadrantChartPointThemePlan {
    css: QuadrantChartCssBinding,
    native: Option<super::native_theme::QuadrantNativeStyle>,
    common_css: crate::svg::PreparedCommonCss,
    point_count: usize,
}

pub(super) struct QuadrantChartPointBinding<'a> {
    pub(super) radius: f64,
    pub(super) fill: &'a str,
    pub(super) stroke_color: &'a str,
    pub(super) stroke_width: &'a str,
}

impl QuadrantChartPointThemePlan {
    pub(crate) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        model: &QuadrantChartDiagramModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let point_count = model.points.len();
        work_meter.charge(point_count)?;
        let mut css = QuadrantChartCssBinding::resolve(config.as_value());
        let native = theme
            .map(|theme| super::native_theme::QuadrantNativeStyle::bind(theme, config, &mut css));
        let common_css = if let Some(style) = &native {
            let family = style
                .body_family
                .clone()
                .unwrap_or_else(|| crate::config::config_font_family_css(config));
            let size = style
                .base_size
                .unwrap_or_else(|| super::config::inherited_font_size_px(config.as_value()));
            crate::svg::PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                family,
                format!("{size}px"),
            )
        } else {
            crate::svg::PreparedCommonCss::new(config.as_value(), None)
        };
        Ok(Self {
            css,
            native,
            common_css,
            point_count,
        })
    }

    pub(super) fn apply_native_layout(&self, cfg: &mut super::config::QuadrantChartLayoutSettings) {
        if let Some(style) = &self.native {
            style.apply_layout(cfg);
        }
    }

    pub(crate) fn native_title_style(&self) -> Option<(&str, u16, u16)> {
        let native = self.native.as_ref()?;
        Some((
            native
                .title_family
                .as_deref()
                .unwrap_or_else(|| self.common_css.font_family()),
            native.title_weight,
            native.label_weight,
        ))
    }

    pub(crate) fn css(&self) -> &QuadrantChartCssBinding {
        &self.css
    }

    pub(super) fn point_binding<'a>(
        &'a self,
        index: usize,
        point: &'a QuadrantChartPointModel,
        class_styles: Option<&'a QuadrantChartStyles>,
        default_radius: f64,
    ) -> QuadrantChartPointBinding<'a> {
        let point_ink = self
            .native
            .as_ref()
            .and_then(|style| style.points.as_deref())
            .map_or(self.css.quadrant_point_fill.as_str(), |points| {
                &points[index % points.len()]
            });
        QuadrantChartPointBinding {
            radius: point
                .styles
                .radius
                .or_else(|| class_styles.and_then(|style| style.radius))
                .unwrap_or(default_radius),
            fill: super::point_source_fill(point, class_styles).unwrap_or(point_ink),
            stroke_color: point
                .styles
                .stroke_color
                .as_deref()
                .or_else(|| class_styles.and_then(|style| style.stroke_color.as_deref()))
                .unwrap_or(point_ink),
            stroke_width: point
                .styles
                .stroke_width
                .as_deref()
                .or_else(|| class_styles.and_then(|style| style.stroke_width.as_deref()))
                .unwrap_or("0px"),
        }
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn point_count(&self) -> usize {
        self.point_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_layout_keeps_source_and_class_values() {
        let model: QuadrantChartDiagramModel = serde_json::from_value(serde_json::json!({
            "title": null, "accTitle": null, "accDescr": null,
            "quadrants": {
                "quadrant1Text": "", "quadrant2Text": "",
                "quadrant3Text": "", "quadrant4Text": ""
            },
            "axes": {
                "xAxisLeftText": "", "xAxisRightText": "",
                "yAxisBottomText": "", "yAxisTopText": ""
            },
            "points": [
                {
                    "text": "source", "x": 0.25, "y": 0.25,
                    "styles": { "color": "var(--source)", "radius": 5 }
                },
                {
                    "text": "class", "x": 0.75, "y": 0.75,
                    "className": "custom", "styles": {}
                }
            ],
            "classes": { "custom": { "color": "#abcdef", "radius": 7 } }
        }))
        .unwrap();
        let config = PresentationConfig::empty_object();
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan =
            QuadrantChartPointThemePlan::resolve(None, &config, &model, &work_meter).unwrap();

        assert_eq!(plan.point_count(), 2);
        let layout = super::super::layout_quadrantchart_diagram_typed(
            &model,
            None,
            config.as_value(),
            &plan,
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        assert_eq!(layout.points[0].radius, 5.0);
        assert_eq!(layout.points[0].fill, "var(--source)");
        assert_eq!(layout.points[1].radius, 7.0);
        assert_eq!(layout.points[1].fill, "#abcdef");
        assert_eq!(plan.point_count(), layout.points.len());
    }
}
