mod full;

pub use full::{Detector, DetectorFn, DetectorRegistry};

use crate::family::DetectorFact;
use crate::{MermaidConfig, OperationControl, OperationControlResult, Result};
use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
#[error("No diagram type detected matching given configuration for text: {text}")]
pub struct DetectTypeError {
    /// Input after front-matter, directives, and Mermaid comments have been removed.
    pub text: String,
}

pub(crate) type BuiltinDetectorFn = fn(text: &str) -> bool;

/// Configuration writes made by the pinned built-in detectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetectorConfigEffect {
    None,
    ElkLayout,
}

impl DetectorConfigEffect {
    fn assignment(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::None => None,
            Self::ElkLayout => Some(("layout", "elk")),
        }
    }

    pub(crate) fn apply(self, config: &mut MermaidConfig) {
        if let Some((path, value)) = self.assignment() {
            config.set_value(path, serde_json::Value::String(value.to_string()));
        }
    }
}

pub(crate) fn builtin_config_effect(diagram_type: &str) -> DetectorConfigEffect {
    crate::family::detector_facts()
        .iter()
        .find(|fact| fact.id == diagram_type)
        .map_or(DetectorConfigEffect::None, |fact| fact.config_effect)
}

pub(crate) fn detect_builtin_controlled(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<DetectorFact>> {
    let cleaned = clean_detection_input_controlled(text, control)?;
    detect_builtin_cleaned_controlled(cleaned.as_ref(), control)
}

pub(crate) fn detect_builtin_precleaned_controlled(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<DetectorFact>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    detect_builtin_cleaned_controlled(text, control)
}

fn detect_builtin_cleaned_controlled(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<DetectorFact>> {
    Ok(detect_precleaned_controlled(
        text,
        crate::family::detector_facts(),
        |fact, text| (fact.detector)(text),
        control,
    )?
    .copied())
}

fn detect_precleaned_controlled<'a, T>(
    text: &str,
    detectors: &'a [T],
    mut matches: impl FnMut(&T, &str) -> bool,
    control: &OperationControl,
) -> OperationControlResult<Result<&'a T>> {
    control.checkpoint()?;
    for (index, detector) in detectors.iter().enumerate() {
        if index % 16 == 0 {
            control.checkpoint()?;
        }
        if matches(detector, text) {
            return Ok(Ok(detector));
        }
    }
    control.checkpoint()?;
    Ok(Err(DetectTypeError {
        text: text.to_string(),
    }
    .into()))
}

fn clean_detection_input_controlled<'a>(
    text: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    control.checkpoint()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let no_frontmatter = crate::preprocess::locate_frontmatter_block_controlled(text, control)?
        .map_or(text, |block| block.stripped);
    control.checkpoint()?;
    let no_directives = remove_directives_controlled(no_frontmatter, control)?;
    control.checkpoint()?;
    let cleaned = match crate::utils::cleanup_mermaid_comments(no_directives.as_ref()) {
        Cow::Borrowed(_) => no_directives,
        Cow::Owned(cleaned) => Cow::Owned(cleaned),
    };
    Ok(cleaned)
}

#[cfg(test)]
fn remove_directives(text: &str) -> Cow<'_, str> {
    let control = OperationControl::new();
    remove_directives_controlled(text, &control)
        .expect("a private parse control cannot be cancelled")
}

fn remove_directives_controlled<'a>(
    text: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    control.checkpoint()?;
    let ranges = crate::preprocess::directive_removal_ranges_controlled(text, control)?;
    if ranges.is_empty() {
        return Ok(Cow::Borrowed(text));
    }

    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for (index, range) in ranges.into_iter().enumerate() {
        if index % 32 == 0 {
            control.checkpoint()?;
        }
        out.push_str(&text[pos..range.start]);
        pos = range.end;
    }
    out.push_str(&text[pos..]);
    control.checkpoint()?;
    Ok(Cow::Owned(out))
}

pub(crate) fn detector_frontmatter_unparsed(txt: &str) -> bool {
    txt.trim_start().starts_with("---")
}

pub(crate) fn detector_error(txt: &str) -> bool {
    txt.trim().eq_ignore_ascii_case("error")
}

pub(crate) fn detector_c4(txt: &str) -> bool {
    // Matches Mermaid's upstream regex exactly (note the missing grouping in JS).
    txt.trim_start_matches(char::is_whitespace)
        .starts_with("C4Context")
        || txt.contains("C4Container")
        || txt.contains("C4Component")
        || txt.contains("C4Dynamic")
        || txt.contains("C4Deployment")
}

pub(crate) fn detector_kanban(txt: &str) -> bool {
    txt.trim_start().starts_with("kanban")
}

pub(crate) fn detector_class_v2(txt: &str) -> bool {
    txt.trim_start().starts_with("classDiagram")
}

pub(crate) fn detector_er(txt: &str) -> bool {
    txt.trim_start().starts_with("erDiagram")
}

pub(crate) fn detector_gantt(txt: &str) -> bool {
    txt.trim_start().starts_with("gantt")
}

pub(crate) fn detector_info(txt: &str) -> bool {
    txt.trim_start().starts_with("info")
}

pub(crate) fn detector_pie(txt: &str) -> bool {
    txt.trim_start().starts_with("pie")
}

pub(crate) fn detector_requirement(txt: &str) -> bool {
    txt.trim_start().starts_with("requirement")
}

pub(crate) fn detector_sequence(txt: &str) -> bool {
    txt.trim_start().starts_with("sequenceDiagram")
}

pub(crate) fn detector_swimlane(txt: &str) -> bool {
    starts_with_js_word_boundary(txt.trim_start(), "swimlane-beta")
}

