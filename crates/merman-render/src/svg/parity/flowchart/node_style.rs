//! Source CSS and native defaults for node SVG.

use super::*;
use crate::flowchart::{FlowchartCompatibilityBinding, FlowchartShape};

#[derive(Debug)]
pub(super) struct PreparedNodeTerminal {
    pub(super) source_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
    pub(super) source: FlowchartCompiledStyles,
    pub(super) rough_group_style: String,
    pub(super) selection: PreparedNodeSelection,
}

#[derive(Debug)]
pub(super) struct PreparedNodeSelection {
    pub(super) inline_style: Option<String>,
    pub(super) junction_style: Option<String>,
    pub(super) theme_style: String,
    native_fill: Option<String>,
    collapsed_agentflow: bool,
    pub(super) stroke_width: f32,
    pub(super) native_radius: Option<f32>,
    pub(super) source_radii: [Option<f64>; 2],
    pub(super) configured_radius: Option<f64>,
    pub(super) native_label_color: Option<String>,
}

/// Structural selectors can own a native terminal without joining its source transport.
#[derive(Default)]
struct NodeSourcePaint {
    fill: bool,
    stroke_width: bool,
    radius: bool,
}

impl NodeSourcePaint {
    fn prepare_shape(
        shape: Option<&str>,
        class_defs: &crate::flowchart::FlowchartClassDefinitions<'_>,
        html_labels: bool,
        hand_drawn: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut owners = Self::default();
        let Some(shape) = shape else {
            return Ok(owners);
        };
        let Some(shape) = FlowchartShape::resolve(shape) else {
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
        // Hand-drawn paths carry their own paint attributes. BPMN retains native primitives.
        let rough_paths = hand_drawn && bpmn_shape.is_none();
        // The normal .node primitive rules specify fill/stroke/width, blocking inheritance.
        // Hand-drawn wrappers use .rough-node, so native BPMN paint may inherit.
        let inherits_paint = hand_drawn && !rough_paths;
        let inherits_width =
            inherits_paint && bpmn_shape.is_none_or(|shape| shape.event_rings().is_none());
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
            let Some(shell) =
                class_defs.prepare_shell(class, |units| work.charge(units).map_err(Into::into))?
            else {
                continue;
            };
            let paint = &shell.paint;
            for (applies, owner) in [
                (
                    paint.fill && (direct_terminal || inherits_paint),
                    &mut owners.fill,
                ),
                // BPMN rings retain a fixed terminal width even under .rough-node.
                (
                    paint.stroke_width && (direct_terminal || inherits_width),
                    &mut owners.stroke_width,
                ),
                // rx/ry do not inherit and only round native Process/RoundedRectangle rects.
                (
                    paint.radius && direct_terminal && native_rectangle,
                    &mut owners.radius,
                ),
            ] {
                *owner |= applies;
            }
        }
        Ok(owners)
    }
}

impl PreparedNodeSelection {
    fn prepare(ctx: &NodeSelectionInputs<'_>, source: &FlowchartCompiledStyles) -> Self {
        let compatibility = ctx.compatibility;
        let width = source.admitted_stroke_width_value().or_else(|| {
            ctx.node_stroke_width_config_override
                .then_some(compatibility.node_stroke_width)
        });
        Self {
            junction_style: None,
            inline_style: None,
            theme_style: String::new(),
            native_fill: None,
            collapsed_agentflow: ctx.collapsed_agentflow,
            stroke_width: width.unwrap_or(1.3),
            native_radius: None,
            source_radii: source.rectangle_source_radii(),
            configured_radius: (compatibility.look.is_neo()
                && ctx.node_corner_radius_config_override)
                .then_some(compatibility.node_corner_radius),
            native_label_color: None,
        }
    }

    fn default_stroke<'a>(&self, compatibility: &'a FlowchartCompatibilityBinding) -> &'a str {
        if self.collapsed_agentflow {
            &compatibility.collapsed_agentflow_stroke
        } else {
            &compatibility.node_border
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct NodeSelectionInputs<'a> {
    compatibility: &'a FlowchartCompatibilityBinding,
    node_stroke_width_config_override: bool,
    node_corner_radius_config_override: bool,
    collapsed_agentflow: bool,
    native: Option<&'a crate::flowchart::native_theme::FlowchartNativeStyle>,
}
impl<'a> NodeSelectionInputs<'a> {
    pub(super) fn new(
        compatibility: &'a FlowchartCompatibilityBinding,
        render_config: &super::render_config::FlowchartRenderConfig,
        native: Option<&'a crate::flowchart::native_theme::FlowchartNativeStyle>,
    ) -> Self {
        Self {
            compatibility,
            node_stroke_width_config_override: render_config.node_stroke_width_config_override,
            node_corner_radius_config_override: render_config.node_corner_radius_config_override,
            collapsed_agentflow: false,
            native,
        }
    }

