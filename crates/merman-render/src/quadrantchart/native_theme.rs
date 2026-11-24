use merman_model::presentation::PresentationConfig;

use crate::theme::{Color, FamilyTheme, font_family_css};

use super::config::QuadrantChartLayoutSettings;
use super::theme::QuadrantChartCssBinding;

#[derive(Debug)]
pub(super) struct QuadrantNativeStyle {
    pub(super) body_family: Option<String>,
    pub(super) title_family: Option<String>,
    pub(super) title_weight: u16,
    pub(super) label_weight: u16,
    pub(super) base_size: Option<f64>,
    title_size: Option<f64>,
    x_axis_size: Option<f64>,
    y_axis_size: Option<f64>,
    quadrant_size: Option<f64>,
    point_size: Option<f64>,
    internal_border_width: Option<f64>,
    external_border_width: Option<f64>,
    pub(super) points: Option<Box<[String]>>,
}

impl QuadrantNativeStyle {
    pub(super) fn bind(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        css: &mut QuadrantChartCssBinding,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |key: &str| config.explicit_config_owns_path(key);
        let node = roles.surface_node.first_color();
        let container = roles.surface_container.first_color();
        let paint = |slot: &mut String, key: &str, native: Color| {
            if !owns(&format!("themeVariables.{key}")) {
                *slot = native.to_string();
            }
        };
        for (slot, key, color) in [
            (&mut css.quadrant1_fill, "quadrant1Fill", node),
            (&mut css.quadrant2_fill, "quadrant2Fill", container),
            (
                &mut css.quadrant3_fill,
                "quadrant3Fill",
                node.composite(container, 0.75),
            ),
            (
                &mut css.quadrant4_fill,
                "quadrant4Fill",
                node.composite(container, 0.25),
            ),
            (
                &mut css.quadrant_internal_border_stroke_fill,
                "quadrantInternalBorderStrokeFill",
                roles.stroke_connector.first_color(),
            ),
            (
                &mut css.quadrant_external_border_stroke_fill,
                "quadrantExternalBorderStrokeFill",
                roles.stroke_border.first_color(),
            ),
        ] {
            paint(slot, key, color);
        }
        if !owns("themeVariables.primaryTextColor") {
            for (slot, key, color) in [
                (
                    &mut css.quadrant1_text_fill,
                    "quadrant1TextFill",
                    roles.text_primary,
                ),
                (
                    &mut css.quadrant2_text_fill,
                    "quadrant2TextFill",
                    roles.text_primary,
                ),
                (
                    &mut css.quadrant3_text_fill,
                    "quadrant3TextFill",
                    roles.text_primary,
                ),
                (
                    &mut css.quadrant4_text_fill,
                    "quadrant4TextFill",
                    roles.text_primary,
                ),
                (
                    &mut css.quadrant_point_text_fill,
                    "quadrantPointTextFill",
                    roles.text_primary,
                ),
                (
                    &mut css.quadrant_x_axis_text_fill,
                    "quadrantXAxisTextFill",
                    roles.text_muted,
                ),
                (
                    &mut css.quadrant_y_axis_text_fill,
                    "quadrantYAxisTextFill",
                    roles.text_muted,
                ),
                (
                    &mut css.quadrant_title_fill,
                    "quadrantTitleFill",
                    roles.text_title,
                ),
            ] {
                paint(slot, key, color);
            }
        }
        let theme_font = !owns("fontFamily") && !owns("themeVariables.fontFamily");
        let base_size = (!owns("fontSize") && !owns("themeVariables.fontSize"))
            .then_some(typography.base_size_px);
        let size =
            |path: &str, scale: f64| base_size.filter(|_| !owns(path)).map(|size| size * scale);
        Self {
            body_family: theme_font
                .then(|| font_family_css(typography.stack(typography.body_family))),
            title_family: theme_font
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
            base_size,
            title_size: size("quadrantChart.titleFontSize", 1.25),
            x_axis_size: size("quadrantChart.xAxisLabelFontSize", 1.0),
            y_axis_size: size("quadrantChart.yAxisLabelFontSize", 1.0),
            quadrant_size: size("quadrantChart.quadrantLabelFontSize", 1.0),
            point_size: size("quadrantChart.pointLabelFontSize", 0.75),
            internal_border_width: (!owns("quadrantChart.quadrantInternalBorderStrokeWidth"))
                .then_some(theme.form.connector_width),
            external_border_width: (!owns("quadrantChart.quadrantExternalBorderStrokeWidth"))
                .then_some(theme.form.stroke_width),
            points: (!owns("themeVariables.quadrantPointFill"))
                .then(|| theme.data.ink.iter().map(ToString::to_string).collect()),
        }
    }

