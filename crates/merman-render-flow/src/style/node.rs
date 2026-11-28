//! Source CSS and native defaults for node SVG.

use std::{fmt::Write as _, sync::Arc};

use merman_render_core::{Result, mermaid_style, resources::OperationWorkMeter};
use merman_theme_contract::v2::Color;

use super::{
    compiled::{FlowchartCompiledStyles, FlowchartNodeClassStyles},
    flowchart_split_mermaid_style_decls,
};
use crate::{
    bpmn::BpmnShape,
    class_style::{FlowchartClassDefinitions, FlowchartStructuralLabelTypography},
    layout_config::FlowchartRenderConfig,
    shapes::FlowchartShape,
    svg_label::FlowchartNodeSourceTypography,
    theme::{FlowchartCompatibilityBinding, FlowchartNativeStyle},
};

#[derive(Debug)]
pub struct PreparedNodeTerminal {
    pub source_text_style: Arc<FlowchartNodeSourceTypography>,
    pub source: FlowchartCompiledStyles,
    pub(crate) has_structural_label_foreground: bool,
    pub rough_group_style: String,
    pub selection: PreparedNodeSelection,
}

#[derive(Debug)]
pub struct PreparedNodeSelection {
    pub junction_style: Option<String>,
    native_fill: Option<PreparedNativeNodeFill>,
    collapsed_agentflow: bool,
    pub stroke_width: f32,
    pub native_radius: Option<f32>,
    pub source_radii: [Option<f64>; 2],
    pub configured_radius: Option<f64>,
    pub native_label_color: Option<String>,
}

/// The full shape style also stores the native-only declaration and raw paint as suffixes.
#[derive(Debug)]
struct PreparedNativeNodeFill {
    inline_style: String,
    declaration_start: usize,
}

impl PreparedNativeNodeFill {
    const DECLARATION_PREFIX: &str = "fill:";

    fn new(source_style: &str, color: Color) -> Self {
        let separator = if source_style.is_empty() { "" } else { ";" };
        let declaration_start = source_style.len() + separator.len();
        let prefix = Self::DECLARATION_PREFIX;
        let mut inline_style =
            String::with_capacity(declaration_start + prefix.len() + "#RRGGBBAA".len());
        let _ = write!(inline_style, "{source_style}{separator}{prefix}{color}");
        Self {
            inline_style,
            declaration_start,
        }
    }

    fn declaration(&self) -> &str {
        &self.inline_style[self.declaration_start..]
    }

    fn color(&self) -> &str {
        &self.declaration()[Self::DECLARATION_PREFIX.len()..]
    }
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
        class_defs: &FlowchartClassDefinitions<'_>,
        html_labels: bool,
        hand_drawn: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
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
                if shape.event_rings().is_some() || shape == BpmnShape::Annotation =>
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

    pub(crate) fn inline_style(&self) -> Option<&str> {
        self.native_fill
            .as_ref()
            .map(|fill| fill.inline_style.as_str())
    }

    pub(crate) fn theme_style(&self) -> &str {
        self.native_fill
            .as_ref()
            .map_or("", PreparedNativeNodeFill::declaration)
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
pub struct NodeSelectionInputs<'a> {
    compatibility: &'a FlowchartCompatibilityBinding,
    node_stroke_width_config_override: bool,
    node_corner_radius_config_override: bool,
    collapsed_agentflow: bool,
    native: Option<&'a FlowchartNativeStyle>,
}
impl<'a> NodeSelectionInputs<'a> {
    pub fn new(
        compatibility: &'a FlowchartCompatibilityBinding,
        render_config: &FlowchartRenderConfig,
        native: Option<&'a FlowchartNativeStyle>,
    ) -> Self {
        Self {
            compatibility,
            node_stroke_width_config_override: render_config.node_stroke_width_config_override,
            node_corner_radius_config_override: render_config.node_corner_radius_config_override,
            collapsed_agentflow: false,
            native,
        }
    }

    pub fn for_collapsed_agentflow(mut self) -> Self {
        self.collapsed_agentflow = true;
        self
    }
}

impl PreparedNodeTerminal {
    #[allow(
        clippy::too_many_arguments,
        reason = "The prepared source typography joins existing independent node facet inputs"
    )]
    pub fn prepare(
        class_defs: &FlowchartClassDefinitions<'_>,
        node_classes: &mut FlowchartNodeClassStyles<'_>,
        (classes, inline_styles, include_default_class): (&[String], &[String], bool),
        terminal_shape: Option<&str>,
        structural: &FlowchartStructuralLabelTypography,
        source_text_style: Option<Arc<FlowchartNodeSourceTypography>>,
        ordinal: Option<usize>,
        inputs: &NodeSelectionInputs<'_>,
        work: &OperationWorkMeter,
    ) -> Result<Option<Self>> {
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
        let source = node_classes.compile(classes, inline_styles, include_default_class);
        let mut selection = PreparedNodeSelection::prepare(inputs, &source);
        if let Some(native) = inputs.native {
            let shape = terminal_shape.and_then(FlowchartShape::resolve);
            if let Some((fill, text)) = native.node_colors(shape, ordinal, classes) {
                if source.fill.is_none() && !terminal_sources.fill {
                    selection.native_fill =
                        Some(PreparedNativeNodeFill::new(&source.node_style, fill));
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
        ) && selection.native_fill.is_none()
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
            has_structural_label_foreground: structural.foreground.is_some(),
            rough_group_style,
            selection,
        }))
    }

    // The terminal owns the compiled declarations and shares the operation's compatibility paint.
    pub fn fill_color<'a>(&'a self, compatibility: &'a FlowchartCompatibilityBinding) -> &'a str {
        self.selection
            .native_fill
            .as_ref()
            .map(PreparedNativeNodeFill::color)
            .or(self.source.fill.as_deref())
            .unwrap_or(&compatibility.main_bkg)
    }

    pub fn stroke_color<'a>(&'a self, compatibility: &'a FlowchartCompatibilityBinding) -> &'a str {
        self.source
            .stroke
            .as_deref()
            .unwrap_or_else(|| self.selection.default_stroke(compatibility))
    }

    pub fn stroke_dasharray(&self) -> &str {
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
        for decl in flowchart_split_mermaid_style_decls(raw) {
            let Some((key, value)) = mermaid_style::parse_safe_style_decl(decl) else {
                continue;
            };
            if mermaid_style::is_label_style_key(key) {
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
mod native_fill_style_tests {
    use super::*;

    #[test]
    fn plain_native_fill_needs_no_style_separator() {
        let fill = PreparedNativeNodeFill::new("", Color::rgb(1, 42, 175));
        assert_eq!(fill.inline_style, "fill:#012AAF");
        assert_eq!(fill.declaration(), "fill:#012AAF");
        assert_eq!(fill.color(), "#012AAF");
    }

    #[test]
    fn native_fill_views_skip_the_authored_utf8_style_prefix() {
        let source = "STROKE:#123456 !important;--custom:é🙂 !important";
        let fill = PreparedNativeNodeFill::new(source, Color::rgba(0, 16, 170, 64));
        assert_eq!(
            fill.inline_style,
            "STROKE:#123456 !important;--custom:é🙂 !important;fill:#0010AA40"
        );
        assert_eq!(fill.declaration(), "fill:#0010AA40");
        assert_eq!(fill.color(), "#0010AA40");
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