    pub(super) fn for_collapsed_agentflow(mut self) -> Self {
        self.collapsed_agentflow = true;
        self
    }
}

impl PreparedNodeTerminal {
    #[allow(
        clippy::too_many_arguments,
        reason = "The prepared source typography joins existing independent node facet inputs"
    )]
    pub(super) fn prepare(
        class_defs: &crate::flowchart::FlowchartClassDefinitions<'_>,
        (classes, inline_styles, include_default_class): (&[String], &[String], bool),
        terminal_shape: Option<&str>,
        structural: &crate::flowchart::FlowchartStructuralLabelTypography,
        source_text_style: Option<std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>>,
        ordinal: Option<usize>,
        inputs: &NodeSelectionInputs<'_>,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Option<Self>> {
        let terminal_sources = NodeSourcePaint::prepare_shape(
            terminal_shape.filter(|_| inputs.native.is_some()),
            class_defs,
            // Class selectors follow the CSS writer's mode, which can differ from node labels.
            inputs.compatibility.html_labels,
            inputs.compatibility.look.is_hand_drawn(),
            work,
        )?;
        // Geometry queries admit structural style work before returning their layout. The
        // assigned and inline style compilation below supplies only SVG paint.
        let Some(source_text_style) = source_text_style else {
            return Ok(None);
        };
        let source = if include_default_class {
            flowchart_compile_node_styles(class_defs.source, classes, inline_styles, &[])
        } else {
            flowchart_compile_styles(class_defs.source, classes, inline_styles, &[])
        };
        let mut selection = PreparedNodeSelection::prepare(inputs, &source);
        if let Some(native) = inputs.native {
            let shape = terminal_shape.and_then(FlowchartShape::resolve);
            if let Some((fill, text)) = native.node_colors(shape, ordinal, classes) {
                if source.fill.is_none() && !terminal_sources.fill {
                    let fill = fill.to_string();
                    selection.theme_style = format!("fill:{fill}");
                    selection.native_fill = Some(fill);
                    let mut inline = source.node_style.clone();
                    if !inline.is_empty() {
                        inline.push(';');
                    }
                    inline.push_str(&selection.theme_style);
                    selection.inline_style = Some(inline);
                }
                if !source.has_source_label_foreground() && structural.foreground.is_none() {
                    selection.native_label_color = Some(text.to_string());
                }
            }
            if matches!(
                shape,
                Some(FlowchartShape::Process | FlowchartShape::RoundedRectangle)
            ) && !source.has_source_radius()
                && !terminal_sources.radius
            {
                selection.native_radius = Some(native.form.radius as f32);
            }
            if source.stroke_width.is_none() && !terminal_sources.stroke_width {
                selection.stroke_width = native.form.stroke_width as f32;
            }
        }
        // Only the no-label junction writer consumes this fallback.
        if matches!(
            terminal_shape,
            Some("f-circ" | "junction" | "filled-circle")
        ) && selection.inline_style.is_none()
            && source.node_style.trim().is_empty()
        {
            let border = selection.default_stroke(inputs.compatibility);
            selection.junction_style = Some(format!("fill: {border} !important;"));
        }
        let rough_group_style = if inputs.compatibility.look.is_hand_drawn() {
            prepare_hand_drawn_shape_group_style(inline_styles)
        } else {
            String::new()
        };
        Ok(Some(Self {
            source_text_style,
            source,
            rough_group_style,
            selection,
        }))
    }

    // The terminal owns the compiled declarations and shares the operation's compatibility paint.
    pub(super) fn fill_color<'a>(
        &'a self,
        compatibility: &'a FlowchartCompatibilityBinding,
    ) -> &'a str {
        self.selection
            .native_fill
            .as_deref()
            .or(self.source.fill.as_deref())
            .unwrap_or(&compatibility.main_bkg)
    }

    pub(super) fn stroke_color<'a>(
        &'a self,
        compatibility: &'a FlowchartCompatibilityBinding,
    ) -> &'a str {
        self.source
            .stroke
            .as_deref()
            .unwrap_or_else(|| self.selection.default_stroke(compatibility))
    }

    pub(super) fn stroke_dasharray(&self) -> &str {
        self.source
            .stroke_dasharray
            .as_deref()
            .map_or("0 0", str::trim)
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
