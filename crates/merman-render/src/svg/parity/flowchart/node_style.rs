//! Prepared source and theme facets shared by node terminals.

use super::*;
use crate::flowchart::{
    FlowchartFacetPrecedence, FlowchartNodeThemeStyle, FlowchartShape, FlowchartSourceFacetStatus,
};

#[derive(Debug)]
pub(super) struct PreparedNodeTerminal {
    pub(super) source_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
    pub(super) style: FlowchartNodeThemeStyle,
    pub(super) source: FlowchartCompiledStyles,
    pub(super) terminal_sources: FlowchartNodeTerminalSourceOwnership,
    pub(super) rough_group_style: String,
    pub(super) selection: PreparedNodeSelection,
}

#[derive(Debug)]
pub(super) struct PreparedNodeSelection {
    pub(super) inline_style: Option<String>,
    pub(super) junction_style: Option<String>,
    pub(super) theme_style: String,
    pub(super) fill: String,
    pub(super) stroke: String,
    pub(super) stroke_width: f32,
    pub(super) effect_stroke_width: f32,
    pub(super) stroke_dasharray: String,
    pub(super) typed_radius: Option<f32>,
    pub(super) source_radii: [Option<f64>; 2],
    pub(super) configured_radius: Option<f64>,
    pub(super) typed_label_fill: Option<String>,
    pub(super) typed_fill_selected: bool,
    pub(super) typed_stroke_selected: bool,
    pub(super) stroke_config_override: bool,
    pub(super) typed_stroke_width_selected: bool,
    pub(super) typed_stroke_dasharray_selected: bool,
    pub(super) typed_font_stack_selected: bool,
    pub(super) typed_font_size_selected: bool,
}

/// Structural selectors can own a native terminal without joining its source transport.
#[derive(Debug)]
pub(super) struct FlowchartNodeTerminalSourceOwnership {
    fill: FlowchartSourceFacetStatus,
    stroke: FlowchartSourceFacetStatus,
    stroke_width: FlowchartSourceFacetStatus,
    stroke_dasharray: FlowchartSourceFacetStatus,
    radius: FlowchartSourceFacetStatus,
    pub(super) label_foreground: FlowchartSourceFacetStatus,
    pub(super) font_stack: FlowchartSourceFacetStatus,
    pub(super) font_size: FlowchartSourceFacetStatus,
    residuals: Vec<crate::diagram_theme::SourceStyleResidual>,
}

#[derive(Debug)]
pub(super) struct FlowchartClusterStructuralSources {
    pub(super) fill: FlowchartSourceFacetStatus,
    pub(super) stroke: FlowchartSourceFacetStatus,
    pub(super) stroke_width: FlowchartSourceFacetStatus,
    pub(super) label_foreground: FlowchartSourceFacetStatus,
}

impl FlowchartClusterStructuralSources {
    pub(super) fn absent() -> Self {
        Self {
            fill: FlowchartSourceFacetStatus::Absent,
            stroke: FlowchartSourceFacetStatus::Absent,
            stroke_width: FlowchartSourceFacetStatus::Absent,
            label_foreground: FlowchartSourceFacetStatus::Absent,
        }
    }

    pub(super) fn prepare(
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        html_labels: bool,
        label_dom: crate::flowchart::FlowchartLabelDom<'_>,
        hand_drawn: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut sources = Self::absent();
        sources.label_foreground = crate::flowchart::FlowchartStructuralLabelTypography::prepare(
            class_defs,
            html_labels,
            label_dom,
            |units| work.charge(units).map_err(Into::into),
        )?
        .foreground_status();
        for class in ["root", "clusters", "cluster"] {
            let Some(declarations) = class_defs.get(class) else {
                continue;
            };
            for declaration in declarations {
                work.charge(declaration.len())?;
            }
            let styles = crate::flowchart::FlowchartClassDefStyles::prepare(declarations);
            // Native selectors reach rects or rough paths. HTML >* only reaches a native
            // cluster rect from the cluster itself; rough paths have explicit paint attributes.
            if !html_labels || (class == "cluster" && !hand_drawn) {
                for declaration in styles.shape.values() {
                    let status = match declaration.property() {
                        "fill" => &mut sources.fill,
                        "stroke" => &mut sources.stroke,
                        "stroke-width" => &mut sources.stroke_width,
                        _ => continue,
                    };
                    *status = FlowchartSourceFacetStatus::Unverified;
                }
            }
        }
        Ok(sources)
    }
}

