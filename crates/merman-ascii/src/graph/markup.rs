use crate::TerminalWidthProfile;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{LabelBreakPolicy, try_plan_normalized_text};
use merman_model::families::flowchart::FlowchartModel;
use merman_model::text::{
    DecodedHtmlFragment, visit_decoded_html_entity_fragments_with_checkpoints,
};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use rustc_hash::FxHashMap;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LabelOwner {
    Node(usize),
    Edge(usize),
    Group(usize),
}

pub(super) struct FlowchartLabels<'a> {
    pub(super) model: &'a FlowchartModel,
    projected: FxHashMap<LabelOwner, String>,
    width_profile: TerminalWidthProfile,
    retained_bytes: usize,
}

impl<'a> FlowchartLabels<'a> {
    pub(super) fn new(
        model: &'a FlowchartModel,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut labels = Self {
            model,
            projected: FxHashMap::default(),
            width_profile,
            retained_bytes: 0,
        };
        for (index, node) in model.nodes.iter().enumerate() {
            resources.checkpoint()?;
            let source = model.label_sources.node_source(&node.id);
            labels.insert(
                LabelOwner::Node(index),
                source.unwrap_or(node.label.as_deref().unwrap_or(&node.id)),
                source.is_some(),
                node.label_type.as_deref(),
                resources,
            )?;
        }
        for (index, edge) in model.edges.iter().enumerate() {
            resources.checkpoint()?;
            let source = model.label_sources.edge_source(index);
            if let Some(raw) = source.or(edge.label.as_deref()) {
                labels.insert(
                    LabelOwner::Edge(index),
                    raw,
                    source.is_some(),
                    edge.label_type.as_deref(),
                    resources,
                )?;
            }
        }
        for (index, group) in model.subgraphs.iter().enumerate() {
            resources.checkpoint()?;
            let source = model.label_sources.subgraph_source(index, &group.id);
            labels.insert(
                LabelOwner::Group(index),
                source.unwrap_or(&group.title),
                source.is_some(),
                group.label_type.as_deref(),
                resources,
            )?;
        }
        Ok(labels)
    }

    fn insert(
        &mut self,
        owner: LabelOwner,
        source: &str,
        decode_entities: bool,
        kind: Option<&str>,
        resources: &ResourceContext,
    ) -> Result<()> {
        let markdown = if kind == Some("markdown") {
            project_markdown_emphasis(source, resources)?
        } else {
            None
        };
        if markdown.is_none() && !decode_entities {
            return Ok(());
        }
        let source = markdown.as_deref().unwrap_or(source);
        let mut output_bytes = 0usize;
        let mut cells = 0usize;
        visit_display_text(source, decode_entities, resources, |fragment| {
            output_bytes = output_bytes
                .checked_add(fragment.len())
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
            let metrics =
                try_plan_normalized_text(fragment, self.width_profile, resources)?.metrics();
            cells = cells
                .checked_add(metrics.document_cells)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
            Ok(())
        })?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(output_bytes)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, self.retained_bytes)?;
        resources.charge_usage(output_bytes.max(1), cells)?;
        let mut output = String::new();
        output
            .try_reserve_exact(output_bytes)
            .map_err(|_| allocation_failed())?;
        visit_display_text(source, decode_entities, resources, |fragment| {
            output.push_str(fragment);
            Ok(())
        })?;
        self.projected
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.projected.insert(owner, output);
        Ok(())
    }

    pub(super) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub(super) fn node(&self, index: usize) -> (&str, LabelBreakPolicy) {
        self.get(
            LabelOwner::Node(index),
            self.model.nodes[index]
                .label
                .as_deref()
                .unwrap_or(&self.model.nodes[index].id),
        )
    }

    pub(super) fn edge(&self, index: usize) -> Option<(&str, LabelBreakPolicy)> {
        if let Some(value) = self.projected.get(&LabelOwner::Edge(index)) {
            return Some((value, LabelBreakPolicy::LiteralRows));
        }
        self.model.edges[index]
            .label
            .as_deref()
            .map(|value| (value, LabelBreakPolicy::MermaidLabelBreaks))
    }

    pub(super) fn group(&self, index: usize) -> (&str, LabelBreakPolicy) {
        self.get(LabelOwner::Group(index), &self.model.subgraphs[index].title)
    }

    fn get<'s>(&'s self, owner: LabelOwner, fallback: &'s str) -> (&'s str, LabelBreakPolicy) {
        self.projected
            .get(&owner)
            .map_or((fallback, LabelBreakPolicy::MermaidLabelBreaks), |value| {
                (value.as_str(), LabelBreakPolicy::LiteralRows)
            })
    }
}

fn visit_display_text(
    source: &str,
    decode_entities: bool,
    resources: &ResourceContext,
    mut visit: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut start = 0;
    let mut index = 0;
    let mut slashes = 0usize;
    while index < source.len() {
        resources.charge_layout_work(1)?;
        let end = crate::text::html_break_end(source, index).or_else(|| {
            (source[index..].starts_with("\\n") && slashes.is_multiple_of(2)).then_some(index + 2)
        });
        if let Some(end) = end {
            visit_decoded(
                &source[start..index],
                decode_entities,
                resources,
                &mut visit,
            )?;
            visit("\n")?;
            index = end;
            start = end;
            slashes = 0;
        } else {
            let Some(ch) = source[index..].chars().next() else {
                break;
            };
            slashes = if ch == '\\' { slashes + 1 } else { 0 };
            index += ch.len_utf8();
        }
    }
    visit_decoded(&source[start..], decode_entities, resources, &mut visit)
}

