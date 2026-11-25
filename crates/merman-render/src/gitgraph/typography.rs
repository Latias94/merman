use merman_model::presentation::PresentationConfig;

use crate::config::{config_f64_css_absolute_px, config_font_family_css, config_string};
use crate::mermaid_style::source_font_size_after_style;
use crate::mermaid_style::{
    css_font_size_is_zero, css_font_size_override, parse_style_declaration,
};
use crate::model::{GitGraphCommitLayout, GitGraphDiagramLayout};
use crate::text::TextStyle;

mod native;
pub(crate) use native::unsupported_native_font_size;

const DEFAULT_FONT_SIZE_PX: f64 = 16.0;
const DEFAULT_LABEL_FONT_SIZE_PX: f64 = 10.0;
const TITLE_FONT_SIZE_PX: f64 = 18.0;

#[derive(Debug)]
struct GitGraphRoleFontSize {
    css: Box<str>,
    measured_px: f64,
    used_px: Option<f64>,
}

impl GitGraphRoleFontSize {
    fn resolve(raw: String, base_font_size_px: f64) -> Self {
        let declaration = format!("font-size:{raw}");
        let parsed = parse_style_declaration(&declaration);
        let fixed = parsed.as_ref().is_some_and(|declaration| {
            declaration.font_size_is_literal_zero()
                || declaration.analysis().context_free_font_size_px().is_some()
        });
        let resolved = source_font_size_after_style(Some(base_font_size_px), &declaration);
        let unresolved_precision = css_font_size_override(&declaration).is_none()
            && parsed.as_ref().is_some_and(|parsed| {
                if !parsed.is_single_component_value()
                    || parsed.font_size_is_literal_zero()
                    || parsed.font_size_is_negative_literal()
                {
                    return false;
                }
                let mut parser = cssparser::Parser::new(parsed.value());
                match parser.next() {
                    Ok(cssparser::Token::Dimension { value, unit, .. })
                        if !value.is_finite() || *value == 0.0 =>
                    {
                        // Ask the shared validator about the decoded unit, independently
                        // of the unrepresentable magnitude. Unknown units still inherit.
                        let mut zero_length = String::from("0");
                        cssparser::serialize_identifier(unit, &mut zero_length)
                            .expect("writing a CSS unit to String cannot fail");
                        css_font_size_is_zero(&zero_length)
                    }
                    Ok(cssparser::Token::Percentage { unit_value, .. }) => {
                        !unit_value.is_finite() || *unit_value == 0.0
                    }
                    _ => false,
                }
            });
        let resolved = resolved.filter(|_| !unresolved_precision);
        let used_px = resolved.filter(|size| {
            *size != 0.0 || css_font_size_is_zero(&raw) || (!fixed && base_font_size_px == 0.0)
        });
        Self {
            css: raw.into_boxed_str(),
            measured_px: resolved.unwrap_or(DEFAULT_LABEL_FONT_SIZE_PX),
            used_px,
        }
    }

    fn from_config(config: &PresentationConfig, key: &str, base_font_size_px: f64) -> Self {
        Self::resolve(
            config_string(config.as_value(), &["themeVariables", key])
                .unwrap_or_else(|| "10px".to_owned()),
            base_font_size_px,
        )
    }

    fn css(&self) -> &str {
        &self.css
    }
}

#[derive(Debug)]
struct GitGraphRoleFontWeight {
    css: Option<Box<str>>,
    measured: Option<String>,
}

impl GitGraphRoleFontWeight {
    fn resolve(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return Self {
                css: None,
                measured: None,
            };
        };
        let declaration = format!("font-weight:{raw}");
        let parsed = crate::mermaid_style::parse_style_declaration(&declaration);
        let measurable = parsed.as_ref().is_some_and(|declaration| {
            declaration.inherits_property_value()
                || crate::mermaid_style::is_supported_css_font_weight_value(declaration.value())
        });
        let measured = if measurable {
            let base = TextStyle::default();
            let mut cascade = crate::mermaid_style::TextStyleCascade::new(
                &base,
                crate::mermaid_style::CssFontSizeContext::uniform(base.font_size),
            );
            cascade.observe_declaration(parsed.as_ref().expect("validated font weight"));
            cascade.finish().font_weight.clone()
        } else {
            // Preserve unresolved CSS for capable hosts without certifying its geometry.
            Some(raw.to_owned())
        };
        Self {
            css: Some(raw.into()),
            measured,
        }
    }
}

