use std::fmt::Write as _;

use merman_model::presentation::PresentationConfig;
use merman_render_charts::venn::VennFontSizes;

use crate::model::VennDiagramLayout;
use crate::resources::OperationWorkMeter;
use crate::theme::{Color, FamilyTheme};

mod css_binding;
pub(crate) use css_binding::{VennAreaPaintBinding, VennCssBinding};

/// Final Venn inherited font stack shared by its four local stylesheet selectors.
#[derive(Debug)]
pub(crate) struct VennTypographyThemePlan {
    css_binding: VennCssBinding,
    native: Option<super::native_theme::VennNativeTypography>,
    font_family_css: String,
}

impl VennTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        layout: &VennDiagramLayout,
        model: &merman_model::families::venn::VennDiagramModel,
        canvas: Option<Color>,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut css_binding = VennCssBinding::resolve(config.as_value(), model, layout, work)?;
        let native = if let Some(theme) = theme {
            super::native_theme::apply_paints(
                &mut css_binding,
                theme,
                config,
                layout,
                canvas,
                work,
            )?;
            Some(super::native_theme::VennNativeTypography::resolve(
                theme, config,
            ))
        } else {
            None
        };
        Ok(Self {
            css_binding,
            native,
            font_family_css: crate::config::config_font_family_css(config),
        })
    }

    pub(crate) fn title_font_size(&self, scale: f64) -> f64 {
        VennFontSizes::new(
            self.native.as_ref().and_then(|native| native.base_size),
            scale,
        )
        .title
    }

    pub(crate) fn area_font_size(&self, scale: f64) -> f64 {
        VennFontSizes::new(
            self.native.as_ref().and_then(|native| native.base_size),
            scale,
        )
        .area
    }

    pub(crate) fn css_binding(&self) -> &VennCssBinding {
        &self.css_binding
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.native
            .as_ref()
            .and_then(|native| native.body_family.as_deref())
            .unwrap_or(&self.font_family_css)
    }

    pub(crate) fn stylesheet(&self, diagram_id: &str) -> String {
        let id = crate::svg::escape_css_identifier(diagram_id);
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
