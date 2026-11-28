use std::collections::BTreeMap;

use merman_model::{
    config::MermaidThemeId, css_color, families::c4::C4Boundary, presentation::PresentationConfig,
};
use merman_theme_contract::v2::{Color, FamilyTheme};

const MERMAID_BOUNDARY_FILL: &str = "none";
const MERMAID_BOUNDARY_STROKE: &str = "#444444";
const MERMAID_BOUNDARY_RADIUS_PX: &str = "2.5";

#[derive(Debug)]
struct C4BoundaryStyle {
    fill: Box<str>,
    stroke: Box<str>,
    radius: Box<str>,
    theme_background: Option<Color>,
}

/// Fill, stroke, and radius for the explicit boundaries rendered by C4.
#[derive(Debug)]
pub struct C4ClusterThemePlan {
    boundaries: BTreeMap<Box<str>, C4BoundaryStyle>,
}

impl C4ClusterThemePlan {
    pub fn resolve(
        native: Option<&FamilyTheme>,
        boundaries: &[C4Boundary],
        config: &PresentationConfig,
        uses_shoal: bool,
    ) -> Self {
        let dark_shoal_stroke = if uses_shoal
            && native.is_none()
            && config
                .get_str("theme")
                .and_then(|name| MermaidThemeId::parse(name).ok())
                .is_some_and(MermaidThemeId::is_dark)
        {
            config.get_str("themeVariables.lineColor")
        } else {
            None
        };
        let mut styles: BTreeMap<Box<str>, C4BoundaryStyle> = boundaries
            .iter()
            .filter(|boundary| boundary.alias != "global")
            .map(|boundary| {
                (
                    boundary.alias.as_str().into(),
                    C4BoundaryStyle {
                        fill: boundary
                            .bg_color
                            .as_deref()
                            .or_else(|| config.get_str("c4.boundary_bg_color"))
                            .unwrap_or(MERMAID_BOUNDARY_FILL)
                            .into(),
                        stroke: boundary
                            .border_color
                            .as_deref()
                            .or_else(|| config.get_str("c4.boundary_border_color"))
                            .or(dark_shoal_stroke)
                            .unwrap_or(MERMAID_BOUNDARY_STROKE)
                            .into(),
                        radius: MERMAID_BOUNDARY_RADIUS_PX.into(),
                        theme_background: None,
                    },
                )
            })
            .collect();
        if let Some(theme) = native {
            let fill = theme.roles.surface_container.first_color();
            let fill_css = fill.to_string();
            let stroke = theme.roles.stroke_border.first_color().to_string();
            let radius = theme.form.container_radius.to_string();
            let owns = |path: &str| {
                config.explicit_config_owns_path(path) && config.get_str(path).is_some()
            };
            for boundary in boundaries {
                let Some(style) = styles.get_mut(boundary.alias.as_str()) else {
                    continue;
                };
                if boundary.bg_color.is_none() && !owns("c4.boundary_bg_color") {
                    style.fill = fill_css.as_str().into();
                    style.theme_background = Some(fill);
                }
                if boundary.border_color.is_none() && !owns("c4.boundary_border_color") {
                    style.stroke = stroke.as_str().into();
                }
                style.radius = radius.as_str().into();
            }
        }
        Self { boundaries: styles }
    }

    pub fn boundary_tokens(&self, alias: &str) -> Option<(&str, &str, &str)> {
        let style = self.boundaries.get(alias)?;
        Some((&style.fill, &style.stroke, &style.radius))
    }

    pub(crate) fn has_separate_background(&self, alias: &str) -> bool {
        self.boundaries.get(alias).is_some_and(|style| {
            !style.fill.eq_ignore_ascii_case("none")
                && !style.fill.eq_ignore_ascii_case("transparent")
        })
    }

    pub(crate) fn uses_theme_background(&self, alias: &str) -> bool {
        self.boundaries
            .get(alias)
            .is_some_and(|style| style.theme_background.is_some())
    }

    pub(super) fn transparent_title_background(
        &self,
        alias: &str,
        font_color: &str,
    ) -> Option<Color> {
        let fill = self.boundaries.get(alias)?.theme_background?;
        Some(readable_opaque_background(fill, literal_color(font_color)))
    }
}

pub(super) fn literal_color(value: &str) -> Option<Color> {
    let [r, g, b, a] = css_color::parse(value.trim())?.rgba.to_rgba8();
    Some(Color::rgba(r, g, b, a))
}

pub(super) fn readable_opaque_background(preferred: Color, ink: Option<Color>) -> Color {
    let [r, g, b, _] = preferred.channels();
    let fill = Color::rgb(r, g, b);
    let Some(ink) = ink else {
        return fill;
    };
    let contrast = |fill| ink.composite(fill, 1.0).contrast(fill);
    if contrast(fill) >= 4.5 {
        return fill;
    }
    let black = Color::rgb(0, 0, 0);
    let white = Color::rgb(255, 255, 255);
    if contrast(black) >= contrast(white) {
        black
    } else {
        white
    }
}
