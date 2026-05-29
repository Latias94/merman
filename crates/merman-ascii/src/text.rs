use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(crate) fn wrap_display_lines(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(1);
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        wrap_display_paragraph(paragraph, max_width, &mut lines);
    }

    lines
}

fn wrap_display_paragraph(text: &str, max_width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = display_width(word);
        if word_width > max_width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            push_wrapped_word(word, max_width, lines);
            continue;
        }

        let separator_width = usize::from(!current.is_empty());
        if current_width + separator_width + word_width > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        if !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }
        current.push_str(word);
        current_width += word_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }
}

fn push_wrapped_word(word: &str, max_width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_width = 0;

    for ch in word.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if current_width + ch_width > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }
}
