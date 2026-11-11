use crate::config::config_css_number_or_string;
use crate::mermaid_style::{
    CssFontSizeContext, css_font_size_is_zero, css_font_size_override,
    css_font_size_value_override, parse_style_declaration,
    resolve_explicit_css_absolute_font_size_px,
};
use crate::svg::PreparedCommonCss;
use crate::text::TextStyle;

use super::prepared_text::PieTextRole;

/// A prepared stylesheet token retains Mermaid's authored spelling, including browser-only CSS.
#[derive(Debug)]
pub(crate) struct PieCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) slice_stroke: PieSurfaceStroke,
    pub(crate) slice_stroke_width: String,
    pub(crate) slice_opacity: String,
    pub(crate) outer_stroke: PieSurfaceStroke,
    pub(crate) outer_stroke_width: String,
    pub(crate) outer_stroke_width_number: f64,
    text_fills: [PieTextFill; 3],
    inherited_font_size: f64,
    text_sizes: [PieTextSize; 3],
    text_weights: [Option<String>; 3],
    text_styles: [Option<String>; 3],
}

#[derive(Debug)]
struct PieTextFill {
    css: String,
    source_path: Option<&'static str>,
}

impl PieTextFill {
    fn resolve(config: &serde_json::Value, role: PieTextRole) -> Self {
        let (paths, fallback): (&[&'static str], &str) = match role {
            PieTextRole::Title => (
                &[
                    "themeVariables.pieTitleTextColor",
                    "themeVariables.taskTextDarkColor",
                ],
                "black",
            ),
            PieTextRole::Section => (
                &[
                    "themeVariables.pieSectionTextColor",
                    "themeVariables.textColor",
                ],
                "#333",
            ),
            PieTextRole::Legend => (
                &[
                    "themeVariables.pieLegendTextColor",
                    "themeVariables.taskTextDarkColor",
                ],
                "black",
            ),
        };
        let (source_path, css) = crate::config::selected_config_string(config, paths)
            .map_or((None, fallback), |(path, css)| (Some(path), css));
        Self {
            css: css.to_owned(),
            source_path,
        }
    }
}

/// Keep CSS spelling, measurement, and configuration ownership on the same prepared role.
#[derive(Debug)]
struct PieTextSize {
    css: String,
    resolution: Option<(f64, bool)>,
    configured_admissible: bool,
}

impl PieTextSize {
    fn new(configured: Option<String>, fallback: &str, inherited_px: f64) -> Self {
        let configured_admissible = configured
            .as_ref()
            .is_some_and(|css| css_font_size_value_override(css).is_some());
        let css = configured.unwrap_or_else(|| fallback.to_owned());
        let resolution = Self::resolve(&css, inherited_px);
        Self {
            css,
            resolution,
            configured_admissible,
        }
    }

    fn resolve(css: &str, inherited_px: f64) -> Option<(f64, bool)> {
        let declaration_list = format!("font-size:{css}");
        let declaration = parse_style_declaration(&declaration_list)?;
        // Invalid CSS, including nonzero unitless numbers, never owns a role's size.
        // A parsed, definitely invalid declaration is ignored by the browser, so its text
        // inherits the root size. Host-dependent values keep their unresolved outcome below.
        if css_font_size_override(&declaration_list).is_none() {
            return Some((inherited_px, true));
        }
        if let Some(size) = resolve_explicit_css_absolute_font_size_px(css) {
            return Some((size, false));
        }
        if css_font_size_is_zero(css) {
            return Some((0.0, false));
        }
        if declaration.analysis().depends_on_root_font_size() {
            return None;
        }
        let size = declaration.resolve_font_size_px(CssFontSizeContext::uniform(inherited_px))?;
        // Only classify inheritance here; the shared parser owns length/keyword arithmetic.
        let mut parser = cssparser::Parser::new(declaration.value());
        let inherited = match parser.next().ok()? {
            cssparser::Token::Dimension { unit, .. } => unit.eq_ignore_ascii_case("em"),
            cssparser::Token::Percentage { .. } => true,
            cssparser::Token::Ident(value) => ["inherit", "unset", "smaller", "larger"]
                .iter()
                .any(|keyword| value.eq_ignore_ascii_case(keyword)),
            _ => false,
        };
        Some((size, inherited))
    }

    fn bind(&mut self, css: &str, inherited_px: f64) {
        self.css = css.to_owned();
        self.resolution = Self::resolve(css, inherited_px);
    }
}

/// An absent surface keeps its baseline CSS without admitting a typed terminal application.
#[derive(Debug)]
pub(crate) struct PieSurfaceStroke {
    visible: String,
    absent: Option<String>,
}

impl PieSurfaceStroke {
    fn baseline(css: String) -> Self {
        Self {
            visible: css,
            absent: None,
        }
    }

    pub(super) fn bind_typed(&mut self, css: &str) {
        self.absent = Some(std::mem::replace(&mut self.visible, css.to_owned()));
    }

