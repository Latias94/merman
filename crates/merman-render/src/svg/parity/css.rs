use super::*;

// Shared Mermaid diagram CSS fragments (split from parity.rs).
//
// Keep Mermaid@11.16.0 ordering quirks to preserve DOM parity.

#[derive(Clone, Copy)]
struct CssSelectorDiagramId<I>(I);

impl<I> std::fmt::Display for CssSelectorDiagramId<I>
where
    I: SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if is_normalized_svg_diagram_id(self.0.semantic_value()) {
            return std::fmt::Display::fmt(&self.0, formatter);
        }
        formatter.write_str(&crate::svg::escape_css_identifier(self.0.semantic_value()))
    }
}

#[derive(Clone, Copy)]
pub(super) struct FragmentDiagramId<I>(pub(super) I);

impl<I> std::fmt::Display for FragmentDiagramId<I>
where
    I: SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if is_normalized_svg_diagram_id(self.0.semantic_value()) {
            return std::fmt::Display::fmt(&self.0, formatter);
        }
        // An unquoted CSS URL has a different token grammar from a selector identifier.
        // Keep punctuation such as `:` and `.` literal; XML metacharacters belong to the sink.
        for character in self.0.semantic_value().chars() {
            if character <= '\u{20}' || character == '\u{7f}' {
                write!(formatter, "\\{:x} ", character as u32)?;
            } else {
                if matches!(character, '"' | '\'' | '(' | ')' | '\\') {
                    formatter.write_str("\\")?;
                }
                formatter.write_char(character)?;
            }
        }
        Ok(())
    }
}

fn is_normalized_svg_diagram_id(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Clone, Copy)]
pub(super) struct MermaidBaseCss<'a> {
    pub(super) font_family: &'a str,
    pub(super) font_size_css: &'a str,
    pub(super) normal_edge_stroke_width_css: &'a str,
    pub(super) text_color: &'a str,
    pub(super) line_color: &'a str,
    pub(super) error_bkg: &'a str,
    pub(super) error_text: &'a str,
}

pub(super) fn write_mermaid_base_css_prefix<I>(
    out: &mut impl SvgOutput,
    id: I,
    css: MermaidBaseCss<'_>,
) -> Result<()>
where
    I: Copy + std::fmt::Display,
{
    emit_mermaid_base_css_prefix(out, &id, css)
}

#[inline(never)]
fn emit_mermaid_base_css_prefix(
    out: &mut dyn SvgOutput,
    id: &dyn std::fmt::Display,
    css: MermaidBaseCss<'_>,
) -> Result<()> {
    let _ = write!(
        out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id, css.font_family, css.font_size_css, css.text_color
    );
    out.checkpoint()?;
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, css.error_bkg, id, css.error_text, css.error_text
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .edge-thickness-normal{{stroke-width:{};}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, css.normal_edge_stroke_width_css, id, id, id, id, id
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, css.line_color, css.line_color, id, css.line_color
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} svg{{font-family:{};font-size:{};}}#{} p{{margin:0;}}"#,
        id, css.font_family, css.font_size_css, id
    );
    out.checkpoint()
}

pub(super) fn write_mermaid_paragraph_css_to(
    out: &mut impl SvgOutput,
    id: impl std::fmt::Display,
) -> Result<()> {
    let _ = write!(out, "#{id} p{{margin:0;}}");
    out.checkpoint()
}

pub(super) fn write_mermaid_base_css_root_rule_to(
    out: &mut impl SvgOutput,
    id: impl Copy + std::fmt::Display,
    font_family: &str,
) -> Result<()> {
    write_mermaid_base_css_root_rule(out, id, font_family)
}

