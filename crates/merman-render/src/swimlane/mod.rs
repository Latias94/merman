mod bounds;
mod config;
mod direction;
mod geometry;
mod prepare;
mod routing;
mod sugiyama;
mod working;

use crate::Result;
use crate::flowchart::FlowchartConfigView;
use crate::math::MathRenderer;
use crate::model::{
    Bounds, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneLayout, SwimlaneNodeLayout,
};
use crate::text::TextMeasurer;
use icu_collator::{Collator, options::CollatorOptions};
use icu_locale_core::Locale;
use merman_core::MermaidConfig;
use merman_core::diagrams::flowchart::FlowchartModel;
use std::cmp::Ordering;
use std::sync::OnceLock;

/// Locale used by the pinned Mermaid/Puppeteer evidence and by the browser's default `localeCompare`.
pub(crate) const MERMAID_LAYOUT_COLLATION_LOCALE: &str = "en-US";

/// Compare identifiers with the same Unicode collation semantics as Mermaid's JavaScript layout.
///
/// Mermaid calls `String.prototype.localeCompare` in the Swimlane ordering passes. Rust's byte or
/// scalar-value ordering is observably different for case, accents, and supplementary characters,
/// so use a baked ICU4X collator with the attested `en-US` locale. The singleton keeps the data
/// tables out of the hot comparator path and makes the result independent of the host process
/// locale.
pub(crate) fn mermaid_identifier_locale_cmp(left: &str, right: &str) -> Ordering {
    static COLLATOR: OnceLock<icu_collator::CollatorBorrowed<'static>> = OnceLock::new();
    let collator = COLLATOR.get_or_init(|| {
        let locale = MERMAID_LAYOUT_COLLATION_LOCALE
            .parse::<Locale>()
            .expect("the pinned Mermaid collation locale must parse");
        Collator::try_new(locale.into(), CollatorOptions::default())
            .expect("compiled ICU4X collation data must contain en-US")
    });
    collator.compare(left, right)
}

fn output_bounds(layout: &working::WorkingLayout) -> Option<Bounds> {
    let mut points = Vec::new();
    for node in layout
        .nodes
        .values()
        .filter(|node| node.kind != working::WorkingNodeKind::Dummy)
    {
        let width = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_width
        } else {
            node.width
        };
        let height = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_height
        } else {
            node.height
        };
        points.push((node.x - width / 2.0, node.y - height / 2.0));
        points.push((node.x + width / 2.0, node.y + height / 2.0));
    }
    for edge in &layout.original_edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
    }
    Bounds::from_points(points)
}

pub fn layout_swimlane_typed(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SwimlaneLayout> {
    let config = config::SwimlaneConfig::from_config(effective_config);
    let mut working = prepare::prepare(model, effective_config, measurer, math_renderer);
    let reversed = sugiyama::run(&mut working, config);
    for edge in &mut working.original_edges {
        edge.reversed_for_layout = reversed.contains(&edge.id);
    }
    bounds::assign_canonical_group_bounds(&mut working);
    routing::route(&mut working);
    direction::post_process(&mut working);

    // Mermaid's swimlane core only normalizes the implicit `basis` curve to
    // `rounded`; an explicit edge/default/config curve remains authoritative.
    // Resolve the same precedence here before the layout artifact is consumed
    // by the SVG renderer.
    let config_curve = FlowchartConfigView::new(effective_config.as_value()).render_curve();
    let default_curve = model
        .edge_defaults
        .as_ref()
        .and_then(|defaults| defaults.interpolate.as_deref())
        .filter(|curve| !curve.is_empty())
        .or(config_curve.as_deref())
        .unwrap_or("basis");
    let curve_by_id: std::collections::HashMap<&str, &str> = model
        .edges
        .iter()
        .map(|edge| {
            let curve = edge
                .interpolate
                .as_deref()
                .filter(|curve| !curve.is_empty())
                .unwrap_or(default_curve);
            (edge.id.as_str(), curve)
        })
        .collect();

    let bounds = output_bounds(&working);
    let nodes = working
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.kind,
                working::WorkingNodeKind::Content | working::WorkingNodeKind::EdgeLabel
            )
        })
        .map(|node| SwimlaneNodeLayout {
            id: node.id.clone(),
            label: node.label.clone(),
            label_type: node.label_type.clone(),
            shape: node.shape.clone(),
            parent_id: node.parent_id.clone(),
            top_lane_id: node.top_lane_id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            label_width: node.label_width,
            label_height: node.label_height,
            layer: node.layer,
            order: node.order,
            is_edge_label: node.kind == working::WorkingNodeKind::EdgeLabel,
        })
        .collect();
    let lanes = working
        .nodes
        .values()
        .filter(|node| node.kind == working::WorkingNodeKind::Group)
        .map(|lane| SwimlaneLaneLayout {
            id: lane.id.clone(),
            title: lane.label.clone(),
            parent_id: lane.parent_id.clone(),
            x: lane.x,
            y: lane.y,
            width: lane.width,
            height: lane.height,
            padding: lane.padding,
            title_label_width: lane.label_width,
            title_label_height: lane.label_height,
            content_top: lane.content_top,
            title_rect: lane.title_rect.clone(),
            requested_dir: lane.requested_dir.clone(),
        })
        .collect();
    let edges = working
        .original_edges
        .iter()
        .map(|edge| SwimlaneEdgeLayout {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            points: edge.points.clone(),
            label_node_id: edge.label_node_id.clone(),
            reversed_for_layout: edge.reversed_for_layout,
            curve: match curve_by_id
                .get(edge.id.as_str())
                .copied()
                .unwrap_or("basis")
            {
                "basis" => "rounded",
                curve => curve,
            }
            .to_string(),
        })
        .collect();

    Ok(SwimlaneLayout {
        direction: working.direction,
        nodes,
        lanes,
        edges,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::mermaid_identifier_locale_cmp;

    #[test]
    fn identifier_order_matches_attested_mermaid_en_us_locale_compare() {
        // Expected order was generated in the pinned Puppeteer/Chromium 131
        // artifact with `ids.sort((a, b) => a.localeCompare(b, 'en-US'))`.
        let mut ids = [
            "B", "a", "A", "b", "a.1", "A.1", "a-1", "A-1", "a_1", "A_1", "A10", "a2", "Z", "z",
            "ä", "å", "Å", "é", "e", "E", "ß", "ss", "中", "阿", "😀", "🧪",
        ];
        ids.sort_by(|left, right| mermaid_identifier_locale_cmp(left, right));
        assert_eq!(
            ids,
            [
                "🧪", "😀", "a", "A", "å", "Å", "ä", "a_1", "A_1", "a-1", "A-1", "a.1", "A.1",
                "A10", "a2", "b", "B", "e", "E", "é", "ss", "ß", "z", "Z", "中", "阿",
            ]
        );
    }
}
