use std::collections::BTreeMap;

use merman_model::families::c4::C4Boundary;
use merman_model::presentation::PresentationConfig;

use crate::theme::FamilyTheme;

const MERMAID_BOUNDARY_FILL: &str = "none";
const MERMAID_BOUNDARY_STROKE: &str = "#444444";
const MERMAID_BOUNDARY_RADIUS_PX: &str = "2.5";

#[derive(Debug)]
struct C4BoundaryStyle {
    fill: Box<str>,
    stroke: Box<str>,
    radius: Box<str>,
}

/// Fill, stroke, and radius for the explicit boundaries rendered by C4.
#[derive(Debug)]
pub(crate) struct C4ClusterThemePlan {
    boundaries: BTreeMap<Box<str>, C4BoundaryStyle>,
}

impl C4ClusterThemePlan {
    pub(crate) fn resolve(
        native: Option<&FamilyTheme>,
        boundaries: &[C4Boundary],
        config: &PresentationConfig,
    ) -> Self {
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
                            .unwrap_or(MERMAID_BOUNDARY_STROKE)
                            .into(),
                        radius: MERMAID_BOUNDARY_RADIUS_PX.into(),
                    },
                )
            })
            .collect();
        if let Some(theme) = native {
            let fill = theme.roles.surface_container.first_color().to_string();
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
                    style.fill = fill.as_str().into();
                }
                if boundary.border_color.is_none() && !owns("c4.boundary_border_color") {
                    style.stroke = stroke.as_str().into();
                }
                style.radius = radius.as_str().into();
            }
        }
        Self { boundaries: styles }
    }

    pub(crate) fn boundary_tokens(&self, alias: &str) -> Option<(&str, &str, &str)> {
        let style = self.boundaries.get(alias)?;
        Some((&style.fill, &style.stroke, &style.radius))
    }

    pub(crate) fn has_separate_background(&self, alias: &str) -> bool {
        self.boundaries.get(alias).is_some_and(|style| {
            !style.fill.eq_ignore_ascii_case("none")
                && !style.fill.eq_ignore_ascii_case("transparent")
        })
    }
}
