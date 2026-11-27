use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::{
        MERMAID_DEFAULT_FONT_FAMILY_CSS, config_f64, config_f64_css_absolute_px, config_string,
        normalize_css_font_family, value_at,
    },
    mermaid_style::{css_font_family_override, font_family_css},
};
use merman_theme_contract::v2::{Color, FamilyTheme};

use super::{
    WardleyLayoutOverrides,
    font_size::{WardleyFontSize, WardleyFontSizes},
};

#[derive(Debug)]
pub(super) struct WardleyPaintBinding {
    pub(super) background_color: String,
    pub(super) axis_color: String,
    pub(super) axis_text_color: String,
    pub(super) grid_color: String,
    pub(super) component_fill: String,
    pub(super) component_stroke: String,
    pub(super) component_label_color: String,
    pub(super) link_stroke: String,
    pub(super) evolution_stroke: String,
    pub(super) anchor_label_color: String,
    pub(super) stage_divider_color: String,
    pub(super) build_fill: String,
    pub(super) build_stroke: String,
    pub(super) buy_fill: String,
    pub(super) outsource_fill: String,
    pub(super) market_fill: String,
    pub(super) annotation_fill: String,
    pub(super) annotation_text: String,
    pub(super) arrow_fill: String,
    pub(super) arrow_text: String,
    pub(super) connector_width: f64,
}

impl WardleyPaintBinding {
    fn from_config(config: &serde_json::Value) -> Self {
        let nested = |key, fallback: &str| {
            config_string(config, &["themeVariables", "wardley", key])
                .unwrap_or_else(|| fallback.to_string())
        };
        let nested_or_root = |key, root_key, fallback: &str| {
            config_string(config, &["themeVariables", "wardley", key])
                .or_else(|| config_string(config, &["themeVariables", root_key]))
                .unwrap_or_else(|| fallback.to_string())
        };
        Self {
            background_color: nested_or_root("backgroundColor", "background", "#fff"),
            axis_color: nested("axisColor", "#000"),
            axis_text_color: nested_or_root("axisTextColor", "primaryTextColor", "#222"),
            grid_color: nested("gridColor", "rgba(100, 100, 100, 0.2)"),
            component_fill: nested("componentFill", "#fff"),
            component_stroke: nested("componentStroke", "#000"),
            component_label_color: nested_or_root(
                "componentLabelColor",
                "primaryTextColor",
                "#222",
            ),
            link_stroke: nested("linkStroke", "#000"),
            evolution_stroke: nested("evolutionStroke", "#dc3545"),
            anchor_label_color: nested("anchorLabelColor", "#000"),
            stage_divider_color: nested("stageDividerColor", "#000"),
            build_fill: nested("buildFill", "#eee"),
            build_stroke: nested("buildStroke", "#000"),
            buy_fill: nested("buyFill", "#ccc"),
            outsource_fill: nested("outsourceFill", "#666"),
            market_fill: nested("marketFill", "white"),
            annotation_fill: nested("annotationBackgroundColor", "white"),
            annotation_text: config_string(
                config,
                &["themeVariables", "wardley", "annotationLabelColor"],
            )
            .unwrap_or_else(|| nested_or_root("axisTextColor", "primaryTextColor", "#222")),
            arrow_fill: nested("arrowFill", "white"),
            arrow_text: nested_or_root("axisTextColor", "primaryTextColor", "#222"),
            connector_width: 1.0,
        }
    }