pub(crate) fn detector_flowchart_elk(txt: &str) -> bool {
    txt.trim_start().starts_with("flowchart-elk")
}

pub(crate) fn detector_flowchart_v2(txt: &str) -> bool {
    txt.trim_start().starts_with("graph") || txt.trim_start().starts_with("flowchart")
}

pub(crate) fn detector_timeline(txt: &str) -> bool {
    txt.trim_start().starts_with("timeline")
}

pub(crate) fn detector_git_graph(txt: &str) -> bool {
    txt.trim_start().starts_with("gitGraph")
}

pub(crate) fn detector_usecase(txt: &str) -> bool {
    txt.trim_start()
        .strip_prefix("usecase-beta")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

pub(crate) fn detector_agentflow(txt: &str) -> bool {
    starts_with_js_word_boundary(txt.trim_start(), "agentflow-beta")
}

pub(crate) fn detector_state_v2(txt: &str) -> bool {
    txt.trim_start().starts_with("stateDiagram")
}

pub(crate) fn detector_journey(txt: &str) -> bool {
    txt.trim_start().starts_with("journey")
}

pub(crate) fn detector_quadrant(txt: &str) -> bool {
    txt.trim_start().starts_with("quadrantChart")
}

pub(crate) fn detector_sankey(txt: &str) -> bool {
    txt.trim_start().starts_with("sankey")
}

pub(crate) fn detector_packet(txt: &str) -> bool {
    txt.trim_start().starts_with("packet")
}

pub(crate) fn detector_xychart(txt: &str) -> bool {
    txt.trim_start().starts_with("xychart")
}

pub(crate) fn detector_block(txt: &str) -> bool {
    txt.trim_start().starts_with("block")
}

pub(crate) fn detector_tree_view(txt: &str) -> bool {
    txt.trim_start().starts_with("treeView-beta")
}

pub(crate) fn detector_ishikawa(txt: &str) -> bool {
    let t = txt.trim_start();
    starts_with_header_case_insensitive(t, "ishikawa-beta")
        || starts_with_header_case_insensitive(t, "ishikawa")
}

pub(crate) fn detector_eventmodeling(txt: &str) -> bool {
    txt.trim_start().starts_with("eventmodeling")
}

pub(crate) fn detector_railroad(txt: &str) -> bool {
    starts_with_case_insensitive_prefix(txt.trim_start(), "railroad-beta")
}

pub(crate) fn detector_railroad_ebnf(txt: &str) -> bool {
    starts_with_case_insensitive_prefix(txt.trim_start(), "railroad-ebnf-beta")
}

pub(crate) fn detector_railroad_abnf(txt: &str) -> bool {
    starts_with_case_insensitive_prefix(txt.trim_start(), "railroad-abnf-beta")
}

pub(crate) fn detector_railroad_peg(txt: &str) -> bool {
    starts_with_case_insensitive_prefix(txt.trim_start(), "railroad-peg-beta")
}

pub(crate) fn detector_wardley(txt: &str) -> bool {
    starts_with_case_insensitive_prefix(txt.trim_start(), "wardley-beta")
}

pub(crate) fn detector_cynefin(txt: &str) -> bool {
    let Some(rest) = txt.trim_start().strip_prefix("cynefin-beta") else {
        return false;
    };
    rest.chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || c == ':')
}

fn starts_with_header_case_insensitive(text: &str, header: &str) -> bool {
    let Some(actual) = text.get(..header.len()) else {
        return false;
    };
    if !actual.eq_ignore_ascii_case(header) {
        return false;
    }
    text[header.len()..]
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || c == ';')
}

fn starts_with_case_insensitive_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|actual| actual.eq_ignore_ascii_case(prefix))
}

fn starts_with_js_word_boundary(text: &str, header: &str) -> bool {
    text.strip_prefix(header).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
    })
}

pub(crate) fn detector_radar(txt: &str) -> bool {
    txt.trim_start().starts_with("radar-beta")
}

pub(crate) fn detector_treemap(txt: &str) -> bool {
    txt.trim_start().starts_with("treemap")
}

pub(crate) fn detector_venn(txt: &str) -> bool {
    txt.trim_start().starts_with("venn-beta")
}

pub(crate) fn detector_mindmap(txt: &str) -> bool {
    txt.trim_start().starts_with("mindmap")
}

pub(crate) fn detector_architecture(txt: &str) -> bool {
    txt.trim_start().starts_with("architecture")
}

pub(crate) fn detector_zenuml(txt: &str) -> bool {
    txt.trim_start().starts_with("zenuml")
}

#[cfg(test)]
mod remove_directives_tests {
    use super::{detect_builtin_controlled, remove_directives};
    use crate::{OperationCancelled, OperationControl};
    use std::borrow::Cow;

    #[test]
    fn builtin_detection_observes_the_same_controlled_frontmatter_cancellation() {
        let source = format!("---\n{}", " \n".repeat(16_000));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        assert!(matches!(
            detect_builtin_controlled(&source, &control),
            Err(OperationCancelled { .. })
        ));
    }

    #[test]
    fn no_directives_is_borrowed() {
        let s = "flowchart TD; A-->B;";
        assert!(matches!(remove_directives(s), Cow::Borrowed(_)));
    }

    #[test]
    fn removes_directive_block() {
        let s = "%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD; A-->B;";
        let out = remove_directives(s);
        assert!(out.as_ref().contains("flowchart TD"));
        assert!(!out.as_ref().contains("init"));
    }

    #[test]
    fn unterminated_directive_truncates_following_source_like_mermaid() {
        let s = "flowchart\n%%{init: {\"theme\": \"dark\"}}\nA-->B;";
        let out = remove_directives(s);
        assert_eq!(out.as_ref(), "flowchart\n");
    }
}