fn visit_decoded(
    source: &str,
    decode_entities: bool,
    resources: &ResourceContext,
    visit: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    if !decode_entities {
        resources.checkpoint()?;
        return visit(source);
    }
    visit_decoded_html_entity_fragments_with_checkpoints(
        source,
        || resources.checkpoint(),
        |fragment| match fragment {
            DecodedHtmlFragment::Borrowed(text) => visit(text),
            DecodedHtmlFragment::Scalar(ch) => {
                let mut buffer = [0u8; 4];
                visit(ch.encode_utf8(&mut buffer))
            }
        },
    )
}

pub(super) fn project_markdown_emphasis(
    source: &str,
    resources: &ResourceContext,
) -> Result<Option<String>> {
    resources.charge_layout_work(source.len().max(1))?;
    if !source.contains(['*', '_']) {
        return Ok(None);
    }
    // Bound the parser's source-sized scratch storage before its eager first pass.
    resources.check(AsciiResourceLimitId::MaxDocumentCells, source.len())?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, source.len())?;
    resources.checkpoint()?;
    let parser = Parser::new(source).into_offset_iter();
    resources.checkpoint()?;
    let mut removed: Vec<Range<usize>> = Vec::new();
    let mut output_bytes = source.len();
    for (event, range) in parser {
        resources.charge_layout_work(1)?;
        let delimiter = match event {
            Event::Start(Tag::Strong) => Some(range.start..range.start + 2),
            Event::End(TagEnd::Strong) => Some(range.end - 2..range.end),
            Event::Start(Tag::Emphasis) => Some(range.start..range.start + 1),
            Event::End(TagEnd::Emphasis) => Some(range.end - 1..range.end),
            _ => None,
        };
        let Some(delimiter) = delimiter else {
            continue;
        };
        debug_assert!(
            source[delimiter.clone()]
                .bytes()
                .all(|ch| matches!(ch, b'*' | b'_'))
        );
        debug_assert!(
            removed
                .last()
                .is_none_or(|previous| previous.end <= delimiter.start)
        );
        output_bytes -= delimiter.len();
        removed.try_reserve(1).map_err(|_| allocation_failed())?;
        removed.push(delimiter);
    }
    if removed.is_empty() {
        return Ok(None);
    }
    resources.charge_layout_work(output_bytes.max(1))?;
    let mut output = String::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|_| allocation_failed())?;
    let mut start = 0;
    for range in removed {
        resources.checkpoint()?;
        output.push_str(&source[start..range.start]);
        start = range.end;
    }
    output.push_str(&source[start..]);
    Ok(Some(output))
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiResourcePolicy;

    #[test]
    fn emphasis_projection_preserves_non_formatting_source() {
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        for (source, expected) in [
            ("***nested*** __strong__ _em_", "nested strong em"),
            (
                r"**shown** \*literal\* foo_bar *unclosed",
                r"shown \*literal\* foo_bar *unclosed",
            ),
            (
                "**shown** `**code**` [*link*](https://example.test/a_b)",
                "shown `**code**` [link](https://example.test/a_b)",
            ),
            (
                "**shown**<br/>&lt;br&gt;\n*next*",
                "shown<br/>&lt;br&gt;\nnext",
            ),
            ("**中文 👩‍💻 é**", "中文 👩‍💻 é"),
        ] {
            assert_eq!(
                project_markdown_emphasis(source, &resources)
                    .unwrap()
                    .as_deref(),
                Some(expected),
                "{source}"
            );
        }
        for source in ["foo_bar", "`**literal**`", "*unclosed", "plain"] {
            assert_eq!(
                project_markdown_emphasis(source, &resources).unwrap(),
                None,
                "{source}"
            );
        }
    }

    #[test]
    fn markdown_projection_admits_exact_work_and_observes_late_cancellation() {
        use crate::operation::AsciiExecution;
        use merman_model::{OperationControl, OperationPhase};
        let source = "**header** with _中文_ and `*literal*`";
        let policy = AsciiResourcePolicy::unbounded();
        let measured = ResourceContext::new(policy);
        project_markdown_emphasis(source, &measured).unwrap();
        let exact = measured.layout_work_used();
        for limit in [exact, exact - 1] {
            let resources = ResourceContext::new(
                policy
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            );
            let result = project_markdown_emphasis(source, &resources);
            if limit == exact {
                assert_eq!(
                    result.unwrap().as_deref(),
                    Some("header with 中文 and `*literal*`")
                );
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
                );
            }
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(4);
        let mut resources = ResourceContext::new(policy);
        AsciiExecution::new(&control, &policy)
            .rebind_resource_context(&mut resources, OperationPhase::Semantic);
        assert!(
            matches!(project_markdown_emphasis(source, &resources), Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic)
        );
    }
}
