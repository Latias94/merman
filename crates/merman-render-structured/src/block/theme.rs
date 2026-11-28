use indexmap::IndexMap;
use merman_model::{families::block::BlockClassDef, presentation::PresentationConfig};
use merman_render_core::{svg_output::cutout, text::TextStyle};
use merman_theme_contract::v2::Color;

use super::{BlockCssThemeBinding, BlockNativeStyle};

/// Final Block typography shared by layout measurement and SVG CSS.
#[derive(Debug)]
pub struct BlockTypographyThemePlan {
    css_binding: BlockCssThemeBinding,
    native: Option<BlockNativeStyle>,
    uses_default_compatibility_edge_background: bool,
    padding: f64,
    text_style: TextStyle,
}

impl BlockTypographyThemePlan {
    pub(crate) fn resolve(config: &PresentationConfig) -> merman_render_core::Result<Self> {
        let settings = super::config::BlockConfigView::new(config.as_value()).layout_settings();
        Ok(Self {
            css_binding: BlockCssThemeBinding::resolve(config.as_value())?,
            native: None,
            uses_default_compatibility_edge_background: !config
                .explicit_config_owns_path("themeVariables.edgeLabelBackground"),
            padding: settings.padding,
            text_style: settings.text_style,
        })
    }

    pub(crate) fn resolve_native(
        theme: &merman_theme_contract::v2::FamilyTheme,
        background: Option<Color>,
        config: &serde_json::Value,
    ) -> Self {
        let (native, css_binding) = BlockNativeStyle::resolve(theme, background, config);
        let text_style = TextStyle {
            font_family: Some(css_binding.root_font_family.clone()),
            font_size: theme.typography.base_size_px,
            font_weight: Some(theme.typography.label_weight.to_string()),
            font_style: None,
        };
        Self {
            css_binding,
            native: Some(native),
            uses_default_compatibility_edge_background: false,
            padding: super::config::BlockConfigView::new(config).padding(),
            text_style,
        }
    }

    pub(crate) fn prepare_class_definitions(
        &mut self,
        definitions: &IndexMap<String, BlockClassDef>,
    ) {
        self.css_binding.prepare_class_definitions(definitions);
    }

    pub(crate) fn transparent_edge_labels(&self, canvas_background: Option<Color>) -> bool {
        cutout::is_transparent_paint(&self.css_binding.edge_label_background)
            || (self.uses_default_compatibility_edge_background
                && canvas_background.is_some_and(|color| color.channels()[3] == 0))
    }

    pub fn css_binding(&self) -> &BlockCssThemeBinding {
        &self.css_binding
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    pub fn native(&self) -> Option<&BlockNativeStyle> {
        self.native.as_ref()
    }

    pub(crate) const fn padding(&self) -> f64 {
        self.padding
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.text_style.font_family.as_deref().unwrap_or_default()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.text_style.font_size
    }
}
