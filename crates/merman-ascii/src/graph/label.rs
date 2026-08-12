use super::model::AsciiGraphNode;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    LabelBreakPolicy, NormalizedLabelPlan, try_measure_normalized_label_lines,
    try_plan_normalized_label_lines_with_policy,
};
use crate::text::display_width_with_profile;
#[cfg(test)]
use crate::text::{split_label_lines, wrap_label_lines_with_profile};

pub(super) const GRAPH_LABEL_LINE_GAP: usize = 1;
const GENERIC_GRAPH_DIAGRAM_TYPE: &str = "graph";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphLabelMetrics {
    pub(super) width: usize,
    pub(super) content_height: usize,
}

#[derive(Debug)]
pub(super) struct GraphNodeLabelPlan {
    kind: GraphNodeLabelPlanKind,
    primary: NormalizedLabelPlan,
    secondary: Option<NormalizedLabelPlan>,
    title_line_count: usize,
    metrics: GraphLabelMetrics,
    document_cells: usize,
    materialized_bytes: usize,
    width_profile: TerminalWidthProfile,
    diagram_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeLabelPlanKind {
    Single,
    Compartmented,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLabel {
    lines: Vec<String>,
    width: usize,
    width_profile: TerminalWidthProfile,
    compartment_break_after: Option<usize>,
}

impl GraphLabel {
    #[cfg(test)]
    pub(super) fn new(raw: &str) -> Self {
        let resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        ));
        Self::try_new_with_profile(raw, TerminalWidthProfile::Unicode, &resources)
            .expect("test graph labels should fit the unbounded policy")
    }

    #[cfg(test)]
    pub(super) fn new_with_profile(raw: &str, width_profile: TerminalWidthProfile) -> Self {
        Self::from_lines(split_label_lines(raw), width_profile, None)
    }

    pub(super) fn empty_with_profile(width_profile: TerminalWidthProfile) -> Self {
        Self::from_lines(vec![String::new()], width_profile, None)
    }

    pub(super) const fn unmaterialized_with_profile(width_profile: TerminalWidthProfile) -> Self {
        // Layout geometry is admitted before label rows are materialized. This placeholder must
        // not escape that phase; `materialize_node_labels` replaces it before rendering.
        Self {
            lines: Vec::new(),
            width: 0,
            width_profile,
            compartment_break_after: None,
        }
    }

    pub(super) fn try_new_with_profile(
        raw: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        Self::try_single_with_profile(raw, None, width_profile, resources, || {})
    }

    #[cfg(test)]
    pub(super) fn compartmented_with_profile(
        title: &str,
        body: &str,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let mut title_lines = split_label_lines(title);
        let title_line_count = title_lines.len().max(1);
        let body_lines = split_label_lines(body);
        title_lines.extend(body_lines);
        Self::from_lines(title_lines, width_profile, Some(title_line_count))
    }

    #[cfg(test)]
    pub(super) fn wrapped_with_profile(
        raw: &str,
        max_width: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self::from_lines(
            wrap_label_lines_with_profile(raw, max_width, width_profile),
            width_profile,
            None,
        )
    }

    pub(super) fn try_wrapped_with_profile(
        raw: &str,
        max_width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        Self::try_single_with_profile(raw, Some(max_width), width_profile, resources, || {})
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    #[cfg(test)]
    pub(super) fn width(&self) -> usize {
        self.width
    }

    pub(super) fn line_width(&self, line: &str) -> usize {
        display_width_with_profile(line, self.width_profile)
    }

    pub(super) fn content_height(&self) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        self.lines.len() + (self.lines.len() - 1) * GRAPH_LABEL_LINE_GAP
    }

    pub(super) fn compartment_break_after(&self) -> Option<usize> {
        self.compartment_break_after
    }

