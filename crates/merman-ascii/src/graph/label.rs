use crate::options::TerminalWidthProfile;
use crate::text::{display_width_with_profile, split_label_lines, wrap_label_lines_with_profile};

pub(super) const GRAPH_LABEL_LINE_GAP: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLabel {
    lines: Vec<String>,
    width: usize,
    width_profile: TerminalWidthProfile,
}

impl GraphLabel {
    #[cfg(test)]
    pub(super) fn new(raw: &str) -> Self {
        Self::new_with_profile(raw, TerminalWidthProfile::Unicode)
    }

    pub(super) fn new_with_profile(raw: &str, width_profile: TerminalWidthProfile) -> Self {
        Self::from_lines(split_label_lines(raw), width_profile)
    }

    pub(super) fn wrapped_with_profile(
        raw: &str,
        max_width: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self::from_lines(
            wrap_label_lines_with_profile(raw, max_width, width_profile),
            width_profile,
        )
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

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

    fn from_lines(mut lines: Vec<String>, width_profile: TerminalWidthProfile) -> Self {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GraphLabel;
    use crate::TerminalWidthProfile;

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
    fn graph_label_uses_selected_ambiguous_width_profile() {
        let unicode = GraphLabel::new_with_profile("A·B", TerminalWidthProfile::Unicode);
        let cjk = GraphLabel::new_with_profile("A·B", TerminalWidthProfile::Cjk);

        assert_eq!(unicode.width(), 3);
        assert_eq!(cjk.width(), 4);
    }
}
