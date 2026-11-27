use std::collections::BTreeMap;

use merman_model::presentation::PresentationConfig;

use merman_render_core::config::configured_theme_font_size_css_or_root_number_px;
use merman_render_core::mermaid_style::{
    CssFontFamilyOwnership, css_font_size_value_override, parse_style_declaration,
};
use merman_render_core::text::TextStyle;

mod font_size;
use font_size::C4ElementFontSize;

/// Resolves C4's inherited base typography for the diagram title and root stylesheet.
///
/// C4 roles keep their independent `c4.*Font*` owners. Inheriting font families resolve against
/// the root for measurement while retaining their configured CSS spelling for the writer.
#[derive(Debug)]
pub struct C4TypographyThemePlan {
    common_css: merman_render_core::svg_css::PreparedCommonCss,
    person_border: String,
    person_background: String,
    element_styles: BTreeMap<String, C4ElementStyle>,
    boundary_fonts: C4BoundaryFonts,
    message_font: C4PreparedFont,
    relationship_text_color: String,
    relationship_line_color: String,
    native: Option<Box<super::C4NativeStyle>>,
}

#[derive(Debug)]
pub(crate) struct C4ElementStyle {
    pub(crate) font: C4PreparedFont,
    source_colors: Option<C4ElementColors>,
}

#[derive(Debug)]
pub(crate) struct C4PreparedFont {
    pub(crate) measurement_style: TextStyle,
    pub(crate) css_font_family: String,
    pub(crate) css_font_size_px: Option<f64>,
    element_font_size: Option<C4ElementFontSize>,
}

impl C4PreparedFont {
    fn resolve(
        mut font: TextStyle,
        root_font_family: &str,
        root_font_size_px: f64,
        emit_font_size: bool,
    ) -> Self {
        let css_font_family = font
            .font_family
            .clone()
            .unwrap_or_else(|| super::C4_DEFAULT_FONT_FAMILY.to_owned());
        let declaration = if css_font_family.is_empty() {
            // An empty property value is ignored by CSS, leaving inherited typography.
            String::new()
        } else {
            format!("font-family:{css_font_family}")
        };
        let family_ownership =
            merman_render_core::mermaid_style::css_font_family_ownership([declaration.as_str()]);
        if family_ownership == CssFontFamilyOwnership::Inherited {
            font.font_family = Some(root_font_family.to_owned());
        }
        // CSS rejects a negative font-size declaration. Legacy rows keep its raw spelling;
        // element CSSOM rules can omit it. Both inherit the diagram root's used size.
        let css_font_size_px = emit_font_size.then_some(font.font_size);
        let font_size_inherits_root = css_font_size_px.is_none_or(|size| size < 0.0);
        if font_size_inherits_root {
            font.font_size = root_font_size_px;
        }
        Self {
            measurement_style: font,
            css_font_family,
            css_font_size_px,
            element_font_size: None,
        }
    }

    fn resolve_element(
        font: TextStyle,
        root_font_family: &str,
        root_font_size_px: f64,
        root_size_known: bool,
        value: Option<&serde_json::Value>,
    ) -> Self {
        let size = C4ElementFontSize::resolve(value, root_size_known.then_some(root_font_size_px));
        let mut prepared = Self::resolve(font, root_font_family, root_font_size_px, false);
        prepared.measurement_style.font_size = size.used_px.unwrap_or(root_font_size_px);
        prepared.element_font_size = Some(size);
        prepared
    }

    pub(crate) fn element_font_size_css(&self) -> Option<&str> {
        self.element_font_size.as_ref()?.css.as_deref()
    }
}

#[derive(Debug)]
pub(crate) struct C4BoundaryFonts {
    pub(crate) name: C4PreparedFont,
    pub(crate) ty: C4PreparedFont,
    pub(crate) descr: C4PreparedFont,
}

impl C4BoundaryFonts {
    fn resolve(font: TextStyle, root_font_family: &str, root_font_size_px: f64) -> Self {
        // Apply Mermaid's row arithmetic to the configured size before CSS validation. Resolving
        // a negative base first would incorrectly turn a -1px name into root + 2px, not 1px.
        let mut name = font.clone();
        name.font_size += 2.0;
        name.font_weight = Some("bold".to_owned());
        let mut descr = font.clone();
        descr.font_size -= 2.0;
        Self {
            name: C4PreparedFont::resolve(name, root_font_family, root_font_size_px, true),
            ty: C4PreparedFont::resolve(font, root_font_family, root_font_size_px, true),
            descr: C4PreparedFont::resolve(descr, root_font_family, root_font_size_px, true),
        }
    }
}

#[derive(Debug)]
pub(super) struct C4ElementColors {
    pub(super) background: String,
    pub(super) border: String,
    pub(super) text: String,
}