/// Final GitGraph typography shared by preparation, layout, stylesheet emission, and text probes.
#[derive(Debug)]
pub(crate) struct GitGraphTypographyThemePlan {
    css_binding: crate::gitgraph::GitGraphCssBinding,
    font_family_css: String,
    font_size_css: Box<str>,
    font_size_px: f64,
    commit_label_font_size: GitGraphRoleFontSize,
    commit_label_font_weight: GitGraphRoleFontWeight,
    tag_label_font_size: GitGraphRoleFontSize,
    native: Option<crate::gitgraph::GitGraphNativeStyle>,
}

fn configured_base_font_size(config: &PresentationConfig) -> Option<f64> {
    config_f64_css_absolute_px(config.as_value(), &["themeVariables", "fontSize"])
        .filter(|size| size.is_finite() && *size >= 0.0)
}

impl GitGraphTypographyThemePlan {
    pub(crate) fn for_native(
        effective_config: &PresentationConfig,
        theme: &crate::theme::FamilyTheme,
    ) -> Self {
        let typography = &theme.typography;
        let font_size_px = typography.base_size_px;
        let font_size_css = format!("{font_size_px}px").into_boxed_str();
        let native = crate::gitgraph::GitGraphNativeStyle::resolve(theme);
        let mut css_binding = crate::gitgraph::GitGraphCssBinding::resolve(
            &serde_json::Value::Null,
            &native.body_font,
            &font_size_css,
        );
        css_binding.look_is_neo =
            crate::config::mermaid_config_diagram_look(effective_config).is_neo();
        Self {
            css_binding,
            font_family_css: native.body_font.clone(),
            commit_label_font_size: GitGraphRoleFontSize::resolve(
                font_size_css.to_string(),
                font_size_px,
            ),
            tag_label_font_size: GitGraphRoleFontSize::resolve(
                font_size_css.to_string(),
                font_size_px,
            ),
            commit_label_font_weight: GitGraphRoleFontWeight::resolve(Some(&native.label_weight)),
            font_size_css,
            font_size_px,
            native: Some(native),
        }
    }

    pub(crate) const fn native(&self) -> Option<&crate::gitgraph::GitGraphNativeStyle> {
        self.native.as_ref()
    }

    pub(crate) fn resolve(effective_config: &PresentationConfig) -> Self {
        let font_family_css = config_font_family_css(effective_config);
        let font_size_px =
            configured_base_font_size(effective_config).unwrap_or(DEFAULT_FONT_SIZE_PX);
        let font_size_css: Box<str> = format!("{font_size_px}px").into_boxed_str();
        let commit_label_font_size = GitGraphRoleFontSize::from_config(
            effective_config,
            "commitLabelFontSize",
            font_size_px,
        );
        let tag_label_font_size =
            GitGraphRoleFontSize::from_config(effective_config, "tagLabelFontSize", font_size_px);

        let css_binding = crate::gitgraph::GitGraphCssBinding::resolve(
            effective_config.as_value(),
            &font_family_css,
            &font_size_css,
        );
        // Mermaid attaches .commit-label before getBBox(); branch prelayout and tags
        // use different classes and must keep their inherited measurement weight.
        let commit_label_font_weight = GitGraphRoleFontWeight::resolve(
            css_binding
                .sources
                .use_color_gen
                .then_some(css_binding.note_font_weight.as_str()),
        );
        Self {
            css_binding,
            font_family_css,
            font_size_css,
            font_size_px,
            commit_label_font_size,
            commit_label_font_weight,
            tag_label_font_size,
            native: None,
        }
    }

