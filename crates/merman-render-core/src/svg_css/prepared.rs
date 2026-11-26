//! Prepared values and ordered emission for the shared Mermaid stylesheet.

use crate::{
    Result,
    config::{
        ConfigLookup, config_css_number_or_string, config_font_family_css,
        config_root_font_family_css, config_string,
        config_theme_font_size_css_or_root_number_px_opt, json_bool, value_at,
    },
    number_format::canonical_number,
    svg_output::SvgOutput,
};

use super::{
    CssSelectorDiagramId, FragmentDiagramId, MermaidBaseCss, SvgDiagramIdValue,
    emit_mermaid_base_css_root_rule, scoped_drop_shadow, write_mermaid_base_css_prefix,
};

// Keep Mermaid@11.16.0 ordering quirks to preserve DOM parity.

fn theme_token(
    effective_config: &(impl ConfigLookup + ?Sized),
    key: &str,
    fallback: &str,
) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

fn mermaid_stroke_width_px(effective_config: &(impl ConfigLookup + ?Sized)) -> String {
    let value = config_css_number_or_string(effective_config, &["themeVariables", "strokeWidth"])
        .unwrap_or_else(|| "1".to_string());
    let value = value.trim();
    if value.parse::<f64>().is_ok() {
        format!("{value}px")
    } else {
        value.to_string()
    }
}

#[derive(Debug, Clone)]
struct MermaidCommonNeoCss {
    node_border: String,
    use_gradient: bool,
    drop_shadow: String,
    stroke_width: String,
}

#[derive(Debug)]
pub struct PreparedCommonNeoCss(MermaidCommonNeoCss);

impl PreparedCommonNeoCss {
    pub fn new(config: &(impl ConfigLookup + ?Sized)) -> Self {
        Self(MermaidCommonNeoCss::new(config))
    }

    pub fn native_node_stroke(color: &str, width_px: &str) -> Self {
        Self(MermaidCommonNeoCss {
            node_border: color.to_owned(),
            use_gradient: false,
            drop_shadow: "none".to_owned(),
            stroke_width: format!("{width_px}px"),
        })
    }

    /// Writes logical CSS. Use `XmlTextOutput` when embedding it in a `<style>` element.
    pub fn write(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        write_mermaid_common_neo_css(
            out,
            CssSelectorDiagramId(diagram_id),
            FragmentDiagramId(diagram_id),
            &self.0,
        )
    }

    /// Writes logical CSS using IDs and a shadow value already formatted for CSS.
    pub fn write_with_ids(
        &self,
        out: &mut impl SvgOutput,
        selector_id: impl Copy + std::fmt::Display,
        gradient_id: impl std::fmt::Display,
        drop_shadow: impl std::fmt::Display,
    ) -> Result<()> {
        if self.0.use_gradient {
            write_mermaid_common_neo_css_values(
                out,
                &selector_id,
                &format_args!("url(#{gradient_id})"),
                &drop_shadow,
                &self.0,
            )
        } else {
            write_mermaid_common_neo_css_values(
                out,
                &selector_id,
                &self.0.node_border,
                &drop_shadow,
                &self.0,
            )
        }
    }
}

impl MermaidCommonNeoCss {
    fn new(effective_config: &(impl ConfigLookup + ?Sized)) -> Self {
        Self {
            node_border: theme_token(effective_config, "nodeBorder", "#9370DB"),
            use_gradient: value_at(effective_config, &["themeVariables", "useGradient"])
                .and_then(json_bool)
                .unwrap_or(false),
            drop_shadow: config_css_number_or_string(
                effective_config,
                &["themeVariables", "dropShadow"],
            )
            .unwrap_or_else(|| "none".to_string()),
            stroke_width: mermaid_stroke_width_px(effective_config),
        }
    }
}

struct NeoStroke<'a, I> {
    fragment_id: I,
    css: &'a MermaidCommonNeoCss,
}

impl<I> std::fmt::Display for NeoStroke<'_, I>
where
    I: Copy + std::fmt::Display,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.css.use_gradient {
            write!(formatter, "url(#{}-gradient)", self.fragment_id)
        } else {
            formatter.write_str(&self.css.node_border)
        }
    }
}

fn write_mermaid_common_neo_css<SelectorId, FragmentId>(
    out: &mut impl SvgOutput,
    selector_id: SelectorId,
    fragment_id: FragmentId,
    css: &MermaidCommonNeoCss,
) -> Result<()>
where
    SelectorId: Copy + std::fmt::Display,
    FragmentId: Copy + std::fmt::Display,
{
    let neo_stroke = NeoStroke { fragment_id, css };
    let drop_shadow = scoped_drop_shadow(fragment_id, &css.drop_shadow);
    write_mermaid_common_neo_css_values(out, &selector_id, &neo_stroke, &drop_shadow, css)
}