fn mermaid_stroke_width_px(
    effective_config: &(impl crate::config::ConfigLookup + ?Sized),
) -> String {
    let value = crate::config::config_css_number_or_string(
        effective_config,
        &["themeVariables", "strokeWidth"],
    )
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

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
#[derive(Debug)]
pub(crate) struct PreparedCommonNeoCss(MermaidCommonNeoCss);

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
impl PreparedCommonNeoCss {
    pub(crate) fn new(config: &(impl crate::config::ConfigLookup + ?Sized)) -> Self {
        Self(MermaidCommonNeoCss::new(config))
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn native_node_stroke(color: &str, width_px: &str) -> Self {
        Self(MermaidCommonNeoCss {
            node_border: color.to_owned(),
            use_gradient: false,
            drop_shadow: "none".to_owned(),
            stroke_width: format!("{width_px}px"),
        })
    }

    #[cfg(feature = "diagram-usecase")]
    pub(super) fn write(
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

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(super) fn write_with_ids(
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
    fn new(effective_config: &(impl crate::config::ConfigLookup + ?Sized)) -> Self {
        Self {
            node_border: theme_token(effective_config, "nodeBorder", "#9370DB"),
            use_gradient: config_bool(effective_config, &["themeVariables", "useGradient"])
                .unwrap_or(false),
            drop_shadow: crate::config::config_css_number_or_string(
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
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].swimlane.cluster rect{{filter:none;}}"#,
        selector_id
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
    let _ = write!(
        out,
        r##"#{} [data-look="neo"].node circle .state-start{{fill:#000000;}}"##,
        selector_id
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

pub(super) fn write_mermaid_base_css_root_rule<I>(
    out: &mut impl SvgOutput,
    id: I,
    font_family: &str,
) -> Result<()>
where
    I: Copy + std::fmt::Display,
{
    emit_mermaid_base_css_root_rule(out, &id, font_family)
}

#[inline(never)]
fn emit_mermaid_base_css_root_rule(
    out: &mut dyn SvgOutput,
    id: &dyn std::fmt::Display,
    font_family: &str,
) -> Result<()> {
    if font_family.is_empty() {
        return Ok(());
    }
    let _ = write!(
        out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font_family
    );
    out.checkpoint()
}

pub(super) struct InfoCssParts {
    pub(super) css_prefix: String,
    pub(super) root_rule: String,
    pub(super) font_family: String,
    pub(super) text_color: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedCommonCss {
    font_family: String,
    root_font_family_override: Option<String>,
    font_size_css: String,
    normal_edge_stroke_width_css: String,
    text_color: String,
    line_color: String,
    error_bkg: String,
    error_text: String,
    neo: MermaidCommonNeoCss,
}

impl PreparedCommonCss {
    #[cfg(any(feature = "diagram-class", feature = "diagram-er"))]
    pub(crate) fn bind(
        effective_config: &serde_json::Value,
        font_family: &str,
        font_size_css: &str,
        typed_font_stack_active: bool,
    ) -> Self {
        Self::bind_resolved(
            effective_config,
            normalize_css_font_family(font_family),
            font_size_css.to_owned(),
            typed_font_stack_active,
        )
    }

    #[cfg(feature = "diagram-class")]
    pub(crate) fn bind_normalized(
        effective_config: &serde_json::Value,
        font_family: &str,
        font_size_css: &str,
        typed_font_stack_active: bool,
    ) -> Self {
        Self::bind_resolved(
            effective_config,
            font_family.to_owned(),
            font_size_css.to_owned(),
            typed_font_stack_active,
        )
    }

    #[cfg(any(feature = "diagram-class", feature = "diagram-er"))]
    fn bind_resolved(
        effective_config: &serde_json::Value,
        font_family: String,
        font_size_css: String,
        typed_font_stack_active: bool,
    ) -> Self {
        let mut values =
            Self::with_resolved_typography(effective_config, font_family, font_size_css);
        if !typed_font_stack_active {
            values = values.with_configured_root_font_family(effective_config);
        }
        values
    }

    #[cfg(any(
        feature = "diagram-class",
        feature = "diagram-c4",
        feature = "diagram-pie",
        feature = "diagram-gantt",
        feature = "diagram-quadrant-chart",
        feature = "diagram-journey",
        feature = "diagram-timeline",
        feature = "diagram-kanban",
        feature = "diagram-treemap",
        feature = "diagram-requirement",
        feature = "diagram-sankey",
        feature = "diagram-cynefin",
        feature = "diagram-xychart"
    ))]
    pub(crate) fn font_family(&self) -> &str {
        &self.font_family
    }

    #[cfg(any(
        feature = "diagram-pie",
        feature = "diagram-timeline",
        feature = "diagram-kanban",
        feature = "diagram-gantt",
        feature = "diagram-sankey",
        feature = "diagram-git-graph",
        feature = "diagram-requirement"
    ))]
    pub(crate) fn text_color(&self) -> &str {
        &self.text_color
    }

    #[cfg(any(
        feature = "diagram-journey",
        feature = "diagram-kanban",
        feature = "diagram-c4",
        feature = "diagram-gantt",
        feature = "diagram-pie",
        feature = "diagram-requirement"
    ))]
    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    #[cfg(any(
        feature = "diagram-journey",
        feature = "diagram-requirement",
        feature = "diagram-git-graph"
    ))]
    pub(crate) fn line_color(&self) -> &str {
        &self.line_color
    }

    #[cfg(any(
        feature = "diagram-journey",
        feature = "diagram-timeline",
        feature = "diagram-kanban",
        feature = "diagram-sankey",
        feature = "diagram-treemap",
        feature = "diagram-requirement"
    ))]
    pub(crate) fn with_text_color(mut self, text_color: &str) -> Self {
        self.text_color = text_color.to_owned();
        self
    }

    pub(crate) fn new(
        effective_config: &serde_json::Value,
        resolved_font_family: Option<&str>,
    ) -> Self {
        let font_family = resolved_font_family.map_or_else(
            || crate::config::config_font_family_css(effective_config),
            str::to_owned,
        );
        let font_size_css =
            crate::config::config_theme_font_size_css_or_root_number_px_opt(effective_config)
                .map(|font_size| format!("{}px", fmt(font_size)))
                .unwrap_or_else(|| "16px".to_string());

        let mut values =
            Self::with_resolved_typography(effective_config, font_family, font_size_css);
        if resolved_font_family.is_none() {
            values = values.with_configured_root_font_family(effective_config);
        }
        values
    }

    pub(crate) fn with_configured_root_font_family(
        mut self,
        effective_config: &(impl crate::config::ConfigLookup + ?Sized),
    ) -> Self {
        self.root_font_family_override =
            Some(crate::config::config_root_font_family_css(effective_config));
        self
    }

    pub(crate) fn with_resolved_typography(
        effective_config: &(impl crate::config::ConfigLookup + ?Sized),
        font_family: String,
        font_size_css: String,
    ) -> Self {
        Self {
            font_family,
            root_font_family_override: None,
            font_size_css,
            normal_edge_stroke_width_css: mermaid_stroke_width_px(effective_config),
            text_color: theme_token(effective_config, "textColor", "#333"),
            line_color: theme_token(effective_config, "lineColor", "#333333"),
            error_bkg: theme_token(effective_config, "errorBkgColor", "#552222"),
            error_text: theme_token(effective_config, "errorTextColor", "#552222"),
            neo: MermaidCommonNeoCss::new(effective_config),
        }
    }

    pub(super) fn write_prefix<I>(&self, out: &mut impl SvgOutput, selector_id: I) -> Result<()>
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

    pub(super) fn write_root<SelectorId, FragmentId>(
        &self,
        out: &mut impl SvgOutput,
        selector_id: SelectorId,
        fragment_id: FragmentId,
    ) -> Result<()>
    where
        SelectorId: Copy + std::fmt::Display,
        FragmentId: Copy + std::fmt::Display,
    {
        emit_prepared_common_root(out, &selector_id, &fragment_id, self)
    }

    #[cfg(test)]
    fn write_all<SelectorId, FragmentId>(
        &self,
        out: &mut impl SvgOutput,
        selector_id: SelectorId,
        fragment_id: FragmentId,
    ) -> Result<()>
    where
        SelectorId: Copy + std::fmt::Display,
        FragmentId: Copy + std::fmt::Display,
    {
        self.write_prefix(out, selector_id)?;
        self.write_root(out, selector_id, fragment_id)
    }
}