    pub(crate) const fn css_binding(&self) -> &crate::gitgraph::GitGraphCssBinding {
        &self.css_binding
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn branch_label_style(&self) -> TextStyle {
        self.text_style(self.font_size_px)
    }

    pub(crate) fn rendered_branch_label_style(&self, slot: Option<usize>) -> TextStyle {
        let mut style = self.branch_label_style();
        if self.css_binding.use_redux_geometry
            && slot.is_some_and(|slot| slot < self.css_binding.theme_color_limit)
        {
            style.font_weight = self.commit_label_font_weight.measured.clone();
        }
        style
    }

    pub(crate) fn commit_label_font_size_css(&self) -> &str {
        self.commit_label_font_size.css()
    }

    pub(crate) fn commit_label_style(&self) -> TextStyle {
        TextStyle {
            font_weight: self.commit_label_font_weight.measured.clone(),
            ..self.text_style(self.commit_label_font_size.measured_px)
        }
    }

    pub(crate) fn commit_label_font_weight_css(&self) -> Option<&str> {
        self.commit_label_font_weight.css.as_deref()
    }

    pub(crate) fn tag_label_font_size_css(&self) -> &str {
        self.tag_label_font_size.css()
    }

    pub(crate) fn tag_label_style(&self) -> TextStyle {
        self.text_style(self.tag_label_font_size.measured_px)
    }

    pub(crate) fn title_style(&self) -> TextStyle {
        if let Some(native) = &self.native {
            return TextStyle {
                font_family: Some(native.title_font.clone()),
                font_weight: Some(native.title_weight.clone()),
                ..self.text_style(self.title_font_size_px())
            };
        }
        self.text_style(TITLE_FONT_SIZE_PX)
    }

    pub(crate) const fn title_font_size_px(&self) -> f64 {
        if self.native.is_some() {
            self.font_size_px * 1.25
        } else {
            TITLE_FONT_SIZE_PX
        }
    }

    fn text_style(&self, font_size_px: f64) -> TextStyle {
        TextStyle {
            font_family: Some(self.font_family_css.clone()),
            font_size: font_size_px,
            font_weight: self.native.as_ref().map(|theme| theme.label_weight.clone()),
            font_style: None,
        }
    }
}

pub(crate) fn gitgraph_commit_label_is_visible(
    layout: &GitGraphDiagramLayout,
    commit: &GitGraphCommitLayout,
) -> bool {
    !commit.id.trim().is_empty()
        && commit_label_is_enabled(
            layout.show_commit_label,
            commit.commit_type,
            commit.custom_id,
        )
}

fn commit_label_is_enabled(show: bool, commit_type: i64, custom_id: Option<bool>) -> bool {
    show && (commit_type != 3 || custom_id.unwrap_or(false)) && commit_type != 4
}

pub(crate) fn gitgraph_tags_in_output_order(
    commit: &GitGraphCommitLayout,
) -> impl ExactSizeIterator<Item = &str> + Clone {
    commit.tags.iter().rev().map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_role_sizes_share_the_css_cascade_with_measurement() {
        for (raw, expected) in [
            ("12PX !IMPORTANT", Some(12.0)),
            (r"24p\74 !IMPORTANT", Some(32.0)),
            ("0rem", Some(0.0)),
            ("inherit", Some(20.0)),
            ("1.5em", Some(30.0)),
            ("125%", Some(25.0)),
            ("1e-999px", None),
            ("1e999px", None),
            ("calc(0px)", None),
            ("1rem", None),
            ("1e-999fr", Some(20.0)),
        ] {
            let resolved = GitGraphRoleFontSize::resolve(raw.to_owned(), 20.0);
            assert_eq!(resolved.css(), raw);
            assert_eq!(resolved.used_px, expected, "{raw}");
            assert_eq!(
                resolved.measured_px,
                expected.unwrap_or(DEFAULT_LABEL_FONT_SIZE_PX),
                "{raw}"
            );
        }
    }
}