    pub(crate) fn css(&self, surface_present: bool) -> &str {
        if surface_present {
            &self.visible
        } else {
            self.absent.as_deref().unwrap_or(&self.visible)
        }
    }
}

impl PieCssBinding {
    pub(super) fn new(
        effective_config: &serde_json::Value,
        font_family_override: Option<&str>,
    ) -> Self {
        let inherited_font_size =
            crate::config::config_theme_font_size_css_or_root_number_px(effective_config, 16.0);
        let mut common = PreparedCommonCss::with_resolved_typography(
            effective_config,
            font_family_override.map_or_else(
                || crate::config::config_font_family_css(effective_config),
                str::to_owned,
            ),
            format!("{inherited_font_size}px"),
        );
        if font_family_override.is_none() {
            common = common.with_configured_root_font_family(effective_config);
        }
        let value = |key: &str, fallback: &str| {
            config_css_number_or_string(effective_config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        Self {
            common,
            slice_stroke: PieSurfaceStroke::baseline(value("pieStrokeColor", "black")),
            slice_stroke_width: value("pieStrokeWidth", "2px"),
            slice_opacity: value("pieOpacity", "0.7"),
            outer_stroke: PieSurfaceStroke::baseline(value("pieOuterStrokeColor", "black")),
            outer_stroke_width: value("pieOuterStrokeWidth", "2px"),
            outer_stroke_width_number: super::super::config::pie_outer_stroke_width(
                effective_config,
            ),
            text_fills: PieTextRole::ALL.map(|role| PieTextFill::resolve(effective_config, role)),
            inherited_font_size,
            text_sizes: [
                ("pieTitleTextSize", "25px"),
                ("pieSectionTextSize", "17px"),
                ("pieLegendTextSize", "17px"),
            ]
            .map(|(key, fallback)| {
                PieTextSize::new(
                    config_css_number_or_string(effective_config, &["themeVariables", key]),
                    fallback,
                    inherited_font_size,
                )
            }),
            text_weights: std::array::from_fn(|_| None),
            text_styles: std::array::from_fn(|_| None),
        }
    }

    pub(super) fn bind_base_font_size(&mut self, config: &serde_json::Value, size: f32) {
        self.inherited_font_size = f64::from(size);
        for text_size in &mut self.text_sizes {
            text_size.resolution = PieTextSize::resolve(&text_size.css, self.inherited_font_size);
        }
        self.common = PreparedCommonCss::with_resolved_typography(
            config,
            self.common.font_family().to_owned(),
            format!("{size}px"),
        );
    }

    pub(super) fn bind_text_fill(&mut self, role: PieTextRole, fill: &str) {
        self.text_fills[role.index()].css = fill.to_owned();
    }

    pub(super) fn config_owns_text_fill(
        &self,
        config: &merman_model::presentation::PresentationConfig,
        role: PieTextRole,
    ) -> bool {
        self.text_fills[role.index()]
            .source_path
            .is_some_and(|path| config.config_paint_overrides_typed_default(path))
    }

    pub(super) fn bind_text_size(&mut self, role: PieTextRole, size: &str) {
        self.text_sizes[role.index()].bind(size, self.inherited_font_size);
    }

    pub(super) fn configured_text_size_is_admissible(&self, role: PieTextRole) -> bool {
        self.text_sizes[role.index()].configured_admissible
    }

    pub(super) fn bind_text_weight(&mut self, role: PieTextRole, weight: String) {
        self.text_weights[role.index()] = Some(weight);
    }

    pub(super) fn bind_text_style(&mut self, role: PieTextRole, style: String) {
        self.text_styles[role.index()] = Some(style);
    }

    pub(crate) fn text_size(&self, role: PieTextRole) -> &str {
        &self.text_sizes[role.index()].css
    }

    pub(crate) fn text_fill(&self, role: PieTextRole) -> &str {
        &self.text_fills[role.index()].css
    }

    pub(crate) fn text_weight(&self, role: PieTextRole) -> Option<&str> {
        self.text_weights[role.index()].as_deref()
    }

    pub(crate) fn text_style(&self, role: PieTextRole) -> Option<&str> {
        self.text_styles[role.index()].as_deref()
    }

    pub(super) fn text_size_resolution(&self, role: PieTextRole) -> Option<(f64, bool)> {
        self.text_sizes[role.index()].resolution
    }

    pub(super) fn text_can_be_visible(&self, role: PieTextRole) -> bool {
        // Host-dependent sizes remain possible consumers; only known zero excludes them.
        self.text_size_resolution(role)
            .is_none_or(|(size, _)| size > 0.0)
    }

    pub(crate) fn legend_measurement_style(&self) -> TextStyle {
        self.measurement_style(PieTextRole::Legend)
    }

    pub(crate) fn title_measurement_style(&self) -> TextStyle {
        self.measurement_style(PieTextRole::Title)
    }

    fn measurement_style(&self, role: PieTextRole) -> TextStyle {
        TextStyle {
            font_family: Some(self.common.font_family().to_owned()),
            font_size: self
                .text_size_resolution(role)
                .map_or(self.inherited_font_size, |(size, _)| size),
            font_weight: self.text_weight(role).map(str::to_owned),
            font_style: self.text_style(role).map(str::to_owned),
        }
    }
}
