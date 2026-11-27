use merman_model::{
    families::quadrant_chart::QuadrantChartDiagramModel, presentation::PresentationConfig,
};
use merman_render_core::{
    config::config_font_family_css,
    resources::{OperationWorkError, OperationWorkMeter},
    svg_css::PreparedCommonCss,
};
use merman_theme_contract::v2::FamilyTheme;

use super::{
    QuadrantChartCssBinding, QuadrantChartDiagramLayout, QuadrantChartLayoutSettings,
    config::inherited_font_size_px, layout_quadrantchart_diagram_typed,
    native_theme::QuadrantNativeStyle,
};

/// Resolves point and typography roles before laying out the semantic chart.
pub fn prepare_quadrantchart_diagram(
    model: &QuadrantChartDiagramModel,
    diagram_title: Option<&str>,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    work_meter: &OperationWorkMeter,
) -> merman_render_core::Result<(QuadrantChartDiagramLayout, QuadrantChartPointThemePlan)> {
    let point_theme = QuadrantChartPointThemePlan::resolve(native, config, model, work_meter)?;
    let mut settings = QuadrantChartLayoutSettings::from_effective_config(config.as_value());
    point_theme.apply_native_layout(&mut settings);
    let layout = layout_quadrantchart_diagram_typed(
        model,
        diagram_title,
        &settings,
        &point_theme.css,
        point_theme.native_point_inks(),
    );
    Ok((layout, point_theme))
}

/// Presentation bound to the prepared point order.
#[derive(Debug)]
pub struct QuadrantChartPointThemePlan {
    css: QuadrantChartCssBinding,
    native: Option<QuadrantNativeStyle>,
    common_css: PreparedCommonCss,
    point_count: usize,
}

impl QuadrantChartPointThemePlan {
    fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        model: &QuadrantChartDiagramModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let point_count = model.points.len();
        work_meter.charge(point_count)?;
        let mut css = QuadrantChartCssBinding::resolve(config.as_value());
        let native = theme.map(|theme| QuadrantNativeStyle::bind(theme, config, &mut css));
        let common_css = if let Some(style) = &native {
            let family = style
                .body_family
                .clone()
                .unwrap_or_else(|| config_font_family_css(config));
            let size = style
                .base_size
                .unwrap_or_else(|| inherited_font_size_px(config.as_value()));
            PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                family,
                format!("{size}px"),
            )
        } else {
            PreparedCommonCss::new(config.as_value(), None)
        };
        Ok(Self {
            css,
            native,
            common_css,
            point_count,
        })
    }

    fn apply_native_layout(&self, cfg: &mut QuadrantChartLayoutSettings) {
        if let Some(style) = &self.native {
            style.apply_layout(cfg);
        }
    }

    pub(super) fn native_title_style(&self) -> Option<(&str, u16, u16)> {
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

    fn native_point_inks(&self) -> Option<&[String]> {
        self.native
            .as_ref()
            .and_then(|style| style.points.as_deref())
    }

    pub(super) fn common_css(&self) -> &PreparedCommonCss {
        &self.common_css
    }

    pub(super) fn point_count(&self) -> usize {
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
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (layout, plan) =
            prepare_quadrantchart_diagram(&model, None, &config, None, &work_meter).unwrap();

        assert_eq!(plan.point_count(), 2);
        assert_eq!(layout.points[0].radius, 5.0);
        assert_eq!(layout.points[0].fill, "var(--source)");
        assert_eq!(layout.points[1].radius, 7.0);
        assert_eq!(layout.points[1].fill, "#abcdef");
        assert_eq!(plan.point_count(), layout.points.len());
    }
}