    pub(super) fn try_measure_with_profile(
        raw: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<GraphLabelMetrics> {
        let metrics = try_measure_normalized_label_lines(raw, width_profile, false, resources)?
            .ok_or(crate::error::AsciiError::UnsupportedFeature {
                diagram_type: GENERIC_GRAPH_DIAGRAM_TYPE,
                feature: "empty graph label metrics",
            })?;
        graph_label_metrics(metrics.max_width, metrics.line_count, resources)
    }

    pub(super) fn try_measure_width_with_profile(
        raw: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<usize> {
        Ok(
            try_measure_normalized_label_lines(raw, width_profile, false, resources)?
                .map(|metrics| metrics.max_width)
                .unwrap_or_default(),
        )
    }

    pub(super) fn try_measure_wrapped_with_profile(
        raw: &str,
        max_width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<GraphLabelMetrics> {
        let metrics = required_label_plan(
            raw,
            Some(max_width),
            GENERIC_GRAPH_DIAGRAM_TYPE,
            width_profile,
            resources,
        )?
        .metrics();
        graph_label_metrics(metrics.max_width, metrics.line_count, resources)
    }

    fn from_lines(
        mut lines: Vec<String>,
        width_profile: TerminalWidthProfile,
        compartment_break_after: Option<usize>,
    ) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        let width = lines
            .iter()
            .map(|line| display_width_with_profile(line, width_profile))
            .max()
            .unwrap_or_default();
        Self {
            lines,
            width,
            width_profile,
            compartment_break_after,
        }
    }

    fn try_single_with_profile(
        raw: &str,
        wrap_width: Option<usize>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> crate::Result<Self> {
        let plan = required_label_plan(
            raw,
            wrap_width,
            GENERIC_GRAPH_DIAGRAM_TYPE,
            width_profile,
            resources,
        )?;
        let metrics = plan.metrics();
        resources.grid_extent(metrics.max_width.max(1), metrics.line_count)?;
        plan.check_materialization_limits(resources)?;
        before_materialize();
        let (lines, width) = plan.materialize(raw, resources)?.into_parts();
        Ok(Self {
            lines,
            width,
            width_profile,
            compartment_break_after: None,
        })
    }

    #[cfg(test)]
    fn try_wrapped_with_probe(
        raw: &str,
        max_width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> crate::Result<Self> {
        Self::try_single_with_profile(raw, Some(max_width), width_profile, resources, || {
            materialized.set(true)
        })
    }
}

impl GraphNodeLabelPlan {
    pub(super) fn try_for_node(
        node: &AsciiGraphNode,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let (
            kind,
            primary,
            secondary,
            title_line_count,
            width,
            line_count,
            document_cells,
            materialized_bytes,
        ) = match node.semantics.compartments.as_ref() {
            Some(compartments) => {
                let title = required_label_plan(
                    &compartments.title,
                    None,
                    diagram_type,
                    width_profile,
                    resources,
                )?;
                let body = required_label_plan(
                    &compartments.body,
                    None,
                    diagram_type,
                    width_profile,
                    resources,
                )?;
                let title_metrics = title.metrics();
                let body_metrics = body.metrics();
                let line_count = title_metrics
                    .line_count
                    .checked_add(body_metrics.line_count)
                    .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
                let materialized_bytes = title_metrics
                    .materialized_bytes
                    .checked_add(body_metrics.materialized_bytes)
                    .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
                let document_cells = title_metrics
                    .document_cells
                    .checked_add(body_metrics.document_cells)
                    .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
                (
                    GraphNodeLabelPlanKind::Compartmented,
                    title,
                    Some(body),
                    title_metrics.line_count,
                    title_metrics.max_width.max(body_metrics.max_width),
                    line_count,
                    document_cells,
                    materialized_bytes,
                )
            }
            None => {
                let plan = required_label_plan(
                    &node.label,
                    wrap_width,
                    diagram_type,
                    width_profile,
                    resources,
                )?;
                let metrics = plan.metrics();
                (
                    GraphNodeLabelPlanKind::Single,
                    plan,
                    None,
                    0,
                    metrics.max_width,
                    metrics.line_count,
                    metrics.document_cells,
                    metrics.materialized_bytes,
                )
            }
        };
        Ok(Self {
            kind,
            primary,
            secondary,
            title_line_count,
            metrics: graph_label_metrics(width, line_count, resources)?,
            document_cells,
            materialized_bytes,
            width_profile,
            diagram_type,
        })
    }

    pub(super) const fn metrics(&self) -> GraphLabelMetrics {
        self.metrics
    }

    pub(super) const fn document_cells(&self) -> usize {
        self.document_cells
    }

    pub(super) const fn materialized_bytes(&self) -> usize {
        self.materialized_bytes
    }

    pub(super) fn materialize(
        &self,
        node: &AsciiGraphNode,
        resources: &ResourceContext,
    ) -> crate::Result<GraphLabel> {
        self.materialize_with_callback(node, resources, || {})
    }

    #[cfg(test)]
    fn materialize_with_body_reserve_probe(
        &self,
        node: &AsciiGraphNode,
        resources: &ResourceContext,
        body_reserve_started: &std::cell::Cell<bool>,
    ) -> crate::Result<GraphLabel> {
        self.materialize_with_callback(node, resources, || body_reserve_started.set(true))
    }

    fn materialize_with_callback(
        &self,
        node: &AsciiGraphNode,
        resources: &ResourceContext,
        before_body_reserve: impl FnOnce(),
    ) -> crate::Result<GraphLabel> {
        match (self.kind, node.semantics.compartments.as_ref()) {
            (GraphNodeLabelPlanKind::Single, None) => {
                let (lines, width) = self
                    .primary
                    .materialize(&node.label, resources)?
                    .into_parts();
                Ok(GraphLabel {
                    lines,
                    width,
                    width_profile: self.width_profile,
                    compartment_break_after: None,
                })
            }
            (GraphNodeLabelPlanKind::Compartmented, Some(compartments)) => {
                let Some(body) = self.secondary else {
                    return Err(invalid_graph_label_plan(self.diagram_type));
                };
                self.admit_compartment_materialization(body, resources)?;
                let (mut lines, title_width) = self
                    .primary
                    .materialize_after_admission(&compartments.title)?
                    .into_parts();
                let body_metrics = body.metrics();
                before_body_reserve();
                lines
                    .try_reserve_exact(body_metrics.line_count)
                    .map_err(|_| label_allocation_failed())?;
                let (mut body_lines, body_width) = body
                    .materialize_after_admission(&compartments.body)?
                    .into_parts();
                lines.append(&mut body_lines);
                Ok(GraphLabel {
                    lines,
                    width: title_width.max(body_width),
                    width_profile: self.width_profile,
                    compartment_break_after: Some(self.title_line_count.max(1)),
                })
            }
            _ => Err(invalid_graph_label_plan(self.diagram_type)),
        }
    }

    fn admit_compartment_materialization(
        &self,
        body: NormalizedLabelPlan,
        resources: &ResourceContext,
    ) -> crate::Result<()> {
        let work_units = resources.checked_work_add(
            self.primary.materialization_work_units(),
            body.materialization_work_units(),
        )?;
        resources.charge_layout_work(work_units)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, self.document_cells)?;
        resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.materialized_bytes,
        )
    }
}

fn required_label_plan(
    raw: &str,
    wrap_width: Option<usize>,
    diagram_type: &'static str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> crate::Result<NormalizedLabelPlan> {
    try_plan_normalized_label_lines_with_policy(
        raw,
        width_profile,
        false,
        wrap_width,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
    )?
    .ok_or(crate::error::AsciiError::UnsupportedFeature {
        diagram_type,
        feature: "empty graph label plan",
    })
}

fn label_allocation_failed() -> crate::error::AsciiError {
    crate::error::AsciiError::allocation_failed(AsciiResourceLimitPhase::Document.as_str())
}

fn invalid_graph_label_plan(diagram_type: &'static str) -> crate::error::AsciiError {
    crate::error::AsciiError::UnsupportedFeature {
        diagram_type,
        feature: "invalid graph node label plan",
    }
}

fn graph_label_metrics(
    width: usize,
    line_count: usize,
    resources: &ResourceContext,
) -> crate::Result<GraphLabelMetrics> {
    let content_height = if line_count == 0 {
        0
    } else {
        let gaps = resources.checked_grid_mul(line_count - 1, GRAPH_LABEL_LINE_GAP)?;
        resources.checked_grid_add(line_count, gaps)?
    };
    Ok(GraphLabelMetrics {
        width,
        content_height,
    })
}

#[cfg(test)]
mod tests {
    use super::{GraphLabel, GraphNodeLabelPlan};
    use crate::graph::model::{
        AsciiGraphNode, GraphNodeCompartments, GraphNodeSemantics, GraphNodeShape, GraphNodeStyle,
    };
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
    use crate::{AsciiError, TerminalWidthProfile};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    #[test]
    fn graph_label_splits_html_breaks() {
        let label = GraphLabel::new("line1<br/>line2<br>line3<br />line4");

        assert_eq!(label.lines(), ["line1", "line2", "line3", "line4"]);
        assert_eq!(label.width(), 5);
        assert_eq!(label.content_height(), 7);
    }

    #[test]
    fn graph_label_splits_escaped_newlines() {
        let label = GraphLabel::new(r"line1\nline2");

        assert_eq!(label.lines(), ["line1", "line2"]);
        assert_eq!(label.width(), 5);
        assert_eq!(label.content_height(), 3);
    }

    #[test]
    fn graph_label_width_uses_display_width() {
        let label = GraphLabel::new("中A");

        assert_eq!(label.lines(), ["中A"]);
        assert_eq!(label.width(), 3);
        assert_eq!(label.content_height(), 1);
    }

    #[test]
    fn graph_label_wrapped_preserves_hard_breaks() {
        let label = GraphLabel::wrapped_with_profile(
            "Alpha Beta<br><br>Gamma Delta",
            6,
            TerminalWidthProfile::Unicode,
        );

        assert_eq!(label.lines(), ["Alpha", "Beta", "", "Gamma", "Delta"]);
        assert_eq!(label.width(), 5);
        assert_eq!(label.content_height(), 9);
    }

    #[test]
    fn wrapped_label_admits_exact_grid_before_materializing_rows() {
        const REQUIRED_CELLS: usize = 25;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, REQUIRED_CELLS)
            .expect("exact graph-label grid limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_materialized = Cell::new(false);

        let label = GraphLabel::try_wrapped_with_probe(
            "Alpha Beta<br><br>Gamma Delta",
            6,
            TerminalWidthProfile::Unicode,
            &exact_resources,
            &exact_materialized,
        )
        .expect("exact graph-label extent should permit materialization");

        assert_eq!(label.lines(), ["Alpha", "Beta", "", "Gamma", "Delta"]);
        assert!(exact_materialized.get());

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, REQUIRED_CELLS - 1)
            .expect("max-minus-one graph-label grid limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let below_materialized = Cell::new(false);
        let error = GraphLabel::try_wrapped_with_probe(
            "Alpha Beta<br><br>Gamma Delta",
            6,
            TerminalWidthProfile::Unicode,
            &below_resources,
            &below_materialized,
        )
        .expect_err("max-minus-one graph-label extent should reject before materialization");

        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == REQUIRED_CELLS
                    && details.max == REQUIRED_CELLS - 1
        ));
    }

