//! Source-index identities and complete GitGraph records, including detached models.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_git_graph(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::GitGraph(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let commits: Vec<&RawValue> = decode(root.remove("commits"), lines)?;
    let branches: Vec<&RawValue> = decode(root.remove("branches"), lines)?;
    if commits.len() != model.commits.len() || branches.len() != model.branches.len() {
        return Err(invalid_projection());
    }
    lines.push("GitGraph records")?;
    write_plain_record(
        "Keys: C1..Cn and B1..Bn follow model order. Parent lists retain literal IDs and order; a cherry-pick's second entry is its source, not ancestry.",
        lines,
    )?;
    for (index, (branch, raw)) in model.branches.iter().zip(branches).enumerate() {
        let mut fields: Fields<'_> = decode(Some(raw), lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("B{} [branch] name=", index + 1), false)?;
        record.text(&branch.name, true)?;
        record.finish()?;
        fields.remove("name");
        details(&format!("B{}", index + 1), &fields, lines)?;
    }
    for (index, (commit, raw)) in model.commits.iter().zip(commits).enumerate() {
        let key = format!("C{}", index + 1);
        let mut fields: Fields<'_> = decode(Some(raw), lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" [commit] id=", false)?;
        record.text(&commit.id, true)?;
        record.text(" branch=", false)?;
        record.text(&commit.branch, true)?;
        record.text(" kind=", false)?;
        record.text(
            crate::git_graph::commit_kind(commit.commit_type).unwrap_or("unknown"),
            false,
        )?;
        record.text(" message=", false)?;
        record.text(&commit.message, true)?;
        record.text(" tags=[", false)?;
        for (index, tag) in commit.tags.iter().enumerate() {
            if index > 0 {
                record.text(", ", false)?;
            }
            record.text(tag, true)?;
        }
        record.text("] parents=[", false)?;
        for (index, parent) in commit.parents.iter().enumerate() {
            if index > 0 {
                record.text(", ", false)?;
            }
            record.text(parent, true)?;
        }
        record.text("]", false)?;
        if commit.commit_type == 4
            && let Some(source) = commit.parents.get(1)
        {
            record.text(" cherry-picked from=", false)?;
            record.text(source, true)?;
        }
        record.finish()?;
        for name in ["id", "branch", "message", "tags", "parents"] {
            fields.remove(name);
        }
        // Keep numeric type, sequence, explicit false overrides, future commit fields, and
        // every metadata field. Warning rule IDs are data, not just human-readable messages.
        details(&key, &fields, lines)?;
    }
    details("gitGraph", &root, lines)?;
    Ok(true)
}

fn decode<'a, T: Deserialize<'a>>(
    raw: Option<&'a RawValue>,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    let raw = raw.ok_or_else(invalid_projection)?;
    lines.admit_node(1)?;
    lines
        .semantic_resources
        .charge_layout_work(raw.get().len())?;
    serde_json::from_str(raw.get()).map_err(|_| invalid_projection())
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "gitGraph",
        feature: "invalid serialized semantic projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiColorMode, AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::{AsciiRenderOptions, AsciiSourceContext};
    use merman_model::families::git_graph::{GitGraphBranch, GitGraphCommit, GitGraphDiagramModel};

    #[test]
    fn primary_peak_admits_retained_tracks_projection_and_records_for_every_encoding() {
        let data = GitGraphDiagramModel {
            diagram_type: "gitGraph".into(),
            commits: vec![GitGraphCommit {
                id: "root".into(),
                message: "x".repeat(8192),
                seq: 0,
                commit_type: 0,
                tags: Vec::new(),
                parents: Vec::new(),
                branch: "main".into(),
                custom_type: None,
                custom_id: Some(false),
            }],
            branches: vec![GitGraphBranch {
                name: "main".into(),
            }],
            current_branch: "main".into(),
            direction: "LR".into(),
            title: None,
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        };
        let config = merman_model::MermaidConfig::empty_object();
        let model = AsciiDiagram::GitGraph(&data);
        let source = AsciiSourceContext {
            diagram_type: "gitGraph",
            config: &config,
            semantic_config: &config,
            title: None,
        };
        let unbounded = AsciiResourcePolicy::unbounded();
        let projection = super::super::semantic_fallback_projection(
            &model,
            source,
            AsciiExecution::for_test(&unbounded),
        )
        .unwrap_or_else(|_| panic!("valid projection"));
        let topology = "retained tracks";
        let baseline = super::super::render_reading_primary(
            &model,
            source,
            &AsciiRenderOptions::ascii(),
            AsciiExecution::for_test(&unbounded),
            Some(topology),
        )
        .unwrap();
        let peak = topology.len() + projection.len() + baseline.len();
        for color in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Html,
        ] {
            let encoded = super::super::render_reading_primary(
                &model,
                source,
                &AsciiRenderOptions::ascii().with_color_mode(color),
                AsciiExecution::for_test(&unbounded),
                Some(topology),
            )
            .unwrap();
            let peak = if color == AsciiColorMode::Html {
                peak.max(topology.len() + baseline.len() + encoded.len())
            } else {
                peak
            };
            for limit in [encoded.len(), peak - 1, peak] {
                let policy = unbounded
                    .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                    .unwrap();
                let result = super::super::render_reading_primary(
                    &model,
                    source,
                    &AsciiRenderOptions::ascii().with_color_mode(color),
                    AsciiExecution::for_test(&policy),
                    Some(topology),
                );
                if limit == peak {
                    assert!(result.is_ok(), "{color:?}: {result:?}");
                } else {
                    assert!(
                        matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes),
                        "{color:?}: {result:?}"
                    );
                }
            }
        }
    }
}
