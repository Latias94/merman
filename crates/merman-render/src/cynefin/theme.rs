use merman_model::presentation::PresentationConfig;

use crate::theme::{Color, FamilyTheme, font_family_css};

#[derive(Debug)]
pub(crate) struct CynefinNativeStyle {
    domain_accents: [String; 5],
    source_domains: [bool; 5],
    item_fill: String,
    pub(crate) item_border_color: String,
    pub(crate) title_family: Option<String>,
    pub(crate) title_weight: u16,
    pub(crate) label_weight: u16,
}

impl CynefinNativeStyle {
    fn domain_slot(domain: &str) -> usize {
        match domain {
            super::DOMAIN_CLEAR => 0,
            super::DOMAIN_COMPLICATED => 1,
            super::DOMAIN_COMPLEX => 2,
            super::DOMAIN_CHAOTIC => 3,
            _ => 4,
        }
    }

    pub(crate) fn domain_accent(&self, domain: &str) -> &str {
        &self.domain_accents[Self::domain_slot(domain)]
    }

    pub(crate) fn item_fill<'a>(&'a self, domain: &str, domain_fill: &'a str) -> &'a str {
        if self.source_domains[Self::domain_slot(domain)] {
            domain_fill
        } else {
            &self.item_fill
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CynefinTextRole {
    DomainLabel,
    Subtitle,
    Item,
    TransitionLabel,
    Title,
}

/// Style shared by Cynefin layout measurement and SVG emission.
#[derive(Debug)]
pub(crate) struct CynefinStyle {
    common_css: crate::svg::PreparedCommonCss,
    colors: super::CynefinTheme,
}

impl CynefinStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        let values = config.as_value();
        let mut colors = super::cynefin_theme(values);
        let common_css = match native {
            Some(theme) => Self::apply_native(&mut colors, theme, config),
            None => {
                let font_family = crate::config::config_font_family_css(values);
                // Match layout measurement and both stylesheet font declarations.
                crate::svg::PreparedCommonCss::new(values, Some(&font_family))
            }
        };
        Self { common_css, colors }
    }

    fn apply_native(
        colors: &mut super::CynefinTheme,
        theme: &FamilyTheme,
        config: &PresentationConfig,
    ) -> crate::svg::PreparedCommonCss {
        let values = config.as_value();
        let variables = &values["themeVariables"];
        let owns = |path: &str| config.explicit_config_owns_path(path);
        let authored_local = |key: &str| {
            owns(&format!("themeVariables.cynefin.{key}"))
                .then(|| variables["cynefin"][key].as_str())
                .flatten()
        };
        let authored_root = |key: &str| {
            owns(&format!("themeVariables.{key}"))
                .then(|| variables[key].as_str())
                .flatten()
        };
        let paint = |key, root: Option<&str>, color: Color| {
            authored_local(key)
                .or_else(|| root.and_then(authored_root))
                .map_or_else(|| color.to_string(), str::to_owned)
        };
        let roles = &theme.roles;
        let mut source_domains = [false; 5];
        let mut domain_accents = std::array::from_fn(|slot| theme.data.fill(slot).to_string());
        let container_color = roles.surface_container.first_color().to_string();
        for (slot, (key, fill)) in [
            ("clearBg", &mut colors.clear_bg),
            ("complicatedBg", &mut colors.complicated_bg),
            ("complexBg", &mut colors.complex_bg),
            ("chaoticBg", &mut colors.chaotic_bg),
            ("confusionBg", &mut colors.confusion_bg),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(color) = authored_local(key) {
                color.clone_into(fill);
                color.clone_into(&mut domain_accents[slot]);
                source_domains[slot] = true;
            } else {
                fill.clone_from(&container_color);
            }
        }
        let line = Some("lineColor");
        let connector = roles.stroke_connector.first_color();
        colors.boundary_color = paint("boundaryColor", line, roles.stroke_guide.first_color());
        colors.cliff_color = paint("cliffColor", None, connector);
        colors.arrow_color = paint("arrowColor", line, connector);
        if !owns("themeVariables.cynefin.arrowWidth") {
            colors.arrow_width = theme.form.connector_width;
        }
        colors.text_color = paint("textColor", Some("textColor"), roles.text_primary);
        colors.label_color = paint("labelColor", Some("primaryTextColor"), roles.text_title);

        let typography = &theme.typography;
        let native_family = !owns("fontFamily") && !owns("themeVariables.fontFamily");
        let family = if native_family {
            font_family_css(typography.stack(typography.body_family))
        } else {
            crate::config::config_font_family_css(values)
        };
        let base_size = [
            (
                "themeVariables.fontSize",
                crate::config::config_f64_css_absolute_px(values, &["themeVariables", "fontSize"]),
            ),
            ("fontSize", crate::config::config_f64(values, &["fontSize"])),
        ]
        .into_iter()
        .find_map(|(path, size)| size.filter(|size| owns(path) && size.is_finite() && *size >= 0.0))
        .unwrap_or(typography.base_size_px);
        colors.font_binding = super::font_binding::CynefinFontBinding::native(config, base_size);
        colors.native = Some(Box::new(CynefinNativeStyle {
            domain_accents,
            source_domains,
            item_fill: roles.surface_node.first_color().to_string(),
            item_border_color: paint("boundaryColor", line, roles.stroke_border.first_color()),
            title_family: native_family
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
        }));
        crate::svg::PreparedCommonCss::with_resolved_typography(
            values,
            family,
            format!("{}px", crate::number_format::canonical_number(base_size)),
        )
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.common_css.font_family()
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn colors(&self) -> &super::CynefinTheme {
        &self.colors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_preserves_scoped_roles_and_shared_measurement_values() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": {
                "fontFamily": "Config Sans", "textColor": "root-text",
                "lineColor": "root-line", "primaryTextColor": "root-label",
                "cynefin": {
                    "textColor": "var(--text)", "boundaryColor": "currentColor",
                    "complexBg": "var(--complex)", "domainFontSize": "23",
                    "itemFontSize": 15
                }
            }
        }));
        let plan = CynefinStyle::resolve(&config, None);
        assert_eq!(plan.colors().text_color, "var(--text)");
        assert_eq!(plan.colors().boundary_color, "currentColor");
        assert_eq!(plan.colors().arrow_color, "root-line");
        assert_eq!(plan.colors().label_color, "root-label");
        assert_eq!(plan.colors().complex_bg, "var(--complex)");
        let fonts = &plan.colors().font_binding;
        assert_eq!(fonts.used_px(CynefinTextRole::DomainLabel), Some(23.0));
        assert_eq!(fonts.used_px(CynefinTextRole::Item), Some(15.0));
        assert_eq!(fonts.css(CynefinTextRole::Title), "232px");
        assert_eq!(plan.font_family_css(), "Config Sans");
    }
}
