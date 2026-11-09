use merman_model::families::git_graph::GitGraphDiagramModel;
use merman_model::presentation::PresentationConfig;
use merman_model::{OperationControl, OperationControlResult};

use crate::text::is_html_collapsible_ascii_whitespace;

use super::{
    DEFAULT_FONT_SIZE_PX, GitGraphRoleFontSize, commit_label_is_enabled, configured_base_font_size,
};

/// Checks only role sizes consumed by text geometry in the final GitGraph writer.
pub(crate) fn unsupported_native_font_size(
    model: &GitGraphDiagramModel,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    let show_commits = config.get_bool("gitGraph.showCommitLabel").unwrap_or(true);
    let base_px = configured_base_font_size(config).unwrap_or(DEFAULT_FONT_SIZE_PX);
    let mut commit_size = None;
    let mut tag_size = None;
    for commit in &model.commits {
        control.checkpoint()?;
        if commit_label_is_enabled(show_commits, commit.commit_type, commit.custom_id)
            && text_is_nonempty(&commit.id, char::is_whitespace, control)?
        {
            let size = commit_size.get_or_insert_with(|| {
                GitGraphRoleFontSize::from_config(config, "commitLabelFontSize", base_px)
            });
            control.checkpoint()?;
            if size.used_px.is_none() {
                return Ok(Some(
                    "themeVariables.commitLabelFontSize (unresolved CSS value)".to_owned(),
                ));
            }
        }
        for tag in &commit.tags {
            control.checkpoint()?;
            if text_is_nonempty(tag, is_html_collapsible_ascii_whitespace, control)? {
                let size = tag_size.get_or_insert_with(|| {
                    GitGraphRoleFontSize::from_config(config, "tagLabelFontSize", base_px)
                });
                control.checkpoint()?;
                if size.used_px.is_none() {
                    return Ok(Some(
                        "themeVariables.tagLabelFontSize (unresolved CSS value)".to_owned(),
                    ));
                }
            }
        }
    }
    control.checkpoint()?;
    Ok(None)
}

fn text_is_nonempty(
    text: &str,
    is_collapsible: fn(char) -> bool,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    let mut next_checkpoint = 0;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            control.checkpoint()?;
            next_checkpoint = offset.saturating_add(1024);
        }
        if !is_collapsible(character) {
            return Ok(true);
        }
    }
    control.checkpoint()?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::OperationPhase;
    use merman_model::families::git_graph::GitGraphCommit;

    fn model() -> GitGraphDiagramModel {
        GitGraphDiagramModel {
            diagram_type: "gitGraph".to_owned(),
            commits: vec![GitGraphCommit {
                id: "A".to_owned(),
                message: String::new(),
                seq: 0,
                commit_type: 0,
                tags: Vec::new(),
                parents: Vec::new(),
                branch: "main".to_owned(),
                custom_type: None,
                custom_id: Some(true),
            }],
            branches: Vec::new(),
            current_branch: "main".to_owned(),
            direction: "LR".to_owned(),
            title: None,
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        }
    }

    #[test]
    fn native_size_admission_skips_hidden_merge_and_cherry_pick_commit_labels() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": {"commitLabelFontSize": "calc(0px)", "tagLabelFontSize": "1rem"}
        }));
        for (commit_type, custom_id, id) in [
            (3, None, "merge"),
            (4, Some(true), "picked"),
            (0, Some(true), "   "),
            (0, Some(true), "\u{00a0}\u{2003}"),
        ] {
            let mut model = model();
            model.commits[0].commit_type = commit_type;
            model.commits[0].custom_id = custom_id;
            model.commits[0].id = id.to_owned();
            model.commits[0].tags.push(" ".to_owned());
            assert_eq!(
                unsupported_native_font_size(&model, &config, &OperationControl::new()).unwrap(),
                None
            );
        }
    }

    #[test]
    fn native_size_admission_observes_cancellation_in_roles_and_long_empty_text() {
        let config = PresentationConfig::default();
        let mut long_list = model();
        let commit = long_list.commits[0].clone();
        long_list.commits.resize(4096, commit);
        let mut long_text = model();
        long_text.commits[0].id = " ".repeat(64 * 1024);
        let mut long_tag = model();
        long_tag.commits[0].tags.push(" ".repeat(64 * 1024));
        for model in [long_list, long_text, long_tag] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(8);
            let error = unsupported_native_font_size(
                &model,
                &config,
                &control.for_phase(OperationPhase::Semantic),
            )
            .unwrap_err();
            assert_eq!(error.phase, OperationPhase::Semantic);
        }
    }
}
