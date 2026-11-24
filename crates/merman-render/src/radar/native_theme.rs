use merman_model::presentation::PresentationConfig;

use crate::theme::{FamilyTheme, font_family_css};

use super::css_binding::RadarRoleFontSize;

#[derive(Debug)]
pub(super) struct RadarNativeStyle {
    pub(super) body_family: Option<String>,
    pub(super) title_family: Option<String>,
    pub(super) title_weight: u16,
    pub(super) label_weight: u16,
    pub(super) show_grid: bool,
}

impl RadarNativeStyle {
    pub(super) fn bind(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        font_size_css: &str,
        binding: &mut super::RadarCssBinding,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |path: &str| config.explicit_config_owns_path(path);
        let scoped = |key: &str| {
            owns(&format!("radar.{key}")) || owns(&format!("themeVariables.radar.{key}"))
        };
        let owns_fill = |key: &str| owns(&format!("themeVariables.{key}"));
        if !owns_fill("textColor") {
            binding.text_color = roles.text_primary.to_string();
        }
        if !owns_fill("titleColor") {
            binding.title_color = roles.text_title.to_string();
        }
        if !scoped("axisColor") {
            binding.axis_color = roles.stroke_border.first_color().to_string();
        }
        binding.native_axis_label_color = Some(if scoped("axisColor") {
            binding.axis_color.clone()
        } else if owns_fill("textColor") {
            binding.text_color.clone()
        } else {
            roles.text_muted.to_string()
        });
        if !scoped("graticuleColor") {
            binding.graticule_color = roles.stroke_guide.first_color().to_string();
        }
        if !scoped("graticuleOpacity") {
            binding.graticule_opacity = 0.0;
        }
        if !scoped("curveOpacity") {
            binding.curve_opacity = 0.28;
        }
        if !scoped("axisStrokeWidth") {
            binding.axis_stroke_width = theme.form.stroke_width;
        }
        if !scoped("curveStrokeWidth") {
            binding.curve_stroke_width = theme.form.connector_width;
        }
        let role_size =
            crate::mermaid_style::resolve_explicit_css_absolute_font_size_px(font_size_css)
                .unwrap_or(typography.base_size_px);
        if !scoped("axisLabelFontSize") {
            binding.axis_label_font_size = RadarRoleFontSize::resolve(role_size);
        }
        if !scoped("legendFontSize") {
            binding.legend_font_size = RadarRoleFontSize::resolve(role_size);
        }
        let theme_font = !owns("fontFamily") && !owns("themeVariables.fontFamily");
        Self {
            body_family: theme_font
                .then(|| font_family_css(typography.stack(typography.body_family))),
            title_family: theme_font
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
            show_grid: theme.form.chart.gridlines
                || scoped("graticuleColor")
                || scoped("graticuleOpacity")
                || scoped("graticuleStrokeWidth"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_radar_wraps_series_paints_and_preserves_source_slots() {
        let theme = ThemeCompiler::new()
            .compile_json(
                br##"{
                    "schema_version":2,"extends":"paper",
                    "roles":{"stroke.guide":"#567889","stroke.border":"#123456","text.muted":"#475869"},
                    "typography":{"sans":["Radar Body","sans-serif"],"base_size_px":19},
                    "form":{"chart":{"gridlines":true}},
                    "appearances":{"light":{"data":{"ink":["#15505D","#54216B","#713336","#433511"],"fill":["#CCEEDD","#EEDDEE","#EEDDDD","#EEEEDD"]}}}
                }"##,
                None,
            )
            .unwrap();
        let mut source = concat!(
            "---\nconfig:\n  themeVariables:\n    cScale0: '#aabbcc'\n---\n",
            "radar-beta\ntitle Coverage\naxis A[\"First\"], B[\"Second\"], C[\"Third\"]\n",
        )
        .to_owned();
        for index in 0..13 {
            writeln!(source, "curve series{index}[\"Series {index}\"]{{1,2,3}}").unwrap();
        }
        source.push_str("showLegend true\n");
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
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
        let css = dom
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(
            css.contains(
                ".radarCurve-0{color:#aabbcc;fill:#aabbcc;fill-opacity:0.28;stroke:#aabbcc"
            )
        );
        assert!(css.contains(
            ".radarCurve-12{color:#15505D;fill:#CCEEDD;fill-opacity:0.28;stroke:#15505D"
        ));
        assert!(css.contains(".radarLegendBox-12{fill:#CCEEDD;"));
        assert!(css.contains(".radarAxisLine{stroke:#123456;"));
        assert!(css.contains(".radarAxisLabel{color:#475869;fill:#475869;}"));
        assert!(css.contains("\"Radar Body\", sans-serif"));
        assert!(
            dom.descendants()
                .any(|node| node.attribute("class") == Some("radarGraticule"))
        );
    }
}