impl C4TypographyThemePlan {
    pub fn resolve(
        native: Option<&merman_theme_contract::v2::FamilyTheme>,
        effective_config: &PresentationConfig,
        model: &merman_model::families::c4::C4DiagramModel,
    ) -> Self {
        use super::native_theme as v2;

        let configured_font_size =
            configured_theme_font_size_css_or_root_number_px(effective_config.as_value());
        let configured_css = effective_config
            .get_str("themeVariables.fontSize")
            .filter(|raw| {
                // Root CSS accepts a trailing declaration terminator; element CSSOM does not.
                parse_style_declaration(&format!("font-size:{raw}")).is_some_and(|declaration| {
                    css_font_size_value_override(declaration.source_value()).is_some()
                })
            });
        let mut font_size_known = configured_css.is_none()
            || configured_font_size.is_some_and(|(_, path)| path == "themeVariables.fontSize");
        let mut font_size_px = if font_size_known {
            configured_font_size.map_or(16.0, |(size, _)| size)
        } else {
            16.0
        };
        if let Some(native) = native
            && !(effective_config.explicit_config_owns_path("themeVariables.fontSize")
                && configured_css.is_some())
            && !configured_font_size
                .is_some_and(|(_, path)| effective_config.explicit_config_owns_path(path))
        {
            font_size_px = native.typography.base_size_px;
            font_size_known = true;
        }
        let font_size_css = if font_size_known {
            format!("{font_size_px}px")
        } else {
            configured_css
                .expect("unknown root size retains its CSS")
                .trim()
                .to_owned()
        };

        let config = effective_config.as_value();
        let view = super::C4ConfigView::new(config);
        let root_family = match native {
            Some(theme) => v2::authored_font_family(effective_config)
                .unwrap_or_else(|| v2::font_stack(theme, theme.typography.body_family)),
            None => merman_render_core::config::config_font_family_css(effective_config),
        };
        let root_family = root_family.as_str();
        let native_size = serde_json::Value::from(font_size_px);
        let element_styles = super::C4_ELEMENT_TYPES
            .iter()
            .copied()
            .chain(
                model
                    .shapes
                    .iter()
                    .map(|shape| shape.type_c4_shape.as_str()),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|name| {
                let mut text_style = view.shape_font(name);
                let mut size_value = view.shape_font_size_value(name);
                if let Some(theme) = native {
                    if !effective_config.explicit_config_owns_path(&format!("c4.{name}FontFamily"))
                    {
                        text_style.font_family = Some(root_family.to_owned());
                    }
                    if !effective_config.explicit_config_owns_path(&format!("c4.{name}FontSize")) {
                        size_value = Some(&native_size);
                    }
                    if !effective_config.explicit_config_owns_path(&format!("c4.{name}FontWeight"))
                    {
                        text_style.font_weight = Some(theme.typography.label_weight.to_string());
                    }
                }
                let font = C4PreparedFont::resolve_element(
                    text_style,
                    root_family,
                    font_size_px,
                    font_size_known,
                    size_value,
                );
                (
                    name.to_owned(),
                    C4ElementStyle {
                        font,
                        source_colors: native
                            .map(|theme| v2::element_colors(name, theme, effective_config)),
                    },
                )
            })
            .collect();
        let common_css = merman_render_core::svg_css::PreparedCommonCss::with_resolved_typography(
            config,
            root_family.to_owned(),
            font_size_css,
        );
        let mut boundary_font = view.boundary_font();
        let mut message_font = view.message_font();
        if let Some(native) = native {
            for (font, family, size, weight) in [
                (
                    &mut boundary_font,
                    "c4.boundaryFontFamily",
                    "c4.boundaryFontSize",
                    "c4.boundaryFontWeight",
                ),
                (
                    &mut message_font,
                    "c4.messageFontFamily",
                    "c4.messageFontSize",
                    "c4.messageFontWeight",
                ),
            ] {
                if !effective_config.explicit_config_owns_path(family) {
                    font.font_family = Some(root_family.to_owned());
                }
                if !effective_config.explicit_config_owns_path(size) {
                    font.font_size = font_size_px;
                }
                if !effective_config.explicit_config_owns_path(weight) {
                    font.font_weight = Some(native.typography.label_weight.to_string());
                }
            }
        }
        let relationship_color = |key, connector: bool| {
            let path = if connector {
                "c4.relationship_line_color"
            } else {
                "c4.relationship_text_color"
            };
            if let Some(theme) = native
                && !effective_config.explicit_config_owns_path(path)
            {
                if connector {
                    theme.roles.stroke_connector.first_color().to_string()
                } else {
                    theme.roles.text_primary.to_string()
                }
            } else {
                view.color(key, "#444444")
            }
        };
        Self {
            common_css,
            person_border: merman_render_core::config::config_string(
                config,
                &["themeVariables", "personBorder"],
            )
            .unwrap_or_else(|| "hsl(240, 60%, 86.2745098039%)".to_owned()),
            person_background: merman_render_core::config::config_string(
                config,
                &["themeVariables", "personBkg"],
            )
            .unwrap_or_else(|| "#ECECFF".to_owned()),
            element_styles,
            boundary_fonts: C4BoundaryFonts::resolve(boundary_font, root_family, font_size_px),
            message_font: C4PreparedFont::resolve(message_font, root_family, font_size_px, true),
            relationship_text_color: relationship_color("relationship_text_color", false),
            relationship_line_color: relationship_color("relationship_line_color", true),
            native: native.map(|theme| {
                Box::new(super::C4NativeStyle::resolve(
                    theme,
                    v2::authored_font_family(effective_config)
                        .unwrap_or_else(|| v2::font_stack(theme, theme.typography.title_family)),
                ))
            }),
        }
    }