    fn apply_native(&mut self, theme: &FamilyTheme, config: &PresentationConfig) -> String {
        let roles = &theme.roles;
        let variables = &config.as_value()["themeVariables"];
        let authored_local = |key: &str| {
            config
                .explicit_config_owns_path(&format!("themeVariables.wardley.{key}"))
                .then(|| variables["wardley"][key].as_str())
                .flatten()
        };
        let authored_root = |key: &str| {
            config
                .explicit_config_owns_path(&format!("themeVariables.{key}"))
                .then(|| variables[key].as_str())
                .flatten()
        };
        let paint = |key, root: Option<&str>, color: Color| {
            authored_local(key)
                .or_else(|| root.and_then(authored_root))
                .map_or_else(|| color.to_string(), str::to_owned)
        };
        let guide = roles.stroke_guide.first_color();
        let border = roles.stroke_border.first_color();
        let connector = roles.stroke_connector.first_color();
        let node = roles.surface_node.first_color();
        let primary_text = Some("primaryTextColor");
        let line = Some("lineColor");
        self.axis_color = paint("axisColor", line, connector);
        self.axis_text_color = paint("axisTextColor", primary_text, roles.text_muted);
        self.grid_color = paint("gridColor", Some("gridColor"), guide);
        self.stage_divider_color = paint("stageDividerColor", None, guide);
        self.component_fill = paint("componentFill", None, node);
        self.component_stroke = paint("componentStroke", line, border);
        self.component_label_color = paint("componentLabelColor", primary_text, roles.text_primary);
        self.anchor_label_color = paint("anchorLabelColor", None, roles.text_primary);
        self.link_stroke = paint("linkStroke", line, connector);
        self.evolution_stroke = paint(
            "evolutionStroke",
            Some("wardleyEvolutionColor"),
            roles.text_title,
        );
        self.build_fill = paint("buildFill", None, theme.data.ink(0));
        self.build_stroke = paint("buildStroke", None, border);
        self.buy_fill = paint("buyFill", None, theme.data.ink(1));
        self.outsource_fill = paint("outsourceFill", None, theme.data.ink(2));
        self.market_fill = paint("marketFill", None, theme.data.ink(3));
        self.annotation_fill = paint(
            "annotationBackgroundColor",
            None,
            roles.surface_callout.first_color(),
        );
        self.annotation_text = authored_local("annotationLabelColor").map_or_else(
            || paint("axisTextColor", primary_text, roles.text_callout),
            str::to_owned,
        );
        self.arrow_fill = paint("arrowFill", None, node);
        self.arrow_text = paint("axisTextColor", primary_text, roles.text_primary);
        self.connector_width = theme.form.connector_width;
        paint("axisTextColor", primary_text, roles.text_title)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WardleyTextRole {
    Title,
    AxisLabel,
    StageLabel,
    LinkLabel,
    NodeLabel,
    AnchorLabel,
    EvolvedLabel,
    AnnotationPoint,
    AnnotationBoxLine,
    Note,
    Accelerator,
    Deaccelerator,
}

/// Style shared by Wardley layout measurement and SVG emission.
#[derive(Debug)]
pub struct WardleyStyle {
    paint_binding: WardleyPaintBinding,
    font_family_css: Box<str>,
    write_font_family: bool,
    font_sizes: WardleyFontSizes,
    native: Option<Box<WardleyNativeStyle>>,
}

#[derive(Debug)]
struct WardleyNativeStyle {
    title_color: String,
    title_family: Option<String>,
    title_weight: u16,
    label_weight: u16,
    gridlines: Option<bool>,
}

impl WardleyStyle {
    pub(super) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        let values = config.as_value();
        let family_font =
            accepted_font_family(values["themeVariables"]["wardley"]["fontFamily"].as_str());
        let root_font = [
            (
                &["themeVariables", "fontFamily"][..],
                "themeVariables.fontFamily",
            ),
            (&["fontFamily"][..], "fontFamily"),
        ]
        .into_iter()
        .find_map(|(path, owner)| {
            accepted_font_family(value_at(values, path).and_then(|v| v.as_str()))
                .map(|font| (font, owner))
        });
        let write_font_family = family_font.is_some()
            || root_font
                .as_ref()
                .is_some_and(|(_, path)| config.explicit_config_owns_path(path));
        let font_family_css = family_font
            .or_else(|| root_font.map(|(font, _)| font))
            .unwrap_or_else(|| MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned());
        let mut style = Self {
            paint_binding: WardleyPaintBinding::from_config(values),
            font_family_css: font_family_css.into_boxed_str(),
            write_font_family,
            font_sizes: WardleyFontSizes::from_config(values),
            native: None,
        };
        if let Some(theme) = native {
            style.apply_native(theme, config);
        }
        style
    }

    fn apply_native(&mut self, theme: &FamilyTheme, config: &PresentationConfig) {
        let owns = |path| config.explicit_config_owns_path(path);
        let typography = &theme.typography;
        let native_family = !owns("fontFamily")
            && !owns("themeVariables.fontFamily")
            && !owns("themeVariables.wardley.fontFamily");
        if native_family {
            self.font_family_css = font_family_css(typography.stack(typography.body_family)).into();
        }
        self.write_font_family = true;
        let values = config.as_value();
        let base_size = [
            (
                "themeVariables.fontSize",
                config_f64_css_absolute_px(values, &["themeVariables", "fontSize"]),
            ),
            ("fontSize", config_f64(values, &["fontSize"])),
        ]
        .into_iter()
        .find_map(|(path, size)| size.filter(|size| owns(path) && size.is_finite() && *size >= 0.0))
        .unwrap_or(typography.base_size_px);
        self.font_sizes.apply_native(config, base_size);
        let title_color = self.paint_binding.apply_native(theme, config);
        self.native = Some(Box::new(WardleyNativeStyle {
            title_color,
            title_family: native_family
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
            gridlines: (!owns("wardley-beta.showGrid")).then_some(theme.form.chart.gridlines),
        }));
    }

    pub(super) fn layout_overrides(&self) -> WardleyLayoutOverrides {
        WardleyLayoutOverrides {
            show_grid: self.native.as_ref().and_then(|native| native.gridlines),
            use_font_sizes_for_layout: self.is_native(),
            font_sizes: self.font_sizes.layout_sizes(),
        }
    }

    pub(super) fn native_title_family(&self) -> Option<&str> {
        self.native.as_ref()?.title_family.as_deref()
    }

    pub(super) fn title_color(&self) -> &str {
        self.native
            .as_deref()
            .map_or(self.paint_binding.axis_text_color.as_str(), |native| {
                &native.title_color
            })
    }

    pub(super) fn native_font_weight(&self, role: WardleyTextRole) -> Option<u16> {
        self.native.as_ref().map(|native| match role {
            WardleyTextRole::Title => native.title_weight,
            _ => native.label_weight,
        })
    }

    pub(super) const fn paint_binding(&self) -> &WardleyPaintBinding {
        &self.paint_binding
    }

    pub(super) const fn is_native(&self) -> bool {
        self.native.is_some()
    }

    pub(super) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(super) const fn should_write_inherited_font(&self) -> bool {
        self.write_font_family
    }

    pub(super) fn font_size(&self, role: WardleyTextRole) -> Option<&WardleyFontSize> {
        self.font_sizes.role(role)
    }
}

fn accepted_font_family(font: Option<&str>) -> Option<String> {
    let font = font?.trim();
    if font.is_empty() {
        return None;
    }
    // These values are emitted as presentation attributes, not declaration lists.
    let mut parser = cssparser::Parser::new(font);
    while let Ok(token) = parser.next() {
        if matches!(
            token,
            cssparser::Token::Semicolon | cssparser::Token::Delim('!')
        ) {
            return None;
        }
    }
    css_font_family_override(&format!("font-family:{font}"))?;
    let normalized = normalize_css_font_family(font);
    (!normalized.is_empty()).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_paint_preserves_defaults_and_raw_nested_root_fallbacks() {
        let defaults = WardleyStyle::resolve(&PresentationConfig::empty_object(), None);
        let paint = defaults.paint_binding();
        assert_eq!(paint.background_color, "#fff");
        assert_eq!(paint.axis_color, "#000");
        assert_eq!(paint.axis_text_color, "#222");
        assert_eq!(paint.grid_color, "rgba(100, 100, 100, 0.2)");
        assert_eq!(paint.component_fill, "#fff");
        assert_eq!(paint.component_stroke, "#000");
        assert_eq!(paint.component_label_color, "#222");
        assert_eq!(paint.link_stroke, "#000");
        assert_eq!(paint.evolution_stroke, "#dc3545");

        let config = super::super::test_support::presentation_config(serde_json::json!({
            "themeVariables": {
                "background": "var(--background)",
                "primaryTextColor": "currentColor",
                "wardley": {
                    "backgroundColor": null,
                    "axisColor": "var(--axis)",
                    "axisTextColor": "",
                    "gridColor": "transparent",
                    "componentFill": "none",
                    "componentStroke": "invalid-color",
                    "componentLabelColor": 7,
                    "linkStroke": "var(--link)",
                    "evolutionStroke": "currentColor"
                }
            }
        }));
        let prepared = WardleyStyle::resolve(&config, None);
        let paint = prepared.paint_binding();
        assert_eq!(paint.background_color, "var(--background)");
        assert_eq!(paint.axis_color, "var(--axis)");
        assert_eq!(paint.axis_text_color, "");
        assert_eq!(paint.grid_color, "transparent");
        assert_eq!(paint.component_fill, "none");
        assert_eq!(paint.component_stroke, "invalid-color");
        assert_eq!(paint.component_label_color, "currentColor");
        assert_eq!(paint.link_stroke, "var(--link)");
        assert_eq!(paint.evolution_stroke, "currentColor");
    }
}
