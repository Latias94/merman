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

const CONFIG_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;
const CONFIG_STACK_CHECKPOINT_ITEMS: usize = 128;
const DIRECTIVE_OPEN: &[u8] = b"%%{";
const DIRECTIVE_CLOSE: &[u8] = b"}%%";

pub(crate) fn frontmatter_config_key_spans_cancellable(
    source: &str,
    matching_paths: &[&[&str]],
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<ByteSpan>, crate::AnalysisCancelled> {
    let Some(frontmatter) = source_frontmatter_block_cancellable(source, cancellation)? else {
        return Ok(Vec::new());
    };

    let scanner = FrontmatterConfigScanner::new(
        source,
        frontmatter.body.start,
        frontmatter.body.end,
        frontmatter.indent,
        matching_paths,
        cancellation,
    );
    let mut spans = Vec::new();
    scanner.visit_matching_config_key_spans(&mut |span| spans.push(span))?;
    Ok(spans)
}

#[cfg(test)]
pub(crate) fn frontmatter_config_key_spans(
    source: &str,
    matching_paths: &[&[&str]],
) -> Vec<ByteSpan> {
    let cancellation = crate::AnalysisCancellationToken::new();
    frontmatter_config_key_spans_cancellable(source, matching_paths, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn directive_keyword_spans_cancellable(
    source: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<ByteSpan>, crate::AnalysisCancelled> {
    let mut spans = Vec::new();
    visit_directive_spans_cancellable(source, cancellation, |directive| {
        if let Some(keyword) = directive_keyword_span_cancellable(
            source,
            directive.body.start,
            directive.body.end,
            cancellation,
        )? {
            spans.push(keyword);
        }
        Ok(())
    })?;
    Ok(spans)
}

#[cfg(test)]
pub(crate) fn directive_keyword_spans(source: &str) -> Vec<ByteSpan> {
    let cancellation = crate::AnalysisCancellationToken::new();
    directive_keyword_spans_cancellable(source, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn init_directive_spans_cancellable(
    source: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<InitDirectiveSpan>, crate::AnalysisCancelled> {
    let mut spans = Vec::new();
    visit_directive_spans_cancellable(source, cancellation, |directive| {
        let Some(keyword) = directive_keyword_span_cancellable(
            source,
            directive.body.start,
            directive.body.end,
            cancellation,
        )?
        else {
            return Ok(());
        };
        if matches!(
            source.get(keyword.start..keyword.end),
            Some("init" | "initialize")
        ) {
            spans.push(InitDirectiveSpan {
                full: directive.full,
                keyword,
            });
        }
        Ok(())
    })?;
    Ok(spans)
}

#[cfg(test)]
pub(crate) fn init_directive_spans(source: &str) -> Vec<InitDirectiveSpan> {
    let cancellation = crate::AnalysisCancellationToken::new();
    init_directive_spans_cancellable(source, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn init_directive_config_key_spans_cancellable(
    source: &str,
    matching_paths: &[&[&str]],
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<ByteSpan>, crate::AnalysisCancelled> {
    let mut spans = Vec::new();
    visit_directive_spans_cancellable(source, cancellation, |directive| {
        let Some(value) = init_directive_value_span_cancellable(
            source,
            directive.body.start,
            directive.body.end,
            cancellation,
        )?
        else {
            return Ok(());
        };
        let mut scanner = DirectiveConfigScanner::new(
            source,
            value.start,
            value.end,
            matching_paths,
            cancellation,
        )
        .with_comment_mode(ConfigCommentMode::Json5);
        scanner.visit_matching_config_value_key_spans(&mut |span| spans.push(span))
    })?;
    Ok(spans)
}

#[cfg(test)]
pub(crate) fn init_directive_config_key_spans(
    source: &str,
    matching_paths: &[&[&str]],
) -> Vec<ByteSpan> {
    let cancellation = crate::AnalysisCancellationToken::new();
    init_directive_config_key_spans_cancellable(source, matching_paths, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

fn init_directive_value_span_cancellable(
    source: &str,
    body_start: usize,
    body_end: usize,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<ByteSpan>, crate::AnalysisCancelled> {
    let Some(keyword) =
        directive_keyword_span_cancellable(source, body_start, body_end, cancellation)?
    else {
        return Ok(None);
    };
    if !matches!(
        source.get(keyword.start..keyword.end),
        Some("init" | "initialize")
    ) {
        return Ok(None);
    }

    let mut pos = keyword.end;
    let mut checkpoints = ScanCheckpoints::new(cancellation, pos)?;
    while pos < body_end {
        checkpoints.at(pos)?;
        let Some(ch) = source[pos..body_end].chars().next() else {
            return Ok(None);
        };
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }

    if !source[pos..body_end].starts_with(':') {
        checkpoints.finish()?;
        return Ok(None);
    }

    checkpoints.finish()?;
    Ok(Some(ByteSpan {
        start: pos + ':'.len_utf8(),
        end: body_end,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectiveSpan {
    full: ByteSpan,
    body: ByteSpan,
}

struct ScanCheckpoints<'cancellation> {
    cancellation: &'cancellation crate::AnalysisCancellationToken,
    next_checkpoint: usize,
}

impl<'cancellation> ScanCheckpoints<'cancellation> {
    fn new(
        cancellation: &'cancellation crate::AnalysisCancellationToken,
        start: usize,
    ) -> Result<Self, crate::AnalysisCancelled> {
        cancellation.checkpoint()?;
        Ok(Self {
            cancellation,
            next_checkpoint: start.saturating_add(CONFIG_SCAN_CHECKPOINT_BYTES),
        })
    }

    fn at(&mut self, position: usize) -> Result<(), crate::AnalysisCancelled> {
        if position >= self.next_checkpoint {
            self.cancellation.checkpoint()?;
            self.next_checkpoint = position.saturating_add(CONFIG_SCAN_CHECKPOINT_BYTES);
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), crate::AnalysisCancelled> {
        self.cancellation.checkpoint()
    }
}

fn source_frontmatter_block_cancellable<'source>(
    source: &'source str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<
    Option<merman_core::preprocess::FrontmatterBlockLocation<'source>>,
    crate::AnalysisCancelled,
> {
    cancellation.checkpoint()?;
    let frontmatter = merman_core::preprocess::locate_frontmatter_block_controlled(
        source,
        cancellation.operation_control(),
    )
    .map_err(|_| crate::AnalysisCancelled)?;
    cancellation.checkpoint()?;
    Ok(frontmatter)
}

fn trim_ascii_line_end_cancellable(
    source: &str,
    start: usize,
    mut end: usize,
    trim: &[u8],
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<usize, crate::AnalysisCancelled> {
    let mut checkpoints = ScanCheckpoints::new(cancellation, 0)?;
    let mut scanned = 0usize;
    while end > start
        && source
            .as_bytes()
            .get(end - 1)
            .is_some_and(|byte| trim.contains(byte))
    {
        end -= 1;
        scanned += 1;
        checkpoints.at(scanned)?;
    }
    checkpoints.finish()?;
    Ok(end)
}

fn first_non_whitespace_cancellable(
    source: &str,
    start: usize,
    end: usize,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<usize>, crate::AnalysisCancelled> {
    let Some(range) = source.get(start..end) else {
        return Ok(None);
    };
    let mut checkpoints = ScanCheckpoints::new(cancellation, start)?;
    for (relative, ch) in range.char_indices() {
        let position = start + relative;
        checkpoints.at(position)?;
        if !ch.is_whitespace() {
            checkpoints.finish()?;
            return Ok(Some(position));
        }
    }
    checkpoints.finish()?;
    Ok(None)
}

fn first_byte_other_than_cancellable(
    source: &str,
    start: usize,
    end: usize,
    skipped: u8,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<usize>, crate::AnalysisCancelled> {
    let Some(bytes) = source.as_bytes().get(start..end) else {
        return Ok(None);
    };
    let mut checkpoints = ScanCheckpoints::new(cancellation, start)?;
    for (relative, byte) in bytes.iter().enumerate() {
        let position = start + relative;
        checkpoints.at(position)?;
        if *byte != skipped {
            checkpoints.finish()?;
            return Ok(Some(position));
        }
    }
    checkpoints.finish()?;
    Ok(None)
}

fn range_starts_with_cancellable(
    source: &str,
    start: usize,
    end: usize,
    prefix: &[u8],
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<bool, crate::AnalysisCancelled> {
    if prefix.len() > end.saturating_sub(start) {
        cancellation.checkpoint()?;
        return Ok(false);
    }
    let Some(actual) = source.as_bytes().get(start..start + prefix.len()) else {
        return Ok(false);
    };
    let mut checkpoints = ScanCheckpoints::new(cancellation, start)?;
    for (relative, (actual, expected)) in actual.iter().zip(prefix).enumerate() {
        checkpoints.at(start + relative)?;
        if actual != expected {
            checkpoints.finish()?;
            return Ok(false);
        }
    }
    checkpoints.finish()?;
    Ok(true)
}

fn visit_directive_spans_cancellable(
    source: &str,
    cancellation: &crate::AnalysisCancellationToken,
    mut visit: impl FnMut(DirectiveSpan) -> Result<(), crate::AnalysisCancelled>,
) -> Result<(), crate::AnalysisCancelled> {
    let mut cursor = source_frontmatter_block_cancellable(source, cancellation)?
        .map_or(0, |frontmatter| frontmatter.full.end);

    while let Some(directive_start) =
        find_ascii_pattern_cancellable(source, cursor, source.len(), DIRECTIVE_OPEN, cancellation)?
    {
        let body_start = directive_start + DIRECTIVE_OPEN.len();
        let Some(body_end) = find_ascii_pattern_cancellable(
            source,
            body_start,
            source.len(),
            DIRECTIVE_CLOSE,
            cancellation,
        )?
        else {
            return Ok(());
        };
        let full_end = body_end + DIRECTIVE_CLOSE.len();
        visit(DirectiveSpan {
            full: ByteSpan {
                start: directive_start,
                end: full_end,
            },
            body: ByteSpan {
                start: body_start,
                end: body_end,
            },
        })?;
        cursor = full_end;
    }

    cancellation.checkpoint()
}

fn find_ascii_pattern_cancellable(
    source: &str,
    start: usize,
    end: usize,
    pattern: &[u8],
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<usize>, crate::AnalysisCancelled> {
    let bytes = source.as_bytes();
    let end = end.min(bytes.len());
    if pattern.is_empty() || start > end || pattern.len() > end.saturating_sub(start) {
        cancellation.checkpoint()?;
        return Ok(None);
    }

    let mut checkpoints = ScanCheckpoints::new(cancellation, start)?;
    let last_start = end - pattern.len();
    let mut cursor = start;
    while cursor <= last_start {
        checkpoints.at(cursor)?;
        if &bytes[cursor..cursor + pattern.len()] == pattern {
            checkpoints.finish()?;
            return Ok(Some(cursor));
        }
        cursor += 1;
    }
    checkpoints.finish()?;
    Ok(None)
}

fn directive_keyword_span_cancellable(
    source: &str,
    body_start: usize,
    body_end: usize,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<ByteSpan>, crate::AnalysisCancelled> {
    if source.get(body_start..body_end).is_none() {
        return Ok(None);
    }

    let mut checkpoints = ScanCheckpoints::new(cancellation, body_start)?;
    let mut pos = body_start;
    while pos < body_end {
        checkpoints.at(pos)?;
        let Some(ch) = source[pos..body_end].chars().next() else {
            return Ok(None);
        };
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }

    let keyword_start = pos;
    while pos < body_end {
        checkpoints.at(pos)?;
        let Some(ch) = source[pos..body_end].chars().next() else {
            break;
        };
        if !ch.is_ascii_alphanumeric() && ch != '_' {
            break;
        }
        pos += ch.len_utf8();
    }
    checkpoints.finish()?;

    Ok((pos > keyword_start).then_some(ByteSpan {
        start: keyword_start,
        end: pos,
    }))
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
    cancellation: crate::AnalysisCancellationToken,
    cancelled: bool,
    next_checkpoint: usize,
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
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Self {
        Self {
            source,
            body_end,
            pos: body_start,
            matching_paths,
            comment_mode: ConfigCommentMode::None,
            cancellation: cancellation.clone(),
            cancelled: false,
            next_checkpoint: body_start.saturating_add(CONFIG_SCAN_CHECKPOINT_BYTES),
        }
    }

    fn with_comment_mode(mut self, comment_mode: ConfigCommentMode) -> Self {
        self.comment_mode = comment_mode;
        self
    }

    #[cfg(test)]
    fn matching_config_value_key_spans(
        &mut self,
    ) -> Result<Vec<ByteSpan>, crate::AnalysisCancelled> {
        let mut spans = Vec::new();
        self.visit_matching_config_value_key_spans(&mut |span| spans.push(span))?;
        Ok(spans)
    }

    fn visit_matching_config_value_key_spans(
        &mut self,
        visit: &mut impl FnMut(ByteSpan),
    ) -> Result<(), crate::AnalysisCancelled> {
        let mut path = Vec::new();
        self.collect_value_spans(&mut path, visit)
    }

    fn collect_value_spans(
        &mut self,
        path: &mut Vec<&'source str>,
        visit: &mut impl FnMut(ByteSpan),
    ) -> Result<(), crate::AnalysisCancelled> {
        if !self.checkpoint() {
            return Err(crate::AnalysisCancelled);
        }
        self.skip_ws();
        if self.peek_char() != Some('{') {
            self.skip_value();
            return self.cancellation_result();
        }
        self.next_char();
        let mut object_parent_path_lengths = vec![path.len()];
        while let Some(parent_path_len) = object_parent_path_lengths.last().copied() {
            if !self.checkpoint() {
                return Err(crate::AnalysisCancelled);
            }
            self.skip_ws_and_commas();
            match self.peek_char() {
                Some('}') => {
                    self.next_char();
                    object_parent_path_lengths.pop();
                    path.truncate(parent_path_len);
                    continue;
                }
                Some(_) => {}
                None => break,
            }

            let Some(key) = self.parse_key() else {
                break;
            };
            self.skip_ws();
            if self.next_char() != Some(':') {
                break;
            }

            if self.matches_path(path, key.name) {
                visit(key.span);
            }

            path.push(key.name);
            self.skip_ws();
            let can_descend = self.peek_char() == Some('{')
                && path.len() < merman_core::MAX_DIAGRAM_NESTING_DEPTH
                && self.can_match_descendant(path);
            if can_descend {
                self.next_char();
                object_parent_path_lengths.push(path.len().saturating_sub(1));
            } else {
                self.skip_value();
                path.pop();
            }
        }
        self.cancellation_result()
    }

    fn matches_path(&self, parents: &[&str], key_name: &str) -> bool {
        self.matching_paths.iter().any(|target| {
            target.len() == parents.len() + 1
                && target[..parents.len()] == *parents
                && target[parents.len()] == key_name
        })
    }

    fn can_match_descendant(&self, path: &[&str]) -> bool {
        self.matching_paths
            .iter()
            .any(|target| target.len() > path.len() && target[..path.len()] == *path)
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
        let mut name_start = None;
        let mut name_end = self.pos;
        while let Some(ch) = self.peek_char() {
            if matches!(ch, ':' | '\n' | '\r' | '}' | ']') {
                break;
            }
            if self.starts_comment() {
                break;
            }
            let char_start = self.pos;
            let consumed = self.next_char()?;
            if !consumed.is_whitespace() {
                name_start.get_or_insert(char_start);
                name_end = self.pos;
            }
        }

        let name_start = name_start?;
        let name = self.source.get(name_start..name_end)?;
        let span = ConfigKeySpan {
            name,
            span: ByteSpan {
                start: name_start,
                end: name_end,
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
        if !self.checkpoint_if_due() {
            return None;
        }
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn checkpoint_if_due(&mut self) -> bool {
        if self.pos < self.next_checkpoint {
            return true;
        }
        self.next_checkpoint = self.pos.saturating_add(CONFIG_SCAN_CHECKPOINT_BYTES);
        self.checkpoint()
    }

    fn checkpoint(&mut self) -> bool {
        if self.cancelled {
            return false;
        }
        if self.cancellation.checkpoint().is_ok() {
            true
        } else {
            self.cancelled = true;
            self.pos = self.body_end;
            false
        }
    }

    fn cancellation_result(&self) -> Result<(), crate::AnalysisCancelled> {
        if self.cancelled {
            Err(crate::AnalysisCancelled)
        } else {
            Ok(())
        }
    }
}

struct FrontmatterConfigScanner<'source, 'query> {
    source: &'source str,
    body_start: usize,
    body_end: usize,
    indent: &'source str,
    matching_paths: &'query [&'query [&'query str]],
    cancellation: crate::AnalysisCancellationToken,
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
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Self {
        Self {
            source,
            body_start,
            body_end,
            indent,
            matching_paths,
            cancellation: cancellation.clone(),
        }
    }

    fn visit_matching_config_key_spans(
        &self,
        visit: &mut impl FnMut(ByteSpan),
    ) -> Result<(), crate::AnalysisCancelled> {
        let mut stack: Vec<(usize, &'source str)> = Vec::new();
        let mut line_start = self.body_start;
        let mut block_scalar_indent = None;

        while line_start < self.body_end {
            let line_end_with_newline = find_ascii_pattern_cancellable(
                self.source,
                line_start,
                self.body_end,
                b"\n",
                &self.cancellation,
            )?
            .map_or(self.body_end, |line_end| line_end + 1);
            let line_end = trim_ascii_line_end_cancellable(
                self.source,
                line_start,
                line_end_with_newline,
                b"\r\n",
                &self.cancellation,
            )?;
            if let Some(scalar_indent) = block_scalar_indent {
                if first_non_whitespace_cancellable(
                    self.source,
                    line_start,
                    line_end,
                    &self.cancellation,
                )?
                .is_none()
                {
                    line_start = line_end_with_newline;
                    continue;
                }
                if self
                    .logical_line_indent(line_start, line_end)?
                    .is_some_and(|indent| indent > scalar_indent)
                {
                    line_start = line_end_with_newline;
                    continue;
                }
                block_scalar_indent = None;
            }
            let line_scan = self.collect_line_key_span(line_start, line_end, &mut stack, visit)?;
            if let Some(scalar_indent) = line_scan.block_scalar_indent {
                block_scalar_indent = Some(scalar_indent);
            }
            line_start = line_scan.skip_until.unwrap_or(line_end_with_newline);
        }

        self.cancellation.checkpoint()?;
        Ok(())
    }

    fn collect_line_key_span(
        &self,
        line_start: usize,
        line_end: usize,
        stack: &mut Vec<(usize, &'source str)>,
        visit: &mut impl FnMut(ByteSpan),
    ) -> Result<FrontmatterLineScan, crate::AnalysisCancelled> {
        if line_start >= line_end {
            return Ok(FrontmatterLineScan::default());
        }

        if self.source.get(line_start..line_end).is_none() {
            return Ok(FrontmatterLineScan::default());
        }
        let Some(first_non_whitespace) = first_non_whitespace_cancellable(
            self.source,
            line_start,
            line_end,
            &self.cancellation,
        )?
        else {
            return Ok(FrontmatterLineScan::default());
        };
        if self.source.as_bytes().get(first_non_whitespace) == Some(&b'#') {
            return Ok(FrontmatterLineScan::default());
        }
        if !self.indent.is_empty()
            && !range_starts_with_cancellable(
                self.source,
                line_start,
                line_end,
                self.indent.as_bytes(),
                &self.cancellation,
            )?
        {
            return Ok(FrontmatterLineScan::default());
        }

        let logical_start = line_start + self.indent.len();
        let content_start = first_byte_other_than_cancellable(
            self.source,
            logical_start,
            line_end,
            b' ',
            &self.cancellation,
        )?
        .unwrap_or(line_end);
        let indent = content_start - logical_start;
        if self.source[content_start..line_end].starts_with('#') {
            return Ok(FrontmatterLineScan::default());
        }

        let Some((key, value_start)) = self.parse_line_key(content_start, line_end)? else {
            return Ok(FrontmatterLineScan::default());
        };
        self.pop_stack_to_indent(stack, indent)?;

        if self.matches_stack_path(stack, key.name) {
            visit(key.span);
        }

        let mut line_scan = FrontmatterLineScan::default();
        if let Some(flow_span) = self.flow_mapping_span(value_start)? {
            let mut inline_path = self.inline_path_from_stack(stack, key.name)?;
            let mut scanner = DirectiveConfigScanner::new(
                self.source,
                flow_span.start,
                flow_span.end,
                self.matching_paths,
                &self.cancellation,
            )
            .with_comment_mode(ConfigCommentMode::Yaml);
            scanner.collect_value_spans(&mut inline_path, visit)?;
            line_scan.skip_until = Some(self.line_start_after_offset(flow_span.end)?);
        }

        if self.value_starts_block_scalar(value_start, line_end)? {
            line_scan.block_scalar_indent = Some(indent);
            return Ok(line_scan);
        }

        if self.value_starts_mapping(value_start, line_end)? {
            stack.push((indent, key.name));
        }

        Ok(line_scan)
    }

    fn pop_stack_to_indent(
        &self,
        stack: &mut Vec<(usize, &'source str)>,
        indent: usize,
    ) -> Result<(), crate::AnalysisCancelled> {
        let mut popped = 0usize;
        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
            popped += 1;
            if popped.is_multiple_of(CONFIG_STACK_CHECKPOINT_ITEMS) {
                self.cancellation.checkpoint()?;
            }
        }
        Ok(())
    }

    fn inline_path_from_stack(
        &self,
        stack: &[(usize, &'source str)],
        key_name: &'source str,
    ) -> Result<Vec<&'source str>, crate::AnalysisCancelled> {
        let mut path = Vec::with_capacity(stack.len().saturating_add(1));
        for (index, (_, name)) in stack.iter().enumerate() {
            if index != 0 && index.is_multiple_of(CONFIG_STACK_CHECKPOINT_ITEMS) {
                self.cancellation.checkpoint()?;
            }
            path.push(*name);
        }
        path.push(key_name);
        Ok(path)
    }

    fn parse_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Result<Option<(ConfigKeySpan<'source>, usize)>, crate::AnalysisCancelled> {
        let Some(first) = self.source[content_start..line_end].chars().next() else {
            return Ok(None);
        };
        match first {
            '"' | '\'' => self.parse_quoted_line_key(content_start, line_end),
            '-' => Ok(None),
            _ => self.parse_bare_line_key(content_start, line_end),
        }
    }

    fn parse_quoted_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Result<Option<(ConfigKeySpan<'source>, usize)>, crate::AnalysisCancelled> {
        let Some(quote) = self.source[content_start..line_end].chars().next() else {
            return Ok(None);
        };
        let name_start = content_start + quote.len_utf8();
        let mut pos = name_start;
        let mut escaped = false;
        let mut checkpoints = ScanCheckpoints::new(&self.cancellation, pos)?;

        while pos < line_end {
            checkpoints.at(pos)?;
            let Some(ch) = self.source[pos..line_end].chars().next() else {
                return Ok(None);
            };
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
                let Some(name) = self.source.get(name_start..pos) else {
                    return Ok(None);
                };
                let Some(colon) = self.colon_after_key(next, line_end)? else {
                    return Ok(None);
                };
                checkpoints.finish()?;
                return Ok(Some((
                    ConfigKeySpan {
                        name,
                        span: ByteSpan {
                            start: name_start,
                            end: pos,
                        },
                    },
                    colon + 1,
                )));
            }
            pos = next;
        }

        checkpoints.finish()?;
        Ok(None)
    }

    fn parse_bare_line_key(
        &self,
        content_start: usize,
        line_end: usize,
    ) -> Result<Option<(ConfigKeySpan<'source>, usize)>, crate::AnalysisCancelled> {
        let mut checkpoints = ScanCheckpoints::new(&self.cancellation, content_start)?;
        let mut pos = content_start;
        let mut name_end = content_start;
        while pos < line_end {
            checkpoints.at(pos)?;
            let Some(ch) = self.source[pos..line_end].chars().next() else {
                return Ok(None);
            };
            if ch == ':' {
                if name_end == content_start {
                    checkpoints.finish()?;
                    return Ok(None);
                }
                let Some(name) = self.source.get(content_start..name_end) else {
                    return Ok(None);
                };
                checkpoints.finish()?;
                return Ok(Some((
                    ConfigKeySpan {
                        name,
                        span: ByteSpan {
                            start: content_start,
                            end: name_end,
                        },
                    },
                    pos + 1,
                )));
            }
            pos += ch.len_utf8();
            if !ch.is_whitespace() {
                name_end = pos;
            }
        }

        checkpoints.finish()?;
        Ok(None)
    }

    fn colon_after_key(
        &self,
        mut pos: usize,
        line_end: usize,
    ) -> Result<Option<usize>, crate::AnalysisCancelled> {
        let mut checkpoints = ScanCheckpoints::new(&self.cancellation, pos)?;
        while pos < line_end {
            checkpoints.at(pos)?;
            let Some(ch) = self.source[pos..line_end].chars().next() else {
                return Ok(None);
            };
            if ch == ':' {
                checkpoints.finish()?;
                return Ok(Some(pos));
            }
            if !ch.is_whitespace() {
                checkpoints.finish()?;
                return Ok(None);
            }
            pos += ch.len_utf8();
        }
        checkpoints.finish()?;
        Ok(None)
    }

    fn value_starts_mapping(
        &self,
        value_start: usize,
        line_end: usize,
    ) -> Result<bool, crate::AnalysisCancelled> {
        Ok(first_non_whitespace_cancellable(
            self.source,
            value_start,
            line_end,
            &self.cancellation,
        )?
        .is_none_or(|position| self.source.as_bytes().get(position) == Some(&b'#')))
    }

    fn flow_mapping_span(
        &self,
        value_start: usize,
    ) -> Result<Option<ByteSpan>, crate::AnalysisCancelled> {
        let mut checkpoints = ScanCheckpoints::new(&self.cancellation, value_start)?;
        let mut pos = value_start;
        while pos < self.body_end {
            checkpoints.at(pos)?;
            let Some(ch) = self.source[pos..self.body_end].chars().next() else {
                return Ok(None);
            };
            if !ch.is_whitespace() {
                break;
            }
            pos += ch.len_utf8();
        }
        if !self.source[pos..self.body_end].starts_with('{') {
            checkpoints.finish()?;
            return Ok(None);
        }

        let start = pos;
        let mut depth = 0usize;
        let mut quote = None;
        let mut escaped = false;
        let mut yaml_comment = false;
        while pos < self.body_end {
            checkpoints.at(pos)?;
            let Some(ch) = self.source[pos..self.body_end].chars().next() else {
                return Ok(None);
            };
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
                        checkpoints.finish()?;
                        return Ok(Some(ByteSpan { start, end: next }));
                    }
                }
                _ => {}
            }
            pos = next;
        }

        checkpoints.finish()?;
        Ok(None)
    }

    fn value_starts_block_scalar(
        &self,
        value_start: usize,
        line_end: usize,
    ) -> Result<bool, crate::AnalysisCancelled> {
        Ok(first_non_whitespace_cancellable(
            self.source,
            value_start,
            line_end,
            &self.cancellation,
        )?
        .and_then(|position| self.source.as_bytes().get(position))
        .is_some_and(|byte| matches!(*byte, b'|' | b'>')))
    }

    fn logical_line_indent(
        &self,
        line_start: usize,
        line_end: usize,
    ) -> Result<Option<usize>, crate::AnalysisCancelled> {
        if line_start >= line_end {
            return Ok(None);
        }
        if self.source.get(line_start..line_end).is_none() {
            return Ok(None);
        }
        if !self.indent.is_empty()
            && !range_starts_with_cancellable(
                self.source,
                line_start,
                line_end,
                self.indent.as_bytes(),
                &self.cancellation,
            )?
        {
            return Ok(None);
        }
        let logical_start = line_start + self.indent.len();
        Ok(first_byte_other_than_cancellable(
            self.source,
            logical_start,
            line_end,
            b' ',
            &self.cancellation,
        )?
        .map(|position| position - logical_start))
    }

    fn matches_stack_path(&self, parents: &[(usize, &str)], key_name: &str) -> bool {
        self.matching_paths.iter().any(|target| {
            target.len() == parents.len() + 1
                && target[..parents.len()]
                    .iter()
                    .zip(parents.iter().map(|(_, name)| *name))
                    .all(|(target, actual)| *target == actual)
                && target[parents.len()] == key_name
        })
    }

    fn line_start_after_offset(&self, offset: usize) -> Result<usize, crate::AnalysisCancelled> {
        if offset >= self.body_end {
            self.cancellation.checkpoint()?;
            return Ok(self.body_end);
        }
        Ok(find_ascii_pattern_cancellable(
            self.source,
            offset,
            self.body_end,
            b"\n",
            &self.cancellation,
        )?
        .map_or(self.body_end, |newline| newline + 1))
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
    fn directive_keyword_spans_cancellable_observes_long_unterminated_directive() {
        let source = format!(
            "flowchart TD\n%%{{ init: {{ ignored: \"{}\" }}",
            "x".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 64),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(12);

        assert_eq!(
            directive_keyword_spans_cancellable(&source, &cancellation),
            Err(crate::AnalysisCancelled),
        );
    }

    #[test]
    fn directive_keyword_spans_find_init_alias() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\n";

        let spans = directive_keyword_spans(source);

        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].start..spans[0].end], "initialize");
    }

    #[test]
    fn directive_keyword_spans_match_core_type_grammar() {
        let source = "%%{ initialize2 garbage }%%\n%%{ initialize garbage }%%\n";

        let keywords = directive_keyword_spans(source)
            .into_iter()
            .map(|span| &source[span.start..span.end])
            .collect::<Vec<_>>();
        let init_spans = init_directive_spans(source);

        assert_eq!(keywords, ["initialize2", "initialize"]);
        assert_eq!(init_spans.len(), 1);
        assert_eq!(
            &source[init_spans[0].keyword.start..init_spans[0].keyword.end],
            "initialize"
        );
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
    fn init_directive_config_key_scanning_is_stack_bounded_for_deep_irrelevant_values() {
        let depth = merman_core::MAX_DIAGRAM_NESTING_DEPTH * 16;
        let source = format!(
            "%%{{ init: {{ flowchart: {{ ignored: {}0{} }} }} }}%%\nflowchart TD\n",
            "{".repeat(depth),
            "}".repeat(depth),
        );

        let spans = std::thread::Builder::new()
            .name("deep-config-key-scan".to_owned())
            .stack_size(64 * 1024)
            .spawn(move || init_directive_config_key_spans(&source, &HTML_LABEL_PATHS))
            .expect("the bounded scanner thread must start")
            .join()
            .expect("the bounded scanner must not overflow its stack");

        assert!(spans.is_empty());
    }

    #[test]
    fn init_directive_config_key_scanning_observes_cancellation_while_skipping_values() {
        let source = format!(
            "{{ flowchart: {{ ignored: [{}] }} }}",
            "0,".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 4),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(6);
        let mut scanner =
            DirectiveConfigScanner::new(&source, 0, source.len(), &HTML_LABEL_PATHS, &cancellation);

        assert_eq!(
            scanner.matching_config_value_key_spans(),
            Err(crate::AnalysisCancelled),
        );
    }

    #[test]
    fn directive_config_scanner_observes_cancellation_in_long_quoted_key() {
        let source = format!(
            "{{ \"{}\": false }}",
            "x".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 64),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(8);
        let mut scanner = DirectiveConfigScanner::new(&source, 0, source.len(), &[], &cancellation);

        assert_eq!(
            scanner.matching_config_value_key_spans(),
            Err(crate::AnalysisCancelled),
        );
    }

    #[test]
    fn directive_config_scanner_observes_cancellation_in_long_bare_key() {
        let source = format!(
            "{{ {}: false }}",
            "x".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 64),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(8);
        let mut scanner = DirectiveConfigScanner::new(&source, 0, source.len(), &[], &cancellation);

        assert_eq!(
            scanner.matching_config_value_key_spans(),
            Err(crate::AnalysisCancelled),
        );
    }

    #[test]
    fn frontmatter_config_key_spans_cancellable_honors_cancelled_long_opening_line() {
        let source = format!(
            "---{}\n---\nflowchart TD\n",
            " ".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 64),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel();

        assert_eq!(
            frontmatter_config_key_spans_cancellable(&source, &HTML_LABEL_PATHS, &cancellation,),
            Err(crate::AnalysisCancelled),
        );
    }

    #[test]
    fn cancellable_frontmatter_locator_matches_core_boundaries() {
        let cases = [
            "---\n---\nflowchart TD\n",
            "---\r\ntitle: demo\r\n---\r\nflowchart TD\r\n",
            "  ---\n  config:\n    flowchart:\n      htmlLabels: false\n  ---\nflowchart TD\n",
            "--- trailing\nconfig: {}\n---\nflowchart TD\n",
            "---\nconfig: {}\nflowchart TD\n",
        ];

        for source in cases {
            let cancellation = crate::AnalysisCancellationToken::new();
            let actual = source_frontmatter_block_cancellable(source, &cancellation)
                .expect("a private analysis cancellation token cannot be cancelled")
                .map(|block| {
                    (
                        ByteSpan {
                            start: block.full.start,
                            end: block.full.end,
                        },
                        ByteSpan {
                            start: block.body.start,
                            end: block.body.end,
                        },
                        block.indent,
                    )
                });
            let expected = merman_core::preprocess::split_frontmatter_block(source).map(|block| {
                (
                    ByteSpan {
                        start: block.full.start,
                        end: block.full.end,
                    },
                    ByteSpan {
                        start: block.body.start,
                        end: block.body.end,
                    },
                    block.indent,
                )
            });

            assert_eq!(actual, expected, "source: {source:?}");
        }
    }

    #[test]
    fn frontmatter_flow_mapping_scan_observes_cancellation() {
        let source = format!(
            "{{ ignored: [{}], flowchart: {{ htmlLabels: false }} }}",
            "0,".repeat(CONFIG_SCAN_CHECKPOINT_BYTES * 64),
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(8);
        let scanner = FrontmatterConfigScanner::new(
            &source,
            0,
            source.len(),
            "",
            &HTML_LABEL_PATHS,
            &cancellation,
        );

        assert_eq!(scanner.flow_mapping_span(0), Err(crate::AnalysisCancelled),);
    }

    #[test]
    fn frontmatter_config_key_scanning_is_stack_bounded_for_deep_block_mapping() {
        let depth = merman_core::MAX_DIAGRAM_NESTING_DEPTH * 4;
        let mut source = String::from("---\n");
        for level in 0..depth {
            source.push_str(&" ".repeat(level));
            source.push_str("level:\n");
        }
        source.push_str(&" ".repeat(depth));
        source.push_str("value: false\n---\nflowchart TD\n");

        let spans = std::thread::Builder::new()
            .name("deep-frontmatter-config-key-scan".to_owned())
            .stack_size(64 * 1024)
            .spawn(move || frontmatter_config_key_spans(&source, &HTML_LABEL_PATHS))
            .expect("the bounded frontmatter scanner thread must start")
            .join()
            .expect("the bounded frontmatter scanner must not overflow its stack");

        assert!(spans.is_empty());
    }

    #[test]
    fn frontmatter_dedent_stack_unwind_observes_cancellation() {
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(1);
        let scanner = FrontmatterConfigScanner::new("", 0, 0, "", &[], &cancellation);
        let mut stack = vec![(1, "level"); 1_024];

        assert_eq!(
            scanner.pop_stack_to_indent(&mut stack, 0),
            Err(crate::AnalysisCancelled)
        );
    }

    #[test]
    fn frontmatter_inline_path_copy_observes_cancellation() {
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(1);
        let scanner = FrontmatterConfigScanner::new("", 0, 0, "", &[], &cancellation);
        let stack = vec![(1, "level"); 1_024];

        assert_eq!(
            scanner.inline_path_from_stack(&stack, "leaf"),
            Err(crate::AnalysisCancelled)
        );
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