    pub fn native_style(&self) -> Option<&super::C4NativeStyle> {
        self.native.as_deref()
    }

    pub(crate) fn common_css(&self) -> &merman_render_core::svg_css::PreparedCommonCss {
        &self.common_css
    }
    pub(crate) fn person_border(&self) -> &str {
        &self.person_border
    }
    pub(crate) fn person_background(&self) -> &str {
        &self.person_background
    }
    pub(crate) fn element_style(&self, name: &str) -> Option<&C4ElementStyle> {
        self.element_styles.get(name)
    }

    pub(super) fn prepare_element_colors(
        &mut self,
        name: &str,
        config: &super::C4ConfigView<'_>,
    ) -> Option<&C4ElementColors> {
        let style = self.element_styles.get_mut(name)?;
        Some(style.source_colors.get_or_insert_with(|| {
            let (background, border) = if name.starts_with("external_") {
                ("#999999", "#8A8A8A")
            } else {
                ("#08427B", "#073B6F")
            };
            C4ElementColors {
                background: config.color(&format!("{name}_bg_color"), background),
                border: config.color(&format!("{name}_border_color"), border),
                text: config.color(&format!("{name}_font_color"), "#FFFFFF"),
            }
        }))
    }
    pub(crate) fn boundary_fonts(&self) -> &C4BoundaryFonts {
        &self.boundary_fonts
    }
    pub(crate) fn message_font(&self) -> &C4PreparedFont {
        &self.message_font
    }

    pub(crate) fn relationship_text_color(&self) -> &str {
        &self.relationship_text_color
    }
    pub(crate) fn relationship_line_color(&self) -> &str {
        &self.relationship_line_color
    }

    #[cfg(test)]
    pub(crate) fn use_config_root_css_for_test(&mut self, config: &serde_json::Value) {
        self.common_css = merman_render_core::svg_css::PreparedCommonCss::new(config, None);
    }
}

#[cfg(test)]
mod tests {
    use super::C4TypographyThemePlan;

    #[test]
    fn prepared_element_styles_keep_actual_types_and_independent_owners() {
        let model = serde_json::from_value(serde_json::json!({
            "shapes": [
                {"alias": "custom", "typeC4Shape": "custom_type"},
                {"alias": "empty"}
            ]
        }))
        .expect("C4 actual-type fixture");
        let config = super::super::test_config(serde_json::json!({"c4": {
            "custom_typeFontFamily": "  Custom Sans ;  ", "custom_typeFontSize": "19",
            "custom_typeFontWeight": 600, "custom_type_bg_color": "var(--card)",
            "custom_type_border_color": "", "custom_type_font_color": 42,
            "boundaryFontFamily": "", "boundaryFontSize": 21,
            "messageFontFamily": "Message Sans", "messageFontSize": 17
        }}));
        let mut plan = C4TypographyThemePlan::resolve(None, &config, &model);
        let actual = plan
            .element_style("custom_type")
            .expect("actual type bound");
        assert_eq!(
            actual.font.measurement_style.font_family.as_deref(),
            Some(plan.common_css().font_family())
        );
        assert_eq!(
            actual.font.css_font_family, "",
            "CSSOM rejects the raw trailing semicolon"
        );
        // A nonzero unitless string is rejected by the element CSSOM and inherits the root.
        assert_eq!(actual.font.measurement_style.font_size, 16.0);
        assert_eq!(
            actual.font.measurement_style.font_weight.as_deref(),
            Some("600")
        );
        assert!(
            plan.element_style("").is_some(),
            "default model type is bound"
        );
        assert_eq!(plan.boundary_fonts().ty.css_font_family, "");
        assert_eq!(
            plan.boundary_fonts()
                .ty
                .measurement_style
                .font_family
                .as_deref(),
            Some(plan.common_css().font_family())
        );
        assert_eq!(plan.boundary_fonts().ty.measurement_style.font_size, 21.0);
        assert_eq!(plan.message_font().measurement_style.font_size, 17.0);
        let view = super::super::C4ConfigView::new(config.as_value());
        let colors = plan
            .prepare_element_colors("custom_type", &view)
            .expect("actual type colors");
        assert_eq!(colors.background, "var(--card)");
        assert_eq!(colors.border, "");
        assert_eq!(colors.text, "#FFFFFF");
        let colors = plan
            .prepare_element_colors("external_person", &view)
            .expect("external type colors");
        assert_eq!(colors.background, "#999999");
        assert_eq!(colors.border, "#8A8A8A");
        assert_eq!(colors.text, "#FFFFFF");
    }
}
