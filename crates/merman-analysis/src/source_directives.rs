#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn directive_keyword_spans(source: &str) -> Vec<ByteSpan> {
    directive_body_spans(source)
        .into_iter()
        .filter_map(|body| directive_keyword_span(source, body.start, body.end))
        .collect()
}

pub(crate) fn init_directive_config_key_spans(
    source: &str,
    matching_paths: &[&[&str]],
) -> Vec<ByteSpan> {
    directive_body_spans(source)
        .into_iter()
        .flat_map(|body| {
            let mut scanner =
                DirectiveConfigScanner::new(source, body.start, body.end, matching_paths);
            scanner.matching_config_key_spans()
        })
        .collect()
}

fn directive_body_spans(source: &str) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = source[cursor..].find("%%{") {
        let directive_start = cursor + relative_start;
        let body_start = directive_start + "%%{".len();
        let Some(body_end) = find_directive_body_end(source, body_start) else {
            break;
        };
        spans.push(ByteSpan {
            start: body_start,
            end: body_end,
        });
        cursor = body_end + "}%%".len();
    }

    spans
}

fn find_directive_body_end(source: &str, body_start: usize) -> Option<usize> {
    let mut cursor = body_start;
    let mut quote = None;
    let mut escaped = false;

    while cursor < source.len() {
        let ch = source[cursor..].chars().next()?;
        let next = cursor + ch.len_utf8();

        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            cursor = next;
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '}' if source[next..].starts_with("%%") => return Some(cursor),
            _ => {}
        }

        cursor = next;
    }

    None
}