impl FlowchartNodeTerminalSourceOwnership {
    fn prepare(
        shape: Option<&str>,
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        html_labels: bool,
        fonts: &crate::flowchart::FlowchartStructuralLabelTypography,
        hand_drawn: bool,
        source: &FlowchartCompiledStyles,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut owners = Self {
            fill: FlowchartSourceFacetStatus::Absent,
            stroke: FlowchartSourceFacetStatus::Absent,
            stroke_width: FlowchartSourceFacetStatus::Absent,
            stroke_dasharray: FlowchartSourceFacetStatus::Absent,
            radius: FlowchartSourceFacetStatus::Absent,
            label_foreground: FlowchartSourceFacetStatus::Absent,
            font_stack: FlowchartSourceFacetStatus::Absent,
            font_size: FlowchartSourceFacetStatus::Absent,
            residuals: Vec::new(),
        };
        owners.label_foreground = fonts.foreground_status();
        for (font_source, status, ordinary) in [
            (
                fonts.font_stack.as_ref(),
                &mut owners.font_stack,
                source.source_font_stack_status(),
            ),
            (
                fonts.font_size.as_ref(),
                &mut owners.font_size,
                source.source_font_size_status(),
            ),
        ] {
            let Some(font_source) = font_source else {
                continue;
            };
            *status = FlowchartSourceFacetStatus::from_parts(true, font_source.measurable());
            // A content declaration remains specified below the ordinary label style;
            // an admitted parent font cannot discharge its measurement residual.
            if !font_source.measurable() && (font_source.owns_content() || ordinary.is_absent()) {
                let declaration = font_source.declaration.bind(
                    crate::diagram_theme::SourceStyleProvenance::generated_class_css(
                        std::sync::Arc::clone(&font_source.class_id),
                        crate::diagram_theme::SourceStyleChannel::Stylesheet,
                        font_source.ordinal,
                    ),
                );
                let reason = super::style::label_source_residual_reason(&declaration, true)
                    .unwrap_or(crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface);
                owners
                    .residuals
                    .push(crate::diagram_theme::SourceStyleResidual::from_declaration(
                        &declaration,
                        reason,
                    ));
            }
        }
        let Some(shape) = shape else {
            return Ok(owners);
        };
        let Ok(shape) = FlowchartShape::resolve(shape) else {
            return Ok(owners);
        };
        let (bpmn_shape, native_rectangle) = match shape {
            FlowchartShape::Process | FlowchartShape::RoundedRectangle => (None, !hand_drawn),
            FlowchartShape::Diamond
            | FlowchartShape::Circle
            | FlowchartShape::DoubleCircle
            | FlowchartShape::LeanLeft
            | FlowchartShape::LeanRight
            | FlowchartShape::Trapezoid
            | FlowchartShape::InvertedTrapezoid
            | FlowchartShape::Subroutine => (None, false),
            FlowchartShape::Bpmn(shape)
                if shape.event_rings().is_some()
                    || shape == crate::flowchart::bpmn::BpmnShape::Annotation =>
            {
                (Some(shape), false)
            }
            _ => return Ok(owners),
        };
        // These hand-drawn writers emit paths with their own presentation attributes for all
        // four paint fields. Subroutine still emits a polygon; BPMN retains native primitives.
        let rough_paths = hand_drawn && bpmn_shape.is_none() && shape != FlowchartShape::Subroutine;
        // The normal .node primitive rules specify fill/stroke/width, blocking inheritance.
        // Hand-drawn wrappers use .rough-node, so native Subroutine/BPMN paint may inherit.
        let inherits_paint = hand_drawn && !rough_paths;
        let inherits_width =
            inherits_paint && bpmn_shape.is_none_or(|shape| shape.event_rings().is_none());
        let inherits_dash = !rough_paths;
        let body_class = bpmn_shape.map(|shape| {
            if shape.event_rings().is_some() {
                "bpmn-event-body"
            } else {
                "bpmn-annotation-body"
            }
        });

        // Every rendered node lies below root/nodes groups. SVG classDef selectors directly
        // reach these native rect/circle/polygon/path terminals. HTML `>*` selects ancestor groups,
        // except for the BPMN body's direct ring/bracket children. Terminal classes do not
        // select themselves. Keep the projections bounded to the audited native writers.
        let classes = ["root", "nodes"]
            .into_iter()
            .chain(
                bpmn_shape
                    .into_iter()
                    .flat_map(|shape| shape.classes().split_ascii_whitespace()),
            )
            .map(|class| (class, !html_labels))
            .chain(body_class.map(|class| (class, true)));
        for (class, direct_terminal) in classes {
            work.charge(1)?;
            let Some(declarations) = class_defs.get(class) else {
                continue;
            };
            for declaration in declarations {
                work.charge(declaration.len())?;
            }
            // Read parsed CSSOM declarations directly, never Mermaid's comma-delimited syntax.
            let styles = crate::flowchart::FlowchartClassDefStyles::prepare(declarations);
            for declaration in styles.shape.values() {
                let owner = match declaration.property() {
                    "fill" if direct_terminal || inherits_paint => &mut owners.fill,
                    "stroke" if direct_terminal || inherits_paint => &mut owners.stroke,
                    // BPMN rings retain a fixed terminal width even under .rough-node.
                    "stroke-width" if direct_terminal || inherits_width => &mut owners.stroke_width,
                    "stroke-dasharray" if direct_terminal || inherits_dash => {
                        &mut owners.stroke_dasharray
                    }
                    // rx/ry do not inherit and only round native Process/RoundedRectangle rects.
                    "rx" | "ry" if direct_terminal && native_rectangle => &mut owners.radius,
                    _ => continue,
                };
                // Ownership blocks typed paint even for values outside the admitted paint set.
                // It does not certify the independent source transport or remove its residuals.
                *owner = FlowchartSourceFacetStatus::Unverified;
            }
        }
        Ok(owners)
    }

