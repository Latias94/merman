use std::fmt::Write as _;

use merman_model::{families::venn::VennDiagramModel, presentation::PresentationConfig};
use merman_render_core::{
    Result, config::config_font_family_css, resources::OperationWorkMeter,
    svg_css::escape_css_identifier,
};
use merman_theme_contract::v2::{Color, FamilyTheme};

use super::{VennDiagramLayout, VennFontSizes, native_theme};

mod css_binding;
pub(super) use css_binding::{VennAreaPaintBinding, VennCssBinding};

/// The paint and typography resolved for a Venn diagram.
#[derive(Debug)]
pub struct VennStyle {
    css_binding: VennCssBinding,
    native: Option<native_theme::VennNativeTypography>,
    font_family_css: String,
}

impl VennStyle {
    pub(super) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        layout: &VennDiagramLayout,
        model: &VennDiagramModel,
        canvas: Option<Color>,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let mut css_binding = VennCssBinding::resolve(config.as_value(), model, layout, work)?;
        let native = if let Some(theme) = theme {
            native_theme::apply_paints(&mut css_binding, theme, config, layout, canvas, work)?;
            Some(native_theme::VennNativeTypography::resolve(theme, config))
        } else {
            None
        };
        Ok(Self {
            css_binding,
            native,
            font_family_css: config_font_family_css(config),
        })
    }

    pub(super) fn title_font_size(&self, scale: f64) -> f64 {
        VennFontSizes::new(
            self.native.as_ref().and_then(|native| native.base_size),
            scale,
        )
        .title
    }

    pub(super) fn area_font_size(&self, scale: f64) -> f64 {
        VennFontSizes::new(
            self.native.as_ref().and_then(|native| native.base_size),
            scale,
        )
        .area
    }

    pub(super) fn css_binding(&self) -> &VennCssBinding {
        &self.css_binding
    }

    fn font_family_css(&self) -> &str {
        self.native
            .as_ref()
            .and_then(|native| native.body_family.as_deref())
            .unwrap_or(&self.font_family_css)
    }

    pub(super) fn stylesheet(&self, diagram_id: &str) -> String {
        let id = escape_css_identifier(diagram_id);
        let mut css = String::new();
        if let Some(fill) = &self.css_binding.inherited_text_fill {
            let _ = write!(css, "#{id}{{fill:{fill};}}");
        }
        let set_text_fill = self
            .css_binding
            .set_text_color
            .as_ref()
            .map(|color| format!("fill:{color};"))
            .unwrap_or_default();
        let text_node_color = self
            .css_binding
            .text_node_color
            .as_ref()
            .map(|color| format!("color:{color};"))
            .unwrap_or_default();
        let title_fill = &self.css_binding.title_color;
        if let Some(native) = &self.native {
            let body = self.font_family_css();
            let title = native.title_family.as_deref().unwrap_or(body);
            let title_weight = native.title_weight;
            let body_weight = native.label_weight;
            let _ = write!(
                css,
                "#{id} .{}{{fill:{title_fill};font-family:{title};font-weight:{title_weight};}}\
                 #{id} .{} text{{font-family:{body};font-weight:{body_weight};}}\
                 #{id} .{} text{{{set_text_fill}font-family:{body};font-weight:{body_weight};}}\
                 #{id} .{}{{font-family:{body};font-weight:{body_weight};{text_node_color}}}",
                super::VENN_TITLE_CLASS,
                super::VENN_CIRCLE_CLASS,
                super::VENN_INTERSECTION_CLASS,
                super::VENN_TEXT_NODE_CLASS,
            );
            return css;
        }
        let _ = write!(
            css,
            "#{id} .{title_class}{{font-size:32px;fill:{title_fill};font-family:{font_family};}}\
#{id} .{circle_class} text{{font-size:48px;font-family:{font_family};}}\
#{id} .{intersection_class} text{{font-size:48px;{set_text_fill}font-family:{font_family};}}\
#{id} .{text_node_class}{{font-family:{font_family};{text_node_color}}}",
            title_class = super::VENN_TITLE_CLASS,
            circle_class = super::VENN_CIRCLE_CLASS,
            intersection_class = super::VENN_INTERSECTION_CLASS,
            text_node_class = super::VENN_TEXT_NODE_CLASS,
            font_family = self.font_family_css(),
        );
        css
    }
}