fn directive_keyword_span(source: &str, body_start: usize, body_end: usize) -> Option<ByteSpan> {
    let body = source.get(body_start..body_end)?;
    let leading = body
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
        .unwrap_or(body.len());
    let keyword_start = body_start + leading;
    let tail = source.get(keyword_start..body_end)?;
    let keyword_len = tail
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_ascii_alphabetic() && ch != '_').then_some(idx))
        .unwrap_or(tail.len());
    if keyword_len == 0 {
        return None;
    }

    let keyword_end = keyword_start + keyword_len;
    let after_keyword = source.get(keyword_end..body_end)?.trim_start();
    if after_keyword.is_empty()
        || after_keyword
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, ':' | '{'))
    {
        Some(ByteSpan {
            start: keyword_start,
            end: keyword_end,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConfigKeySpan<'a> {
    name: &'a str,
    span: ByteSpan,
}

struct DirectiveConfigScanner<'source, 'query> {
    source: &'source str,
    body_end: usize,
    pos: usize,
    matching_paths: &'query [&'query [&'query str]],
}

impl<'source, 'query> DirectiveConfigScanner<'source, 'query> {
    fn new(
        source: &'source str,
        body_start: usize,
        body_end: usize,
        matching_paths: &'query [&'query [&'query str]],
    ) -> Self {
        Self {
            source,
            body_end,
            pos: body_start,
            matching_paths,
        }
    }

    fn matching_config_key_spans(&mut self) -> Vec<ByteSpan> {
        let mut spans = Vec::new();

        while self.pos < self.body_end {
            self.skip_ws_and_commas();
            let Some(key) = self.parse_key() else {
                break;
            };
            self.skip_ws();
            if self.next_char() != Some(':') {
                break;
            }
            self.skip_ws();
            if matches!(key.name, "init" | "initialize") {
                let mut path = Vec::new();
                self.collect_value_spans(&mut path, &mut spans);
            } else {
                self.skip_value();
            }
        }

        spans
    }

    fn collect_value_spans(&mut self, path: &mut Vec<&'source str>, spans: &mut Vec<ByteSpan>) {
        self.skip_ws();
        if self.peek_char() != Some('{') {
            self.skip_value();
            return;
        }
        self.next_char();
        self.collect_object_entries(path, spans);
    }

    fn collect_object_entries(&mut self, path: &mut Vec<&'source str>, spans: &mut Vec<ByteSpan>) {
        loop {
            self.skip_ws_and_commas();
            match self.peek_char() {
                Some('}') => {
                    self.next_char();
                    return;
                }
                Some(_) => {}
                None => return,
            }

            let Some(key) = self.parse_key() else {
                return;
            };
            self.skip_ws();
            if self.next_char() != Some(':') {
                return;
            }

            if self.matches_path(path, key.name) {
                spans.push(key.span);
            }

            path.push(key.name);
            self.collect_value_spans(path, spans);
            path.pop();
        }
    }

    fn matches_path(&self, parents: &[&str], key_name: &str) -> bool {
        self.matching_paths.iter().any(|target| {
            target.len() == parents.len() + 1
                && target[..parents.len()] == *parents
                && target[parents.len()] == key_name
        })
    }

    fn parse_key(&mut self) -> Option<ConfigKeySpan<'source>> {
        self.skip_ws();
        match self.peek_char()? {
            '"' | '\'' => self.parse_quoted_key(),
            '}' | ']' => None,
            _ => self.parse_bare_key(),
        }
    }

    fn parse_quoted_key(&mut self) -> Option<ConfigKeySpan<'source>> {
        let quote = self.next_char()?;
        let start = self.pos;
        let mut escaped = false;

        while self.pos < self.body_end {
            let ch = self.next_char()?;
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                let end = self.pos - quote.len_utf8();
                let name = self.source.get(start..end)?;
                return Some(ConfigKeySpan {
                    name,
                    span: ByteSpan { start, end },
                });
            }
        }

        None
    }

    fn parse_bare_key(&mut self) -> Option<ConfigKeySpan<'source>> {
        let raw_start = self.pos;
        while let Some(ch) = self.peek_char() {
            if matches!(ch, ':' | '\n' | '\r' | '}' | ']') {
                break;
            }
            self.next_char();
        }

        let raw_end = self.pos;
        let raw = self.source.get(raw_start..raw_end)?;
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        Some(ConfigKeySpan {
            name: trimmed,
            span: ByteSpan {
                start: raw_start + leading,
                end: raw_start + leading + trimmed.len(),
            },
        })
    }

    fn skip_value(&mut self) {
        self.skip_ws();
        match self.peek_char() {
            Some('{') => self.skip_balanced('{', '}'),
            Some('[') => self.skip_balanced('[', ']'),
            Some('"') | Some('\'') => self.skip_quoted(),
            Some(_) => {
                while let Some(ch) = self.peek_char() {
                    if matches!(ch, ',' | '\n' | '\r' | '}' | ']') {
                        break;
                    }
                    self.next_char();
                }
            }
            None => {}
        }
    }

    fn skip_balanced(&mut self, open: char, close: char) {
        if self.next_char() != Some(open) {
            return;
        }
        let mut depth = 1usize;
        while self.pos < self.body_end && depth > 0 {
            match self.peek_char() {
                Some('"') | Some('\'') => self.skip_quoted(),
                Some(ch) if ch == open => {
                    self.next_char();
                    depth += 1;
                }
                Some(ch) if ch == close => {
                    self.next_char();
                    depth -= 1;
                }
                Some(_) => {
                    self.next_char();
                }
                None => return,
            }
        }
    }

    fn skip_quoted(&mut self) {
        let Some(quote @ ('"' | '\'')) = self.next_char() else {
            return;
        };
        let mut escaped = false;
        while self.pos < self.body_end {
            let Some(ch) = self.next_char() else {
                return;
            };
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                return;
            }
        }
    }

    fn skip_ws(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.next_char();
        }
    }

    fn skip_ws_and_commas(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_whitespace() || ch == ',')
        {
            self.next_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos >= self.body_end {
            None
        } else {
            self.source[self.pos..self.body_end].chars().next()
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_LABEL_PATHS: [&[&str]; 2] = [
        &["flowchart", "htmlLabels"],
        &["config", "flowchart", "htmlLabels"],
    ];

    #[test]
    fn directive_keyword_spans_ignore_unterminated_directives() {
        assert!(directive_keyword_spans("%%{ initialize: {\"theme\":\"dark\"}").is_empty());
    }

    #[test]
    fn directive_keyword_spans_find_init_alias() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\n";

        let spans = directive_keyword_spans(source);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "initialize");
    }

    #[test]
    fn init_directive_config_key_spans_ignore_closing_marker_inside_strings() {
        let source = "%%{ init: { \"themeCSS\": \"}%%\", \"flowchart\": { \"htmlLabels\": true } } }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_match_config_wrapper_path() {
        let source =
            "%%{ initialize: { config: { flowchart: { htmlLabels: false } } } }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_skip_root_keys_and_non_init_directives() {
        let source = "%%{ init: { htmlLabels: false, flowchart: { curve: \"linear\" } } }%%\n%%{ other: { flowchart: { htmlLabels: true } } }%%\nflowchart TD\n";

        assert!(init_directive_config_key_spans(source, &HTML_LABEL_PATHS).is_empty());
    }
}
