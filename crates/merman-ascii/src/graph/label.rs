use super::model::{AsciiGraphNode, GraphNodeCompartments};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    LabelBreakPolicy, NormalizedLabelMetrics, NormalizedLabelPlan,
    try_measure_normalized_label_lines, try_plan_normalized_label_lines_with_policy,
};
use crate::text::display_width_with_profile;
#[cfg(test)]
use crate::text::{split_label_lines, wrap_label_lines_with_profile};
use std::ops::Deref;

pub(super) const GRAPH_LABEL_LINE_GAP: usize = 1;
const GENERIC_GRAPH_DIAGRAM_TYPE: &str = "graph";
const GRAPH_LABEL_CHECKPOINT_INTERVAL: usize = 64;
const GRAPH_LABEL_COPY_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphLabelMetrics {
    pub(super) width: usize,
    pub(super) content_height: usize,
}

#[derive(Debug, Clone)]
pub(super) struct GraphNodeLabelPlan {
    kind: GraphNodeLabelPlanKind,
    primary: GraphLabelSectionPlan,
    secondary: Option<GraphLabelSectionPlan>,
    title_line_count: usize,
    metrics: GraphLabelMetrics,
    document_cells: usize,
    materialized_bytes: usize,
    width_profile: TerminalWidthProfile,
    diagram_type: &'static str,
    wrap_width: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeLabelPlanKind {
    Single,
    Compartmented,
}

#[derive(Debug, Clone)]
struct GraphLabelSectionPlan {
    parts: GraphLabelSectionParts,
    metrics: NormalizedLabelMetrics,
}

#[derive(Debug, Clone)]
// Keeping the common single-part plan inline avoids an allocation during the aggregate
// admission pass. Joined sections allocate only when a family has already admitted their
// borrowed source batch.
#[allow(clippy::large_enum_variant)]
enum GraphLabelSectionParts {
    Single(NormalizedLabelPlan),
    Joined(Vec<GraphLabelPartPlan>),
}

#[derive(Debug, Clone, Copy)]
struct GraphLabelPartPlan {
    source_start: usize,
    source_end: usize,
    plan: NormalizedLabelPlan,
}

/// A borrowed node-label plan retained until the whole projection batch has passed admission.
///
/// Family adapters may assemble one logical label from multiple authored fields without cloning
/// or joining those fields first. The owned source and replay metadata are created only after the
/// caller admits the aggregate work, document-cell, and output-byte bounds.
#[derive(Debug)]
pub(crate) struct DeferredGraphNodeLabelPlan<'a> {
    kind: GraphNodeLabelPlanKind,
    primary: DeferredGraphLabelSectionPlan<'a>,
    secondary: Option<DeferredGraphLabelSectionPlan<'a>>,
    metrics: GraphLabelMetrics,
    document_cells: usize,
    materialized_bytes: usize,
    source_materialization_work_units: usize,
    width_profile: TerminalWidthProfile,
    diagram_type: &'static str,
    wrap_width: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct DeferredGraphLabelSectionPlan<'a> {
    parts: Vec<DeferredGraphLabelPartPlan<'a>>,
    metrics: NormalizedLabelMetrics,
    source_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
struct DeferredGraphLabelPartPlan<'a> {
    source: &'a str,
    plan: NormalizedLabelPlan,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGraphNodeText {
    label: String,
    compartments: Option<GraphNodeCompartments>,
    plans: Vec<GraphNodeLabelPlan>,
}

#[derive(Debug)]
// An owned plan is intentionally inline: the first aggregate planning pass must not allocate
// plan storage before document/output admission. Prepared family nodes use the borrowed variant.
#[allow(clippy::large_enum_variant)]
pub(super) enum GraphNodeLabelPlanHandle<'a> {
    Borrowed(&'a GraphNodeLabelPlan),
    Owned(GraphNodeLabelPlan),
}

impl Deref for GraphNodeLabelPlanHandle<'_> {
    type Target = GraphNodeLabelPlan;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(plan) => plan,
            Self::Owned(plan) => plan,
        }
    }
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
        resources.transaction(|resources| {
            Self::try_single_with_profile_transactional(
                raw,
                wrap_width,
                width_profile,
                resources,
                before_materialize,
            )
        })
    }

    fn try_single_with_profile_transactional(
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

impl GraphLabelSectionPlan {
    fn single(plan: NormalizedLabelPlan) -> Self {
        Self {
            metrics: plan.metrics(),
            parts: GraphLabelSectionParts::Single(plan),
        }
    }

    fn replay_work_units(&self, resources: &ResourceContext) -> crate::Result<usize> {
        match &self.parts {
            GraphLabelSectionParts::Single(plan) => Ok(plan.materialization_work_units()),
            GraphLabelSectionParts::Joined(parts) => {
                let mut work = 0usize;
                for (index, part) in parts.iter().enumerate() {
                    checkpoint_label_loop(resources, index)?;
                    work =
                        resources.checked_work_add(work, part.plan.materialization_work_units())?;
                }
                Ok(work)
            }
        }
    }

    fn materialize_after_admission(
        &self,
        source: &str,
        resources: &ResourceContext,
    ) -> crate::Result<(Vec<String>, usize)> {
        match &self.parts {
            GraphLabelSectionParts::Single(plan) => plan
                .materialize_after_admission_with_checkpoint(source, || resources.checkpoint())
                .map(|lines| lines.into_parts()),
            GraphLabelSectionParts::Joined(parts) => {
                let mut lines = Vec::new();
                lines
                    .try_reserve_exact(self.metrics.line_count)
                    .map_err(|_| label_allocation_failed())?;
                let mut width = 0usize;
                for part in parts.iter().copied() {
                    resources.checkpoint()?;
                    let raw = source
                        .get(part.source_start..part.source_end)
                        .ok_or_else(|| invalid_graph_label_plan(GENERIC_GRAPH_DIAGRAM_TYPE))?;
                    let (part_lines, part_width) = part
                        .plan
                        .materialize_after_admission_with_checkpoint(raw, || {
                            resources.checkpoint()
                        })?
                        .into_parts();
                    width = width.max(part_width);
                    for (line_index, line) in part_lines.into_iter().enumerate() {
                        checkpoint_label_loop(resources, line_index)?;
                        lines.push(line);
                    }
                }
                if lines.len() != self.metrics.line_count || width != self.metrics.max_width {
                    return Err(invalid_graph_label_plan(GENERIC_GRAPH_DIAGRAM_TYPE));
                }
                Ok((lines, width))
            }
        }
    }
}

impl<'a> DeferredGraphLabelSectionPlan<'a> {
    pub(crate) fn try_single(
        raw: &'a str,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let plan = required_label_plan(raw, wrap_width, diagram_type, width_profile, resources)?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(1)
            .map_err(|_| label_allocation_failed())?;
        parts.push(DeferredGraphLabelPartPlan { source: raw, plan });
        Self::from_parts(parts, resources)
    }

    pub(crate) fn try_joined(
        fragments: impl IntoIterator<Item = &'a str>,
        fallback: Option<&'a str>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Option<Self>> {
        let mut parts = Vec::new();
        for raw in fragments {
            resources.checkpoint()?;
            let Some(plan) = try_plan_normalized_label_lines_with_policy(
                raw,
                width_profile,
                true,
                None,
                LabelBreakPolicy::MermaidLabelBreaks,
                resources,
            )?
            else {
                continue;
            };
            parts
                .try_reserve(1)
                .map_err(|_| label_allocation_failed())?;
            parts.push(DeferredGraphLabelPartPlan { source: raw, plan });
        }
        if parts.is_empty() {
            let Some(fallback) = fallback else {
                return Ok(None);
            };
            return Self::try_single(fallback, None, diagram_type, width_profile, resources)
                .map(Some);
        }
        Self::from_parts(parts, resources).map(Some)
    }

    fn from_parts(
        parts: Vec<DeferredGraphLabelPartPlan<'a>>,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let mut metrics = empty_normalized_label_metrics();
        let mut source_bytes = 0usize;
        for (index, part) in parts.iter().enumerate() {
            checkpoint_label_loop(resources, index)?;
            let part_metrics = part.plan.metrics();
            metrics.materialized_bytes = checked_label_metric_add(
                resources,
                AsciiResourceLimitId::MaxOutputBytes,
                metrics.materialized_bytes,
                part_metrics.materialized_bytes,
            )?;
            metrics.document_cells = checked_label_metric_add(
                resources,
                AsciiResourceLimitId::MaxDocumentCells,
                metrics.document_cells,
                part_metrics.document_cells,
            )?;
            metrics.line_count = checked_label_metric_add(
                resources,
                AsciiResourceLimitId::MaxDocumentCells,
                metrics.line_count,
                part_metrics.line_count,
            )?;
            metrics.max_width = metrics.max_width.max(part_metrics.max_width);
            if index != 0 {
                source_bytes = resources.checked_work_add(source_bytes, 1)?;
            }
            source_bytes = resources.checked_work_add(source_bytes, part.source.len())?;
        }
        Ok(Self {
            parts,
            metrics,
            source_bytes,
        })
    }

    fn source_materialization_work_units(&self) -> usize {
        self.source_bytes.max(1)
    }

    pub(crate) const fn document_cells(&self) -> usize {
        self.metrics.document_cells
    }

    pub(crate) fn normalized_joined_bytes(
        &self,
        resources: &ResourceContext,
    ) -> crate::Result<usize> {
        self.metrics
            .materialized_bytes
            .checked_add(self.metrics.line_count.saturating_sub(1))
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
    }

    pub(crate) fn normalized_materialization_work_units(
        &self,
        resources: &ResourceContext,
    ) -> crate::Result<usize> {
        let mut replay_work = 0usize;
        for (index, part) in self.parts.iter().enumerate() {
            checkpoint_label_loop(resources, index)?;
            replay_work =
                resources.checked_work_add(replay_work, part.plan.materialization_work_units())?;
        }
        resources.checked_work_add(replay_work, self.normalized_joined_bytes(resources)?.max(1))
    }

    pub(crate) fn materialize_normalized_after_admission(
        self,
        resources: &ResourceContext,
    ) -> crate::Result<String> {
        let expected_bytes = self.normalized_joined_bytes(resources)?;
        resources.checkpoint()?;
        let mut output = String::new();
        output
            .try_reserve_exact(expected_bytes)
            .map_err(|_| label_allocation_failed())?;
        let mut first_line = true;
        for part in self.parts {
            resources.checkpoint()?;
            let (lines, _) = part
                .plan
                .materialize_after_admission_with_checkpoint(part.source, || {
                    resources.checkpoint()
                })?
                .into_parts();
            for (line_index, line) in lines.into_iter().enumerate() {
                checkpoint_label_loop(resources, line_index)?;
                if !first_line {
                    output.push('\n');
                }
                push_graph_label_source(&mut output, &line, resources)?;
                first_line = false;
            }
        }
        if output.len() != expected_bytes {
            return Err(invalid_graph_label_plan(GENERIC_GRAPH_DIAGRAM_TYPE));
        }
        Ok(output)
    }

    fn materialize_source(
        self,
        resources: &ResourceContext,
    ) -> crate::Result<(String, GraphLabelSectionPlan)> {
        resources.checkpoint()?;
        let mut source = String::new();
        source
            .try_reserve_exact(self.source_bytes)
            .map_err(|_| label_allocation_failed())?;
        let mut owned_parts = Vec::new();
        owned_parts
            .try_reserve_exact(self.parts.len())
            .map_err(|_| label_allocation_failed())?;
        for (index, part) in self.parts.into_iter().enumerate() {
            resources.checkpoint()?;
            if index != 0 {
                source.push('\n');
            }
            let source_start = source.len();
            push_graph_label_source(&mut source, part.source, resources)?;
            let source_end = source.len();
            owned_parts.push(GraphLabelPartPlan {
                source_start,
                source_end,
                plan: part.plan,
            });
        }
        if source.len() != self.source_bytes {
            return Err(invalid_graph_label_plan(GENERIC_GRAPH_DIAGRAM_TYPE));
        }
        let parts = match owned_parts.as_slice() {
            [part] if part.source_start == 0 && part.source_end == source.len() => {
                GraphLabelSectionParts::Single(part.plan)
            }
            _ => GraphLabelSectionParts::Joined(owned_parts),
        };
        Ok((
            source,
            GraphLabelSectionPlan {
                parts,
                metrics: self.metrics,
            },
        ))
    }
}

impl<'a> DeferredGraphNodeLabelPlan<'a> {
    pub(crate) fn single(
        primary: DeferredGraphLabelSectionPlan<'a>,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let metrics = primary.metrics;
        Ok(Self {
            kind: GraphNodeLabelPlanKind::Single,
            secondary: None,
            metrics: graph_label_metrics(metrics.max_width, metrics.line_count, resources)?,
            document_cells: metrics.document_cells,
            materialized_bytes: metrics.materialized_bytes,
            source_materialization_work_units: primary.source_materialization_work_units(),
            primary,
            width_profile,
            diagram_type,
            wrap_width,
        })
    }

    pub(crate) fn compartmented(
        primary: DeferredGraphLabelSectionPlan<'a>,
        secondary: DeferredGraphLabelSectionPlan<'a>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let line_count = checked_label_metric_add(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            primary.metrics.line_count,
            secondary.metrics.line_count,
        )?;
        let document_cells = checked_label_metric_add(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            primary.metrics.document_cells,
            secondary.metrics.document_cells,
        )?;
        let materialized_bytes = checked_label_metric_add(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            primary.metrics.materialized_bytes,
            secondary.metrics.materialized_bytes,
        )?;
        let source_materialization_work_units = resources.checked_work_add(
            primary.source_materialization_work_units(),
            secondary.source_materialization_work_units(),
        )?;
        Ok(Self {
            kind: GraphNodeLabelPlanKind::Compartmented,
            metrics: graph_label_metrics(
                primary.metrics.max_width.max(secondary.metrics.max_width),
                line_count,
                resources,
            )?,
            document_cells,
            materialized_bytes,
            source_materialization_work_units,
            primary,
            secondary: Some(secondary),
            width_profile,
            diagram_type,
            wrap_width: None,
        })
    }

    pub(crate) const fn document_cells(&self) -> usize {
        self.document_cells
    }

    pub(crate) const fn materialized_bytes(&self) -> usize {
        self.materialized_bytes
    }

    pub(crate) const fn source_materialization_work_units(&self) -> usize {
        self.source_materialization_work_units
    }

    pub(crate) fn materialize_after_admission(
        self,
        resources: &ResourceContext,
    ) -> crate::Result<PreparedGraphNodeText> {
        let title_line_count = if self.kind == GraphNodeLabelPlanKind::Compartmented {
            self.primary.metrics.line_count
        } else {
            0
        };
        let (primary_source, primary) = self.primary.materialize_source(resources)?;
        let (label, compartments, secondary) = match self.secondary {
            Some(secondary) => {
                let (body, secondary) = secondary.materialize_source(resources)?;
                (
                    String::new(),
                    Some(GraphNodeCompartments::new(primary_source, body)),
                    Some(secondary),
                )
            }
            None => (primary_source, None, None),
        };
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(1)
            .map_err(|_| label_allocation_failed())?;
        plans.push(GraphNodeLabelPlan {
            kind: self.kind,
            primary,
            secondary,
            title_line_count,
            metrics: self.metrics,
            document_cells: self.document_cells,
            materialized_bytes: self.materialized_bytes,
            width_profile: self.width_profile,
            diagram_type: self.diagram_type,
            wrap_width: self.wrap_width,
        });
        Ok(PreparedGraphNodeText {
            label,
            compartments,
            plans,
        })
    }
}

