use merman_model::presentation::PresentationConfig;

use merman_render_core::{config, mermaid_style::font_family_css};
use merman_theme_contract::v2::FamilyTheme;

use super::config::{PacketBitOrder, PacketConfigView, PacketStyleSettings};

/// Presentation settings bound before emitting Packet SVG.
#[derive(Debug)]
pub struct PacketStyle {
    pub(super) css_binding: PacketStyleSettings,
    pub(super) bit_order: PacketBitOrder,
    use_max_width: bool,
    pub(super) font_family_css: Box<str>,
    pub(super) root_font_size_px: f64,
    pub(super) native_typography: Option<Box<PacketNativeTypography>>,
}

#[derive(Debug)]
pub(super) struct PacketNativeTypography {
    pub(super) byte_font_family: Option<String>,
    pub(super) title_font_family: Option<String>,
    pub(super) label_weight: u16,
    pub(super) title_weight: u16,
}

impl PacketStyle {
    fn from_config(config: &PresentationConfig, packet: &PacketConfigView<'_>) -> Self {
        Self {
            css_binding: packet.style_settings(),
            bit_order: packet.bit_order(),
            use_max_width: packet.use_max_width(),
            font_family_css: config::config_font_family_css(config).into_boxed_str(),
            root_font_size_px: config::config_theme_font_size_css_or_root_number_px_opt(config)
                .unwrap_or(16.0),
            native_typography: None,
        }
    }

    pub fn resolve(native: Option<&FamilyTheme>, config: &PresentationConfig) -> Self {
        let packet = PacketConfigView::new(config);
        let mut plan = Self::from_config(config, &packet);
        let Some(theme) = native else { return plan };
        let roles = &theme.roles;
        let css = &mut plan.css_binding;
        let owns_style =
            |key, path| config.explicit_config_owns_path(path) && packet.style_value(key).is_some();
        for (target, key, path, color) in [
            (
                &mut css.block_fill_color,
                "blockFillColor",
                "themeVariables.packet.blockFillColor",
                roles.surface_node.first_color(),
            ),
            (
                &mut css.block_stroke_color,
                "blockStrokeColor",
                "themeVariables.packet.blockStrokeColor",
                roles.stroke_border.first_color(),
            ),
            (
                &mut css.start_byte_color,
                "startByteColor",
                "themeVariables.packet.startByteColor",
                roles.text_muted,
            ),
            (
                &mut css.end_byte_color,
                "endByteColor",
                "themeVariables.packet.endByteColor",
                roles.text_muted,
            ),
            (
                &mut css.label_color,
                "labelColor",
                "themeVariables.packet.labelColor",
                roles.text_primary,
            ),
            (
                &mut css.title_color,
                "titleColor",
                "themeVariables.packet.titleColor",
                roles.text_title,
            ),
        ] {
            if !owns_style(key, path) {
                *target = color.to_string().into();
            }
        }

        let typography = &theme.typography;
        let base_size = typography.base_size_px;
        for (target, key, path, size) in [
            (
                &mut css.byte_font_size,
                "byteFontSize",
                "themeVariables.packet.byteFontSize",
                base_size * 10.0 / 14.0,
            ),
            (
                &mut css.label_font_size,
                "labelFontSize",
                "themeVariables.packet.labelFontSize",
                base_size * 12.0 / 14.0,
            ),
            (
                &mut css.title_font_size,
                "titleFontSize",
                "themeVariables.packet.titleFontSize",
                base_size,
            ),
        ] {
            if !owns_style(key, path) {
                *target = format!("{size:.3}px").into();
            }
        }
        if !owns_style("blockStrokeWidth", "themeVariables.packet.blockStrokeWidth") {
            css.block_stroke_width = theme.form.stroke_width.to_string().into();
        }
        plan.root_font_size_px = config::configured_theme_font_size_css_or_root_number_px(config)
            .filter(|(_, path)| config.explicit_config_owns_path(path))
            .map_or(base_size, |(size, _)| size);
        let explicit_font = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| config.explicit_config_owns_path(path))
            .find_map(|path| {
                config
                    .get_str(path)
                    .and_then(config::normalized_config_font_family)
            });
        let has_explicit_font = explicit_font.is_some();
        plan.font_family_css = explicit_font
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)))
            .into_boxed_str();
        plan.native_typography = Some(Box::new(PacketNativeTypography {
            byte_font_family: (!has_explicit_font)
                .then(|| font_family_css(typography.stack(typography.code_family))),
            title_font_family: (!has_explicit_font)
                .then(|| font_family_css(typography.stack(typography.title_family))),
            label_weight: typography.label_weight,
            title_weight: typography.title_weight,
        }));
        plan
    }

    pub const fn use_max_width(&self) -> bool {
        self.use_max_width
    }
}
