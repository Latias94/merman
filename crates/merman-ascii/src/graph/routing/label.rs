use super::super::layout::CanvasCoord;
use super::super::model::{AsciiGraphEdge, GraphEdgeStroke};
use crate::canvas::{Canvas, CanvasColor};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    LabelBreakPolicy, NormalizedLabelPlan, SafeLine, try_plan_normalized_label_lines_with_policy,
};
use crate::text::display_width_with_profile;
#[cfg(test)]
use crate::text::{normalize_optional_text, split_label_lines};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel<'a> {
    pub(super) text: &'a RoutedLabelText,
    pub(super) placement: RoutedLabelPlacement,
    pub(super) color: CanvasColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph) struct RoutedLabelDescriptor {
    catalog_index: usize,
    width: usize,
    line_count: usize,
}

#[derive(Debug)]
pub(in crate::graph) struct RoutedLabelCatalogPlan<'a> {
    labels: Vec<Option<RoutedLabelPlan<'a>>>,
}

#[derive(Debug)]
pub(in crate::graph) struct RoutedLabelCatalog {
    labels: Vec<Option<RoutedLabelText>>,
}

#[derive(Debug, Clone, Copy)]
struct RoutedLabelPlan<'a> {
    raw: &'a str,
    normalized: NormalizedLabelPlan,
    descriptor: RoutedLabelDescriptor,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::graph) struct RoutedLabelText {
    lines: Vec<String>,
    width: usize,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph) struct RoutedLabelPlacement {
    x: usize,
    y: usize,
    width: usize,
}

impl RoutedLabelPlacement {
    pub(in crate::graph) fn new(x: usize, y: usize, width: usize) -> Self {
        Self { x, y, width }
    }

    #[cfg(test)]
    pub(in crate::graph) fn canvas_extent(self) -> (usize, usize) {
        self.canvas_extent_for_lines(1)
    }

    pub(in crate::graph) fn canvas_extent_for_lines(self, line_count: usize) -> (usize, usize) {
        (self.x + self.width, self.y + line_count.max(1))
    }

    pub(in crate::graph) fn x(self) -> usize {
        self.x
    }

    pub(in crate::graph) fn y(self) -> usize {
        self.y
    }

    pub(in crate::graph) fn width(self) -> usize {
        self.width
    }

    pub(in crate::graph) fn with_position(self, x: usize, y: usize) -> Self {
        Self { x, y, ..self }
    }
}

impl RoutedLabelDescriptor {
    pub(in crate::graph) const fn width(self) -> usize {
        self.width
    }

    pub(in crate::graph) const fn line_count(self) -> usize {
        self.line_count
    }

    const fn catalog_index(self) -> usize {
        self.catalog_index
    }

    #[cfg(test)]
    pub(in crate::graph) fn for_test(
        catalog_index: usize,
        raw: &str,
        width_profile: TerminalWidthProfile,
    ) -> Option<Self> {
        let text = RoutedLabelText::new_with_profile(raw, width_profile)?;
        Some(Self {
            catalog_index,
            width: text.width(),
            line_count: text.line_count(),
        })
    }
}

impl From<RoutedLabelText> for RoutedLabelDescriptor {
    fn from(text: RoutedLabelText) -> Self {
        Self {
            catalog_index: 0,
            width: text.width,
            line_count: text.lines.len(),
        }
    }
}