#[inline(never)]
fn write_mermaid_common_neo_css_values(
    out: &mut dyn SvgOutput,
    selector_id: &dyn std::fmt::Display,
    neo_stroke: &dyn std::fmt::Display,
    drop_shadow: &dyn std::fmt::Display,
    css: &MermaidCommonNeoCss,
) -> Result<()> {
    let _ = write!(
        out,
        r#"#{} .node .neo-node{{stroke:{};}}"#,
        selector_id, css.node_border
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].node rect,#{} [data-look="neo"].cluster rect,#{} [data-look="neo"].node polygon{{stroke:{};filter:{};}}"#,
        selector_id, selector_id, selector_id, neo_stroke, drop_shadow
    );
    out.checkpoint()?;
    let _ = crate::write_static_css!(
        out,
        "#",
        selector_id => r#" [data-look="neo"].swimlane.cluster rect{filter:none;}"#,
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].node path{{stroke:{};stroke-width:{};}}"#,
        selector_id, neo_stroke, css.stroke_width
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].node .outer-path{{filter:{};}}"#,
        selector_id, drop_shadow
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].node .neo-line path{{stroke:{};filter:none;}}"#,
        selector_id, css.node_border
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].node circle{{stroke:{};filter:{};}}"#,
        selector_id, neo_stroke, drop_shadow
    );
    out.checkpoint()?;
    let _ = crate::write_static_css!(
        out,
        "#",
        selector_id => r#" [data-look="neo"].node circle .state-start{fill:#000000;}"#,
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].icon-shape .icon{{fill:{};filter:{};}}"#,
        selector_id, neo_stroke, drop_shadow
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].icon-shape .icon-neo path{{stroke:{};filter:{};}}"#,
        selector_id, neo_stroke, drop_shadow
    );
    out.checkpoint()
}

/// Common stylesheet values, including the optional independent Mermaid `:root` font.
#[derive(Debug, Clone)]
pub struct PreparedCommonCss {
    font_family: String,
    root_font_family: Option<String>,
    font_size_css: String,
    normal_edge_stroke_width_css: String,
    text_color: String,
    line_color: String,
    error_bkg: String,
    error_text: String,
    neo: MermaidCommonNeoCss,
}

impl PreparedCommonCss {
    pub fn new(config: &(impl ConfigLookup + ?Sized), resolved_font_family: Option<&str>) -> Self {
        let font_family =
            resolved_font_family.map_or_else(|| config_font_family_css(config), str::to_owned);
        let font_size_css = config_theme_font_size_css_or_root_number_px_opt(config)
            .map(|size| format!("{}px", canonical_number(size)))
            .unwrap_or_else(|| "16px".to_owned());
        let values = Self::with_resolved_typography(config, font_family, font_size_css);
        if resolved_font_family.is_some() {
            values
        } else {
            values.with_configured_root_font_family(config)
        }
    }

    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    pub fn text_color(&self) -> &str {
        &self.text_color
    }

    pub fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub fn line_color(&self) -> &str {
        &self.line_color
    }

    pub fn with_text_color(mut self, text_color: &str) -> Self {
        self.text_color = text_color.to_owned();
        self
    }

    pub fn with_configured_root_font_family(
        mut self,
        config: &(impl ConfigLookup + ?Sized),
    ) -> Self {
        self.root_font_family = Some(config_root_font_family_css(config));
        self
    }

    pub fn with_resolved_typography(
        effective_config: &(impl ConfigLookup + ?Sized),
        font_family: String,
        font_size_css: String,
    ) -> Self {
        Self {
            font_family,
            root_font_family: None,
            font_size_css,
            normal_edge_stroke_width_css: mermaid_stroke_width_px(effective_config),
            text_color: theme_token(effective_config, "textColor", "#333"),
            line_color: theme_token(effective_config, "lineColor", "#333333"),
            error_bkg: theme_token(effective_config, "errorBkgColor", "#552222"),
            error_text: theme_token(effective_config, "errorTextColor", "#552222"),
            neo: MermaidCommonNeoCss::new(effective_config),
        }
    }

    /// Writes logical CSS. Use `XmlTextOutput` when embedding it in a `<style>` element.
    pub fn write(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        let selector_id = CssSelectorDiagramId(diagram_id);
        self.write_prefix(out, selector_id)?;
        self.write_root(out, selector_id, FragmentDiagramId(diagram_id))
    }

    /// Writes logical CSS using a selector ID already formatted for CSS.
    pub fn write_prefix<I>(&self, out: &mut impl SvgOutput, selector_id: I) -> Result<()>
    where
        I: Copy + std::fmt::Display,
    {
        write_mermaid_base_css_prefix(
            out,
            selector_id,
            MermaidBaseCss {
                font_family: &self.font_family,
                font_size_css: &self.font_size_css,
                normal_edge_stroke_width_css: &self.normal_edge_stroke_width_css,
                text_color: &self.text_color,
                line_color: &self.line_color,
                error_bkg: &self.error_bkg,
                error_text: &self.error_text,
            },
        )
    }

    /// Appends the logical CSS for Neo and the prepared `:root` font.
    /// Format selector and fragment IDs for their respective CSS contexts before calling.
    pub fn write_root<SelectorId, FragmentId>(
        &self,
        out: &mut impl SvgOutput,
        selector_id: SelectorId,
        fragment_id: FragmentId,
    ) -> Result<()>
    where
        SelectorId: Copy + std::fmt::Display,
        FragmentId: Copy + std::fmt::Display,
    {
        let root_font_family = self
            .root_font_family
            .as_deref()
            .unwrap_or(&self.font_family);
        emit_prepared_common_root(out, &selector_id, &fragment_id, &self.neo, root_font_family)
    }
}

#[inline(never)]
fn emit_prepared_common_root(
    out: &mut dyn SvgOutput,
    selector_id: &dyn std::fmt::Display,
    fragment_id: &dyn std::fmt::Display,
    css: &MermaidCommonNeoCss,
    root_font_family: &str,
) -> Result<()> {
    let neo_stroke = NeoStroke { fragment_id, css };
    let drop_shadow = scoped_drop_shadow(fragment_id, &css.drop_shadow);
    write_mermaid_common_neo_css_values(out, selector_id, &neo_stroke, &drop_shadow, css)?;
    emit_mermaid_base_css_root_rule(out, selector_id, root_font_family)
}