    pub(super) fn merge_into_evidence(
        &self,
        evidence: &mut super::style::FlowchartNodeSourceEvidence,
    ) {
        evidence.fill = evidence.fill.merge(self.fill);
        evidence.stroke = evidence.stroke.merge(self.stroke);
        evidence.stroke_width = evidence.stroke_width.merge(self.stroke_width);
        evidence.stroke_dasharray = evidence.stroke_dasharray.merge(self.stroke_dasharray);
        evidence.radius = evidence.radius.merge(self.radius);
        evidence.residuals.extend(self.residuals.iter().cloned());
    }
}

impl PreparedNodeSelection {
    fn prepare(
        ctx: &NodeSelectionInputs<'_>,
        source: &FlowchartCompiledStyles,
        style: &FlowchartNodeThemeStyle,
        terminal_sources: &FlowchartNodeTerminalSourceOwnership,
    ) -> Self {
        let fill = style.fill_value(
            FlowchartFacetPrecedence::new(
                source.source_fill_status().merge(terminal_sources.fill),
                ctx.node_fill_config_override,
            ),
            true,
        );
        let stroke = style.stroke_value(
            FlowchartFacetPrecedence::new(
                source.source_stroke_status().merge(terminal_sources.stroke),
                ctx.node_border_config_override,
            ),
            true,
        );
        let stroke_width = style.stroke_width_value(
            FlowchartFacetPrecedence::new(
                source
                    .source_stroke_width_status()
                    .merge(terminal_sources.stroke_width),
                ctx.node_stroke_width_config_override,
            ),
            true,
        );
        let stroke_dasharray = style.stroke_dasharray_value(
            FlowchartFacetPrecedence::new(
                source
                    .source_stroke_dasharray_status()
                    .merge(terminal_sources.stroke_dasharray),
                false,
            ),
            true,
        );
        let typed_radius = style.radius_value(
            FlowchartFacetPrecedence::new(
                source.source_radius_status().merge(terminal_sources.radius),
                ctx.node_corner_radius_config_override,
            ),
            true,
        );
        let typed_label_fill = style.label_fill_value(
            FlowchartFacetPrecedence::new(
                source
                    .source_label_foreground_status()
                    .merge(terminal_sources.label_foreground),
                ctx.node_label_fill_config_override,
            ),
            true,
        );
        let mut theme_style = String::new();
        for (property, value) in [("fill", fill), ("stroke", stroke)] {
            if let Some(value) = value {
                if !theme_style.is_empty() {
                    theme_style.push(';');
                }
                let _ = write!(theme_style, "{property}:{value} !important");
            }
        }
        if let Some(value) = stroke_width {
            if !theme_style.is_empty() {
                theme_style.push(';');
            }
            let _ = write!(theme_style, "stroke-width:{value}px !important");
        }
        if let Some(value) = stroke_dasharray {
            if !theme_style.is_empty() {
                theme_style.push(';');
            }
            let _ = write!(theme_style, "stroke-dasharray:{value} !important");
        }
        let inline_style = (!theme_style.is_empty()).then(|| {
            let mut inline_style = source.node_style.clone();
            if !inline_style.is_empty() {
                inline_style.push(';');
            }
            inline_style.push_str(&theme_style);
            inline_style
        });
        let selected_width = source
            .admitted_stroke_width_value()
            .or_else(|| {
                ctx.node_stroke_width_config_override
                    .then_some(ctx.node_stroke_width)
            })
            .or(stroke_width);
        Self {
            junction_style: inline_style
                .as_deref()
                .unwrap_or(&source.node_style)
                .trim()
                .is_empty()
                .then(|| format!("fill: {} !important;", ctx.node_border_color)),
            inline_style,
            theme_style,
            fill: source
                .fill
                .as_deref()
                .or(fill)
                .unwrap_or(ctx.node_fill_color)
                .to_owned(),
            stroke: source
                .stroke
                .as_deref()
                .or(stroke)
                .unwrap_or(ctx.node_border_color)
                .to_owned(),
            stroke_width: selected_width.unwrap_or(1.3),
            // Effect outsets retain their configured fallback; shape writers use 1.3px.
            effect_stroke_width: selected_width.unwrap_or(ctx.node_stroke_width),
            stroke_dasharray: source
                .stroke_dasharray
                .as_deref()
                .or(stroke_dasharray)
                .unwrap_or("0 0")
                .trim()
                .to_owned(),
            typed_radius,
            source_radii: source.rectangle_source_radii(),
            configured_radius: (ctx.look.is_neo() && ctx.node_corner_radius_config_override)
                .then_some(ctx.node_corner_radius),
            typed_label_fill: typed_label_fill.map(str::to_owned),
            typed_fill_selected: fill.is_some(),
            typed_stroke_selected: stroke.is_some(),
            stroke_config_override: ctx.node_border_config_override,
            typed_stroke_width_selected: stroke_width.is_some(),
            typed_stroke_dasharray_selected: stroke_dasharray.is_some(),
            typed_font_stack_selected: style.font_stack_selected(FlowchartFacetPrecedence::new(
                source
                    .source_font_stack_status()
                    .merge(terminal_sources.font_stack),
                ctx.node_typography_config_ownership.font_stack,
            )),
            typed_font_size_selected: style.font_size_selected(FlowchartFacetPrecedence::new(
                source
                    .source_font_size_status()
                    .merge(terminal_sources.font_size),
                ctx.node_typography_config_ownership.font_size,
            )),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct NodeSelectionInputs<'a> {
    classdef_html_labels: bool,
    node_fill_config_override: bool,
    node_border_config_override: bool,
    node_stroke_width_config_override: bool,
    node_corner_radius_config_override: bool,
    node_label_fill_config_override: bool,
    node_typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    node_fill_color: &'a str,
    node_border_color: &'a str,
    agentflow_container_stroke: &'a str,
    agentflow_container_stroke_config_override: bool,
    node_stroke_width: f32,
    node_corner_radius: f64,
    look: crate::config::DiagramLook<'static>,
}
impl<'a> NodeSelectionInputs<'a> {
    pub(super) fn new(
        compatibility: &'a crate::flowchart::FlowchartCompatibilityBinding,
        render_config: &super::render_config::FlowchartRenderConfig,
    ) -> Self {
        Self {
            // Class selectors follow the CSS writer's mode, which can differ from node labels.
            classdef_html_labels: compatibility.html_labels,
            node_fill_config_override: render_config.node_fill_config_override,
            node_border_config_override: render_config.node_border_config_override,
            node_stroke_width_config_override: render_config.node_stroke_width_config_override,
            node_corner_radius_config_override: render_config.node_corner_radius_config_override,
            node_label_fill_config_override: render_config.node_label_fill_config_override,
            node_typography_config_ownership: render_config.node_typography_config_ownership,
            node_fill_color: &compatibility.main_bkg,
            node_border_color: &compatibility.node_border,
            agentflow_container_stroke: &compatibility.collapsed_agentflow_stroke,
            agentflow_container_stroke_config_override: render_config
                .cluster_stroke_config_override,
            node_stroke_width: compatibility.node_stroke_width,
            node_corner_radius: compatibility.node_corner_radius,
            look: compatibility.look,
        }
    }

    pub(super) fn for_collapsed_agentflow(mut self) -> Self {
        self.node_border_config_override = self.agentflow_container_stroke_config_override;
        self.node_border_color = self.agentflow_container_stroke;
        self
    }
}

impl PreparedNodeTerminal {
    #[allow(
        clippy::too_many_arguments,
        reason = "The prepared source typography joins existing independent node facet inputs"
    )]
    pub(super) fn prepare(
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        source_styles: (&[String], &[String]),
        terminal_shape: Option<&str>,
        structural: &crate::flowchart::FlowchartStructuralLabelTypography,
        source_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
        ordinal: Option<usize>,
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        inputs: &NodeSelectionInputs<'_>,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let style = FlowchartNodeThemeStyle::resolve(theme, ordinal, work)?;
        let source =
            flowchart_compile_node_styles(class_defs, source_styles.0, source_styles.1, &[]);
        let terminal_sources = FlowchartNodeTerminalSourceOwnership::prepare(
            terminal_shape.filter(|_| theme.is_some()),
            class_defs,
            inputs.classdef_html_labels,
            structural,
            inputs.look.is_hand_drawn(),
            &source,
            work,
        )?;
        let selection = PreparedNodeSelection::prepare(inputs, &source, &style, &terminal_sources);
        let rough_group_style = if inputs.look.is_hand_drawn() {
            prepare_hand_drawn_shape_group_style(source_styles.1)
        } else {
            String::new()
        };
        Ok(Self {
            source_text_style,
            style,
            source,
            terminal_sources,
            rough_group_style,
            selection,
        })
    }
}