    #[test]
    fn graph_label_uses_selected_ambiguous_width_profile() {
        let unicode = GraphLabel::new_with_profile("A·B", TerminalWidthProfile::Unicode);
        let cjk = GraphLabel::new_with_profile("A·B", TerminalWidthProfile::Cjk);

        assert_eq!(unicode.width(), 3);
        assert_eq!(cjk.width(), 4);
    }

    #[test]
    fn compartmented_label_admits_replay_document_and_output_before_reserving_body_rows() {
        let node = compartmented_node("Title<br>continued", "Body<br>detail");
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let measured_resources = ResourceContext::new(unbounded);
        let measured_plan = GraphNodeLabelPlan::try_for_node(
            &node,
            None,
            "state",
            TerminalWidthProfile::Unicode,
            &measured_resources,
        )
        .expect("compartmented label planning should succeed");
        let exact_document_cells = measured_plan.document_cells();
        let exact_output_bytes = measured_plan.materialized_bytes();
        let measured_reserve = Cell::new(false);
        measured_plan
            .materialize_with_body_reserve_probe(&node, &measured_resources, &measured_reserve)
            .expect("unbounded compartmented label materialization should succeed");
        assert!(measured_reserve.get());
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 1);
        assert!(exact_document_cells > 1);
        assert!(exact_output_bytes > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact compartment replay-work limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_document_cells)
            .expect("exact compartment document limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes)
            .expect("exact compartment output limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_plan = GraphNodeLabelPlan::try_for_node(
            &node,
            None,
            "state",
            TerminalWidthProfile::Unicode,
            &exact_resources,
        )
        .expect("exact compartmented label planning should succeed");
        let label = exact_plan
            .materialize(&node, &exact_resources)
            .expect("exact replay-work, document, and output limits should permit materialization");
        assert_eq!(label.lines(), ["Title", "continued", "Body", "detail"]);

        for (limit, actual) in [
            (AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work),
            (AsciiResourceLimitId::MaxDocumentCells, exact_document_cells),
            (AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes),
        ] {
            let below_policy = exact_policy
                .with_limit(limit, actual - 1)
                .expect("max-minus-one compartment limit should be valid");
            let below_resources = ResourceContext::new(below_policy);
            let below_plan = GraphNodeLabelPlan::try_for_node(
                &node,
                None,
                "state",
                TerminalWidthProfile::Unicode,
                &below_resources,
            )
            .expect("compartment planning should remain non-materializing");
            let body_reserve_started = Cell::new(false);
            let error = below_plan
                .materialize_with_body_reserve_probe(&node, &below_resources, &body_reserve_started)
                .expect_err("max-minus-one limit should reject before body allocation");

            assert!(!body_reserve_started.get(), "limit={limit:?}");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit
                        && details.actual == actual
                        && details.max == actual - 1
            ));
        }
    }

    #[test]
    fn compartmented_label_keeps_multiline_title_boundary() {
        let label = GraphLabel::compartmented_with_profile(
            "Title<br>continued",
            "Body<br>detail",
            TerminalWidthProfile::Unicode,
        );

        assert_eq!(label.lines(), ["Title", "continued", "Body", "detail"]);
        assert_eq!(label.compartment_break_after(), Some(2));
    }

    fn compartmented_node(title: &str, body: &str) -> AsciiGraphNode {
        AsciiGraphNode {
            id: "node".to_string(),
            label: title.to_string(),
            shape: GraphNodeShape::StateWithTitle,
            style: GraphNodeStyle::default(),
            semantics: GraphNodeSemantics {
                compartments: Some(GraphNodeCompartments::new(title, body)),
                ..GraphNodeSemantics::default()
            },
        }
    }
}
