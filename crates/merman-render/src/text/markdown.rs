//! Mermaid-like Markdown tokenization helpers.

use super::{is_ecmascript_whitespace, trim_ecmascript_whitespace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MermaidMarkdownWordType {
    Normal,
    Strong,
    Em,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MermaidMarkdownAnalysis {
    pub(crate) lines: Vec<Vec<(String, MermaidMarkdownWordType)>>,
    pub(crate) has_styled_runs: bool,
    pub(crate) line_count: usize,
}

impl MermaidMarkdownAnalysis {
    pub(crate) fn all_runs_normal(&self) -> bool {
        !self.has_styled_runs
    }
}

pub(crate) fn analyze_mermaid_markdown(
    markdown: &str,
    markdown_auto_wrap: bool,
) -> MermaidMarkdownAnalysis {
    let lines = mermaid_markdown_to_lines(markdown, markdown_auto_wrap);
    let has_styled_runs = lines
        .iter()
        .flatten()
        .any(|(_, word_type)| !matches!(word_type, MermaidMarkdownWordType::Normal));
    let line_count = lines.len();
    MermaidMarkdownAnalysis {
        lines,
        has_styled_runs,
        line_count,
    }
}

/// Minimal, deterministic subset of Mermaid's `markdownToLines(...)` output.
///
/// This aims to match Mermaid's token boundaries for emphasis/strong delimiters (including `_`
/// behavior) well enough to reproduce upstream SVG-label layout and baseline DOM.
pub(crate) fn mermaid_markdown_to_lines(
    markdown: &str,
    markdown_auto_wrap: bool,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    markdown_to_lines(markdown, markdown_auto_wrap, false)
}

pub(super) fn html_markdown_measurement_lines(
    markdown: &str,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    markdown_to_lines(markdown, true, true)
}

/// Marked accepts single-tilde deletions inside words; pulldown only accepts double tildes there.
/// Project paired deletions in paragraph text through the existing SVG token allowlist.
/// Retained HTML, code, escaped punctuation and unpaired delimiters keep their source spelling.
fn project_svg_markdown_deletions(markdown: &str) -> Option<String> {
    if !markdown.contains('~') {
        return None;
    }
    let escaped = |index| {
        markdown[..index]
            .bytes()
            .rev()
            .take_while(|&byte| byte == b'\\')
            .count()
            % 2
            != 0
    };
    let closers = markdown
        .match_indices('~')
        .filter_map(|(index, _)| {
            let previous = markdown[..index].chars().next_back();
            let next = markdown[index + 1..].chars().next();
            (previous.is_some_and(|ch| ch != '~' && !is_ecmascript_whitespace(ch))
                && next != Some('~')
                && !escaped(index))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let parser = pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    );
    let mut contexts = Vec::new();
    let mut deletions = Vec::new();
    let mut consumed_until = 0;
    let mut next_closer = 0;
    for (event, range) in parser.into_offset_iter() {
        match event {
            pulldown_cmark::Event::Start(tag) => {
                if matches!(tag, pulldown_cmark::Tag::Strikethrough)
                    && contexts.last().is_some_and(|&(_, allowed)| allowed)
                    && range.start >= consumed_until
                {
                    deletions.push((range.start, range.end));
                    consumed_until = range.end;
                }
                let allowed = match tag {
                    pulldown_cmark::Tag::Paragraph => contexts.is_empty(),
                    pulldown_cmark::Tag::Strong | pulldown_cmark::Tag::Emphasis => {
                        contexts.last().is_some_and(|&(_, allowed)| allowed)
                    }
                    _ => false,
                };
                contexts.push((range.end, allowed));
            }
            pulldown_cmark::Event::End(_) => {
                contexts.pop();
            }
            pulldown_cmark::Event::Text(_) => {
                let Some(&(parent_end, true)) = contexts.last() else {
                    continue;
                };
                for (offset, _) in markdown[range.clone()].match_indices('~') {
                    let index = range.start + offset;
                    if index < consumed_until
                        || markdown[..index].ends_with('~')
                        || markdown[index + 1..]
                            .chars()
                            .next()
                            .is_none_or(|ch| ch == '~' || is_ecmascript_whitespace(ch))
                        || escaped(index)
                    {
                        continue;
                    }
                    while closers
                        .get(next_closer)
                        .is_some_and(|&closer| closer <= index + 1)
                    {
                        next_closer += 1;
                    }
                    if let Some(&closer) = closers.get(next_closer)
                        && closer < parent_end
                    {
                        deletions.push((index, closer + 1));
                        consumed_until = closer + 1;
                    }
                }
            }
            _ => {}
        }
    }
    if deletions.is_empty() {
        return None;
    }
    let mut normalized = String::with_capacity(markdown.len());
    let mut start = 0;
    for (opening, end) in deletions {
        normalized.push_str(&markdown[start..opening]);
        // The SVG projection drops the entire deletion. Use an opaque del token so original
        // double tildes or inline markup inside it cannot close the normalized token early.
        normalized.push_str("~~x~~");
        start = end;
    }
    normalized.push_str(&markdown[start..]);
    Some(normalized)
}

fn markdown_to_lines(
    markdown: &str,
    markdown_auto_wrap: bool,
    preserve_inline_literals: bool,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    fn preprocess_mermaid_markdown(markdown: &str) -> String {
        let markdown = markdown.replace("\r\n", "\n");

        // Mermaid preprocessing:
        // - Replace `<br/>` with `\n`
        // - Replace multiple newlines with a single newline
        // - Dedent common indentation
        let mut s = markdown
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("<br>", "\n")
            .replace("</br>", "\n")
            .replace("</br/>", "\n")
            .replace("</br />", "\n")
            .replace("</br >", "\n");

        // Collapse multiple consecutive newlines to a single `\n`.
        let mut collapsed = String::with_capacity(s.len());
        let mut prev_nl = false;
        for ch in s.chars() {
            if ch == '\n' {
                if prev_nl {
                    continue;
                }
                prev_nl = true;
                collapsed.push('\n');
            } else {
                prev_nl = false;
                collapsed.push(ch);
            }
        }
        s = collapsed;

        // Dedent: remove the smallest common leading indentation of non-empty lines.
        let lines: Vec<&str> = s.split('\n').collect();
        let mut min_indent: Option<usize> = None;
        for l in &lines {
            if trim_ecmascript_whitespace(l).is_empty() {
                continue;
            }
            let indent = l
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>();
            min_indent = Some(min_indent.map_or(indent, |m| m.min(indent)));
        }
        let min_indent = min_indent.unwrap_or(0);
        if min_indent > 0 {
            let mut dedented = String::with_capacity(s.len());
            for (idx, l) in lines.iter().enumerate() {
                if idx > 0 {
                    dedented.push('\n');
                }
                let mut remaining = min_indent;
                let mut it = l.chars().peekable();
                while remaining > 0 {
                    match it.peek().copied() {
                        Some(' ') => {
                            let _ = it.next();
                            remaining = remaining.saturating_sub(1);
                        }
                        Some('\t') => {
                            let _ = it.next();
                            remaining = remaining.saturating_sub(4);
                        }
                        _ => break,
                    }
                }
                for ch in it {
                    dedented.push(ch);
                }
            }
            s = dedented;
        }

        s
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DelimKind {
        Strong,
        Em,
    }

    fn is_punctuation(ch: char) -> bool {
        !is_ecmascript_whitespace(ch) && !ch.is_alphanumeric()
    }

    fn mermaid_delim_can_open_close(
        ch: char,
        prev: Option<char>,
        next: Option<char>,
    ) -> (bool, bool) {
        let prev_is_ws = prev.is_none_or(is_ecmascript_whitespace);
        let next_is_ws = next.is_none_or(is_ecmascript_whitespace);
        let prev_is_punct = prev.is_some_and(is_punctuation);
        let next_is_punct = next.is_some_and(is_punctuation);

        let left_flanking = !next_is_ws && (!next_is_punct || prev_is_ws || prev_is_punct);
        let right_flanking = !prev_is_ws && (!prev_is_punct || next_is_ws || next_is_punct);

        if ch == '_' {
            let can_open = left_flanking && (!right_flanking || prev_is_ws || prev_is_punct);
            let can_close = right_flanking && (!left_flanking || next_is_ws || next_is_punct);
            (can_open, can_close)
        } else {
            (left_flanking, right_flanking)
        }
    }

    fn mermaid_delim_has_closer(
        chars: &[char],
        mut i: usize,
        delimiter: char,
        run_len: usize,
    ) -> bool {
        let mut in_code_span = false;
        while i < chars.len() {
            if chars[i] == '<'
                && let Some(end) = chars[i..].iter().position(|ch| *ch == '>')
            {
                i += end + 1;
                continue;
            }
            if !in_code_span
                && chars[i] == '\\'
                && chars.get(i + 1).is_some_and(char::is_ascii_punctuation)
            {
                i += 2;
                continue;
            }
            if chars[i] == '`' {
                in_code_span = !in_code_span;
                i += 1;
                continue;
            }
            if !in_code_span && chars[i] == delimiter {
                let candidate_len = if i + 1 < chars.len() && chars[i + 1] == delimiter {
                    2
                } else {
                    1
                };
                let prev = i.checked_sub(1).map(|index| chars[index]);
                let next = chars.get(i + candidate_len).copied();
                let (_, can_close) = mermaid_delim_can_open_close(delimiter, prev, next);
                if candidate_len == run_len && can_close {
                    return true;
                }
                i += candidate_len;
                continue;
            }
            i += 1;
        }
        false
    }

    // Mermaid wraps SVG-label Markdown strings in single backticks; strip to avoid inline-code
    // suppressing `**`/`_` formatting.
    let markdown = if !preserve_inline_literals
        && markdown.len() >= 2
        && markdown.starts_with('`')
        && markdown.ends_with('`')
        && !markdown.starts_with("``")
        && !markdown.ends_with("``")
    {
        &markdown[1..markdown.len() - 1]
    } else {
        markdown
    };

    let pre = preprocess_mermaid_markdown(markdown);
    let deletions = if preserve_inline_literals {
        None
    } else {
        project_svg_markdown_deletions(&pre)
    };
    let projected_deletions = deletions.is_some();
    let pre = deletions.unwrap_or(pre);

    fn needs_full_delimiter_resolution(markdown: &str) -> bool {
        // Code spans and images are omitted by the pinned SVG token allowlist. Keep their
        // delimiter handling in the full parser instead of a second partial implementation.
        if markdown.contains("![") || markdown.contains('`') {
            return true;
        }
        let chars: Vec<char> = markdown.chars().collect();
        if chars
            .windows(2)
            .any(|pair| (pair[0] == '*' && pair[1] == '_') || (pair[0] == '_' && pair[1] == '*'))
        {
            return true;
        }

        ['*', '_'].into_iter().any(|delimiter| {
            let mut has_single = false;
            let mut has_double = false;
            let mut index = 0;
            while index < chars.len() {
                if chars[index] != delimiter {
                    index += 1;
                    continue;
                }
                let run_len = chars[index..]
                    .iter()
                    .take_while(|candidate| **candidate == delimiter)
                    .count();
                match run_len {
                    1 => has_single = true,
                    2 => has_double = true,
                    _ => return true,
                }
                if has_single && has_double {
                    return true;
                }
                index += run_len;
            }
            false
        })
    }

    fn resolve_full_delimiter_runs(
        markdown: &str,
        preserve_inline_literals: bool,
        markdown_auto_wrap: bool,
    ) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
        fn line_mut(
            out: &mut Vec<Vec<(String, MermaidMarkdownWordType)>>,
            line_idx: usize,
        ) -> &mut Vec<(String, MermaidMarkdownWordType)> {
            if out.len() <= line_idx {
                out.resize_with(line_idx + 1, Vec::new);
            }
            &mut out[line_idx]
        }

        fn append_text(
            out: &mut Vec<Vec<(String, MermaidMarkdownWordType)>>,
            line_idx: &mut usize,
            text: &str,
            word_type: MermaidMarkdownWordType,
            join_first_word: bool,
        ) {
            for (part_index, part) in text.split('\n').enumerate() {
                if part_index != 0 {
                    *line_idx += 1;
                    out.push(Vec::new());
                }
                for (word_index, word) in part.split(' ').enumerate() {
                    if word.is_empty() {
                        continue;
                    }
                    let word = word.replace("&#39;", "'");
                    let can_join = join_first_word && part_index == 0 && word_index == 0;
                    let line = line_mut(out, *line_idx);
                    if can_join
                        && let Some((previous, previous_type)) = line.last_mut()
                        && *previous_type == word_type
                    {
                        previous.push_str(&word);
                    } else {
                        line.push((word, word_type));
                    }
                }
            }
        }

        fn append_source_text(
            out: &mut Vec<Vec<(String, MermaidMarkdownWordType)>>,
            line_idx: &mut usize,
            text: &str,
            range: std::ops::Range<usize>,
            word_type: MermaidMarkdownWordType,
            previous_text: &mut Option<(usize, bool)>,
        ) {
            if text.is_empty() {
                return;
            }
            let join_first_word = previous_text
                .is_some_and(|(end, can_join)| end == range.start && can_join)
                && text
                    .chars()
                    .next()
                    .is_some_and(|ch| !is_ecmascript_whitespace(ch));
            append_text(out, line_idx, text, word_type, join_first_word);
            *previous_text = Some((
                range.end,
                text.chars()
                    .last()
                    .is_some_and(|ch| !is_ecmascript_whitespace(ch)),
            ));
        }

        let mut parser = pulldown_cmark::Parser::new_ext(
            markdown,
            pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                | pulldown_cmark::Options::ENABLE_TASKLISTS,
        )
        .into_offset_iter()
        .peekable();
        let mut out = vec![Vec::new()];
        let mut line_idx = 0;
        let mut style_stack = Vec::new();
        let mut in_paragraph = false;
        let mut skipped_inline_depth = 0usize;
        let mut skipped_block_depth = 0usize;
        let mut previous_text = None;
        let mut escape_scan_offset = 0;
        let mut escape_pending = false;

        let mut source_end = 0;
        while let Some((event, mut range)) = parser.next() {
            let previous_source_end = source_end;
            if !matches!(event, pulldown_cmark::Event::Start(_)) {
                source_end = range.end;
            }
            if skipped_block_depth > 0 {
                match event {
                    pulldown_cmark::Event::Start(_) => skipped_block_depth += 1,
                    pulldown_cmark::Event::End(_) => skipped_block_depth -= 1,
                    _ => {}
                }
                continue;
            }

            match event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {
                    in_paragraph = true;
                }
                pulldown_cmark::Event::Start(tag) if in_paragraph => {
                    if skipped_inline_depth > 0 {
                        skipped_inline_depth += 1;
                    } else {
                        match tag {
                            pulldown_cmark::Tag::Strong => {
                                style_stack.push(MermaidMarkdownWordType::Strong);
                            }
                            pulldown_cmark::Tag::Emphasis => {
                                style_stack.push(MermaidMarkdownWordType::Em);
                            }
                            _ if preserve_inline_literals => {
                                // HTML keeps unsupported inline syntax, but still formats its
                                // children. Emit the source prefix/suffix around those children.
                                let prefix_end =
                                    parser.peek().map_or(range.end, |(event, next)| {
                                        if matches!(event, pulldown_cmark::Event::End(_)) {
                                            range.end
                                        } else {
                                            next.start
                                        }
                                    });
                                let prefix = range.start..prefix_end;
                                let kind = style_stack
                                    .last()
                                    .copied()
                                    .unwrap_or(MermaidMarkdownWordType::Normal);
                                append_source_text(
                                    &mut out,
                                    &mut line_idx,
                                    &markdown[prefix.clone()],
                                    prefix,
                                    kind,
                                    &mut previous_text,
                                );
                                source_end = prefix_end;
                            }
                            _ => {
                                skipped_inline_depth = 1;
                                previous_text = None;
                            }
                        }
                    }
                }
                pulldown_cmark::Event::Start(_) => {
                    if let Some(raw) = markdown.get(range) {
                        line_mut(&mut out, line_idx)
                            .push((raw.to_string(), MermaidMarkdownWordType::Normal));
                    }
                    skipped_block_depth = 1;
                    previous_text = None;
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                    in_paragraph = false;
                    skipped_inline_depth = 0;
                    style_stack.clear();
                    previous_text = None;
                }
                pulldown_cmark::Event::End(tag) if in_paragraph => {
                    if skipped_inline_depth > 0 {
                        skipped_inline_depth -= 1;
                        if skipped_inline_depth == 0 {
                            previous_text = None;
                        }
                    } else if matches!(
                        tag,
                        pulldown_cmark::TagEnd::Strong | pulldown_cmark::TagEnd::Emphasis
                    ) {
                        let _ = style_stack.pop();
                    } else if preserve_inline_literals {
                        let suffix = previous_source_end..range.end;
                        let kind = style_stack
                            .last()
                            .copied()
                            .unwrap_or(MermaidMarkdownWordType::Normal);
                        append_source_text(
                            &mut out,
                            &mut line_idx,
                            &markdown[suffix.clone()],
                            suffix,
                            kind,
                            &mut previous_text,
                        );
                    }
                }
                pulldown_cmark::Event::Text(text) if in_paragraph && skipped_inline_depth == 0 => {
                    // Mermaid's processNode ignores Marked escape tokens. pulldown folds the
                    // escaped character into a text event whose source range starts after '\'.
                    let mut escaped_prefix = false;
                    while escape_scan_offset <= range.start {
                        let byte = markdown.as_bytes()[escape_scan_offset];
                        if escape_scan_offset == range.start {
                            escaped_prefix = escape_pending && byte.is_ascii_punctuation();
                        }
                        escape_pending = byte == b'\\' && !escape_pending;
                        escape_scan_offset += 1;
                    }
                    let text = if escaped_prefix {
                        previous_text = None;
                        range.start += 1;
                        &text[1..]
                    } else {
                        text.as_ref()
                    };
                    // pulldown-cmark eagerly synthesizes Unicode for HTML entities, while Marked
                    // keeps entity spellings in `node.text`. Preserve the source only when its
                    // full HTML-entity decode is exactly the emitted event; unrelated synthesized
                    // text (autolinks and other extensions) keeps pulldown's payload.
                    let source_text = markdown
                        .get(range.clone())
                        .filter(|raw| {
                            raw.contains('&')
                                && merman_model::text::decode_html_entities_to_unicode(raw).as_ref()
                                    == text
                        })
                        .unwrap_or(text);
                    let source_text = if markdown_auto_wrap {
                        std::borrow::Cow::Borrowed(source_text)
                    } else {
                        std::borrow::Cow::Owned(source_text.replace(' ', "&nbsp;"))
                    };
                    let word_type = style_stack
                        .last()
                        .copied()
                        .unwrap_or(MermaidMarkdownWordType::Normal);
                    append_source_text(
                        &mut out,
                        &mut line_idx,
                        &source_text,
                        range,
                        word_type,
                        &mut previous_text,
                    );
                }
                pulldown_cmark::Event::Code(_) if in_paragraph && skipped_inline_depth == 0 => {
                    if preserve_inline_literals && let Some(raw) = markdown.get(range.clone()) {
                        let kind = style_stack
                            .last()
                            .copied()
                            .unwrap_or(MermaidMarkdownWordType::Normal);
                        append_source_text(
                            &mut out,
                            &mut line_idx,
                            raw,
                            range,
                            kind,
                            &mut previous_text,
                        );
                    } else {
                        previous_text = None;
                    }
                }
                pulldown_cmark::Event::Html(html) | pulldown_cmark::Event::InlineHtml(html)
                    if skipped_inline_depth == 0 =>
                {
                    let raw = markdown.get(range).unwrap_or(html.as_ref());
                    line_mut(&mut out, line_idx)
                        .push((raw.to_string(), MermaidMarkdownWordType::Normal));
                    previous_text = None;
                }
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak
                    if in_paragraph && skipped_inline_depth == 0 =>
                {
                    line_idx += 1;
                    out.push(Vec::new());
                    previous_text = None;
                }
                pulldown_cmark::Event::Rule if skipped_inline_depth == 0 => {
                    if let Some(raw) = markdown.get(range) {
                        line_mut(&mut out, line_idx)
                            .push((raw.to_string(), MermaidMarkdownWordType::Normal));
                    }
                    previous_text = None;
                }
                _ => {}
            }
        }

        while out.last().is_some_and(|line| line.is_empty()) && out.len() > 1 {
            out.pop();
        }
        out
    }

    if projected_deletions || needs_full_delimiter_resolution(&pre) {
        return resolve_full_delimiter_runs(&pre, preserve_inline_literals, markdown_auto_wrap);
    }

    let pre = if markdown_auto_wrap {
        pre
    } else {
        pre.replace(' ', "&nbsp;")
    };
    let chars: Vec<char> = pre.chars().collect();

    let mut out: Vec<Vec<(String, MermaidMarkdownWordType)>> = vec![Vec::new()];
    let mut line_idx: usize = 0;

    let mut stack: Vec<MermaidMarkdownWordType> = vec![MermaidMarkdownWordType::Normal];
    let mut word = String::new();
    let mut word_ty = MermaidMarkdownWordType::Normal;

    fn line_mut(
        out: &mut Vec<Vec<(String, MermaidMarkdownWordType)>>,
        line_idx: usize,
    ) -> &mut Vec<(String, MermaidMarkdownWordType)> {
        if out.len() <= line_idx {
            out.resize_with(line_idx + 1, Vec::new);
        }
        &mut out[line_idx]
    }

    let flush_word = |out: &mut Vec<Vec<(String, MermaidMarkdownWordType)>>,
                      line_idx: &mut usize,
                      word: &mut String,
                      word_ty: MermaidMarkdownWordType| {
        if word.is_empty() {
            return;
        }
        let mut w = std::mem::take(word);
        if w.contains("&#39;") {
            w = w.replace("&#39;", "'");
        }
        line_mut(out, *line_idx).push((w, word_ty));
    };

    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];

        // The pinned Mermaid SVG Markdown projection omits Marked's escape nodes,
        // rather than printing their escaped punctuation as a generic Markdown renderer would.
        if ch == '\\' && chars.get(i + 1).is_some_and(char::is_ascii_punctuation) {
            flush_word(&mut out, &mut line_idx, &mut word, word_ty);
            word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
            i += 2;
            continue;
        }

        if ch == '\n' {
            flush_word(&mut out, &mut line_idx, &mut word, word_ty);
            word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
            line_idx += 1;
            out.push(Vec::new());
            i += 1;
            continue;
        }
        if ch == ' ' {
            flush_word(&mut out, &mut line_idx, &mut word, word_ty);
            word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
            i += 1;
            continue;
        }

        if ch == '<'
            && let Some(end) = chars[i..].iter().position(|c| *c == '>')
        {
            let end = i + end;
            let html: String = chars[i..=end].iter().collect();
            flush_word(&mut out, &mut line_idx, &mut word, word_ty);
            if html.eq_ignore_ascii_case("<br>")
                || html.eq_ignore_ascii_case("<br/>")
                || html.eq_ignore_ascii_case("<br />")
                || html.eq_ignore_ascii_case("</br>")
                || html.eq_ignore_ascii_case("</br/>")
                || html.eq_ignore_ascii_case("</br />")
                || html.eq_ignore_ascii_case("</br >")
            {
                word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
                line_idx += 1;
                out.push(Vec::new());
            } else {
                line_mut(&mut out, line_idx).push((html, MermaidMarkdownWordType::Normal));
                word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
            }
            i = end + 1;
            continue;
        }

        if ch == '*' || ch == '_' {
            let prev = i.checked_sub(1).map(|index| chars[index]);
            let run_len = if i + 1 < chars.len() && chars[i + 1] == ch {
                2
            } else {
                1
            };
            let kind = if run_len == 2 {
                DelimKind::Strong
            } else {
                DelimKind::Em
            };
            let next = chars.get(i + run_len).copied();
            let (can_open, can_close) = mermaid_delim_can_open_close(ch, prev, next);

            let want_ty = match kind {
                DelimKind::Strong => MermaidMarkdownWordType::Strong,
                DelimKind::Em => MermaidMarkdownWordType::Em,
            };
            let cur_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);

            if can_close && cur_ty == want_ty {
                flush_word(&mut out, &mut line_idx, &mut word, word_ty);
                stack.pop();
                word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
                i += run_len;
                continue;
            }
            if can_open && mermaid_delim_has_closer(&chars, i + run_len, ch, run_len) {
                flush_word(&mut out, &mut line_idx, &mut word, word_ty);
                stack.push(want_ty);
                word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
                i += run_len;
                continue;
            }

            // Treat the delimiter run as literal if it can't open/close. Mermaid's upstream
            // behavior does not reinterpret a failed `__` run as two separate `_` runs (e.g.
            // `a__b` must remain literal, not split into `a_` + `_b_`).
            if word.is_empty() {
                word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
            }
            for _ in 0..run_len {
                word.push(ch);
            }
            i += run_len;
            continue;
        }

        if word.is_empty() {
            word_ty = *stack.last().unwrap_or(&MermaidMarkdownWordType::Normal);
        }
        word.push(ch);
        i += 1;
    }

    flush_word(&mut out, &mut line_idx, &mut word, word_ty);

    if out.is_empty() {
        out.push(Vec::new());
    }
    while out.last().is_some_and(|l| l.is_empty()) && out.len() > 1 {
        out.pop();
    }
    out
}

