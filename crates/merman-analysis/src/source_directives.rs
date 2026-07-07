#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InitDirectiveSpan {
    pub(crate) full: ByteSpan,
    pub(crate) keyword: ByteSpan,
}

pub(crate) fn frontmatter_config_key_spans(
    source: &str,
    matching_paths: &[&[&str]],
) -> Vec<ByteSpan> {
    let Some(frontmatter) = merman_core::preprocess::split_frontmatter_block(source) else {
        return Vec::new();
    };

    FrontmatterConfigScanner::new(
        source,
        frontmatter.body.start,
        frontmatter.body.end,
        frontmatter.indent,
        matching_paths,
    )
    .matching_config_key_spans()
}

pub(crate) fn directive_keyword_spans(source: &str) -> Vec<ByteSpan> {
    directive_spans(source)
        .into_iter()
        .filter_map(|directive| {
            directive_keyword_span(source, directive.body.start, directive.body.end)
        })
        .collect()
}

pub(crate) fn init_directive_spans(source: &str) -> Vec<InitDirectiveSpan> {
    directive_spans(source)
        .into_iter()
        .filter_map(|directive| {
            let keyword = directive_keyword_span(source, directive.body.start, directive.body.end)?;
            matches!(
                source.get(keyword.start..keyword.end),
                Some("init" | "initialize")
            )
            .then_some(InitDirectiveSpan {
                full: directive.full,
                keyword,
            })
        })
        .collect()
}

pub(crate) fn init_directive_config_key_spans(
    source: &str,
    matching_paths: &[&[&str]],
) -> Vec<ByteSpan> {
    directive_spans(source)
        .into_iter()
        .filter_map(|directive| {
            init_directive_value_span(source, directive.body.start, directive.body.end)
        })
        .flat_map(|value| {
            let mut scanner =
                DirectiveConfigScanner::new(source, value.start, value.end, matching_paths)
                    .with_comment_mode(ConfigCommentMode::Json5);
            scanner.matching_config_value_key_spans()
        })
        .collect()
}

