//! Shared Mermaid source and scalar normalization.

#[cfg(feature = "diagram-agentflow")]
pub(crate) use merman_markup::url::format_url;

#[cfg(any(
    feature = "diagram-block",
    feature = "diagram-kanban",
    feature = "diagram-packet",
    feature = "diagram-pie",
    feature = "diagram-radar",
    feature = "diagram-wardley",
    feature = "diagram-xychart"
))]
pub(crate) fn format_js_number(value: f64) -> String {
    // Mermaid template strings use Number text, including nonfinite values and unsigned zero.
    ryu_js::Buffer::new().format(value).to_owned()
}

#[cfg(any(feature = "diagram-journey", feature = "diagram-quadrant-chart"))]
pub(crate) use merman_model::number::parse_js_number;

pub(crate) fn cleanup_mermaid_comments(input: &str) -> std::borrow::Cow<'_, str> {
    if !input.contains("%%") {
        return std::borrow::Cow::Borrowed(input.trim_start());
    }

    let mut out = String::with_capacity(input.len());
    let mut removed = false;
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(after_marker) = trimmed.strip_prefix("%%") {
            let has_comment_body = after_marker.chars().next().is_some_and(|ch| ch != '\n');
            if !after_marker.starts_with('{') && has_comment_body {
                removed = true;
                continue;
            }
        }
        out.push_str(line);
    }

    let trimmed = out.trim_start();
    if !removed && trimmed.len() == input.len() {
        std::borrow::Cow::Borrowed(input)
    } else {
        std::borrow::Cow::Owned(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_mermaid_comments_matches_mermaid_line_comment_shape() {
        let text =
            "\n\n%% This is a comment\ngraph TD\n    A-->B\n    %% This is another comment\n";
        assert_eq!(
            cleanup_mermaid_comments(text).as_ref(),
            "graph TD\n    A-->B\n"
        );

        let eof = "graph TD\n    A-->B\n%% This is a comment";
        assert_eq!(
            cleanup_mermaid_comments(eof).as_ref(),
            "graph TD\n    A-->B\n"
        );

        let directive = "\n%% comment\n%%{init: {'theme': 'forest'}}%%\ngraph TD\nA-->B\n";
        assert_eq!(
            cleanup_mermaid_comments(directive).as_ref(),
            "%%{init: {'theme': 'forest'}}%%\ngraph TD\nA-->B\n"
        );

        let bare_marker = "%%\ngraph TD\nA-->B\n";
        assert_eq!(cleanup_mermaid_comments(bare_marker).as_ref(), bare_marker);
    }
}