    pub(super) fn apply_layout(&self, cfg: &mut QuadrantChartLayoutSettings) {
        cfg.title_font_size = self.title_size.unwrap_or(cfg.title_font_size);
        cfg.x_axis_label_font_size = self.x_axis_size.unwrap_or(cfg.x_axis_label_font_size);
        cfg.y_axis_label_font_size = self.y_axis_size.unwrap_or(cfg.y_axis_label_font_size);
        cfg.quadrant_label_font_size = self.quadrant_size.unwrap_or(cfg.quadrant_label_font_size);
        cfg.point_label_font_size = self.point_size.unwrap_or(cfg.point_label_font_size);
        cfg.quadrant_internal_border_stroke_width = self
            .internal_border_width
            .unwrap_or(cfg.quadrant_internal_border_stroke_width);
        cfg.quadrant_external_border_stroke_width = self
            .external_border_width
            .unwrap_or(cfg.quadrant_external_border_stroke_width);
    }
}

#[cfg(test)]
mod tests {
    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_quadrant_resolves_roles_before_layout_and_preserves_point_overrides() {
        let theme = ThemeCompiler::new()
            .compile_json(
                br##"{
                    "schema_version":2,"extends":"paper",
                    "roles":{"surface.node":"#DDEEFF","surface.container":"#EEF1F4","stroke.connector":"#123456","text.title":"#213243"},
                    "typography":{"sans":["Chart Body","sans-serif"],"serif":["Chart Title","serif"],"title_family":"serif","base_size_px":20},
                    "appearances":{"light":{"data":{"ink":["#15505D","#54216B","#713336","#433511"]}}}
                }"##,
                None,
            )
            .unwrap();
        let source = concat!(
            "---\nconfig:\n  quadrantChart:\n    pointLabelFontSize: 13\n---\n",
            "quadrantChart\ntitle Value\nx-axis Low --> High\nquadrant-1 First\n",
            "Native: [0.2, 0.7]\nStyled: [0.75, 0.9] radius: 9, color: #aabbcc\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let prepared = family::prepare(
            parsed.presentation(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic()
                .begin_session_with_native_theme(NativeTheme::new(theme))
                .unwrap(),
        )
        .unwrap();
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let dom = roxmltree::Document::parse(rendered.svg()).unwrap();
        let find_label = |name| {
            dom.descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some(name))
                .unwrap()
        };
        let point_for = |name| {
            find_label(name)
                .parent()
                .unwrap()
                .children()
                .find(|node| node.has_tag_name("circle"))
                .unwrap()
        };
        let native_label = find_label("Native");
        assert_eq!(point_for("Native").attribute("fill"), Some("#54216B"));
        let styled = point_for("Styled");
        assert_eq!(styled.attribute("fill"), Some("#aabbcc"));
        assert_eq!(styled.attribute("r"), Some("9"));
        assert_eq!(native_label.attribute("font-size"), Some("13"));
        let title = dom
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Value"))
            .unwrap();
        assert_eq!(title.attribute("fill"), Some("#213243"));
        assert_eq!(title.attribute("font-size"), Some("25"));
        assert!(
            dom.descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#DDEEFF"))
        );
    }
}
