use super::*;

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(crate) use merman_render_core::svg_css::PreparedCommonNeoCss;
use merman_render_core::svg_css::{
    CssSelectorDiagramId, PreparedCommonCss as SharedPreparedCommonCss,
};
pub(super) use merman_render_core::svg_css::{
    FragmentDiagramId, MermaidBaseCss, write_mermaid_base_css_prefix,
    write_mermaid_base_css_root_rule,
    write_mermaid_base_css_root_rule as write_mermaid_base_css_root_rule_to,
};

pub(super) fn write_mermaid_paragraph_css_to(
    out: &mut impl SvgOutput,
    id: impl std::fmt::Display,
) -> Result<()> {
    let _ = write!(out, "#{id} p{{margin:0;}}");
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
    shared: SharedPreparedCommonCss,
    root_font_family_override: Option<String>,
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
        self.shared.font_family()
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
        self.shared.text_color()
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
        self.shared.font_size_css()
    }

    #[cfg(any(
        feature = "diagram-journey",
        feature = "diagram-requirement",
        feature = "diagram-git-graph"
    ))]
    pub(crate) fn line_color(&self) -> &str {
        self.shared.line_color()
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
        self.shared = self.shared.with_text_color(text_color);
        self
    }

    pub(crate) fn new(
        effective_config: &(impl crate::config::ConfigLookup + ?Sized),
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
            shared: SharedPreparedCommonCss::with_resolved_typography(
                effective_config,
                font_family,
                font_size_css,
            ),
            root_font_family_override: None,
        }
    }

    pub(super) fn write_prefix<I>(&self, out: &mut impl SvgOutput, selector_id: I) -> Result<()>
    where
        I: Copy + std::fmt::Display,
    {
        self.shared.write_prefix(out, selector_id)
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
        let root_font_family = self
            .root_font_family_override
            .as_deref()
            .unwrap_or_else(|| self.shared.font_family());
        self.shared
            .write_root(out, selector_id, fragment_id, root_font_family)
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

    let (font_family, text_color) = values.shared.into_font_family_and_text_color();
    InfoCssParts {
        css_prefix: out,
        root_rule,
        font_family,
        text_color,
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
pub(super) fn write_pie_css(
    out: &mut output::XmlTextOutput<'_, impl SvgOutput>,
    diagram_id: SvgId<'_>,
    paint_plan: &crate::pie::PieThemePlan,
) -> Result<()> {
    let common = paint_plan.common_css();
    common.write_prefix(out, diagram_id)?;
    merman_render_charts::pie::write_mermaid_css_rules(
        out,
        diagram_id,
        paint_plan.mermaid_style().css_binding(),
        common.font_family(),
    )?;
    common.write_root(out, diagram_id, diagram_id)
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
