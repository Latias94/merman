use super::layout::SequenceLayout;

pub(super) fn padded_line(line: String, width: usize) -> Vec<char> {
    let mut line = line.chars().collect::<Vec<_>>();
    if line.len() < width {
        line.extend(std::iter::repeat_n(' ', width - line.len()));
    }
    line
}

pub(super) fn ensure_self_width(line: String, layout: &SequenceLayout, needed: usize) -> Vec<char> {
    let width = (layout.total_width + layout.self_message_width + 1).max(needed);
    padded_line(line, width)
}

pub(super) fn write_text(line: &mut [char], start: usize, text: &str) {
    for (offset, ch) in text.chars().enumerate() {
        let index = start + offset;
        if index < line.len() {
            line[index] = ch;
        }
    }
}

pub(super) fn trim_right(mut line: Vec<char>) -> String {
    while line.last() == Some(&' ') {
        line.pop();
    }
    line.into_iter().collect()
}