impl PreparedGraphNodeText {
    pub(super) fn label(&self) -> &str {
        &self.label
    }

    pub(super) fn compartments(&self) -> Option<&GraphNodeCompartments> {
        self.compartments.as_ref()
    }

    pub(super) fn plan(&self) -> &GraphNodeLabelPlan {
        // `materialize_after_admission` is the sole constructor and always installs one plan.
        &self.plans[0]
    }
}

impl GraphNodeLabelPlan {
    pub(super) fn try_for_node<'a>(
        node: &'a AsciiGraphNode,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<GraphNodeLabelPlanHandle<'a>> {
        resources.transaction(|resources| {
            Self::try_for_node_transactional(
                node,
                wrap_width,
                diagram_type,
                width_profile,
                resources,
            )
        })
    }

    fn try_for_node_transactional<'a>(
        node: &'a AsciiGraphNode,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<GraphNodeLabelPlanHandle<'a>> {
        if let Some(plan) = node.prepared_label_plan() {
            if plan.matches_node(node, wrap_width, diagram_type, width_profile) {
                return Ok(GraphNodeLabelPlanHandle::Borrowed(plan));
            }
            return Err(invalid_graph_label_plan(diagram_type));
        }
        let (
            kind,
            primary,
            secondary,
            title_line_count,
            width,
            line_count,
            document_cells,
            materialized_bytes,
        ) = match node.compartments() {
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
                    GraphLabelSectionPlan::single(title),
                    Some(GraphLabelSectionPlan::single(body)),
                    title_metrics.line_count,
                    title_metrics.max_width.max(body_metrics.max_width),
                    line_count,
                    document_cells,
                    materialized_bytes,
                )
            }
            None => {
                let plan = required_label_plan(
                    node.label(),
                    wrap_width,
                    diagram_type,
                    width_profile,
                    resources,
                )?;
                let metrics = plan.metrics();
                (
                    GraphNodeLabelPlanKind::Single,
                    GraphLabelSectionPlan::single(plan),
                    None,
                    0,
                    metrics.max_width,
                    metrics.line_count,
                    metrics.document_cells,
                    metrics.materialized_bytes,
                )
            }
        };
        Ok(GraphNodeLabelPlanHandle::Owned(Self {
            kind,
            primary,
            secondary,
            title_line_count,
            metrics: graph_label_metrics(width, line_count, resources)?,
            document_cells,
            materialized_bytes,
            width_profile,
            diagram_type,
            wrap_width,
        }))
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
        resources.transaction(|resources| {
            self.materialize_with_callback_transactional(node, resources, before_body_reserve)
        })
    }

    fn materialize_with_callback_transactional(
        &self,
        node: &AsciiGraphNode,
        resources: &ResourceContext,
        before_body_reserve: impl FnOnce(),
    ) -> crate::Result<GraphLabel> {
        self.admit_materialization(resources)?;
        match (self.kind, node.compartments()) {
            (GraphNodeLabelPlanKind::Single, None) => {
                let (lines, width) = self
                    .primary
                    .materialize_after_admission(node.label(), resources)?;
                Ok(GraphLabel {
                    lines,
                    width,
                    width_profile: self.width_profile,
                    compartment_break_after: None,
                })
            }
            (GraphNodeLabelPlanKind::Compartmented, Some(compartments)) => {
                let Some(body) = self.secondary.as_ref() else {
                    return Err(invalid_graph_label_plan(self.diagram_type));
                };
                let (mut lines, title_width) = self
                    .primary
                    .materialize_after_admission(&compartments.title, resources)?;
                before_body_reserve();
                lines
                    .try_reserve_exact(body.metrics.line_count)
                    .map_err(|_| label_allocation_failed())?;
                let (body_lines, body_width) =
                    body.materialize_after_admission(&compartments.body, resources)?;
                for (line_index, line) in body_lines.into_iter().enumerate() {
                    checkpoint_label_loop(resources, line_index)?;
                    lines.push(line);
                }
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

    fn admit_materialization(&self, resources: &ResourceContext) -> crate::Result<()> {
        let mut work_units = self.primary.replay_work_units(resources)?;
        if let Some(body) = self.secondary.as_ref() {
            work_units =
                resources.checked_work_add(work_units, body.replay_work_units(resources)?)?;
        }
        // The final canvas owns document accounting. This phase only checks the planned label
        // bound and commits replay work after every dimension has passed.
        resources.check_usage(work_units, self.document_cells)?;
        resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.materialized_bytes,
        )?;
        resources.charge_layout_work(work_units)
    }

    fn matches_node(
        &self,
        node: &AsciiGraphNode,
        wrap_width: Option<usize>,
        diagram_type: &'static str,
        width_profile: TerminalWidthProfile,
    ) -> bool {
        self.diagram_type == diagram_type
            && self.width_profile == width_profile
            && self.wrap_width == wrap_width
            && matches!(
                (self.kind, node.compartments()),
                (GraphNodeLabelPlanKind::Single, None)
                    | (GraphNodeLabelPlanKind::Compartmented, Some(_))
            )
    }
}

fn empty_normalized_label_metrics() -> NormalizedLabelMetrics {
    NormalizedLabelMetrics {
        materialized_bytes: 0,
        document_cells: 0,
        line_count: 0,
        max_width: 0,
    }
}

fn checkpoint_label_loop(resources: &ResourceContext, iteration: usize) -> crate::Result<()> {
    if iteration.is_multiple_of(GRAPH_LABEL_CHECKPOINT_INTERVAL) {
        resources.checkpoint()?;
    }
    Ok(())
}

fn push_graph_label_source(
    output: &mut String,
    source: &str,
    resources: &ResourceContext,
) -> crate::Result<()> {
    let mut start = 0usize;
    while start < source.len() {
        resources.checkpoint()?;
        let mut end = start
            .saturating_add(GRAPH_LABEL_COPY_CHUNK_BYTES)
            .min(source.len());
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        output.push_str(&source[start..end]);
        start = end;
    }
    Ok(())
}

fn checked_label_metric_add(
    resources: &ResourceContext,
    limit: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> crate::Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(limit))
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
    use super::{
        DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan, GraphLabel, GraphNodeLabelPlan,
        PreparedGraphNodeText,
    };
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
    fn deferred_joined_label_accepts_exact_projection_bounds_and_rejects_n_minus_one() {
        fn project(
            policy: AsciiResourcePolicy,
        ) -> crate::Result<(PreparedGraphNodeText, usize, usize, usize)> {
            let resources = ResourceContext::new(policy);
            let section = DeferredGraphLabelSectionPlan::try_joined(
                [" A ", " B "],
                None,
                "state",
                TerminalWidthProfile::Unicode,
                &resources,
            )?
            .expect("non-empty fragments should produce a joined section");
            let plan = DeferredGraphNodeLabelPlan::single(
                section,
                None,
                "state",
                TerminalWidthProfile::Unicode,
                &resources,
            )?;
            let document_cells = plan.document_cells();
            let output_bytes = plan.materialized_bytes();
            let source_work = plan.source_materialization_work_units();
            resources.check_usage(source_work, document_cells)?;
            resources.check(AsciiResourceLimitId::MaxOutputBytes, output_bytes)?;
            resources.charge_layout_work(source_work)?;
            let prepared = plan.materialize_after_admission(&resources)?;
            Ok((
                prepared,
                resources.layout_work_used(),
                document_cells,
                output_bytes,
            ))
        }

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let (prepared, exact_work, exact_document_cells, exact_output_bytes) =
            project(unbounded).expect("unbounded deferred projection should succeed");
        assert_eq!(prepared.label(), " A \n B ");
        assert!(prepared.compartments().is_none());
        let node = AsciiGraphNode::new_with_prepared_text(
            "node".to_string(),
            prepared,
            GraphNodeShape::Rect,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
        let plan = node
            .prepared_label_plan()
            .expect("the node should own its prepared label plan");
        let replay_resources = ResourceContext::new(unbounded);
        let materialized = plan
            .materialize(&node, &replay_resources)
            .expect("the retained joined plan should replay without rescanning");
        assert_eq!(materialized.lines(), ["A", "B"]);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact work limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_document_cells)
            .expect("exact document limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes)
            .expect("exact output limit should be valid");
        project(exact_policy).expect("exact deferred projection bounds should pass");

        for (limit, exact) in [
            (AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work),
            (AsciiResourceLimitId::MaxDocumentCells, exact_document_cells),
            (AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes),
        ] {
            let below = exact_policy
                .with_limit(limit, exact - 1)
                .expect("max-minus-one limit should be valid");
            let error =
                project(below).expect_err("max-minus-one deferred projection bound should reject");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit && details.max == exact - 1
            ));
        }
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
            let work_before = below_resources.layout_work_used();
            let document_cells_before = below_resources.document_cells_used();
            let body_reserve_started = Cell::new(false);
            let error = below_plan
                .materialize_with_body_reserve_probe(&node, &below_resources, &body_reserve_started)
                .expect_err("max-minus-one limit should reject before body allocation");

            assert!(!body_reserve_started.get(), "limit={limit:?}");
            assert_eq!(
                below_resources.layout_work_used(),
                work_before,
                "limit={limit:?}"
            );
            assert_eq!(
                below_resources.document_cells_used(),
                document_cells_before,
                "limit={limit:?}"
            );
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
        AsciiGraphNode::new_with_compartments(
            "node".to_string(),
            title.to_string(),
            Some(GraphNodeCompartments::new(title, body)),
            GraphNodeShape::StateWithTitle,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        )
    }
}