impl<'a> RoutedLabelCatalogPlan<'a> {
    pub(in crate::graph) fn try_new(
        edges: &'a [AsciiGraphEdge],
        canonical_source_indices: &[usize],
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            Self::try_new_transactional(edges, canonical_source_indices, width_profile, resources)
        })
    }

    fn try_new_transactional(
        edges: &'a [AsciiGraphEdge],
        canonical_source_indices: &[usize],
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.charge_layout_work(canonical_source_indices.len())?;
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(canonical_source_indices.len())
            .map_err(|_| label_allocation_failed())?;
        for (catalog_index, source_index) in canonical_source_indices.iter().copied().enumerate() {
            let edge = edges.get(source_index).ok_or_else(label_catalog_mismatch)?;
            let plan = if edge.stroke == GraphEdgeStroke::Invisible {
                None
            } else {
                edge.label
                    .as_deref()
                    .map(|raw| {
                        RoutedLabelPlan::try_new(catalog_index, raw, width_profile, resources)
                    })
                    .transpose()?
                    .flatten()
            };
            labels.push(plan);
        }
        Ok(Self { labels })
    }

    pub(in crate::graph) fn descriptor(
        &self,
        catalog_index: usize,
    ) -> Option<RoutedLabelDescriptor> {
        self.labels
            .get(catalog_index)
            .and_then(Option::as_ref)
            .map(|plan| plan.descriptor)
    }

    pub(in crate::graph) fn materialize(
        self,
        resources: &ResourceContext,
    ) -> Result<RoutedLabelCatalog> {
        self.materialize_with_callback(resources, || {})
    }

    #[cfg(test)]
    fn materialize_with_probe(
        self,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<RoutedLabelCatalog> {
        self.materialize_with_callback(resources, || materialized.set(true))
    }

    fn materialize_with_callback(
        self,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<RoutedLabelCatalog> {
        resources.transaction(|resources| {
            self.materialize_with_callback_transactional(resources, before_materialize)
        })
    }

    fn materialize_with_callback_transactional(
        self,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<RoutedLabelCatalog> {
        let mut work_units = self.labels.len();
        let mut document_cells = 0usize;
        let mut materialized_bytes = 0usize;
        for plan in self.labels.iter().flatten() {
            work_units = resources
                .checked_work_add(work_units, plan.normalized.materialization_work_units())?;
            document_cells = resources
                .checked_work_add(document_cells, plan.normalized.metrics().document_cells)?;
            materialized_bytes = resources.checked_work_add(
                materialized_bytes,
                plan.normalized.metrics().materialized_bytes,
            )?;
        }
        resources.check_usage(work_units, document_cells)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, materialized_bytes)?;
        resources.charge_usage(work_units, document_cells)?;
        before_materialize();
        self.materialize_after_admission()
    }

    fn materialize_after_admission(self) -> Result<RoutedLabelCatalog> {
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(self.labels.len())
            .map_err(|_| label_allocation_failed())?;
        for plan in self.labels {
            labels.push(plan.map(RoutedLabelPlan::materialize).transpose()?);
        }
        Ok(RoutedLabelCatalog { labels })
    }
}

impl RoutedLabelCatalog {
    pub(in crate::graph) fn get(
        &self,
        descriptor: RoutedLabelDescriptor,
    ) -> Result<&RoutedLabelText> {
        self.labels
            .get(descriptor.catalog_index())
            .and_then(Option::as_ref)
            .ok_or_else(label_catalog_mismatch)
    }

    #[cfg(test)]
    pub(in crate::graph) fn for_test(texts: Vec<Option<RoutedLabelText>>) -> Self {
        Self { labels: texts }
    }
}

impl<'a> RoutedLabelPlan<'a> {
    fn try_new(
        catalog_index: usize,
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let Some(normalized) = try_plan_normalized_label_lines_with_policy(
            raw,
            width_profile,
            true,
            None,
            LabelBreakPolicy::MermaidLabelBreaks,
            resources,
        )?
        else {
            return Ok(None);
        };
        let metrics = normalized.metrics();
        if metrics.max_width == 0 {
            return Ok(None);
        }
        Ok(Some(Self {
            raw,
            normalized,
            descriptor: RoutedLabelDescriptor {
                catalog_index,
                width: metrics.max_width,
                line_count: metrics.line_count,
            },
            width_profile,
        }))
    }

    fn materialize(self) -> Result<RoutedLabelText> {
        let (lines, width) = self
            .normalized
            .materialize_after_admission(self.raw)?
            .into_parts();
        if width != self.descriptor.width || lines.len() != self.descriptor.line_count {
            return Err(label_catalog_mismatch());
        }
        Ok(RoutedLabelText {
            lines,
            width,
            width_profile: self.width_profile,
        })
    }
}

impl RoutedLabelText {
    #[cfg(test)]
    pub(super) fn new(raw: &str) -> Option<Self> {
        Self::new_with_profile(raw, TerminalWidthProfile::Unicode)
    }

    #[cfg(test)]
    pub(super) fn new_with_profile(raw: &str, width_profile: TerminalWidthProfile) -> Option<Self> {
        let normalized = normalize_optional_text(Some(raw))?;
        let lines = split_label_lines(&normalized);
        let width = lines
            .iter()
            .map(|line| display_width_with_profile(line, width_profile))
            .max()
            .unwrap_or_default();
        if width == 0 {
            return None;
        }

        Some(Self {
            lines,
            width,
            width_profile,
        })
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    fn line_width(&self, line: &str) -> usize {
        display_width_with_profile(line, self.width_profile)
    }

    pub(super) fn line_count(&self) -> usize {
        self.lines.len()
    }
}

pub(crate) fn draw_routed_label(canvas: &mut Canvas, label: &EdgeLabel<'_>) -> Result<()> {
    for (line_index, line) in label.text.lines().iter().enumerate() {
        let line_width = label.text.line_width(line);
        let x = label
            .placement
            .x
            .saturating_add(label.text.width().saturating_sub(line_width) / 2);
        write_label_overlay(
            canvas,
            x,
            label.placement.y + line_index,
            line,
            label.text.width_profile,
            label.color,
        )?;
    }
    Ok(())
}

fn label_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn label_catalog_mismatch() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "graph",
        feature: "route label catalog mismatch",
    }
}