#[inline(never)]
fn emit_prepared_common_root(
    out: &mut dyn SvgOutput,
    selector_id: &dyn std::fmt::Display,
    fragment_id: &dyn std::fmt::Display,
    values: &PreparedCommonCss,
) -> Result<()> {
    let css = &values.neo;
    let neo_stroke = NeoStroke { fragment_id, css };
    let drop_shadow = scoped_drop_shadow(fragment_id, &css.drop_shadow);
    write_mermaid_common_neo_css_values(out, selector_id, &neo_stroke, &drop_shadow, css)?;
    emit_mermaid_base_css_root_rule(
        out,
        selector_id,
        values
            .root_font_family_override
            .as_deref()
            .unwrap_or(&values.font_family),
    )
}

pub(super) struct InfoCssWriter {
    values: PreparedCommonCss,
}

impl InfoCssWriter {
    pub(super) fn from_prepared(values: &PreparedCommonCss) -> Self {
        Self {
            values: values.clone(),
        }
    }

    pub(super) fn into_parts<I>(self, diagram_id: I) -> InfoCssParts
    where
        I: Copy + std::fmt::Display,
    {
        info_css_parts_from_values(diagram_id, self.values)
    }
}

fn info_css_parts_from_values<I>(diagram_id: I, values: PreparedCommonCss) -> InfoCssParts
where
    I: Copy + std::fmt::Display,
{
    let mut out = String::new();
    values
        .write_prefix(&mut out, diagram_id)
        .expect("String-backed Mermaid base CSS emission cannot fail");
    // Keep `:root` last (matches upstream Mermaid SVG baselines).
    let mut root_rule = String::new();
    values
        .write_root(&mut root_rule, diagram_id, diagram_id)
        .expect("String-backed Mermaid root CSS emission cannot fail");

    InfoCssParts {
        css_prefix: out,
        root_rule,
        font_family: values.font_family,
        text_color: values.text_color,
    }
}

/// Writes the common Mermaid rules for an Info-style diagram.
pub(super) fn write_prepared_info_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    values: &PreparedCommonCss,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    let selector_id = CssSelectorDiagramId(diagram_id);
    values.write_prefix(out, selector_id)?;
    values.write_root(out, selector_id, FragmentDiagramId(diagram_id))
}

