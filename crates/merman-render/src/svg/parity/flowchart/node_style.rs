//! Prepared source and theme facets shared by node terminals.

use super::*;
use crate::flowchart::{FlowchartFacetPrecedence, FlowchartNodeThemeStyle};

#[derive(Debug)]
pub(super) struct PreparedNodeTerminal {
    pub(super) source_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
    pub(super) style: FlowchartNodeThemeStyle,
    pub(super) source: FlowchartCompiledStyles,
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
    pub(super) typed_stroke_width_selected: bool,
    pub(super) typed_stroke_dasharray_selected: bool,
    pub(super) typed_font_stack_selected: bool,
    pub(super) typed_font_size_selected: bool,
}

impl PreparedNodeSelection {
    fn prepare(
        ctx: &NodeSelectionInputs<'_>,
        source: &FlowchartCompiledStyles,
        style: &FlowchartNodeThemeStyle,
    ) -> Self {
        let fill = style.fill_value(
            FlowchartFacetPrecedence::new(
                source.source_fill_status(),
                ctx.node_fill_config_override,
            ),
            true,
        );
        let stroke = style.stroke_value(
            FlowchartFacetPrecedence::new(
                source.source_stroke_status(),
                ctx.node_border_config_override,
            ),
            true,
        );
        let stroke_width = style.stroke_width_value(
            FlowchartFacetPrecedence::new(
                source.source_stroke_width_status(),
                ctx.node_stroke_width_config_override,
            ),
            true,
        );
        let stroke_dasharray = style.stroke_dasharray_value(
            FlowchartFacetPrecedence::new(source.source_stroke_dasharray_status(), false),
            true,
        );
        let typed_radius = style.radius_value(
            FlowchartFacetPrecedence::new(
                source.source_radius_status(),
                ctx.node_corner_radius_config_override,
            ),
            true,
        );
        let typed_label_fill = style.label_fill_value(
            FlowchartFacetPrecedence::new(
                source.source_label_foreground_status(),
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
            typed_stroke_width_selected: stroke_width.is_some(),
            typed_stroke_dasharray_selected: stroke_dasharray.is_some(),
            typed_font_stack_selected: style.font_stack_selected(FlowchartFacetPrecedence::new(
                source.source_font_stack_status(),
                ctx.node_typography_config_ownership.font_stack,
            )),
            typed_font_size_selected: style.font_size_selected(FlowchartFacetPrecedence::new(
                source.source_font_size_status(),
                ctx.node_typography_config_ownership.font_size,
            )),
        }
    }
}

pub(super) struct NodeSelectionInputs<'a> {
    node_fill_config_override: bool,
    node_border_config_override: bool,
    node_stroke_width_config_override: bool,
    node_corner_radius_config_override: bool,
    node_label_fill_config_override: bool,
    node_typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    node_fill_color: &'a str,
    node_border_color: &'a str,
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
            node_fill_config_override: render_config.node_fill_config_override,
            node_border_config_override: render_config.node_border_config_override,
            node_stroke_width_config_override: render_config.node_stroke_width_config_override,
            node_corner_radius_config_override: render_config.node_corner_radius_config_override,
            node_label_fill_config_override: render_config.node_label_fill_config_override,
            node_typography_config_ownership: render_config.node_typography_config_ownership,
            node_fill_color: &compatibility.main_bkg,
            node_border_color: &compatibility.node_border,
            node_stroke_width: compatibility.node_stroke_width,
            node_corner_radius: compatibility.node_corner_radius,
            look: compatibility.look,
        }
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
        source_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
        ordinal: Option<usize>,
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        inputs: &NodeSelectionInputs<'_>,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let style = FlowchartNodeThemeStyle::resolve(theme, ordinal, work)?;
        let source =
            flowchart_compile_node_styles(class_defs, source_styles.0, source_styles.1, &[]);
        let selection = PreparedNodeSelection::prepare(inputs, &source, &style);
        let rough_group_style = if inputs.look.is_hand_drawn() {
            prepare_hand_drawn_shape_group_style(source_styles.1)
        } else {
            String::new()
        };
        Ok(Self {
            source_text_style,
            style,
            source,
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