pub(crate) fn mermaid_markdown_contains_html_tags(markdown: &str) -> bool {
    pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .any(|ev| {
        matches!(
            ev,
            pulldown_cmark::Event::Html(_) | pulldown_cmark::Event::InlineHtml(_)
        )
    })
}

pub(crate) fn mermaid_markdown_is_plain_text(markdown: &str) -> bool {
    pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .all(|event| {
        matches!(
            event,
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph)
                | pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph)
                | pulldown_cmark::Event::Text(_)
                | pulldown_cmark::Event::SoftBreak
                | pulldown_cmark::Event::HardBreak
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_markdown_omits_deletion_tokens_without_converting_entity_generics() {
        use MermaidMarkdownWordType::Normal;
        for deleted in ["~T~", "~~T~~"] {
            assert_eq!(
                mermaid_markdown_to_lines(&format!("<b>WWWW</b> List{deleted}"), true),
                vec![vec![
                    ("<b>".into(), Normal),
                    ("WWWW".into(), Normal),
                    ("</b>".into(), Normal),
                    ("List".into(), Normal),
                ]],
            );
        }
        assert_eq!(
            mermaid_markdown_to_lines("List~T", true),
            vec![vec![("List~T".into(), Normal)]],
        );
    }

    #[test]
    fn single_tilde_projection_respects_inline_styles_and_literal_contexts() {
        use MermaidMarkdownWordType::{Em, Normal, Strong};
        for (source, expected) in [
            ("List~**T**~", vec![("List", Normal)]),
            ("**List~T~**", vec![("List", Strong)]),
            ("List~T~Body~U~", vec![("List", Normal), ("Body", Normal)]),
            ("List~T~~U~", vec![("List", Normal)]),
            ("List~T", vec![("List~T", Normal)]),
            ("List~ T~", vec![("List~", Normal), ("T~", Normal)]),
            ("List~T ~", vec![("List~T", Normal), ("~", Normal)]),
            (r"List\~T~", vec![("List", Normal), ("T~", Normal)]),
            (r"List~T\~", vec![("List~T", Normal)]),
            (r"List~T\\~", vec![("List", Normal)]),
            (
                "Before `List~T~` After",
                vec![("Before", Normal), ("After", Normal)],
            ),
            (
                "<span title='List~T~'>Label</span>",
                vec![
                    ("<span title='List~T~'>", Normal),
                    ("Label", Normal),
                    ("</span>", Normal),
                ],
            ),
            ("*List~T* Tail~", vec![("List~T", Em), ("Tail~", Normal)]),
        ] {
            assert_eq!(
                mermaid_markdown_to_lines(source, true),
                vec![
                    expected
                        .into_iter()
                        .map(|(word, kind)| (word.into(), kind))
                        .collect::<Vec<_>>()
                ],
                "{source}"
            );
        }
        assert_eq!(
            html_markdown_measurement_lines("List~T~"),
            vec![vec![("List~T~".into(), MermaidMarkdownWordType::Normal)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("~ T~", false),
            vec![vec![("~&nbsp;T~".into(), Normal)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("List~T~", false),
            vec![vec![("List".into(), Normal)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("A~B~ C~~ D~~", false),
            vec![vec![
                ("A".into(), Normal),
                ("&nbsp;C~~&nbsp;D~~".into(), Normal),
            ]],
        );
    }

    #[test]
    fn html_inline_literal_projection_keeps_child_styles_and_code_adjacency() {
        use MermaidMarkdownWordType::*;
        assert_eq!(
            html_markdown_measurement_lines("**pre`code`post**"),
            vec![vec![("pre`code`post".to_owned(), Strong)]],
        );
        assert_eq!(
            html_markdown_measurement_lines("`x` [**bold** label](u)"),
            vec![vec![
                ("`x`".to_owned(), Normal),
                ("[".to_owned(), Normal),
                ("bold".to_owned(), Strong),
                ("label](u)".to_owned(), Normal),
            ]],
        );
    }

    #[test]
    fn svg_markdown_omits_marked_escape_nodes_in_plain_and_nested_delimiters() {
        use MermaidMarkdownWordType::{Em, Normal, Strong};
        for (markdown, expected) in [
            (
                r"\longfieldname",
                vec![vec![(r"\longfieldname".into(), Normal)]],
            ),
            (
                r"\\longfieldname",
                vec![vec![("longfieldname".into(), Normal)]],
            ),
            (
                r"\\\longfieldname",
                vec![vec![(r"\longfieldname".into(), Normal)]],
            ),
            (
                r"a\\b",
                vec![vec![("a".into(), Normal), ("b".into(), Normal)]],
            ),
            (
                r"before \*bold\* end",
                vec![vec![
                    ("before".into(), Normal),
                    ("bold".into(), Normal),
                    ("end".into(), Normal),
                ]],
            ),
            (
                r"**_styled_ \#tag &amp; value**",
                vec![vec![
                    ("styled".into(), Em),
                    ("tag".into(), Strong),
                    ("&amp;".into(), Strong),
                    ("value".into(), Strong),
                ]],
            ),
        ] {
            assert_eq!(
                mermaid_markdown_to_lines(markdown, true),
                expected,
                "{markdown}"
            );
        }
    }

    #[test]
    fn underscore_delimiters_match_mermaid() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("`a__b`", true),
            vec![vec![("a__b".to_string(), Normal)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`_a_b_`", true),
            vec![vec![("a_b".to_string(), Em)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`_a__b_`", true),
            vec![vec![("a__b".to_string(), Em)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`__a__`", true),
            vec![vec![("a".to_string(), Strong)]]
        );
        // Marked 16.3 tokenizes this as three nested `em` nodes. Mermaid passes
        // the innermost parent type to the text token, so the effective word is `Em`.
        assert_eq!(
            mermaid_markdown_to_lines("`*_*word*_*`", true),
            vec![vec![("word".to_string(), Em)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`***word***`", true),
            vec![vec![("word".to_string(), Strong)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`___word___`", true),
            vec![vec![("word".to_string(), Strong)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**_word_**`", true),
            vec![vec![("word".to_string(), Em)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`_*word*_`", true),
            vec![vec![("word".to_string(), Em)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`***foo* bar**`", true),
            vec![vec![("foo".to_string(), Em), ("bar".to_string(), Strong),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`***foo** bar*`", true),
            vec![vec![("foo".to_string(), Strong), ("bar".to_string(), Em),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**foo***`", true),
            vec![vec![("foo".to_string(), Strong), ("*".to_string(), Normal),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`___foo_ bar__`", true),
            vec![vec![("foo".to_string(), Em), ("bar".to_string(), Strong),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**foo*`", true),
            vec![vec![("*".to_string(), Normal), ("foo".to_string(), Em),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`*foo**`", true),
            vec![vec![("foo".to_string(), Em), ("*".to_string(), Normal),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`__foo_`", true),
            vec![vec![("_".to_string(), Normal), ("foo".to_string(), Em),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`_foo__`", true),
            vec![vec![("foo".to_string(), Em), ("_".to_string(), Normal),]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**CoreResult<T>**`", true),
            vec![vec![
                ("CoreResult".to_string(), Strong),
                ("<T>".to_string(), Normal),
            ]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**CoreResult&lt;T&gt;**`", true),
            vec![vec![("CoreResult&lt;T&gt;".to_string(), Strong)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines(r"`**_styled_ \#tag &amp; value**`", true),
            vec![vec![
                ("styled".to_string(), Em),
                ("tag".to_string(), Strong),
                ("&amp;".to_string(), Strong),
                ("value".to_string(), Strong),
            ]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`**CoreResult~T~**`", true),
            vec![vec![("CoreResult".to_string(), Strong)]]
        );
    }

    #[test]
    fn full_delimiter_fallback_preserves_marked_entity_spelling() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("***&copy;***", true),
            vec![vec![("&copy;".to_string(), Strong)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("**_x_ &nbsp; y**", true),
            vec![vec![
                ("x".to_string(), Em),
                ("&nbsp;".to_string(), Strong),
                ("y".to_string(), Strong),
            ]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***&#160;***", true),
            vec![vec![("&#160;".to_string(), Strong)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***x&nbsp;y***", true),
            vec![vec![("x&nbsp;y".to_string(), Strong)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***&NotEqualTilde;***", true),
            vec![vec![("&NotEqualTilde;".to_string(), Strong)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***&#39;***", true),
            vec![vec![("'".to_string(), Strong)]],
        );
    }

    #[test]
    fn svg_markdown_code_spans_match_the_full_parser_projection() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("inline: `**not bold**`", true),
            vec![vec![("inline:".to_string(), Normal)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`a ``x`` b`", true),
            vec![vec![("a".to_string(), Normal), ("b".to_string(), Normal)]]
        );
        assert_eq!(
            mermaid_markdown_to_lines("`a ``x`` ***b***`", true),
            vec![vec![("a".to_string(), Normal), ("b".to_string(), Strong)]]
        );
    }

    #[test]
    fn full_delimiter_fallback_matches_mermaid_inline_token_allowlist() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("***foo*** [bar](u)", true),
            vec![vec![("foo".to_string(), Strong)]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***foo*** ~~bar~~ baz", true),
            vec![vec![
                ("foo".to_string(), Strong),
                ("baz".to_string(), Normal),
            ]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***foo*** [bar **baz** `qux` <i>zap</i>](u) tail", true,),
            vec![vec![
                ("foo".to_string(), Strong),
                ("tail".to_string(), Normal),
            ]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("***foo*** `bar` <i>baz</i>", true),
            vec![vec![
                ("foo".to_string(), Strong),
                ("<i>".to_string(), Normal),
                ("baz".to_string(), Normal),
                ("</i>".to_string(), Normal),
            ]],
        );
    }

    #[test]
    fn full_delimiter_fallback_preserves_unsupported_block_raw_text() {
        use MermaidMarkdownWordType::*;

        for (markdown, raw) in [
            ("# ***foo***", "# ***foo***"),
            ("- ***foo***\n- bar", "- ***foo***\n- bar"),
            ("> ***foo***", "> ***foo***"),
            ("```text\n***foo***\n```", "```text\n***foo***\n```"),
            ("<div>\n***foo***\n</div>", "<div>\n***foo***\n</div>"),
            (
                "| label |\n| --- |\n| ***foo*** |",
                "| label |\n| --- |\n| ***foo*** |",
            ),
        ] {
            assert_eq!(
                mermaid_markdown_to_lines(markdown, true),
                vec![vec![(raw.to_string(), Normal)]],
                "markdown={markdown:?}",
            );
        }
    }

    #[test]
    fn full_delimiter_fallback_preserves_rule_as_one_top_level_raw_token() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("***foo***\n- - -", true),
            vec![vec![
                ("foo".to_string(), Strong),
                ("- - -".to_string(), Normal),
            ]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("# ***foo***\n- - -", true),
            vec![vec![
                ("# ***foo***\n".to_string(), Normal),
                ("- - -".to_string(), Normal),
            ]],
        );
    }

    #[test]
    fn full_delimiter_fallback_keeps_nested_inline_styles_inside_paragraphs() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("***foo _bar_*** baz", true),
            vec![vec![
                ("foo".to_string(), Strong),
                ("bar".to_string(), Em),
                ("baz".to_string(), Normal),
            ]],
        );
        assert_eq!(
            mermaid_markdown_to_lines("# ***foo***\n***bar*** baz", true),
            vec![vec![
                ("# ***foo***\n".to_string(), Normal),
                ("bar".to_string(), Strong),
                ("baz".to_string(), Normal),
            ]],
        );
    }

    #[test]
    fn html_tags_after_newline_stay_on_current_markdown_line() {
        use MermaidMarkdownWordType::*;

        assert_eq!(
            mermaid_markdown_to_lines("alpha\n<strong>bravo</strong>", true),
            vec![
                vec![("alpha".to_string(), Normal)],
                vec![
                    ("<strong>".to_string(), Normal),
                    ("bravo".to_string(), Normal),
                    ("</strong>".to_string(), Normal),
                ],
            ]
        );
    }

    #[test]
    fn svg_analysis_distinguishes_literal_markers_from_styled_runs() {
        for literal in [
            "driver_license",
            "*unclosed",
            "literal ` backtick",
            "`code`",
        ] {
            let analysis = analyze_mermaid_markdown(literal, true);
            assert!(
                analysis.all_runs_normal(),
                "literal={literal:?}, analysis={analysis:?}"
            );
            assert_eq!(analysis.line_count, 1, "literal={literal:?}");
        }

        for styled in ["*emphasis*", "__strong__"] {
            let analysis = analyze_mermaid_markdown(styled, true);
            assert!(
                analysis.has_styled_runs,
                "styled={styled:?}, analysis={analysis:?}"
            );
            assert_eq!(analysis.line_count, 1, "styled={styled:?}");
        }

        let multiline = analyze_mermaid_markdown("first<br/>second", true);
        assert_eq!(multiline.line_count, 2);
        assert!(multiline.all_runs_normal());
    }

    #[test]
    fn plain_text_analysis_rejects_semantic_markdown_constructs() {
        for plain in [
            "Lane",
            "driver_license",
            "first\nsecond",
            "literal * marker",
        ] {
            assert!(mermaid_markdown_is_plain_text(plain), "plain={plain:?}");
        }
        for structured in [
            "**Lane**",
            "[Lane](https://example.invalid)",
            "![Lane](lane.png)",
            "`Lane`",
            "<b>Lane</b>",
            "- Lane",
        ] {
            assert!(
                !mermaid_markdown_is_plain_text(structured),
                "structured={structured:?}"
            );
        }
    }

    #[test]
    fn svg_markdown_images_skip_alt_text_and_resume_following_styles() {
        use MermaidMarkdownWordType::*;
        assert_eq!(
            mermaid_markdown_to_lines("![alt](image.png)", true),
            vec![vec![]]
        );
        assert_eq!(
            mermaid_markdown_to_lines(
                "Before ![alt **ignored**](image.png) **after** *tail*",
                true
            ),
            vec![vec![
                ("Before".to_string(), Normal),
                ("after".to_string(), Strong),
                ("tail".to_string(), Em),
            ]]
        );
        assert_eq!(
            mermaid_markdown_to_lines(
                r"Before \*literal\* ![ignored](image.png) **after** \#tail",
                true
            ),
            vec![vec![
                ("Before".to_string(), Normal),
                ("literal".to_string(), Normal),
                ("after".to_string(), Strong),
                ("tail".to_string(), Normal),
            ]]
        );
    }
}