fn init_directive_value_span(source: &str, body_start: usize, body_end: usize) -> Option<ByteSpan> {
    let keyword = directive_keyword_span(source, body_start, body_end)?;
    if !matches!(
        source.get(keyword.start..keyword.end),
        Some("init" | "initialize")
    ) {
        return None;
    }

    let mut pos = keyword.end;
    while pos < body_end {
        let ch = source[pos..body_end].chars().next()?;
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }

    if source[pos..body_end].chars().next()? != ':' {
        return None;
    }

    Some(ByteSpan {
        start: pos + ':'.len_utf8(),
        end: body_end,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectiveSpan {
    full: ByteSpan,
    body: ByteSpan,
}

fn directive_spans(source: &str) -> Vec<DirectiveSpan> {
    let mut spans = Vec::new();
    let mut cursor = merman_core::preprocess::split_frontmatter_block(source)
        .map_or(0, |frontmatter| frontmatter.full.end);

    while let Some(relative_start) = source[cursor..].find("%%{") {
        let directive_start = cursor + relative_start;
        let body_start = directive_start + "%%{".len();
        let Some(body_end) = find_directive_body_end(source, body_start) else {
            break;
        };
        let full_end = body_end + "}%%".len();
        spans.push(DirectiveSpan {
            full: ByteSpan {
                start: directive_start,
                end: full_end,
            },
            body: ByteSpan {
                start: body_start,
                end: body_end,
            },
        });
        cursor = full_end;
    }

    spans
}

fn find_directive_body_end(source: &str, body_start: usize) -> Option<usize> {
    source[body_start..]
        .find("}%%")
        .map(|relative| body_start + relative)
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
    if after_keyword.is_empty() || after_keyword.chars().next().is_some_and(|ch| ch == ':') {
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
    comment_mode: ConfigCommentMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigCommentMode {
    None,
    Json5,
    Yaml,
}

fn starts_yaml_comment(source: &str, pos: usize, lower_bound: usize) -> bool {
    if source.as_bytes().get(pos) != Some(&b'#') {
        return false;
    }
    if pos <= lower_bound {
        return true;
    }
    source[..pos]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '{' | '[' | ','))
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
            comment_mode: ConfigCommentMode::None,
        }
    }

    fn with_comment_mode(mut self, comment_mode: ConfigCommentMode) -> Self {
        self.comment_mode = comment_mode;
        self
    }

    fn matching_config_value_key_spans(&mut self) -> Vec<ByteSpan> {
        let mut spans = Vec::new();
        let mut path = Vec::new();
        self.collect_value_spans(&mut path, &mut spans);
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
            if self.starts_comment() {
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

        let span = ConfigKeySpan {
            name: trimmed,
            span: ByteSpan {
                start: raw_start + leading,
                end: raw_start + leading + trimmed.len(),
            },
        };
        self.skip_ws();
        Some(span)
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
                    if self.starts_comment() && self.skip_comment() {
                        continue;
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
                Some(_) if self.starts_comment() && self.skip_comment() => {}
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
        loop {
            let mut advanced = false;
            while self.peek_char().is_some_and(char::is_whitespace) {
                self.next_char();
                advanced = true;
            }
            if self.skip_comment() {
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
    }

    fn skip_ws_and_commas(&mut self) {
        loop {
            let mut advanced = false;
            while self
                .peek_char()
                .is_some_and(|ch| ch.is_whitespace() || ch == ',')
            {
                self.next_char();
                advanced = true;
            }
            if self.skip_comment() {
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
    }

    fn skip_comment(&mut self) -> bool {
        let Some(tail) = self.source.get(self.pos..self.body_end) else {
            return false;
        };
        if self.comment_mode == ConfigCommentMode::Yaml {
            if !tail.starts_with('#') || !starts_yaml_comment(self.source, self.pos, 0) {
                return false;
            }
            self.pos += '#'.len_utf8();
            while let Some(ch) = self.peek_char() {
                if matches!(ch, '\n' | '\r') {
                    break;
                }
                self.next_char();
            }
            return true;
        }
        if self.comment_mode != ConfigCommentMode::Json5 {
            return false;
        }
        if tail.starts_with("//") {
            self.pos += 2;
            while let Some(ch) = self.peek_char() {
                if matches!(ch, '\n' | '\r') {
                    break;
                }
                self.next_char();
            }
            return true;
        }
        if tail.starts_with("/*") {
            self.pos += 2;
            while self.pos < self.body_end {
                let Some(tail) = self.source.get(self.pos..self.body_end) else {
                    self.pos = self.body_end;
                    return true;
                };
                if tail.starts_with("*/") {
                    self.pos += 2;
                    return true;
                }
                self.next_char();
            }
            return true;
        }
        false
    }

    fn starts_comment(&self) -> bool {
        match self.comment_mode {
            ConfigCommentMode::None => false,
            ConfigCommentMode::Json5 => self
                .source
                .get(self.pos..self.body_end)
                .is_some_and(|tail| tail.starts_with("//") || tail.starts_with("/*")),
            ConfigCommentMode::Yaml => {
                self.source
                    .get(self.pos..self.body_end)
                    .is_some_and(|tail| tail.starts_with('#'))
                    && starts_yaml_comment(self.source, self.pos, 0)
            }
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

struct FrontmatterConfigScanner<'source, 'query> {
    source: &'source str,
    body_start: usize,
    body_end: usize,
    indent: &'source str,
    matching_paths: &'query [&'query [&'query str]],
}

#[derive(Debug, Default)]
struct FrontmatterLineScan {
    block_scalar_indent: Option<usize>,
    skip_until: Option<usize>,
}

impl<'source, 'query> FrontmatterConfigScanner<'source, 'query> {
    fn new(
        source: &'source str,
        body_start: usize,
        body_end: usize,
        indent: &'source str,
        matching_paths: &'query [&'query [&'query str]],
    ) -> Self {
        Self {
            source,
            body_start,
            body_end,
            indent,
            matching_paths,
        }
    }

    fn matching_config_key_spans(&self) -> Vec<ByteSpan> {
        let mut spans = Vec::new();
        let mut stack: Vec<(usize, &'source str)> = Vec::new();
        let mut line_start = self.body_start;
        let mut block_scalar_indent = None;

        while line_start < self.body_end {
            let line_end_with_newline = self.source[line_start..self.body_end]
                .find('\n')
                .map_or(self.body_end, |relative| line_start + relative + 1);
            let line_end = self.trim_line_end(line_start, line_end_with_newline);
            if let Some(scalar_indent) = block_scalar_indent {
                if self
                    .source
                    .get(line_start..line_end)
                    .is_some_and(|line| line.trim().is_empty())
                {
                    line_start = line_end_with_newline;
                    continue;
                }
                if self
                    .logical_line_indent(line_start, line_end)
                    .is_some_and(|indent| indent > scalar_indent)
                {
                    line_start = line_end_with_newline;
                    continue;
                }
                block_scalar_indent = None;
            }
            let line_scan =
                self.collect_line_key_span(line_start, line_end, &mut stack, &mut spans);
            if let Some(scalar_indent) = line_scan.block_scalar_indent {
                block_scalar_indent = Some(scalar_indent);
            }
            line_start = line_scan.skip_until.unwrap_or(line_end_with_newline);
        }

        spans
    }

    fn collect_line_key_span(
        &self,
        line_start: usize,
        line_end: usize,
        stack: &mut Vec<(usize, &'source str)>,
        spans: &mut Vec<ByteSpan>,
    ) -> FrontmatterLineScan {
        if line_start >= line_end {
            return FrontmatterLineScan::default();
        }

        let Some(line) = self.source.get(line_start..line_end) else {
            return FrontmatterLineScan::default();
        };
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            return FrontmatterLineScan::default();
        }
        if !self.indent.is_empty() && !line.starts_with(self.indent) {
            return FrontmatterLineScan::default();
        }

        let logical_start = line_start + self.indent.len();
        let logical = &self.source[logical_start..line_end];
        let content_offset = logical
            .as_bytes()
            .iter()
            .position(|byte| *byte != b' ')
            .unwrap_or(logical.len());
        let indent = content_offset;
        let content_start = logical_start + content_offset;
        if self.source[content_start..line_end].starts_with('#') {
            return FrontmatterLineScan::default();
        }

        let Some((key, value_start)) = self.parse_line_key(content_start, line_end) else {
            return FrontmatterLineScan::default();
        };
        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
        }

        let parents = stack.iter().map(|(_, name)| *name).collect::<Vec<_>>();
        if self.matches_path(&parents, key.name) {
            spans.push(key.span);
        }

        let mut line_scan = FrontmatterLineScan::default();
        if let Some(flow_span) = self.flow_mapping_span(value_start) {
            let mut inline_path = parents;
            inline_path.push(key.name);
            let mut scanner = DirectiveConfigScanner::new(
                self.source,
                flow_span.start,
                flow_span.end,
                self.matching_paths,
            )
            .with_comment_mode(ConfigCommentMode::Yaml);
            scanner.collect_value_spans(&mut inline_path, spans);
            line_scan.skip_until = Some(self.line_start_after_offset(flow_span.end));
        }

        if self.value_starts_block_scalar(value_start, line_end) {
            line_scan.block_scalar_indent = Some(indent);
            return line_scan;
        }

        if self.value_starts_mapping(value_start, line_end) {
            stack.push((indent, key.name));
        }

        line_scan
    }

    fn parse_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Option<(ConfigKeySpan<'source>, usize)> {
        match self.source[content_start..line_end].chars().next()? {
            '"' | '\'' => self.parse_quoted_line_key(content_start, line_end),
            '-' => None,
            _ => self.parse_bare_line_key(content_start, line_end),
        }
    }

    fn parse_quoted_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Option<(ConfigKeySpan<'source>, usize)> {
        let quote = self.source[content_start..line_end].chars().next()?;
        let name_start = content_start + quote.len_utf8();
        let mut pos = name_start;
        let mut escaped = false;

        while pos < line_end {
            let ch = self.source[pos..line_end].chars().next()?;
            let next = pos + ch.len_utf8();
            if escaped {
                escaped = false;
                pos = next;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                pos = next;
                continue;
            }
            if ch == quote {
                let name = self.source.get(name_start..pos)?;
                let colon = self.colon_after_key(next, line_end)?;
                return Some((
                    ConfigKeySpan {
                        name,
                        span: ByteSpan {
                            start: name_start,
                            end: pos,
                        },
                    },
                    colon + 1,
                ));
            }
            pos = next;
        }

        None
    }

    fn parse_bare_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Option<(ConfigKeySpan<'source>, usize)> {
        let colon = self.source[content_start..line_end].find(':')? + content_start;
        let raw = self.source.get(content_start..colon)?;
        let trimmed_end = raw.trim_end().len();
        let name = raw.get(..trimmed_end)?;
        if name.is_empty() {
            return None;
        }

        Some((
            ConfigKeySpan {
                name,
                span: ByteSpan {
                    start: content_start,
                    end: content_start + name.len(),
                },
            },
            colon + 1,
        ))
    }

    fn colon_after_key(&self, mut pos: usize, line_end: usize) -> Option<usize> {
        while pos < line_end {
            let ch = self.source[pos..line_end].chars().next()?;
            if ch == ':' {
                return Some(pos);
            }
            if !ch.is_whitespace() {
                return None;
            }
            pos += ch.len_utf8();
        }
        None
    }

    fn value_starts_mapping(&self, value_start: usize, line_end: usize) -> bool {
        self.source
            .get(value_start..line_end)
            .map(str::trim)
            .is_some_and(|value| value.is_empty() || value.starts_with('#'))
    }

    fn flow_mapping_span(&self, value_start: usize) -> Option<ByteSpan> {
        let mut pos = value_start;
        while pos < self.body_end {
            let ch = self.source[pos..self.body_end].chars().next()?;
            if !ch.is_whitespace() {
                break;
            }
            pos += ch.len_utf8();
        }
        if self.source[pos..self.body_end].chars().next()? != '{' {
            return None;
        }

        let start = pos;
        let mut depth = 0usize;
        let mut quote = None;
        let mut escaped = false;
        let mut yaml_comment = false;
        while pos < self.body_end {
            let ch = self.source[pos..self.body_end].chars().next()?;
            let next = pos + ch.len_utf8();
            if yaml_comment {
                if matches!(ch, '\n' | '\r') {
                    yaml_comment = false;
                }
                pos = next;
                continue;
            }
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == active_quote {
                    quote = None;
                }
                pos = next;
                continue;
            }

            match ch {
                '"' | '\'' => quote = Some(ch),
                '#' if starts_yaml_comment(self.source, pos, start) => yaml_comment = true,
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(ByteSpan { start, end: next });
                    }
                }
                _ => {}
            }
            pos = next;
        }

        None
    }

    fn value_starts_block_scalar(&self, value_start: usize, line_end: usize) -> bool {
        self.source
            .get(value_start..line_end)
            .map(str::trim_start)
            .is_some_and(|value| value.starts_with('|') || value.starts_with('>'))
    }

    fn logical_line_indent(&self, line_start: usize, line_end: usize) -> Option<usize> {
        if line_start >= line_end {
            return None;
        }
        let line = self.source.get(line_start..line_end)?;
        if !self.indent.is_empty() && !line.starts_with(self.indent) {
            return None;
        }
        let logical_start = line_start + self.indent.len();
        let logical = &self.source[logical_start..line_end];
        logical.as_bytes().iter().position(|byte| *byte != b' ')
    }

    fn matches_path(&self, parents: &[&str], key_name: &str) -> bool {
        self.matching_paths.iter().any(|target| {
            target.len() == parents.len() + 1
                && target[..parents.len()] == *parents
                && target[parents.len()] == key_name
        })
    }

    fn trim_line_end(&self, line_start: usize, line_end_with_newline: usize) -> usize {
        let mut line_end = line_end_with_newline;
        if line_end > line_start && self.source.as_bytes()[line_end - 1] == b'\n' {
            line_end -= 1;
        }
        if line_end > line_start && self.source.as_bytes()[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        line_end
    }

    fn line_start_after_offset(&self, offset: usize) -> usize {
        if offset >= self.body_end {
            return self.body_end;
        }
        self.source[offset..self.body_end]
            .find('\n')
            .map_or(self.body_end, |relative| offset + relative + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_LABEL_PATHS: [&[&str]; 3] = [
        &["flowchart", "htmlLabels"],
        &["config", "htmlLabels"],
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
    fn init_directive_spans_include_full_directive_and_keyword() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\n%%{ wrap }%%\n%%{ init: {} }%%\n";

        let spans = init_directive_spans(source);

        assert_eq!(spans.len(), 2);
        assert_eq!(
            &source[spans[0].full.start..spans[0].full.end],
            "%%{ initialize: {\"theme\":\"dark\"} }%%"
        );
        assert_eq!(
            &source[spans[0].keyword.start..spans[0].keyword.end],
            "initialize"
        );
        assert_eq!(
            &source[spans[1].full.start..spans[1].full.end],
            "%%{ init: {} }%%"
        );
        assert_eq!(
            &source[spans[1].keyword.start..spans[1].keyword.end],
            "init"
        );
    }

    #[test]
    fn init_directive_config_key_spans_stop_at_first_closing_marker_like_core() {
        let source = "%%{ init: { \"themeCSS\": \"}%%\", \"flowchart\": { \"htmlLabels\": true } } }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert!(spans.is_empty());
    }

    #[test]
    fn init_directive_spans_skip_frontmatter_body_like_core_preprocess() {
        let source = "---\nnotes: \"%%{ init: { flowchart: { htmlLabels: false } } }%%\"\n---\nflowchart TD\n";

        assert!(init_directive_spans(source).is_empty());
        assert!(init_directive_config_key_spans(source, &HTML_LABEL_PATHS).is_empty());
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
    fn init_directive_config_key_spans_match_quoted_config_wrapper_path() {
        let source = "%%{init: { \"config\": { \"flowchart\": { \"htmlLabels\": true } } }}%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_match_json5_line_comments() {
        let source = "%%{ init: {\n  // kept by json5\n  flowchart: {\n    // deprecated fallback\n    htmlLabels: false\n  }\n} }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_match_json5_block_comments() {
        let source = "%%{ init: { config: /* wrapper { ignored } */ { flowchart: { /* fallback */ htmlLabels: true } } } }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_match_json5_comments_after_bare_keys() {
        let source = "%%{ init: { flowchart /* family */: { htmlLabels /* leaf */: false } } }%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn init_directive_config_key_spans_skip_json5_comments_after_unquoted_scalars() {
        let cases = [
            "%%{ init: { theme: false /* comment hides } ] */, flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
            "%%{ init: { themeVariables: 42 /* comment hides } ] */, flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
            "%%{ init: { theme: null // comment hides } ]\n, flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
        ];

        for source in cases {
            let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

            assert_eq!(spans.len(), 1, "source: {source}");
            assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
        }
    }

    #[test]
    fn init_directive_config_key_spans_only_scan_single_directive_value() {
        let cases = [
            "%%{ init: \"not config\", init: { flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
            "%%{ init /* comment */: { flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
            "%%{ init { flowchart: { htmlLabels: false } } }%%\nflowchart TD\n",
        ];

        for source in cases {
            let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

            assert!(spans.is_empty(), "source: {source}");
        }
    }

    #[test]
    fn init_directive_config_key_spans_match_config_wrapped_root_path() {
        let source = "%%{init: { \"config\": { \"htmlLabels\": true } }}%%\nflowchart TD\n";

        let spans = init_directive_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn frontmatter_config_key_spans_match_flow_style_nested_config() {
        let source = "---\nconfig: { flowchart: { htmlLabels: false } }\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn frontmatter_config_key_spans_do_not_treat_json5_comments_as_yaml_comments() {
        let source =
            "---\nconfig: { flowchart /* family */: { htmlLabels: false } }\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert!(spans.is_empty());
    }

    #[test]
    fn frontmatter_config_key_spans_do_not_lift_multiline_flow_mapping_children() {
        let source = "---\nmetadata: {\n  flowchart: { htmlLabels: false }\n}\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert!(spans.is_empty());
    }

    #[test]
    fn frontmatter_config_key_spans_match_multiline_flow_mapping_under_block_parent() {
        let source =
            "---\nconfig:\n  flowchart: {\n    htmlLabels: false\n  }\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn frontmatter_config_key_spans_match_flow_style_root_config() {
        let source = "---\nconfig: { htmlLabels: true }\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn frontmatter_config_key_spans_match_flow_style_yaml_comments() {
        let source = "---\nconfig: {\n  url: https://example.com/#section,\n  # braces in comments do not close the flow mapping }\n  flowchart: { htmlLabels: false }\n}\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
    }

    #[test]
    fn frontmatter_config_key_spans_skip_block_scalar_contents() {
        let source = "---\nconfig:\n  notes: |\n    flowchart:\n      htmlLabels: false\n  flowchart:\n    htmlLabels: true\n---\nflowchart TD\n";

        let spans = frontmatter_config_key_spans(source, &HTML_LABEL_PATHS);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "htmlLabels");
        assert_eq!(spans[0].start, source.find("htmlLabels: true").unwrap());
    }

    #[test]
    fn init_directive_config_key_spans_skip_root_keys_and_non_init_directives() {
        let source = "%%{ init: { htmlLabels: false, flowchart: { curve: \"linear\" } } }%%\n%%{ other: { flowchart: { htmlLabels: true } } }%%\nflowchart TD\n";

        assert!(init_directive_config_key_spans(source, &HTML_LABEL_PATHS).is_empty());
    }
}
