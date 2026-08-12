//! Flowchart edge helpers (markers, class attr, marker-color resolution).

use indexmap::IndexMap;

pub(super) struct FlowchartEdgeMarkerColor {
    value: String,
    residual: Option<crate::diagram_theme::SourceStyleResidual>,
}

impl FlowchartEdgeMarkerColor {
    pub(super) fn value(&self) -> &str {
        &self.value
    }

    pub(super) fn residual(&self) -> Option<&crate::diagram_theme::SourceStyleResidual> {
        self.residual.as_ref()
    }
}

pub(super) fn flowchart_edge_marker_end_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<&'static str> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some("pointEnd"),
        Some("double_arrow_circle") => Some("circleEnd"),
        Some("double_arrow_cross") => Some("crossEnd"),
        Some("arrow_point") => Some("pointEnd"),
        Some("arrow_cross") => Some("crossEnd"),
        Some("arrow_circle") => Some("circleEnd"),
        Some("arrow_open") => None,
        _ => Some("pointEnd"),
    }
}

pub(super) fn flowchart_edge_marker_start_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<&'static str> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some("pointStart"),
        Some("double_arrow_circle") => Some("circleStart"),
        Some("double_arrow_cross") => Some("crossStart"),
        _ => None,
    }
}

pub(super) fn flowchart_resolve_edge_marker_color(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    default_edge_style: &[String],
    edge_style: &[String],
    owner_id: &str,
    hand_drawn: bool,
) -> Option<FlowchartEdgeMarkerColor> {
    let inline_declaration_ordinal_base = class_declaration_count(class_defs, classes);
    let mut inline_declaration_ordinal = inline_declaration_ordinal_base;
    for raw in default_edge_style.iter().chain(edge_style.iter()) {
        let declaration_count = crate::flowchart::flowchart_split_mermaid_style_decls(raw).count();
        let source = raw.trim_start();
        let Some(value) = source.strip_prefix("stroke:") else {
            inline_declaration_ordinal =
                inline_declaration_ordinal.saturating_add(declaration_count);
            continue;
        };
        if value.trim().is_empty() {
            inline_declaration_ordinal =
                inline_declaration_ordinal.saturating_add(declaration_count);
            continue;
        }
        let provenance = crate::diagram_theme::SourceStyleProvenance::inline(
            owner_id,
            crate::diagram_theme::SourceStyleChannel::Shape,
            inline_declaration_ordinal,
        );
        return Some(FlowchartEdgeMarkerColor {
            value: if hand_drawn { source } else { value }.to_string(),
            residual: marker_source_residual(source, provenance),
        });
    }

    if hand_drawn {
        return None;
    }

    let mut stroke = None;
    let mut declaration_ordinal = 0usize;

    for (assignment_ordinal, c) in classes.iter().enumerate() {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            let ordinal = declaration_ordinal;
            declaration_ordinal = declaration_ordinal
                .saturating_add(crate::flowchart::flowchart_split_mermaid_style_decls(d).count());
            let Some((k, v)) = super::parse_style_decl(d) else {
                continue;
            };
            if k == "stroke" {
                stroke = Some((c.as_str(), assignment_ordinal, ordinal, d.as_str(), v));
            }
        }
    }

    stroke.map(
        |(class_id, assignment_ordinal, declaration_ordinal, raw, value)| {
            let provenance = crate::diagram_theme::SourceStyleProvenance::assigned_class(
                owner_id,
                class_id,
                crate::diagram_theme::SourceStyleChannel::Shape,
                assignment_ordinal,
                declaration_ordinal,
            );
            FlowchartEdgeMarkerColor {
                value: value.to_string(),
                residual: marker_source_residual(raw, provenance),
            }
        },
    )
}

fn class_declaration_count(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
) -> usize {
    classes
        .iter()
        .filter_map(|class_id| class_defs.get(class_id))
        .flatten()
        .map(|style| crate::flowchart::flowchart_split_mermaid_style_decls(style).count())
        .sum()
}

fn marker_source_residual(
    raw: &str,
    provenance: crate::diagram_theme::SourceStyleProvenance,
) -> Option<crate::diagram_theme::SourceStyleResidual> {
    match crate::diagram_theme::SourceStyleDeclaration::parse(raw, provenance) {
        Ok(declaration) => super::style::emitted_shape_source_residual_reason(&declaration, true)
            .map(|reason| {
                crate::diagram_theme::SourceStyleResidual::from_declaration(&declaration, reason)
            }),
        Err(residual) => Some(residual),
    }
}
