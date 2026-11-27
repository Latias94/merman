#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{
    LangiumCommonFacts, parse_langium_common, parse_langium_string, strip_langium_inline_comment,
};
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};

use super::{
    BranchDb, COMMIT_TYPE_HIGHLIGHT, COMMIT_TYPE_NORMAL, COMMIT_TYPE_REVERSE, CherryPickDb,
    CommitDb, GitGraphCommand, GitGraphDiagnostic, GitGraphDiagnosticLocation, GitGraphOperation,
    GitGraphResult, GitGraphSyntax, MergeDb,
};

#[derive(Debug, Clone)]
struct SpannedValue {
    text: String,
    span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy)]
pub enum GitGraphArgumentRole {
    Entity,
    Reference,
    Payload,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy)]
pub enum GitGraphArgumentKind {
    CommitMessage,
    CommitId,
    CommitTag,
    CommitType,
    BranchName,
    BranchOrder,
    MergeBranch,
    MergeId,
    MergeTag,
    MergeType,
    CherryPickId,
    CherryPickParent,
    CherryPickTag,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone)]
pub struct GitGraphArgumentFact {
    pub text: String,
    pub span: SourceSpan,
    pub kind: GitGraphArgumentKind,
    pub role: GitGraphArgumentRole,
}

/// Source evidence observed at the original command or common-terminal position.
#[cfg(feature = "editor-facts")]
pub enum GitGraphSyntaxEvent<'a> {
    Common(&'a LangiumCommonFact),
    Argument(&'a GitGraphArgumentFact),
}

struct GitGraphCommandParseError {
    error: Box<GitGraphDiagnostic>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Vec<GitGraphArgumentFact>>,
    recovery_span: SourceSpan,
}

enum GitGraphCommandParseAbort {
    Cancelled(OperationCancelled),
    Invalid(GitGraphCommandParseError),
}

impl From<OperationCancelled> for GitGraphCommandParseAbort {
    fn from(cancelled: OperationCancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

impl From<GitGraphCommandParseError> for GitGraphCommandParseAbort {
    fn from(error: GitGraphCommandParseError) -> Self {
        Self::Invalid(error)
    }
}

fn parse_commit_type(raw: &str) -> GitGraphResult<i64> {
    match raw.trim_matches(is_gitgraph_whitespace) {
        "NORMAL" => Ok(COMMIT_TYPE_NORMAL),
        "REVERSE" => Ok(COMMIT_TYPE_REVERSE),
        "HIGHLIGHT" => Ok(COMMIT_TYPE_HIGHLIGHT),
        other => Err(GitGraphDiagnostic::fallback(format!(
            "Unknown commit type: {other}"
        ))),
    }
}

struct LineParser<'a> {
    input: &'a str,
    pos: usize,
    base_offset: usize,
}

impl<'a> LineParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            base_offset: 0,
        }
    }

    fn with_base(mut self, base_offset: usize) -> Self {
        self.base_offset = base_offset;
        self
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self, control: &OperationControl) -> OperationControlResult<()> {
        control.checkpoint()?;
        let mut next_checkpoint = self.pos.saturating_add(4096);
        while self.peek_char().is_some_and(is_gitgraph_whitespace) {
            self.bump();
            if self.pos >= next_checkpoint {
                control.checkpoint()?;
                next_checkpoint = self.pos.saturating_add(4096);
            }
        }
        Ok(())
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn parse_command_spanned(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<Option<SpannedValue>> {
        self.skip_ws(control)?;
        let start = self.pos;
        let mut next_checkpoint = self.pos.saturating_add(4096);
        while let Some(c) = self.peek_char() {
            if is_gitgraph_whitespace(c) || matches!(c, ':' | '"' | '\'') {
                break;
            }
            self.bump();
            if self.pos >= next_checkpoint {
                control.checkpoint()?;
                next_checkpoint = self.pos.saturating_add(4096);
            }
        }
        if self.pos == start {
            return Ok(None);
        }
        Ok(Some(SpannedValue {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        }))
    }

    fn consume_argument_name(
        &mut self,
        name: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Option<(SourceSpan, SourceSpan)>> {
        self.skip_ws(control)?;
        let rest = self.remaining();
        let Some(after_name) = rest.strip_prefix(name) else {
            return Ok(None);
        };
        if !after_name.starts_with(':') {
            return Ok(None);
        }
        let keyword_start = self.base_offset + self.pos;
        self.pos += name.len();
        let keyword = SourceSpan::new(keyword_start, keyword_start + name.len());
        let colon = SourceSpan::new(keyword.end, keyword.end + 1);
        self.pos += 1;
        Ok(Some((keyword, colon)))
    }

    fn parse_quoted_spanned(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<GitGraphResult<SpannedValue>> {
        self.skip_ws(control)?;
        let Some(parsed) = parse_langium_string(self.remaining(), self.base_offset + self.pos)
        else {
            return Ok(Err(GitGraphDiagnostic::fallback(
                "expected quoted string".to_string(),
            )));
        };
        control.checkpoint()?;
        self.pos += parsed.consumed;
        Ok(Ok(SpannedValue {
            text: parsed.value,
            span: parsed.value_span,
        }))
    }

    fn parse_name_token_spanned(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<GitGraphResult<SpannedValue>> {
        self.skip_ws(control)?;
        if matches!(self.peek_char(), Some('"' | '\'')) {
            return self.parse_quoted_spanned(control);
        }
        let start = self.pos;
        let mut next_checkpoint = self.pos.saturating_add(4096);
        while let Some(c) = self.peek_char() {
            if is_gitgraph_whitespace(c) {
                break;
            }
            self.bump();
            if self.pos >= next_checkpoint {
                control.checkpoint()?;
                next_checkpoint = self.pos.saturating_add(4096);
            }
        }
        if self.pos == start {
            return Ok(Err(GitGraphDiagnostic::fallback(
                "expected name".to_string(),
            )));
        }
        let text = &self.input[start..self.pos];
        if !is_gitgraph_reference_controlled(text, control)? {
            return Ok(Err(GitGraphDiagnostic::fallback(format!(
                "invalid gitGraph reference: {text}"
            ))));
        }
        Ok(Ok(SpannedValue {
            text: text.to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        }))
    }

    fn parse_bare_token_spanned(
        &mut self,
        expected: &str,
        control: &OperationControl,
    ) -> OperationControlResult<GitGraphResult<SpannedValue>> {
        self.skip_ws(control)?;
        let start = self.pos;
        let mut next_checkpoint = self.pos.saturating_add(4096);
        while self
            .peek_char()
            .is_some_and(|ch| !is_gitgraph_whitespace(ch) && !matches!(ch, '"' | '\''))
        {
            self.bump();
            if self.pos >= next_checkpoint {
                control.checkpoint()?;
                next_checkpoint = self.pos.saturating_add(4096);
            }
        }
        if self.pos == start {
            return Ok(Err(GitGraphDiagnostic::fallback(format!(
                "expected {expected}"
            ))));
        }
        Ok(Ok(SpannedValue {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        }))
    }

    fn expect_eof(
        &mut self,
        command: &str,
        control: &OperationControl,
    ) -> OperationControlResult<GitGraphResult<()>> {
        self.skip_ws(control)?;
        if self.is_eof() {
            return Ok(Ok(()));
        }
        Ok(Err(GitGraphDiagnostic::fallback(format!(
            "unexpected {command} argument: {}",
            self.remaining()
        ))))
    }
}

#[cfg(feature = "editor-facts")]
pub fn is_gitgraph_reference(value: &str) -> bool {
    fn is_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    let bytes = value.as_bytes();
    bytes.first().is_some_and(|byte| is_word(*byte))
        && bytes
            .last()
            .is_some_and(|byte| is_word(*byte) || *byte == b'-')
        && bytes
            .iter()
            .all(|byte| is_word(*byte) || matches!(*byte, b'-' | b'.' | b'/'))
}

fn is_gitgraph_reference_controlled(
    value: &str,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    fn is_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    let bytes = value.as_bytes();
    if !bytes.first().is_some_and(|byte| is_word(*byte))
        || !bytes
            .last()
            .is_some_and(|byte| is_word(*byte) || *byte == b'-')
    {
        return Ok(false);
    }
    for (index, byte) in bytes.iter().enumerate() {
        if index % 4096 == 0 {
            control.checkpoint()?;
        }
        if !is_word(*byte) && !matches!(*byte, b'-' | b'.' | b'/') {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_gitgraph_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

fn parse_gitgraph_int(value: &SpannedValue) -> GitGraphResult<f64> {
    let bytes = value.text.as_bytes();
    let valid = bytes == b"0"
        || (bytes
            .first()
            .is_some_and(|byte| matches!(*byte, b'1'..=b'9'))
            && bytes[1..].iter().all(u8::is_ascii_digit));
    if !valid {
        return Err(GitGraphDiagnostic::fallback(format!(
            "invalid branch order: {}",
            value.text
        )));
    }
    value
        .text
        .parse::<f64>()
        .map_err(|error| GitGraphDiagnostic::fallback(error.to_string()))
}

impl GitGraphCommand {
    #[cfg(feature = "editor-facts")]
    fn emit_arguments_controlled(
        &self,
        emit: &mut dyn FnMut(GitGraphSyntaxEvent<'_>),
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let Some(command_facts) = self.editor_facts.as_ref() else {
            return Ok(());
        };
        for fact in command_facts {
            control.checkpoint()?;
            emit(GitGraphSyntaxEvent::Argument(fact));
        }
        Ok(())
    }
}

#[cfg(feature = "editor-facts")]
fn push_gitgraph_editor_fact(
    facts: &mut Option<Vec<GitGraphArgumentFact>>,
    value: &SpannedValue,
    kind: GitGraphArgumentKind,
    role: GitGraphArgumentRole,
) {
    let Some(facts) = facts.as_mut() else {
        return;
    };
    facts.push(GitGraphArgumentFact {
        text: value.text.clone(),
        span: value.span,
        kind,
        role,
    });
}

#[cfg(feature = "editor-facts")]
fn push_gitgraph_scalar_editor_fact(
    facts: &mut Option<Vec<GitGraphArgumentFact>>,
    last_binding: &mut Option<usize>,
    value: &SpannedValue,
    kind: GitGraphArgumentKind,
    role: GitGraphArgumentRole,
) {
    let Some(command_facts) = facts.as_mut() else {
        return;
    };
    if let Some(previous) = last_binding
        .take()
        .and_then(|index| command_facts.get_mut(index))
    {
        previous.role = GitGraphArgumentRole::Payload;
    }
    *last_binding = Some(command_facts.len());
    push_gitgraph_editor_fact(facts, value, kind, role);
}

fn command_parse_result<T>(
    result: GitGraphResult<T>,
    #[cfg(feature = "editor-facts")] editor_facts: &Option<Vec<GitGraphArgumentFact>>,
    recovery_span: SourceSpan,
) -> Result<T, GitGraphCommandParseError> {
    result.map_err(|error| GitGraphCommandParseError {
        error: Box::new(error),
        #[cfg(feature = "editor-facts")]
        editor_facts: editor_facts.clone(),
        recovery_span,
    })
}

fn unexpected_gitgraph_argument(
    command: &str,
    parser: &LineParser<'_>,
    #[cfg(feature = "editor-facts")] editor_facts: Option<Vec<GitGraphArgumentFact>>,
    statement_span: SourceSpan,
) -> GitGraphCommandParseError {
    GitGraphCommandParseError {
        error: Box::new(GitGraphDiagnostic::exact(
            format!("unexpected {command} argument: {}", parser.remaining()),
            statement_span,
        )),
        #[cfg(feature = "editor-facts")]
        editor_facts,
        recovery_span: statement_span,
    }
}

fn parse_git_graph_command(
    raw: &str,
    line_start: usize,
    #[cfg(feature = "editor-facts")] capture_events: bool,
    control: &OperationControl,
) -> Result<Option<GitGraphCommand>, GitGraphCommandParseAbort> {
    control.checkpoint()?;
    let visible = strip_langium_inline_comment(raw);
    let trimmed = visible.trim_matches(is_gitgraph_whitespace);
    if trimmed.is_empty() || trimmed.starts_with("%%") {
        return Ok(None);
    }

    let trimmed_start = visible
        .len()
        .saturating_sub(visible.trim_start_matches(is_gitgraph_whitespace).len());
    let statement_span = SourceSpan::new(
        line_start + trimmed_start,
        line_start + trimmed_start + trimmed.len(),
    );
    let mut parser = LineParser::new(trimmed).with_base(line_start + trimmed_start);
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = capture_events.then(Vec::new);
    let Some(command) = parser.parse_command_spanned(control)? else {
        return Err(GitGraphCommandParseError {
            error: Box::new(GitGraphDiagnostic::exact(
                "expected gitGraph command".to_string(),
                statement_span,
            )),
            #[cfg(feature = "editor-facts")]
            editor_facts,
            recovery_span: statement_span,
        }
        .into());
    };

    let operation = match command.text.as_str() {
        "commit" => {
            parser.skip_ws(control)?;
            let mut commit = CommitDb {
                id: String::new(),
                msg: String::new(),
                commit_type: COMMIT_TYPE_NORMAL,
                tags: Vec::new(),
            };
            #[cfg(feature = "editor-facts")]
            let mut id_fact = None;
            loop {
                control.checkpoint()?;
                parser.skip_ws(control)?;
                if parser.is_eof() {
                    break;
                }
                if matches!(parser.peek_char(), Some('"' | '\'')) {
                    let message = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &message,
                        GitGraphArgumentKind::CommitMessage,
                        GitGraphArgumentRole::Payload,
                    );
                    commit.msg = message.text;
                    continue;
                }
                if parser.consume_argument_name("id", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_scalar_editor_fact(
                        &mut editor_facts,
                        &mut id_fact,
                        &value,
                        GitGraphArgumentKind::CommitId,
                        GitGraphArgumentRole::Entity,
                    );
                    commit.id = value.text;
                    continue;
                }
                if parser.consume_argument_name("msg", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::CommitMessage,
                        GitGraphArgumentRole::Payload,
                    );
                    commit.msg = value.text;
                    continue;
                }
                if parser.consume_argument_name("tag", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::CommitTag,
                        GitGraphArgumentRole::Payload,
                    );
                    commit.tags.push(value.text);
                    continue;
                }
                if parser.consume_argument_name("type", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_bare_token_spanned("commit type", control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::CommitType,
                        GitGraphArgumentRole::Payload,
                    );
                    commit.commit_type = command_parse_result(
                        parse_commit_type(&value.text),
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    continue;
                }
                return Err(unexpected_gitgraph_argument(
                    "commit",
                    &parser,
                    #[cfg(feature = "editor-facts")]
                    editor_facts,
                    statement_span,
                )
                .into());
            }
            GitGraphOperation::Commit(commit)
        }
        "branch" => {
            let name = command_parse_result(
                parser.parse_name_token_spanned(control)?,
                #[cfg(feature = "editor-facts")]
                &editor_facts,
                statement_span,
            )?;
            #[cfg(feature = "editor-facts")]
            push_gitgraph_editor_fact(
                &mut editor_facts,
                &name,
                GitGraphArgumentKind::BranchName,
                GitGraphArgumentRole::Entity,
            );
            let mut order = 0.0;
            parser.skip_ws(control)?;
            if !parser.is_eof() {
                let Some(_) = parser.consume_argument_name("order", control)? else {
                    return Err(unexpected_gitgraph_argument(
                        "branch",
                        &parser,
                        #[cfg(feature = "editor-facts")]
                        editor_facts,
                        statement_span,
                    )
                    .into());
                };
                let value = command_parse_result(
                    parser.parse_bare_token_spanned("branch order", control)?,
                    #[cfg(feature = "editor-facts")]
                    &editor_facts,
                    statement_span,
                )?;
                #[cfg(feature = "editor-facts")]
                push_gitgraph_editor_fact(
                    &mut editor_facts,
                    &value,
                    GitGraphArgumentKind::BranchOrder,
                    GitGraphArgumentRole::Payload,
                );
                order = command_parse_result(
                    parse_gitgraph_int(&value),
                    #[cfg(feature = "editor-facts")]
                    &editor_facts,
                    statement_span,
                )?;
            }
            command_parse_result(
                parser.expect_eof("branch", control)?,
                #[cfg(feature = "editor-facts")]
                &editor_facts,
                statement_span,
            )?;
            GitGraphOperation::Branch(BranchDb {
                name: name.text,
                order,
            })
        }
        "checkout" | "switch" => {
            let name = command_parse_result(
                parser.parse_name_token_spanned(control)?,
                #[cfg(feature = "editor-facts")]
                &editor_facts,
                statement_span,
            )?;
            #[cfg(feature = "editor-facts")]
            push_gitgraph_editor_fact(
                &mut editor_facts,
                &name,
                GitGraphArgumentKind::BranchName,
                GitGraphArgumentRole::Reference,
            );
            command_parse_result(
                parser.expect_eof(command.text.as_str(), control)?,
                #[cfg(feature = "editor-facts")]
                &editor_facts,
                statement_span,
            )?;
            GitGraphOperation::Checkout(name.text)
        }
        "merge" => {
            let branch = command_parse_result(
                parser.parse_name_token_spanned(control)?,
                #[cfg(feature = "editor-facts")]
                &editor_facts,
                statement_span,
            )?;
            #[cfg(feature = "editor-facts")]
            push_gitgraph_editor_fact(
                &mut editor_facts,
                &branch,
                GitGraphArgumentKind::MergeBranch,
                GitGraphArgumentRole::Reference,
            );
            let mut merge = MergeDb {
                branch: branch.text,
                id: None,
                commit_type: None,
                tags: Vec::new(),
            };
            #[cfg(feature = "editor-facts")]
            let mut id_fact = None;
            loop {
                control.checkpoint()?;
                parser.skip_ws(control)?;
                if parser.is_eof() {
                    break;
                }
                if parser.consume_argument_name("id", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_scalar_editor_fact(
                        &mut editor_facts,
                        &mut id_fact,
                        &value,
                        GitGraphArgumentKind::MergeId,
                        GitGraphArgumentRole::Entity,
                    );
                    merge.id = Some(value.text);
                    continue;
                }
                if parser.consume_argument_name("tag", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::MergeTag,
                        GitGraphArgumentRole::Payload,
                    );
                    merge.tags.push(value.text);
                    continue;
                }
                if parser.consume_argument_name("type", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_bare_token_spanned("merge type", control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::MergeType,
                        GitGraphArgumentRole::Payload,
                    );
                    merge.commit_type = Some(command_parse_result(
                        parse_commit_type(&value.text),
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?);
                    continue;
                }
                return Err(unexpected_gitgraph_argument(
                    "merge",
                    &parser,
                    #[cfg(feature = "editor-facts")]
                    editor_facts,
                    statement_span,
                )
                .into());
            }
            GitGraphOperation::Merge(merge)
        }
        "cherry-pick" => {
            let mut cherry_pick = CherryPickDb {
                id: String::new(),
                parent: String::new(),
                tags: None,
            };
            #[cfg(feature = "editor-facts")]
            let mut id_fact = None;
            #[cfg(feature = "editor-facts")]
            let mut parent_fact = None;
            loop {
                control.checkpoint()?;
                parser.skip_ws(control)?;
                if parser.is_eof() {
                    break;
                }
                if parser.consume_argument_name("id", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_scalar_editor_fact(
                        &mut editor_facts,
                        &mut id_fact,
                        &value,
                        GitGraphArgumentKind::CherryPickId,
                        GitGraphArgumentRole::Reference,
                    );
                    cherry_pick.id = value.text;
                    continue;
                }
                if parser.consume_argument_name("parent", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_scalar_editor_fact(
                        &mut editor_facts,
                        &mut parent_fact,
                        &value,
                        GitGraphArgumentKind::CherryPickParent,
                        GitGraphArgumentRole::Reference,
                    );
                    cherry_pick.parent = value.text;
                    continue;
                }
                if parser.consume_argument_name("tag", control)?.is_some() {
                    let value = command_parse_result(
                        parser.parse_quoted_spanned(control)?,
                        #[cfg(feature = "editor-facts")]
                        &editor_facts,
                        statement_span,
                    )?;
                    #[cfg(feature = "editor-facts")]
                    push_gitgraph_editor_fact(
                        &mut editor_facts,
                        &value,
                        GitGraphArgumentKind::CherryPickTag,
                        GitGraphArgumentRole::Payload,
                    );
                    cherry_pick
                        .tags
                        .get_or_insert_with(Vec::new)
                        .push(value.text);
                    continue;
                }
                return Err(unexpected_gitgraph_argument(
                    "cherry-pick",
                    &parser,
                    #[cfg(feature = "editor-facts")]
                    editor_facts,
                    statement_span,
                )
                .into());
            }
            GitGraphOperation::CherryPick(cherry_pick)
        }
        _ => {
            return Err(GitGraphCommandParseError {
                error: Box::new(GitGraphDiagnostic::exact(
                    format!("Unknown statement: {}", command.text),
                    command.span,
                )),
                #[cfg(feature = "editor-facts")]
                editor_facts,
                recovery_span: statement_span,
            }
            .into());
        }
    };

    control.checkpoint()?;
    Ok(Some(GitGraphCommand {
        operation,
        #[cfg(feature = "editor-facts")]
        editor_facts,
        statement_span,
    }))
}

/// The recognized GitGraph header and the single body that follows it.
pub struct GitGraphHeader<'source> {
    source: &'source str,
    direction: Option<String>,
    body_start: usize,
}

impl<'source> GitGraphHeader<'source> {
    /// Parses the header before the facade decides which editor evidence to retain.
    pub fn parse(
        code: &'source str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, GitGraphDiagnostic>> {
        let mut offset = 0usize;
        while offset < code.len() {
            control.checkpoint()?;
            let (line, next_offset) = physical_line(code, offset);
            let visible = line.split_once("%%").map_or(line, |(before, _)| before);
            let trimmed = visible.trim_start_matches(is_gitgraph_whitespace);
            if trimmed.trim_end_matches(is_gitgraph_whitespace).is_empty() {
                offset = next_offset;
                continue;
            }
            let Some(after_keyword) = trimmed.strip_prefix("gitGraph") else {
                return Ok(Err(GitGraphDiagnostic::fallback(
                    "expected gitGraph header".to_string(),
                )));
            };
            if after_keyword.chars().next().is_some_and(|ch| {
                !is_gitgraph_whitespace(ch) && ch != ':' && !after_keyword.starts_with("%%")
            }) {
                return Ok(Err(GitGraphDiagnostic::fallback(
                    "expected gitGraph header".to_string(),
                )));
            }

            let leading = visible.len() - trimmed.len();
            let keyword_start = offset + leading;
            let keyword_end = keyword_start + "gitGraph".len();
            let body_start = keyword_end;
            let mut rest = &code[body_start..];
            let whitespace = rest
                .chars()
                .take_while(|ch| matches!(ch, ' ' | '\t'))
                .map(char::len_utf8)
                .sum::<usize>();
            rest = &rest[whitespace..];

            if rest.starts_with(':') {
                let colon = body_start + whitespace;
                return Ok(Ok(Self {
                    source: code,
                    direction: None,
                    body_start: colon + 1,
                }));
            }
            for direction in ["LR", "TB", "BT"] {
                let Some(after_direction) = rest.strip_prefix(direction) else {
                    continue;
                };
                if after_direction
                    .chars()
                    .next()
                    .is_some_and(|ch| !ch.is_whitespace() && ch != ':')
                {
                    continue;
                }
                let direction_ws = after_direction
                    .chars()
                    .take_while(|ch| matches!(ch, ' ' | '\t'))
                    .map(char::len_utf8)
                    .sum::<usize>();
                if after_direction.as_bytes().get(direction_ws) == Some(&b':') {
                    let direction_start = body_start + whitespace;
                    let direction_end = direction_start + direction.len();
                    let colon = direction_end + direction_ws;
                    return Ok(Ok(Self {
                        source: code,
                        direction: Some(direction.to_string()),
                        body_start: colon + 1,
                    }));
                }
            }
            return Ok(Ok(Self {
                source: code,
                direction: None,
                body_start,
            }));
        }

        Ok(Err(GitGraphDiagnostic::fallback("empty input".to_string())))
    }

    /// Parses commands without collecting rich editor evidence.
    pub fn into_syntax(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GitGraphSyntax, GitGraphDiagnostic>> {
        self.into_syntax_controlled(
            control,
            #[cfg(feature = "editor-facts")]
            None,
        )
    }

    /// Parses commands once and emits events in source order, including facts from invalid commands.
    #[cfg(feature = "editor-facts")]
    pub fn into_syntax_with_events(
        self,
        control: &OperationControl,
        mut on_event: impl FnMut(GitGraphSyntaxEvent<'_>),
    ) -> OperationControlResult<Result<GitGraphSyntax, GitGraphDiagnostic>> {
        self.into_syntax_controlled(control, Some(&mut on_event))
    }

    fn into_syntax_controlled(
        self,
        control: &OperationControl,
        #[cfg(feature = "editor-facts")] mut on_event: Option<
            &mut dyn FnMut(GitGraphSyntaxEvent<'_>),
        >,
    ) -> OperationControlResult<GitGraphResult<GitGraphSyntax>> {
        let Self {
            source: code,
            direction,
            body_start: mut offset,
        } = self;
        #[cfg(feature = "editor-facts")]
        let capture_events = on_event.is_some();
        let mut commands = Vec::new();
        let mut common = LangiumCommonFacts::default();
        let mut first_error = None;
        while offset < code.len() {
            control.checkpoint()?;
            if let Some(parsed) = parse_langium_common(code, offset) {
                #[cfg(feature = "editor-facts")]
                if let Some(emit) = on_event.as_deref_mut() {
                    emit(GitGraphSyntaxEvent::Common(&parsed.fact));
                }
                if let Some(diagnostic) = parsed.diagnostic {
                    first_error.get_or_insert_with(|| GitGraphDiagnostic {
                        message: diagnostic.message,
                        location: GitGraphDiagnosticLocation::CommonInsertionPoint(
                            diagnostic.span.start,
                        ),
                    });
                }
                common.push(parsed.fact);
                offset += parsed.consumed;
                continue;
            }
            let line_start = offset;
            let (line, next_offset) = gitgraph_statement(code, offset, control)?;
            offset = next_offset;
            match parse_git_graph_command(
                line,
                line_start,
                #[cfg(feature = "editor-facts")]
                capture_events,
                control,
            ) {
                Ok(Some(command)) => {
                    #[cfg(feature = "editor-facts")]
                    if let Some(emit) = on_event.as_deref_mut() {
                        command.emit_arguments_controlled(emit, control)?;
                    }
                    commands.push(command);
                }
                Ok(None) => {}
                Err(GitGraphCommandParseAbort::Cancelled(cancelled)) => return Err(cancelled),
                Err(GitGraphCommandParseAbort::Invalid(error)) => {
                    #[cfg(feature = "editor-facts")]
                    if let (Some(recovered), Some(emit)) =
                        (error.editor_facts, on_event.as_deref_mut())
                    {
                        for fact in recovered {
                            control.checkpoint()?;
                            emit(GitGraphSyntaxEvent::Argument(&fact));
                        }
                    }
                    first_error.get_or_insert_with(|| {
                        (*error.error).with_exact_span_if_missing(error.recovery_span)
                    });
                }
            }
        }
        Ok(match first_error {
            Some(error) => Err(error),
            None => Ok(GitGraphSyntax {
                direction,
                commands,
                common,
            }),
        })
    }
}

fn physical_line(source: &str, offset: usize) -> (&str, usize) {
    let rest = &source[offset..];
    if let Some(newline) = rest.find('\n') {
        let line = rest[..newline]
            .strip_suffix('\r')
            .unwrap_or(&rest[..newline]);
        (line, offset + newline + 1)
    } else {
        (rest, source.len())
    }
}

/// Finds an EOL outside STRING/comment terminals without changing source byte offsets.
fn gitgraph_statement<'a>(
    source: &'a str,
    offset: usize,
    control: &OperationControl,
) -> OperationControlResult<(&'a str, usize)> {
    let rest = &source[offset..];
    let bytes = rest.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if index % 4096 == 0 {
            control.checkpoint()?;
        }
        if byte == b'\n' && (quote.is_none() || escaped || comment) {
            let end = index - usize::from(index > 0 && bytes[index - 1] == b'\r');
            return Ok((&rest[..end], offset + index + 1));
        }
        if comment {
            continue;
        }
        if escaped {
            // An escaped line terminator is not part of common.langium's STRING terminal.
            if byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                return Ok((&rest[..index], offset + index + 2));
            }
            escaped = false;
        } else if let Some(open) = quote {
            if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                quote = None;
            }
        } else if matches!(byte, b'"' | b'\'') {
            quote = Some(byte);
        } else if byte == b'%' && bytes.get(index + 1) == Some(&b'%') {
            comment = true;
        }
    }
    Ok((rest, source.len()))
}

#[cfg(test)]
mod tests;