fn prepare_hand_drawn_shape_group_style(inline_styles: &[String]) -> String {
    let mut node_decls: Vec<String> = Vec::new();
    let mut text_decls: Vec<String> = Vec::new();

    for raw in inline_styles {
        for decl in crate::flowchart::flowchart_split_mermaid_style_decls(raw) {
            let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(decl) else {
                continue;
            };
            if is_text_style_key(key) {
                text_decls.push(format!("{key}:{value}"));
            } else {
                node_decls.push(format!("{key}:{value} !important"));
            }
        }
    }

    if node_decls.is_empty() {
        text_decls.join(";")
    } else {
        node_decls.join(";")
    }
}

#[cfg(test)]
mod rough_group_style_tests {
    use super::prepare_hand_drawn_shape_group_style;

    #[test]
    fn shape_declarations_keep_source_order_and_original_priority_suffix() {
        let source = [
            "font-size:19px,fill:#112233,stroke:#445566 !important".to_owned(),
            "fill:#aabbcc".to_owned(),
        ];
        assert_eq!(
            prepare_hand_drawn_shape_group_style(&source),
            "fill:#112233 !important;stroke:#445566 !important !important;fill:#aabbcc !important"
        );
    }

    #[test]
    fn text_fallback_preserves_priority_and_rejects_unsafe_source_values() {
        let source = [
            "color:#112233 !important,font-size:19px".to_owned(),
            "fill:red;</style><svg>".to_owned(),
            "background:url(javascript:alert(1))".to_owned(),
        ];
        assert_eq!(
            prepare_hand_drawn_shape_group_style(&source),
            "color:#112233 !important;font-size:19px"
        );
    }
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod structural_font_residual_tests {
    use crate::LayoutOptions;
    use crate::diagram_theme::{
        DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, Specified,
        ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::environment::RenderEnvironment;
    use crate::family::{
        self, FamilyStyleChannel, FamilyStyleOrigin, FamilyStyleResidualReason, RenderedFamilySvg,
    };
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use merman_core::{MermaidEngine, ParseOptions};
    use serde_json::json;

    const LAYOUTS: &[&str] = &[
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ];

    fn render(source: &str, theme: Option<&DiagramTheme>, layout: &str) -> RenderedFamilySvg {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "layout": layout, "look": "classic", "htmlLabels": true,
                "flowchart": {"htmlLabels": true, "minNodeWidth": 0}
            })),
        );
        let engine = match theme {
            Some(theme) => {
                let plan = merman_core::presentation::ThemeCompatibilityPlan::from_recipe(
                    theme.compatibility_recipe().clone(),
                );
                engine.with_theme_plan(&plan)
            }
            None => engine,
        };
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let environment = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
        let session = match theme {
            Some(theme) => environment.begin_session_with_theme(theme),
            None => environment.begin_session(),
        }
        .unwrap();
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
    }

    fn label_size(svg: &str) -> (f64, f64) {
        let document = roxmltree::Document::parse(svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some("A"))
            .unwrap();
        let label = node
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        (
            label.attribute("width").unwrap().parse().unwrap(),
            label.attribute("height").unwrap().parse().unwrap(),
        )
    }

    #[test]
    fn content_font_residuals_survive_admitted_parent_fonts() {
        use crate::diagram_theme::{ThemeTextStyle, TypographySpec};

        for (property, parent_value, content_value) in [
            ("font-size", "24px", "2em"),
            ("font-family", "serif", "var(--source-font)"),
        ] {
            for base in [false, true] {
                let spec = if base {
                    let typography = if property == "font-size" {
                        ThemeTextStyle::default().with_font_size_px(29.0).unwrap()
                    } else {
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("monospace").unwrap())
                    };
                    DiagramThemeSpec::new()
                        .with_typography(TypographySpec::default().with_default(typography))
                } else {
                    let mut patch = ThemeStylePatch::default();
                    if property == "font-size" {
                        patch.typography.font_size_px = Specified::Value(29.0);
                    } else {
                        patch.typography.font_stack =
                            Specified::Value(FontStack::single("monospace").unwrap());
                    }
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(ThemeTarget::NodeLabel, patch)),
                    )
                };
                let typed = DiagramThemeCompiler::new().compile(spec).unwrap();
                for layout in LAYOUTS {
                    for class in ["nodeLabel", "root"] {
                        let source = format!(
                            "flowchart LR\nA[WIDE]\nstyle A {property}:{parent_value}\nclassDef {class} {property}:{content_value}\n"
                        );
                        let baseline = render(&source, None, layout);
                        let rendered = render(&source, Some(&typed), layout);
                        assert!(rendered.svg().contains(&format!(
                            " .{class}>*{{{property}:{content_value}!important;}}"
                        )));
                        assert_eq!(
                            label_size(rendered.svg()),
                            label_size(baseline.svg()),
                            "{layout}, {class}, {property}, base={base}: unresolved source must not measure the typed font"
                        );
                        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                        let span = document
                            .descendants()
                            .find(|node| {
                                node.has_tag_name("span")
                                    && node.attribute("class").is_some_and(|classes| {
                                        classes
                                            .split_ascii_whitespace()
                                            .any(|class| class == "nodeLabel")
                                    })
                            })
                            .unwrap();
                        assert!(
                            span.children().any(|node| node.has_tag_name("p")),
                            "nodeLabel source must have a real direct content child"
                        );
                        let completion = rendered.into_completion();
                        let residuals = completion.report().style_report().residuals();
                        let raw = format!("{property}:{content_value}");
                        let generated = residuals
                            .iter()
                            .filter(|residual| {
                                residual.origin() == FamilyStyleOrigin::GeneratedClassCss
                                    && residual.channel() == FamilyStyleChannel::Stylesheet
                                    && residual.owner_id() == class
                                    && residual.class_id() == Some(class)
                                    && residual.property() == Some(property)
                                    && residual.raw() == raw
                                    && residual.assignment_ordinal().is_none()
                                    && residual.declaration_ordinal() == 0
                            })
                            .count();
                        assert_eq!(
                            generated,
                            usize::from(class == "nodeLabel"),
                            "{layout}, {class}, {property}, base={base}: only the content-owned generated declaration retains its residual; report={residuals:?}"
                        );
                        if base && class == "root" && property == "font-size" {
                            // The base-only path has no NodeLabel prepared-font receipt. Its
                            // conservative inline-source residual is independent of root CSS.
                            assert!(
                                residuals.iter().any(|residual| {
                                    residual.origin() == FamilyStyleOrigin::InlineStyle
                                        && residual.channel() == FamilyStyleChannel::Label
                                        && residual.owner_id() == "A"
                                        && residual.class_id().is_none()
                                        && residual.property() == Some("font-size")
                                        && residual.raw() == "font-size:24px"
                                        && residual.reason()
                                            == FamilyStyleResidualReason::UnsupportedSurface
                                }),
                                "expected the independent inline-source residual: {residuals:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