#[cfg(test)]
pub(super) fn routed_label_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    let text = RoutedLabelText::new(text)?;
    routed_label_placement_for_text(start, end, &text)
}

#[cfg(test)]
pub(super) fn routed_label_placement_for_text(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &RoutedLabelText,
) -> Option<RoutedLabelPlacement> {
    routed_label_placement_for_descriptor(
        start,
        end,
        RoutedLabelDescriptor {
            catalog_index: 0,
            width: text.width(),
            line_count: text.line_count(),
        },
    )
}

pub(super) fn routed_label_placement_for_descriptor(
    start: CanvasCoord,
    end: CanvasCoord,
    descriptor: RoutedLabelDescriptor,
) -> Option<RoutedLabelPlacement> {
    if start.y == end.y {
        let x = horizontal_label_x(start, end, descriptor.width());
        let y = label_block_y(start.y, descriptor.line_count());
        return Some(RoutedLabelPlacement::new(x, y, descriptor.width()));
    }

    let x = start.x.saturating_sub(descriptor.width() / 2);
    let y = label_block_y(vertical_label_y(start, end), descriptor.line_count());
    Some(RoutedLabelPlacement::new(x, y, descriptor.width()))
}

#[cfg(test)]
pub(super) fn routed_label_right_of_vertical_route_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    let text = RoutedLabelText::new(text)?;
    routed_label_right_of_vertical_route_placement_for_text(start, end, &text)
}

#[cfg(test)]
pub(super) fn routed_label_right_of_vertical_route_placement_for_text(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &RoutedLabelText,
) -> Option<RoutedLabelPlacement> {
    routed_label_right_of_vertical_route_placement_for_descriptor(
        start,
        end,
        RoutedLabelDescriptor {
            catalog_index: 0,
            width: text.width(),
            line_count: text.line_count(),
        },
    )
}

pub(super) fn routed_label_right_of_vertical_route_placement_for_descriptor(
    start: CanvasCoord,
    end: CanvasCoord,
    descriptor: RoutedLabelDescriptor,
) -> Option<RoutedLabelPlacement> {
    if start.x != end.x {
        return None;
    }

    Some(RoutedLabelPlacement::new(
        start.x + 1,
        label_block_y(vertical_label_y(start, end), descriptor.line_count()),
        descriptor.width(),
    ))
}

fn horizontal_label_x(start: CanvasCoord, end: CanvasCoord, width: usize) -> usize {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let middle_x = min_x + (max_x - min_x) / 2;
    middle_x.saturating_sub(width / 2)
}

fn vertical_label_y(start: CanvasCoord, end: CanvasCoord) -> usize {
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);
    min_y + (max_y - min_y) / 2
}

fn label_block_y(center_y: usize, line_count: usize) -> usize {
    center_y.saturating_sub(line_count.saturating_sub(1) / 2)
}