#[cfg(feature = "diagram-er")]
pub(super) fn er_css_with_resolved_typography<I>(
    diagram_id: I,
    values: &crate::er::ErCssBinding,
) -> Result<String>
where
    I: SvgDiagramIdValue,
{
    // Mirrors pinned Mermaid ER unified renderer stylesheet ordering (see `diagrams/er/styles.ts`
    // and shared base stylesheet).
    // Keep `:root` last (matches upstream fixtures).
    let id = CssSelectorDiagramId(diagram_id);
    let fragment_id = FragmentDiagramId(diagram_id);
    let font = &values.font_family;
    let text_color = &values.text_color;
    let line_color = &values.line_color;
    let main_bkg = &values.main_bkg;
    let node_border = &values.node_border;
    let cluster_bkg = &values.cluster_bkg;
    let cluster_border = &values.cluster_border;
    let title_color = &values.title_color;
    let node_text_color = &values.node_text_color;
    let tertiary_color = &values.tertiary_color;
    let label_background = values.label_background.clone()?;
    let edge_label_background = &values.edge_label_background;
    let stroke_width = &values.stroke_width;
    let mut out = String::new();
    values.common.write_prefix(&mut out, id)?;
    let _ = write!(
        &mut out,
        r#"#{} .entityBox{{fill:{};stroke:{};}}"#,
        id, main_bkg, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox{{fill:{};opacity:0.7;background-color:{};}}"#,
        id, tertiary_color, tertiary_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox rect{{opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBkg{{background-color:{};}}"#,
        id, label_background
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .edgeLabel .label text{{fill:{};}}#{} .edgeLabel .label{{fill:{};font-size:14px;}}"#,
        id, edge_label_background, id, edge_label_background, id, text_color, id, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        id, font, node_text_color
    );
    // Mermaid duplicates `.edge-pattern-dashed` (base rule earlier sets dasharray:3).
    let _ = write!(
        &mut out,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:8,8;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, id, id, id, main_bkg, node_border, stroke_width
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLine{{stroke:{};stroke-width:{};fill:none;}}"#,
        id, line_color, stroke_width
    );
    // Mermaid duplicates `.marker` (base rule earlier sets fill/stroke from the line color).
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:none!important;stroke:{}!important;stroke-width:1;}}"#,
        id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [data-look=neo].labelBkg{{background-color:{};}}"#,
        id,
        values.neo_label_background.clone()?
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster-label text{{fill:{};}}"#,
        id, cluster_bkg, cluster_border, id, title_color, id, title_color
    );
    values.common.write_root(&mut out, id, fragment_id)?;
    Ok(out)
}

#[cfg(feature = "diagram-pie")]
pub(super) fn write_pie_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    paint_plan: &crate::pie::PieThemePlan,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    use crate::pie::PieTextRole;

    let values = paint_plan.css_binding();
    let selector_id = CssSelectorDiagramId(diagram_id);
    let fragment_id = FragmentDiagramId(diagram_id);
    values.common.write_prefix(out, selector_id)?;
    let id = selector_id;
    let _ = write!(
        out,
        r#"#{} .pieCircle{{stroke:{};stroke-width:{};opacity:{};}}#{} .pieCircle.highlighted{{scale:1.05;opacity:1;}}#{} .pieCircle.highlightedOnHover:hover{{transition-duration:250ms;scale:1.05;opacity:1;}}#{} .pieOuterCircle{{stroke:{};stroke-width:{};fill:none;}}"#,
        id,
        values.slice_stroke,
        values.slice_stroke_width,
        values.slice_opacity,
        id,
        id,
        id,
        values.outer_stroke,
        values.outer_stroke_width,
    );
    for role in PieTextRole::ALL {
        // Preserve Mermaid's baseline declaration order and bytes for each independent role.
        let _ = write!(out, "#{} .{}{{", id, role.selector());
        match role {
            PieTextRole::Title => {
                let _ = write!(
                    out,
                    "text-anchor:middle;font-size:{};fill:{};font-family:{};",
                    values.text_size(role),
                    values.text_fill(role),
                    values.common.font_family()
                );
            }
            PieTextRole::Section => {
                let _ = write!(
                    out,
                    "font-family:{};fill:{};font-size:{};",
                    values.common.font_family(),
                    values.text_fill(role),
                    values.text_size(role)
                );
            }
            PieTextRole::Legend => {
                let _ = write!(
                    out,
                    "fill:{};font-family:{};font-size:{};",
                    values.text_fill(role),
                    values.common.font_family(),
                    values.text_size(role)
                );
            }
        }
        out.push('}');
    }
    out.checkpoint()?;
    values.common.write_root(out, selector_id, fragment_id)
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
