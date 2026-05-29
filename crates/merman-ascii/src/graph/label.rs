use crate::text::display_width;

pub(super) const GRAPH_LABEL_LINE_GAP: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLabel {
    lines: Vec<String>,
    width: usize,
}

impl GraphLabel {
    pub(super) fn new(raw: &str) -> Self {
        let normalized = normalize_label_breaks(raw);
        let mut lines = normalized
            .split('\n')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
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
}

fn normalize_label_breaks(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        if let Some(end) = html_break_end(raw, index) {
            normalized.push('\n');
            index = end;
            continue;
        }
        if raw[index..].starts_with("\\n") {
            normalized.push('\n');
            index += 2;
            continue;
        }

        let ch = raw[index..]
            .chars()
            .next()
            .expect("index is always on a char boundary");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    normalized
}

fn html_break_end(raw: &str, start: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    if bytes.get(start).copied()? != b'<' {
        return None;
    }
    if !byte_eq_ignore_ascii_case(bytes.get(start + 1).copied()?, b'b')
        || !byte_eq_ignore_ascii_case(bytes.get(start + 2).copied()?, b'r')
    {
        return None;
    }

    let mut index = start + 3;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    if bytes.get(index).copied() == Some(b'/') {
        index += 1;
    }
    if bytes.get(index).copied() != Some(b'>') {
        return None;
    }
    Some(index + 1)
}

fn byte_eq_ignore_ascii_case(left: u8, right: u8) -> bool {
    left.eq_ignore_ascii_case(&right)
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
}