fn write_label_overlay(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &str,
    width_profile: TerminalWidthProfile,
    color: CanvasColor,
) -> Result<()> {
    let mut offset = 0;
    let label = SafeLine::new(label);
    for grapheme in label.graphemes(width_profile) {
        if grapheme.text() != " " {
            match color {
                CanvasColor::Role(role) => {
                    canvas.write_text_role(x + offset, y, grapheme.text(), role)?
                }
                CanvasColor::Direct(color) => {
                    canvas.write_text_color(x + offset, y, grapheme.text(), color)?
                }
            }
        }
        offset += grapheme.width();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::{GraphEdgeMarker, GraphEdgeStroke, GraphEdgeStyle};
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
    use crate::terminal::TerminalCellText;
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    fn edge(label: &str, from: &str, to: &str) -> AsciiGraphEdge {
        AsciiGraphEdge {
            id: None,
            is_user_defined_id: false,
            from: from.to_string(),
            to: to.to_string(),
            label: Some(label.to_string()),
            stroke: GraphEdgeStroke::Normal,
            start_marker: GraphEdgeMarker::Open,
            end_marker: GraphEdgeMarker::Point,
            length: 1,
            style: GraphEdgeStyle::default(),
        }
    }

    #[test]
    fn routed_label_placement_centers_horizontal_route_labels() {
        let start = CanvasCoord { x: 4, y: 5 };
        let end = CanvasCoord { x: 12, y: 5 };

        assert_eq!(
            routed_label_placement(start, end, "flow"),
            Some(RoutedLabelPlacement::new(6, 5, 4))
        );
    }

    #[test]
    fn routed_label_placement_centers_vertical_route_labels() {
        let start = CanvasCoord { x: 10, y: 1 };
        let end = CanvasCoord { x: 10, y: 7 };

        assert_eq!(
            routed_label_placement(start, end, "back"),
            Some(RoutedLabelPlacement::new(8, 4, 4))
        );
    }

    #[test]
    fn routed_label_placement_accounts_for_multiline_labels() {
        let start = CanvasCoord { x: 4, y: 5 };
        let end = CanvasCoord { x: 12, y: 5 };

        assert_eq!(
            routed_label_placement(start, end, "north<br>south"),
            Some(RoutedLabelPlacement::new(6, 5, 5))
        );
        assert_eq!(
            routed_label_right_of_vertical_route_placement(start, end, "north<br>south"),
            None
        );
    }

    #[test]
    fn routed_label_right_of_vertical_route_requires_vertical_route() {
        let start = CanvasCoord { x: 10, y: 1 };
        let end = CanvasCoord { x: 10, y: 7 };

        assert_eq!(
            routed_label_right_of_vertical_route_placement(start, end, "back"),
            Some(RoutedLabelPlacement::new(11, 4, 4))
        );
        assert_eq!(
            routed_label_right_of_vertical_route_placement(
                CanvasCoord { x: 1, y: 1 },
                CanvasCoord { x: 4, y: 1 },
                "bad",
            ),
            None
        );
    }

    #[test]
    fn routed_label_overlay_preserves_complete_grapheme_cells() {
        let text = RoutedLabelText::new_with_profile(
            "e\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}",
            TerminalWidthProfile::Unicode,
        )
        .expect("label should exist");
        let mut canvas = Canvas::with_width_profile(7, 1, TerminalWidthProfile::Unicode);
        for x in 0..7 {
            canvas.set(x, 0, '-');
        }

        draw_routed_label(
            &mut canvas,
            &EdgeLabel {
                placement: RoutedLabelPlacement::new(0, 0, text.width()),
                text: &text,
                color: CanvasColor::Role(crate::color::AsciiColorRole::EdgeLabel),
            },
        )
        .expect("test routed label should fit the unbounded resource policy");

        assert_eq!(
            canvas.get_text(0, 0),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
        assert_eq!(
            canvas.get_text(2, 0),
            Some(TerminalCellText::Grapheme("\u{1f469}\u{200d}\u{1f4bb}"))
        );
        assert_eq!(
            canvas.get_text(5, 0),
            Some(TerminalCellText::Grapheme("\u{1f1fa}\u{1f1f8}"))
        );
    }

    #[test]
    fn routed_label_text_uses_selected_ambiguous_width_profile() {
        let unicode = RoutedLabelText::new_with_profile("A·B", TerminalWidthProfile::Unicode)
            .expect("Unicode label should exist");
        let cjk = RoutedLabelText::new_with_profile("A·B", TerminalWidthProfile::Cjk)
            .expect("CJK label should exist");

        assert_eq!(unicode.width(), 3);
        assert_eq!(cjk.width(), 4);
    }

    #[test]
    fn routed_label_catalog_admits_exact_materialization_and_rejects_max_minus_one() {
        let edges = vec![
            edge("north<br>south", "a", "b"),
            edge("Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb}", "b", "c"),
        ];
        let indices = [0, 1];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let measured_resources = ResourceContext::new(unbounded);
        let measured_plan = RoutedLabelCatalogPlan::try_new(
            &edges,
            &indices,
            TerminalWidthProfile::Unicode,
            &measured_resources,
        )
        .expect("catalog planning should succeed");
        let exact_output_bytes = measured_plan
            .labels
            .iter()
            .flatten()
            .map(|plan| plan.normalized.metrics().materialized_bytes)
            .sum::<usize>();
        let measured_probe = Cell::new(false);
        measured_plan
            .materialize_with_probe(&measured_resources, &measured_probe)
            .expect("unbounded catalog materialization should succeed");
        assert!(measured_probe.get());
        let exact_work = measured_resources.layout_work_used();
        let exact_document_cells = measured_resources.document_cells_used();
        assert!(exact_work > 1);
        assert!(exact_document_cells > 1);
        assert!(exact_output_bytes > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact work limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_document_cells)
            .expect("exact document limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes)
            .expect("exact output limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_plan = RoutedLabelCatalogPlan::try_new(
            &edges,
            &indices,
            TerminalWidthProfile::Unicode,
            &exact_resources,
        )
        .expect("exact catalog planning should succeed");
        let exact_probe = Cell::new(false);
        exact_plan
            .materialize_with_probe(&exact_resources, &exact_probe)
            .expect("exact catalog materialization should succeed");
        assert!(exact_probe.get());

        let below_work_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one work limit should be valid");
        let below_work_resources = ResourceContext::new(below_work_policy);
        let below_work_plan = RoutedLabelCatalogPlan::try_new(
            &edges,
            &indices,
            TerminalWidthProfile::Unicode,
            &below_work_resources,
        )
        .expect("planning should fit before the final work debit");
        let work_before = below_work_resources.layout_work_used();
        let document_cells_before = below_work_resources.document_cells_used();
        let below_work_probe = Cell::new(false);
        let error = below_work_plan
            .materialize_with_probe(&below_work_resources, &below_work_probe)
            .expect_err("max-minus-one work limit should reject before materialization");
        assert!(!below_work_probe.get());
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
        ));
        assert_eq!(below_work_resources.layout_work_used(), work_before);
        assert_eq!(
            below_work_resources.document_cells_used(),
            document_cells_before
        );

        let below_document_policy = unbounded
            .with_limit(
                AsciiResourceLimitId::MaxDocumentCells,
                exact_document_cells - 1,
            )
            .expect("max-minus-one document limit should be valid");
        let below_document_resources = ResourceContext::new(below_document_policy);
        let below_document_plan = RoutedLabelCatalogPlan::try_new(
            &edges,
            &indices,
            TerminalWidthProfile::Unicode,
            &below_document_resources,
        )
        .expect("document planning should be non-materializing");
        let work_before = below_document_resources.layout_work_used();
        let document_cells_before = below_document_resources.document_cells_used();
        let below_document_probe = Cell::new(false);
        let error = below_document_plan
            .materialize_with_probe(&below_document_resources, &below_document_probe)
            .expect_err("max-minus-one document limit should reject before materialization");
        assert!(!below_document_probe.get());
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxDocumentCells
        ));
        assert_eq!(below_document_resources.layout_work_used(), work_before);
        assert_eq!(
            below_document_resources.document_cells_used(),
            document_cells_before
        );

        let below_output_policy = exact_policy
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes - 1)
            .expect("max-minus-one output limit should be valid");
        let below_output_resources = ResourceContext::new(below_output_policy);
        let below_output_plan = RoutedLabelCatalogPlan::try_new(
            &edges,
            &indices,
            TerminalWidthProfile::Unicode,
            &below_output_resources,
        )
        .expect("output planning should be non-materializing");
        let work_before = below_output_resources.layout_work_used();
        let document_cells_before = below_output_resources.document_cells_used();
        let below_output_probe = Cell::new(false);
        let error = below_output_plan
            .materialize_with_probe(&below_output_resources, &below_output_probe)
            .expect_err("max-minus-one output limit should reject before materialization");
        assert!(!below_output_probe.get());
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
        ));
        assert_eq!(below_output_resources.layout_work_used(), work_before);
        assert_eq!(
            below_output_resources.document_cells_used(),
            document_cells_before
        );
    }
}
