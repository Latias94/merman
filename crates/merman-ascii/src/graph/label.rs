use crate::text::{display_width, split_label_lines, wrap_label_lines};

pub(super) const GRAPH_LABEL_LINE_GAP: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLabel {
    lines: Vec<String>,
    width: usize,
}

impl GraphLabel {
    pub(super) fn new(raw: &str) -> Self {
        Self::from_lines(split_label_lines(raw))
    }

    pub(super) fn wrapped(raw: &str, max_width: usize) -> Self {
        Self::from_lines(wrap_label_lines(raw, max_width))
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    pub(super) fn content_height(&self) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        self.lines.len() + (self.lines.len() - 1) * GRAPH_LABEL_LINE_GAP
    }

    fn from_lines(mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        let width = lines
            .iter()
            .map(|line| display_width(line))
            .max()
            .unwrap_or_default();
        Self { lines, width }
    }
}

#[cfg(test)]
mod tests {
    use super::GraphLabel;

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
        let label = GraphLabel::wrapped("Alpha Beta<br><br>Gamma Delta", 6);

        assert_eq!(label.lines(), ["Alpha", "Beta", "", "Gamma", "Delta"]);
        assert_eq!(label.width(), 5);
        assert_eq!(label.content_height(), 9);
    }
}
