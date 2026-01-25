use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

lalrpop_util::lalrpop_mod!(flowchart_grammar, "/diagrams/flowchart_grammar.rs");

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub id: String,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub shape: Option<String>,
    pub shape_data: Option<String>,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub from: String,
    pub to: String,
    pub id: Option<String>,
    pub link: LinkToken,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub style: Vec<String>,
    pub classes: Vec<String>,
    pub interpolate: Option<String>,
    pub is_user_defined_id: bool,
    pub animate: Option<bool>,
    pub animation: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LinkToken {
    pub end: String,
    pub edge_type: String,
    pub stroke: String,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct EdgeDefaults {
    pub style: Vec<String>,
    pub interpolate: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TitleKind {
    Text,
    String,
    Markdown,
}

#[derive(Debug, Clone)]
pub(crate) struct LabeledText {
    pub text: String,
    pub kind: TitleKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubgraphHeader {
    pub raw_id: String,
    pub raw_title: String,
    pub title_kind: TitleKind,
    pub id_equals_title: bool,
}

impl Default for SubgraphHeader {
    fn default() -> Self {
        Self {
            raw_id: String::new(),
            raw_title: String::new(),
            title_kind: TitleKind::Text,
            id_equals_title: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StyleStmt {
    pub target: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDefStmt {
    pub ids: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassAssignStmt {
    pub targets: Vec<String>,
    pub class_name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum ClickAction {
    Callback {
        function_name: String,
        function_args: Option<String>,
    },
    Link {
        href: String,
        target: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ClickStmt {
    pub ids: Vec<String>,
    pub tooltip: Option<String>,
    pub action: ClickAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkStylePos {
    Default,
    Index(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct LinkStyleStmt {
    pub positions: Vec<LinkStylePos>,
    pub interpolate: Option<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlowchartAst {
    pub keyword: String,
    pub direction: Option<String>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(crate) struct SubgraphBlock {
    pub header: SubgraphHeader,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Chain { nodes: Vec<Node>, edges: Vec<Edge> },
    Node(Node),
    Subgraph(SubgraphBlock),
    Direction(String),
    Style(StyleStmt),
    ClassDef(ClassDefStmt),
    ClassAssign(ClassAssignStmt),
    Click(ClickStmt),
    LinkStyle(LinkStyleStmt),
    ShapeData { target: String, yaml: String },
}

#[derive(Debug, Clone)]
pub(crate) struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub label_type: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    KwGraph,
    KwFlowchart,
    KwFlowchartElk,
    KwSubgraph,
    KwEnd,

    Sep,
    Amp,
    StyleSep,
    NodeLabel(NodeLabelToken),

    Direction(String),
    DirectionStmt(String),
    Id(String),
    Arrow(LinkToken),
    EdgeLabel(LabeledText),
    SubgraphHeader(SubgraphHeader),

    StyleStmt(StyleStmt),
    ClassDefStmt(ClassDefStmt),
    ClassAssignStmt(ClassAssignStmt),
    ClickStmt(ClickStmt),
    LinkStyleStmt(LinkStyleStmt),

    EdgeId(String),
    ShapeData(String),
}

#[derive(Debug, Clone)]
pub(crate) struct NodeLabelToken {
    pub shape: String,
    pub text: LabeledText,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
}

struct Lexer<'input> {
    input: &'input str,
    pos: usize,
    pending: VecDeque<(usize, Tok, usize)>,
    allow_header_direction: bool,
}

impl<'input> Lexer<'input> {
    fn normalize_direction_token(dir: &str) -> &str {
        if dir == "TD" { "TB" } else { dir }
    }

    fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            allow_header_direction: false,
        }
    }

    fn bump(&mut self) -> Option<u8> {
        if self.pos >= self.input.len() {
            return None;
        }
        let b = self.input.as_bytes()[self.pos];
        self.pos += 1;
        Some(b)
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn peek2(&self) -> Option<[u8; 2]> {
        if self.pos + 1 >= self.input.len() {
            return None;
        }
        Some([
            self.input.as_bytes()[self.pos],
            self.input.as_bytes()[self.pos + 1],
        ])
    }

    fn starts_with_kw(&self, kw: &str) -> bool {
        let rest = &self.input[self.pos..];
        if !rest.starts_with(kw) {
            return false;
        }
        let after = self.pos + kw.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        !b.is_ascii_alphanumeric() && b != b'_' && b != b'-'
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn lex_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'\n' => {
                let bytes = self.input.as_bytes();
                let mut look = self.pos + 1;
                while look < bytes.len() {
                    match bytes[look] {
                        b' ' | b'\t' | b'\r' => look += 1,
                        _ => break,
                    }
                }
                if look < bytes.len() {
                    let is_linkish = match bytes[look] {
                        b'~' => {
                            look + 2 < bytes.len()
                                && bytes[look + 1] == b'~'
                                && bytes[look + 2] == b'~'
                        }
                        b'=' => look + 1 < bytes.len() && bytes[look + 1] == b'=',
                        b'-' => {
                            look + 1 < bytes.len()
                                && (bytes[look + 1] == b'-' || bytes[look + 1] == b'.')
                        }
                        b'o' | b'x' | b'<' => {
                            look + 2 < bytes.len()
                                && ((bytes[look + 1] == b'-'
                                    && (bytes[look + 2] == b'-' || bytes[look + 2] == b'.'))
                                    || (bytes[look + 1] == b'=' && bytes[look + 2] == b'='))
                        }
                        _ => false,
                    };
                    if is_linkish {
                        self.pos = look;
                        return None;
                    }
                }

                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            b';' => {
                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            _ => None,
        }
    }

    fn lex_comment(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let Some([b'%', b'%']) = self.peek2() else {
            return None;
        };
        // Consume until newline or EOF. If newline exists, emit Sep to keep statement boundaries.
        self.pos += 2;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                self.pos += 1;
                return Some((start, Tok::Sep, self.pos));
            }
            self.pos += 1;
        }
        None
    }

    fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        if !self.allow_header_direction {
            return None;
        }
        let start = self.pos;
        let rest = &self.input[self.pos..];
        for d in ["TB", "TD", "BT", "LR", "RL"] {
            if rest.starts_with(d) {
                let after = self.pos + d.len();
                if after < self.input.len() {
                    let b = self.input.as_bytes()[after];
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                        continue;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                let d = Self::normalize_direction_token(d);
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        if let Some(&b) = rest.as_bytes().first() {
            let mapped = match b {
                b'>' => Some("LR"),
                b'<' => Some("RL"),
                b'^' => Some("BT"),
                b'v' => Some("TB"),
                _ => None,
            };
            if let Some(d) = mapped {
                let after = self.pos + 1;
                if after < self.input.len() {
                    let next = self.input.as_bytes()[after];
                    if next.is_ascii_alphanumeric() || next == b'_' || next == b'-' {
                        return None;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        None
    }

    fn lex_direction_stmt(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if !self.starts_with_kw("direction") {
            return None;
        }
        self.pos += "direction".len();
        self.skip_ws();

        let rest = &self.input[self.pos..];
        let mut dir: Option<&str> = None;
        for d in ["TB", "TD", "BT", "LR", "RL"] {
            if rest.starts_with(d) {
                dir = Some(d);
                self.pos += d.len();
                break;
            }
        }
        let Some(dir) = dir else {
            return Some((start, Tok::DirectionStmt("".to_string()), self.pos));
        };
        let dir = Self::normalize_direction_token(dir);

        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }

        Some((start, Tok::DirectionStmt(dir.to_string()), self.pos))
    }

    fn capture_to_stmt_end(&mut self) -> (usize, String, usize) {
        let start = self.pos;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        (start, self.input[start..self.pos].to_string(), self.pos)
    }

    fn lex_style_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.input[self.pos..].starts_with(":::") {
            self.pos += 3;
            return Some((start, Tok::StyleSep, self.pos));
        }
        None
    }

    fn lex_shape_data(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if !self.input[self.pos..].starts_with("@{") {
            return None;
        }
        self.pos += 2;

        // Mermaid's Jison lexer has dedicated states for shapeData strings:
        // - it allows `}` inside double-quoted strings
        // - it rewrites `\n\s*` inside double-quoted strings to `<br/>`
        //
        // We mimic that behavior here while returning a single `ShapeData` token.
        let bytes = self.input.as_bytes();
        let mut out = String::new();
        let mut segment_start = self.pos;
        let mut in_string = false;

        while self.pos < self.input.len() {
            let b = bytes[self.pos];
            if !in_string {
                if b == b'"' {
                    out.push_str(&self.input[segment_start..self.pos + 1]);
                    self.pos += 1;
                    segment_start = self.pos;
                    in_string = true;
                    continue;
                }
                if b == b'}' {
                    out.push_str(&self.input[segment_start..self.pos]);
                    self.pos += 1;
                    return Some((start, Tok::ShapeData(out), self.pos));
                }
                self.pos += 1;
                continue;
            }

            if b == b'"' {
                out.push_str(&self.input[segment_start..self.pos + 1]);
                self.pos += 1;
                segment_start = self.pos;
                in_string = false;
                continue;
            }

            if b == b'\n' {
                out.push_str(&self.input[segment_start..self.pos]);
                out.push_str("<br/>");
                self.pos += 1;
                while self.pos < self.input.len() {
                    match bytes[self.pos] {
                        b' ' | b'\t' | b'\r' => self.pos += 1,
                        _ => break,
                    }
                }
                segment_start = self.pos;
                continue;
            }

            self.pos += 1;
        }

        out.push_str(&self.input[segment_start..self.pos]);
        Some((start, Tok::ShapeData(out), self.pos))
    }

    fn lex_edge_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if start >= bytes.len() {
            return None;
        }
        let first = bytes[start];
        if !first.is_ascii_alphanumeric() && first != b'_' {
            return None;
        }
        let mut i = start;
        while i < bytes.len() {
            let b = bytes[i];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                i += 1;
                continue;
            }
            break;
        }
        if i >= bytes.len() || bytes[i] != b'@' {
            return None;
        }
        let next = bytes.get(i + 1).copied();
        if matches!(next, Some(b'{') | Some(b'"')) {
            return None;
        }
        self.pos = i + 1;
        let id = self.input[start..i].to_string();
        Some((start, Tok::EdgeId(id), self.pos))
    }

    fn lex_style_stmt(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("style") {
            return None;
        }
        self.pos += "style".len();
        self.skip_ws();
        let (_rest_start, rest, end) = self.capture_to_stmt_end();
        match parse_style_stmt(&rest) {
            Ok(stmt) => Some(Ok((start, Tok::StyleStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    fn lex_classdef_stmt(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("classDef") {
            return None;
        }
        self.pos += "classDef".len();
        self.skip_ws();
        let (_rest_start, rest, end) = self.capture_to_stmt_end();
        match parse_classdef_stmt(&rest) {
            Ok(stmt) => Some(Ok((start, Tok::ClassDefStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    fn lex_class_assign_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("class") {
            return None;
        }
        self.pos += "class".len();
        self.skip_ws();
        let (_rest_start, rest, end) = self.capture_to_stmt_end();
        match parse_class_assign_stmt(&rest) {
            Ok(stmt) => Some(Ok((start, Tok::ClassAssignStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    fn lex_click_stmt(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("click") {
            return None;
        }
        self.pos += "click".len();
        self.skip_ws();
        let (_rest_start, rest, end) = self.capture_to_stmt_end();
        match parse_click_stmt(&rest) {
            Ok(stmt) => Some(Ok((start, Tok::ClickStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    fn lex_link_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("linkStyle") {
            return None;
        }
        self.pos += "linkStyle".len();
        self.skip_ws();
        let (_rest_start, rest, end) = self.capture_to_stmt_end();
        match parse_link_style_stmt(&rest) {
            Ok(stmt) => Some(Ok((start, Tok::LinkStyleStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    fn lex_subgraph_header_after_keyword(&mut self) -> Option<(usize, Tok, usize)> {
        self.skip_ws();
        let start = self.pos;
        if start >= self.input.len() {
            return None;
        }
        match self.input.as_bytes()[start] {
            b'\n' | b';' => return None,
            _ => {}
        }

        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if b == b'\n' || b == b';' || b == b'[' {
                break;
            }
            self.pos += 1;
        }

        let raw_id = self.input[start..self.pos].to_string();
        let mut raw_title = raw_id.clone();
        let mut title_kind = TitleKind::Text;
        let mut id_equals_title = true;

        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'[' {
            id_equals_title = false;
            self.pos += 1;
            let title_start = self.pos;
            while self.pos < self.input.len() && self.input.as_bytes()[self.pos] != b']' {
                if self.input.as_bytes()[self.pos] == b'\n'
                    || self.input.as_bytes()[self.pos] == b';'
                {
                    break;
                }
                self.pos += 1;
            }
            raw_title = self.input[title_start..self.pos].to_string();
            let trimmed = raw_title.trim();
            if (trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            {
                title_kind = TitleKind::String;
            }
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                self.pos += 1;
            }
        }

        Some((
            start,
            Tok::SubgraphHeader(SubgraphHeader {
                raw_id,
                raw_title,
                title_kind,
                id_equals_title,
            }),
            self.pos,
        ))
    }

    fn lex_amp(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b'&' {
            return None;
        }
        self.pos += 1;
        Some((start, Tok::Amp, self.pos))
    }

    fn lex_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if start >= bytes.len() {
            return None;
        }
        let first = bytes[start];
        if !first.is_ascii_alphanumeric() && first != b'_' {
            return None;
        }
        self.pos += 1;

        while self.pos < bytes.len() {
            if self.pos + 1 < bytes.len() {
                if bytes[self.pos] == b'-' && bytes[self.pos + 1] == b'-'
                    || bytes[self.pos] == b'=' && bytes[self.pos + 1] == b'='
                {
                    break;
                }
            }
            let b = bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
                continue;
            }
            if b == b'-' {
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                // Dotted edges start with `-.` (e.g. `A-.->B`). Avoid consuming the link start as
                // part of the id while still allowing ids like `subcontainer-child`.
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'.' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            if b == b'.' {
                // Allow dots inside ids (Mermaid supports nodes like `P1.5`), but avoid consuming
                // the `.` that starts a dotted link token like `.->` when it is directly adjacent
                // to an id (e.g. `A.->B`).
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            break;
        }

        if self.pos <= start {
            return None;
        }

        let id = self.input[start..self.pos].to_string();
        Some((start, Tok::Id(id), self.pos))
    }

    fn lex_arrow_and_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let bytes = self.input.as_bytes();

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum LinkFamily {
            Normal,
            Thick,
            Dotted,
            Invisible,
        }

        fn is_link_ws(b: u8) -> bool {
            matches!(b, b' ' | b'\t' | b'\r' | b'\n')
        }

        fn is_space_ws(b: u8) -> bool {
            matches!(b, b' ' | b'\t' | b'\r')
        }

        let match_link_end = |mut pos: usize,
                              family: LinkFamily,
                              allow_leading_ws: bool|
         -> Option<(usize, usize, String)> {
            let len = bytes.len();
            let match_start = pos;
            if allow_leading_ws {
                while pos < len && is_link_ws(bytes[pos]) {
                    pos += 1;
                }
            }
            let token_start = pos;
            if token_start >= len {
                return None;
            }

            let mut cur = token_start;
            let start_marker = bytes[cur];
            if matches!(start_marker, b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            match family {
                LinkFamily::Invisible => {
                    cur = token_start;
                    let mut tildes = 0usize;
                    while cur < len && bytes[cur] == b'~' {
                        tildes += 1;
                        cur += 1;
                    }
                    if tildes < 3 {
                        return None;
                    }
                }
                LinkFamily::Normal => {
                    let hyphen_start = cur;
                    while cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let hyphens = cur - hyphen_start;
                    if hyphens < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `--+` + `-` requires at least 3 hyphens total.
                                if hyphens < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if hyphens < 3 {
                        return None;
                    }
                }
                LinkFamily::Thick => {
                    let eq_start = cur;
                    while cur < len && bytes[cur] == b'=' {
                        cur += 1;
                    }
                    let eqs = cur - eq_start;
                    if eqs < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `==+` + `=` requires at least 3 '=' total.
                                if eqs < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if eqs < 3 {
                        return None;
                    }
                }
                LinkFamily::Dotted => {
                    if cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let mut dots = 0usize;
                    while cur < len && bytes[cur] == b'.' {
                        dots += 1;
                        cur += 1;
                    }
                    if dots == 0 {
                        return None;
                    }
                    if cur >= len || bytes[cur] != b'-' {
                        return None;
                    }
                    cur += 1;
                    if cur < len && matches!(bytes[cur], b'x' | b'o' | b'>') {
                        cur += 1;
                    }
                }
            }

            let token_end = cur;
            let token = self.input[token_start..token_end]
                .split_whitespace()
                .collect::<String>();
            Some((match_start, token_end, token))
        };

        let compute_link = |end: String,
                            start: Option<String>|
         -> std::result::Result<LinkToken, LexError> {
            let (end_type, stroke, length) = destruct_end_link(&end);
            let mut edge_type = end_type;

            if let Some(start_str) = start.as_deref() {
                let (start_type, start_stroke) = destruct_start_link(start_str);
                if start_stroke != stroke.as_str() {
                    return Err(LexError {
                        message: "Invalid link: stroke mismatch between start and end".to_string(),
                    });
                }

                if start_type == "arrow_open" {
                    edge_type = edge_type.clone();
                } else {
                    if start_type != edge_type.as_str() {
                        return Err(LexError {
                            message: "Invalid link: start/end arrowhead mismatch".to_string(),
                        });
                    }
                    edge_type = format!("double_{start_type}");
                }

                if edge_type == "double_arrow" {
                    edge_type = "double_arrow_point".to_string();
                }
            }

            Ok(LinkToken {
                end,
                edge_type,
                stroke,
                length,
            })
        };

        // 1) Prefer full LINK tokens (matches Jison longest-match behavior).
        let families = [
            LinkFamily::Invisible,
            LinkFamily::Thick,
            LinkFamily::Normal,
            LinkFamily::Dotted,
        ];
        for family in families {
            if let Some((_mstart, mend, arrow)) = match_link_end(self.pos, family, false) {
                self.pos = mend;
                let arrow_end = mend;
                let link = match compute_link(arrow, None) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };

                // Optional pipe label: A--x|label|B
                if self.pos < self.input.len() && bytes[self.pos] == b'|' {
                    self.pos += 1;
                    let label_start = self.pos;
                    while self.pos < self.input.len() && bytes[self.pos] != b'|' {
                        self.pos += 1;
                    }
                    if self.pos < self.input.len() && bytes[self.pos] == b'|' {
                        let raw = self.input[label_start..self.pos].trim();
                        let (text, kind) = parse_label_text(raw);
                        self.pos += 1;
                        self.pending.push_back((
                            label_start - 1,
                            Tok::EdgeLabel(LabeledText { text, kind }),
                            self.pos,
                        ));
                    } else {
                        return Some(Ok((start, Tok::Arrow(link), arrow_end)));
                    }
                }

                return Some(Ok((start, Tok::Arrow(link), arrow_end)));
            }
        }

        // 2) START_LINK + edgeText + LINK (new notation): A-- text -->B
        let parse_start_link = |pos: usize| -> Option<(usize, LinkFamily, String, usize)> {
            let len = bytes.len();
            let token_start = pos;
            if token_start >= len {
                return None;
            }
            let mut cur = token_start;
            if matches!(bytes[cur], b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'-' {
                cur += 2;
                let token = self.input[token_start..cur]
                    .split_whitespace()
                    .collect::<String>();
                return Some((token_start, LinkFamily::Normal, token, cur));
            }
            if cur + 1 < len && bytes[cur] == b'=' && bytes[cur + 1] == b'=' {
                cur += 2;
                let token = self.input[token_start..cur]
                    .split_whitespace()
                    .collect::<String>();
                return Some((token_start, LinkFamily::Thick, token, cur));
            }
            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'.' {
                cur += 2;
                let token = self.input[token_start..cur]
                    .split_whitespace()
                    .collect::<String>();
                return Some((token_start, LinkFamily::Dotted, token, cur));
            }
            None
        };

        let Some((_sstart, family, start_link, after_start)) = parse_start_link(self.pos) else {
            return None;
        };
        let edge_text_start = after_start;
        let mut scan = edge_text_start;
        while scan < self.input.len() {
            if let Some((match_start, match_end, arrow)) = match_link_end(scan, family, true) {
                let raw_text = self.input[edge_text_start..match_start].trim();
                let (text, kind) = parse_label_text(raw_text);
                self.pos = match_end;

                while self.pos < self.input.len() && is_space_ws(bytes[self.pos]) {
                    self.pos += 1;
                }

                if !text.is_empty() {
                    self.pending.push_back((
                        edge_text_start,
                        Tok::EdgeLabel(LabeledText { text, kind }),
                        match_start,
                    ));
                }
                let link = match compute_link(arrow, Some(start_link)) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                return Some(Ok((start, Tok::Arrow(link), match_end)));
            }
            scan += 1;
        }

        Some(Err(LexError {
            message: "Unterminated edge label (missing link terminator)".to_string(),
        }))
    }

    fn lex_node_label(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let rest = &self.input[self.pos..];

        if rest.starts_with("[\\") {
            let open = "[\\";
            let content_start = self.pos + open.len();
            let end_slash = find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    return Some(Err(LexError {
                        message: "Unterminated node label (missing `/]` or `\\]`)".to_string(),
                    }));
                }
                (Some(p), None) => (p, "/]", "inv_trapezoid"),
                (None, Some(p)) => (p, "\\]", "lean_left"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "inv_trapezoid")
                    } else {
                        (b, "\\]", "lean_left")
                    }
                }
            };

            let raw = self.input[content_start..end_start].trim();
            let lt = match parse_node_label_text(raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + close.len();
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: shape.to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        if rest.starts_with("[/") {
            let open = "[/";
            let content_start = self.pos + open.len();
            let end_slash = find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    return Some(Err(LexError {
                        message: "Unterminated node label (missing `/]` or `\\]`)".to_string(),
                    }));
                }
                (Some(p), None) => (p, "/]", "lean_right"),
                (None, Some(p)) => (p, "\\]", "trapezoid"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "lean_right")
                    } else {
                        (b, "\\]", "trapezoid")
                    }
                }
            };

            let raw = self.input[content_start..end_start].trim();
            let lt = match parse_node_label_text(raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + close.len();
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: shape.to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        let candidates: [(&str, &str, &str); 8] = [
            ("(((", ")))", "doublecircle"),
            ("{{", "}}", "hexagon"),
            ("[[", "]]", "subroutine"),
            ("(-", "-)", "ellipse"),
            ("([", "])", "stadium"),
            ("[(", ")]", "cylinder"),
            ("((", "))", "circle"),
            (">", "]", "odd"),
        ];

        for (open, close, shape) in candidates {
            if !rest.starts_with(open) {
                continue;
            }
            let content_start = self.pos + open.len();
            let Some(end_start) = find_unquoted_delim(self.input, content_start, close) else {
                return Some(Err(LexError {
                    message: format!("Unterminated node label (missing `{close}`)"),
                }));
            };
            let raw = self.input[content_start..end_start].trim();
            let lt = match parse_node_label_text(raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + close.len();
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: shape.to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        if rest.starts_with("[") {
            let content_start = self.pos + 1;
            let Some(end_start) = find_unquoted_delim(self.input, content_start, "]") else {
                return Some(Err(LexError {
                    message: "Unterminated node label (missing `]`)".to_string(),
                }));
            };
            let raw = self.input[content_start..end_start].trim();
            let (shape, label_raw) = parse_rect_border_label(raw);
            let lt = match parse_node_label_text(label_raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + 1;
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: shape.to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        if rest.starts_with("{") {
            let content_start = self.pos + 1;
            let Some(end_start) = find_unquoted_delim(self.input, content_start, "}") else {
                return Some(Err(LexError {
                    message: "Unterminated node label (missing `}`)".to_string(),
                }));
            };
            let raw = self.input[content_start..end_start].trim();
            let lt = match parse_node_label_text(raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + 1;
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: "diamond".to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        if rest.starts_with("(") {
            let content_start = self.pos + 1;
            let Some(end_start) = find_unquoted_delim(self.input, content_start, ")") else {
                return Some(Err(LexError {
                    message: "Unterminated node label (missing `)`)".to_string(),
                }));
            };
            let raw = self.input[content_start..end_start].trim();
            let lt = match parse_node_label_text(raw) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.pos = end_start + 1;
            return Some(Ok((
                start,
                Tok::NodeLabel(NodeLabelToken {
                    shape: "round".to_string(),
                    text: lt,
                }),
                self.pos,
            )));
        }

        None
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(Ok(tok));
        }

        loop {
            if self.pos >= self.input.len() {
                return None;
            }

            if let Some(sep) = self.lex_sep() {
                self.allow_header_direction = false;
                return Some(Ok(sep));
            }
            self.skip_ws();
            if self.pos >= self.input.len() {
                return None;
            }
            if let Some(sep) = self.lex_sep() {
                self.allow_header_direction = false;
                return Some(Ok(sep));
            }
            if let Some(sep) = self.lex_comment() {
                self.allow_header_direction = false;
                return Some(Ok(sep));
            }
            self.skip_ws();
            if self.pos >= self.input.len() {
                return None;
            }

            let start = self.pos;
            if let Some(tok) = self.lex_direction_stmt() {
                return Some(Ok(tok));
            }
            if let Some(res) = self.lex_style_stmt() {
                return Some(res);
            }
            if let Some(res) = self.lex_classdef_stmt() {
                return Some(res);
            }
            if let Some(res) = self.lex_class_assign_stmt() {
                return Some(res);
            }
            if let Some(res) = self.lex_click_stmt() {
                return Some(res);
            }
            if let Some(res) = self.lex_link_style_stmt() {
                return Some(res);
            }
            if let Some(tok) = self.lex_shape_data() {
                return Some(Ok(tok));
            }
            if self.starts_with_kw("flowchart-elk") {
                self.pos += "flowchart-elk".len();
                self.allow_header_direction = true;
                return Some(Ok((start, Tok::KwFlowchartElk, self.pos)));
            }
            if self.starts_with_kw("flowchart") {
                self.pos += "flowchart".len();
                self.allow_header_direction = true;
                return Some(Ok((start, Tok::KwFlowchart, self.pos)));
            }
            if self.starts_with_kw("graph") {
                self.pos += "graph".len();
                self.allow_header_direction = true;
                return Some(Ok((start, Tok::KwGraph, self.pos)));
            }
            if self.starts_with_kw("subgraph") {
                self.pos += "subgraph".len();
                if let Some(header) = self.lex_subgraph_header_after_keyword() {
                    self.pending.push_back(header);
                }
                return Some(Ok((start, Tok::KwSubgraph, self.pos)));
            }
            if self.starts_with_kw("end") {
                self.pos += "end".len();
                return Some(Ok((start, Tok::KwEnd, self.pos)));
            }

            if let Some(tok) = self.lex_style_sep() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_direction() {
                return Some(Ok(tok));
            }
            self.allow_header_direction = false;

            if let Some(res) = self.lex_node_label() {
                return Some(res);
            }

            if let Some(res) = self.lex_arrow_and_label() {
                return Some(res);
            }

            if let Some(tok) = self.lex_edge_id() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_id() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_amp() {
                return Some(Ok(tok));
            }

            // Skip unknown single byte to avoid infinite loops.
            let _ = self.bump();
            return Some(Err(LexError {
                message: format!("Unexpected character at {start}"),
            }));
        }
    }
}

pub fn parse_flowchart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let (code, acc_title, acc_descr) = extract_flowchart_accessibility_statements(code);
    let ast = flowchart_grammar::FlowchartAstParser::new()
        .parse(Lexer::new(&code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut build = FlowchartBuildState::new();
    build
        .add_statements(&ast.statements)
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    let FlowchartBuildState {
        nodes,
        edges,
        vertex_calls,
        ..
    } = build;
    let mut nodes = nodes;
    let mut edges = edges;

    let inherit_dir = meta
        .effective_config
        .as_value()
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    let _ = builder.eval_statements(&ast.statements);

    let mut class_defs: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut tooltips: HashMap<String, String> = HashMap::new();
    let mut edge_defaults = EdgeDefaults {
        style: Vec::new(),
        interpolate: None,
    };

    let mut node_index: HashMap<String, usize> = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        node_index.insert(n.id.clone(), idx);
    }
    let mut subgraph_index: HashMap<String, usize> = HashMap::new();
    for (idx, sg) in builder.subgraphs.iter().enumerate() {
        subgraph_index.insert(sg.id.clone(), idx);
    }

    let security_level_loose = meta.effective_config.get_str("securityLevel") == Some("loose");
    apply_semantic_statements(
        &ast.statements,
        &mut nodes,
        &mut node_index,
        &mut edges,
        &mut builder.subgraphs,
        &mut subgraph_index,
        &mut class_defs,
        &mut tooltips,
        &mut edge_defaults,
        security_level_loose,
        &meta.diagram_type,
        &meta.effective_config,
    )?;

    fn get_layout_shape(n: &Node) -> String {
        // Mirrors Mermaid FlowDB `getTypeFromVertex` logic at 11.12.2.
        if n.img.is_some() {
            return "imageSquare".to_string();
        }
        if n.icon.is_some() {
            match n.form.as_deref() {
                Some("circle") => return "iconCircle".to_string(),
                Some("square") => return "iconSquare".to_string(),
                Some("rounded") => return "iconRounded".to_string(),
                _ => return "icon".to_string(),
            }
        }
        match n.shape.as_deref() {
            Some("square") | None => "squareRect".to_string(),
            Some("round") => "roundedRect".to_string(),
            Some("ellipse") => "ellipse".to_string(),
            Some(other) => other.to_string(),
        }
    }

    fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
        // Mermaid encodes `#quot;`-style placeholders in the parser and later decodes them before
        // rendering (`encodeEntities` / `decodeEntities`). In our headless pipeline we decode them
        // at parse time so layout + SVG output match upstream.
        if !input.contains('#') {
            return std::borrow::Cow::Borrowed(input);
        }

        fn decode_entity(entity: &str) -> Option<char> {
            match entity {
                "nbsp" => Some(' '),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => {
                    if let Some(hex) = entity
                        .strip_prefix("x")
                        .or_else(|| entity.strip_prefix("X"))
                    {
                        u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                    } else if entity.chars().all(|c| c.is_ascii_digit() || c == '+') {
                        entity
                            .trim_start_matches('+')
                            .parse::<u32>()
                            .ok()
                            .and_then(char::from_u32)
                    } else {
                        None
                    }
                }
            }
        }

        let mut out = String::with_capacity(input.len());
        let mut it = input.chars().peekable();
        while let Some(ch) = it.next() {
            if ch != '#' {
                out.push(ch);
                continue;
            }
            let mut entity = String::new();
            let mut ok = false;
            for _ in 0..32 {
                match it.peek().copied() {
                    Some(';') => {
                        it.next();
                        ok = true;
                        break;
                    }
                    Some(c) if c.is_ascii_alphanumeric() || c == '+' => {
                        entity.push(c);
                        it.next();
                    }
                    _ => break,
                }
            }
            if ok {
                if let Some(decoded) = decode_entity(&entity) {
                    out.push(decoded);
                } else {
                    out.push('#');
                    out.push_str(&entity);
                    out.push(';');
                }
            } else {
                out.push('#');
                out.push_str(&entity);
            }
        }
        std::borrow::Cow::Owned(out)
    }

    Ok(json!({
        "type": meta.diagram_type,
        "keyword": ast.keyword,
        "direction": ast.direction,
        "accTitle": acc_title,
        "accDescr": acc_descr,
        "classDefs": class_defs,
        "tooltips": tooltips.into_iter().collect::<HashMap<_, _>>(),
        "edgeDefaults": {
            "style": edge_defaults.style,
            "interpolate": edge_defaults.interpolate,
        },
        "vertexCalls": vertex_calls,
        "nodes": nodes.into_iter().map(|n| {
            let layout_shape = get_layout_shape(&n);
            let label_raw = n.label.clone().unwrap_or_else(|| n.id.clone());
            let label_raw = decode_mermaid_hash_entities(&label_raw);
            let mut label = sanitize_text(&label_raw, &meta.effective_config);
            if label.len() >= 2 && label.starts_with('\"') && label.ends_with('\"') {
                label = label[1..label.len() - 1].to_string();
            }
            json!({
                "id": n.id,
                "label": label,
                "labelType": title_kind_str(&n.label_type),
                "shape": n.shape,
                "layoutShape": layout_shape,
                "icon": n.icon,
                "form": n.form,
                "pos": n.pos,
                "img": n.img,
                "constraint": n.constraint,
                "assetWidth": n.asset_width,
                "assetHeight": n.asset_height,
                "styles": n.styles,
                "classes": n.classes,
                "link": n.link,
                "linkTarget": n.link_target,
                "haveCallback": n.have_callback,
            })
        }).collect::<Vec<_>>(),
        "edges": edges.into_iter().map(|e| {
            let label = e
                .label
                .as_ref()
                .map(|s| {
                    let decoded = decode_mermaid_hash_entities(s);
                    sanitize_text(&decoded, &meta.effective_config)
                });
            json!({
                "from": e.from,
                "to": e.to,
                "id": e.id,
                "isUserDefinedId": e.is_user_defined_id,
                "arrow": e.link.end,
                "type": e.link.edge_type,
                "stroke": e.link.stroke,
                "length": e.link.length,
                "label": label,
                "labelType": title_kind_str(&e.label_type),
                "style": e.style,
                "classes": e.classes,
                "interpolate": e.interpolate,
                "animate": e.animate,
                "animation": e.animation,
            })
        }).collect::<Vec<_>>(),
        "subgraphs": builder.subgraphs.into_iter().map(flow_subgraph_to_json).collect::<Vec<_>>(),
    }))
}

fn extract_flowchart_accessibility_statements(
    code: &str,
) -> (String, Option<String>, Option<String>) {
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;
    let mut out = String::with_capacity(code.len());

    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("accTitle") {
            let rest = rest.trim_start();
            if rest.starts_with(':') {
                acc_title = Some(rest[1..].trim().to_string());
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("accDescr") {
            let rest = rest.trim_start();
            if rest.starts_with(':') {
                acc_descr = Some(rest[1..].trim().to_string());
                continue;
            }

            if rest.starts_with('{') {
                let mut buf = String::new();

                let mut after = rest[1..].to_string();
                if let Some(end) = after.find('}') {
                    after.truncate(end);
                    acc_descr = Some(after.trim().to_string());
                    continue;
                }
                let after = after.trim_start();
                if !after.is_empty() {
                    buf.push_str(after);
                }

                while let Some(raw) = lines.next() {
                    if let Some(pos) = raw.find('}') {
                        let part = &raw[..pos];
                        if !buf.is_empty() {
                            buf.push('\n');
                        }
                        buf.push_str(part);
                        break;
                    }

                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(raw);
                }

                acc_descr = Some(buf.trim().to_string());
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    (out, acc_title, acc_descr)
}

struct FlowchartBuildState {
    nodes: Vec<Node>,
    node_index: HashMap<String, usize>,
    edges: Vec<Edge>,
    used_edge_ids: HashSet<String>,
    edge_pair_counts: HashMap<(String, String), usize>,
    vertex_calls: Vec<String>,
}

impl FlowchartBuildState {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            used_edge_ids: HashSet::new(),
            edge_pair_counts: HashMap::new(),
            vertex_calls: Vec::new(),
        }
    }

    fn add_statements(&mut self, statements: &[Stmt]) -> std::result::Result<(), String> {
        for stmt in statements {
            match stmt {
                Stmt::Chain { nodes, edges } => {
                    for mut n in nodes.iter().cloned() {
                        // Mermaid FlowDB `vertexCounter` increments on every `addVertex(...)` call.
                        // Our grammar models `shapeData` attachments in the AST, so we can replay the
                        // observable call sequence:
                        // - once for the vertex token itself
                        // - once more if a `@{ ... }` shapeData block is present
                        self.vertex_calls.push(n.id.clone());
                        if n.shape_data.is_some() {
                            self.vertex_calls.push(n.id.clone());
                        }
                        if let Some(sd) = n.shape_data.take() {
                            apply_shape_data_to_node(&mut n, &sd)?;
                        }
                        self.upsert_node(n);
                    }
                    for e in edges.iter().cloned() {
                        self.push_edge(e);
                    }
                }
                Stmt::Node(n) => {
                    let mut n = n.clone();
                    self.vertex_calls.push(n.id.clone());
                    if n.shape_data.is_some() {
                        self.vertex_calls.push(n.id.clone());
                    }
                    if let Some(sd) = n.shape_data.take() {
                        apply_shape_data_to_node(&mut n, &sd)?;
                    }
                    self.upsert_node(n);
                }
                Stmt::ShapeData { target, .. } => {
                    // Mermaid applies shapeData to edges if (and only if) an edge with that ID exists.
                    // For ordering parity we only insert a placeholder node when this currently refers to a node.
                    if !self.used_edge_ids.contains(target) {
                        // The upstream flowchart parser calls `addVertex(id)` and then
                        // `addVertex(id, ..., shapeData)` for `id@{...}` statements.
                        self.vertex_calls.push(target.clone());
                        self.vertex_calls.push(target.clone());
                    }
                    if !self.used_edge_ids.contains(target) && !self.node_index.contains_key(target)
                    {
                        let idx = self.nodes.len();
                        self.nodes.push(Node {
                            id: target.clone(),
                            label: None,
                            label_type: TitleKind::Text,
                            shape: None,
                            shape_data: None,
                            icon: None,
                            form: None,
                            pos: None,
                            img: None,
                            constraint: None,
                            asset_width: None,
                            asset_height: None,
                            styles: Vec::new(),
                            classes: Vec::new(),
                            link: None,
                            link_target: None,
                            have_callback: false,
                        });
                        self.node_index.insert(target.clone(), idx);
                    }
                }
                Stmt::Subgraph(sg) => self.add_statements(&sg.statements)?,
                Stmt::Direction(_)
                | Stmt::ClassDef(_)
                | Stmt::ClassAssign(_)
                | Stmt::Click(_)
                | Stmt::LinkStyle(_) => {}
                Stmt::Style(s) => {
                    // Mermaid's `style` statement routes through FlowDB `addVertex(id, ..., styles)`.
                    // This increments `vertexCounter` for nodes (but is a no-op for edges).
                    if !self.used_edge_ids.contains(&s.target) {
                        self.vertex_calls.push(s.target.clone());
                        if !self.node_index.contains_key(&s.target) {
                            let idx = self.nodes.len();
                            self.nodes.push(Node {
                                id: s.target.clone(),
                                label: None,
                                label_type: TitleKind::Text,
                                shape: None,
                                shape_data: None,
                                icon: None,
                                form: None,
                                pos: None,
                                img: None,
                                constraint: None,
                                asset_width: None,
                                asset_height: None,
                                styles: Vec::new(),
                                classes: Vec::new(),
                                link: None,
                                link_target: None,
                                have_callback: false,
                            });
                            self.node_index.insert(s.target.clone(), idx);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn upsert_node(&mut self, n: Node) {
        if let Some(&idx) = self.node_index.get(&n.id) {
            if n.label.is_some() {
                self.nodes[idx].label = n.label;
                self.nodes[idx].label_type = n.label_type;
            }
            if n.shape.is_some() {
                self.nodes[idx].shape = n.shape;
            }
            if n.icon.is_some() {
                self.nodes[idx].icon = n.icon;
            }
            if n.form.is_some() {
                self.nodes[idx].form = n.form;
            }
            if n.pos.is_some() {
                self.nodes[idx].pos = n.pos;
            }
            if n.img.is_some() {
                self.nodes[idx].img = n.img;
            }
            if n.constraint.is_some() {
                self.nodes[idx].constraint = n.constraint;
            }
            if n.asset_width.is_some() {
                self.nodes[idx].asset_width = n.asset_width;
            }
            if n.asset_height.is_some() {
                self.nodes[idx].asset_height = n.asset_height;
            }
            self.nodes[idx].styles.extend(n.styles);
            self.nodes[idx].classes.extend(n.classes);
            return;
        }
        let idx = self.nodes.len();
        self.node_index.insert(n.id.clone(), idx);
        self.nodes.push(n);
    }

    fn push_edge(&mut self, mut e: Edge) {
        let key = (e.from.clone(), e.to.clone());
        let existing = *self.edge_pair_counts.get(&key).unwrap_or(&0);

        let mut final_id = e.id.clone();
        let mut is_user_defined_id = false;
        if let Some(user_id) = e.id.clone() {
            if !self.used_edge_ids.contains(&user_id) {
                is_user_defined_id = true;
                self.used_edge_ids.insert(user_id);
            } else {
                final_id = None;
            }
        }

        if final_id.is_none() {
            let counter = if existing == 0 { 0 } else { existing + 1 };
            final_id = Some(format!("L_{}_{}_{}", e.from, e.to, counter));
            if let Some(id) = final_id.clone() {
                self.used_edge_ids.insert(id);
            }
        }

        self.edge_pair_counts.insert(key, existing + 1);

        e.id = final_id;
        e.is_user_defined_id = is_user_defined_id;
        e.link.length = e.link.length.min(10);
        self.edges.push(e);
    }
}

fn parse_shape_data_yaml(yaml_body: &str) -> std::result::Result<serde_yaml::Value, String> {
    let yaml_data = if yaml_body.contains('\n') {
        format!("{yaml_body}\n")
    } else {
        format!("{{\n{yaml_body}\n}}")
    };
    serde_yaml::from_str(&yaml_data).map_err(|e| format!("{e}"))
}

const MERMAID_SHAPES_11_12_2: &[&str] = &[
    "anchor",
    "bang",
    "bolt",
    "bow-rect",
    "bow-tie-rectangle",
    "brace",
    "brace-l",
    "brace-r",
    "braces",
    "card",
    "choice",
    "circ",
    "circle",
    "classBox",
    "cloud",
    "collate",
    "com-link",
    "comment",
    "cross-circ",
    "crossed-circle",
    "curv-trap",
    "curved-trapezoid",
    "cyl",
    "cylinder",
    "das",
    "database",
    "db",
    "dbl-circ",
    "decision",
    "defaultMindmapNode",
    "delay",
    "diam",
    "diamond",
    "disk",
    "display",
    "div-proc",
    "div-rect",
    "divided-process",
    "divided-rectangle",
    "doc",
    "docs",
    "document",
    "documents",
    "double-circle",
    "doublecircle",
    "erBox",
    "event",
    "extract",
    "f-circ",
    "filled-circle",
    "flag",
    "flip-tri",
    "flipped-triangle",
    "fork",
    "forkJoin",
    "fr-circ",
    "fr-rect",
    "framed-circle",
    "framed-rectangle",
    "h-cyl",
    "half-rounded-rectangle",
    "hex",
    "hexagon",
    "horizontal-cylinder",
    "hourglass",
    "icon",
    "iconCircle",
    "iconRounded",
    "iconSquare",
    "imageSquare",
    "in-out",
    "internal-storage",
    "inv-trapezoid",
    "inv_trapezoid",
    "join",
    "junction",
    "kanbanItem",
    "labelRect",
    "lean-l",
    "lean-left",
    "lean-r",
    "lean-right",
    "lean_left",
    "lean_right",
    "lightning-bolt",
    "lin-cyl",
    "lin-doc",
    "lin-proc",
    "lin-rect",
    "lined-cylinder",
    "lined-document",
    "lined-process",
    "lined-rectangle",
    "loop-limit",
    "manual",
    "manual-file",
    "manual-input",
    "mindmapCircle",
    "notch-pent",
    "notch-rect",
    "notched-pentagon",
    "notched-rectangle",
    "note",
    "odd",
    "out-in",
    "paper-tape",
    "pill",
    "prepare",
    "priority",
    "proc",
    "process",
    "processes",
    "procs",
    "question",
    "rect",
    "rectWithTitle",
    "rect_left_inv_arrow",
    "rectangle",
    "requirementBox",
    "rounded",
    "roundedRect",
    "shaded-process",
    "sl-rect",
    "sloped-rectangle",
    "sm-circ",
    "small-circle",
    "squareRect",
    "st-doc",
    "st-rect",
    "stacked-document",
    "stacked-rectangle",
    "stadium",
    "start",
    "state",
    "stateEnd",
    "stateStart",
    "stop",
    "stored-data",
    "subproc",
    "subprocess",
    "subroutine",
    "summary",
    "tag-doc",
    "tag-proc",
    "tag-rect",
    "tagged-document",
    "tagged-process",
    "tagged-rectangle",
    "terminal",
    "text",
    "trap-b",
    "trap-t",
    "trapezoid",
    "trapezoid-bottom",
    "trapezoid-top",
    "tri",
    "triangle",
    "win-pane",
    "window-pane",
];

fn is_valid_shape_11_12_2(shape: &str) -> bool {
    MERMAID_SHAPES_11_12_2.binary_search(&shape).is_ok()
}

fn yaml_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_to_bool(v: &serde_yaml::Value) -> Option<bool> {
    match v {
        serde_yaml::Value::Bool(b) => Some(*b),
        serde_yaml::Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn yaml_to_f64(v: &serde_yaml::Value) -> Option<f64> {
    match v {
        serde_yaml::Value::Number(n) => n.as_f64(),
        serde_yaml::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn apply_shape_data_to_node(node: &mut Node, yaml_body: &str) -> std::result::Result<(), String> {
    // If shapeData is attached to a node reference, Mermaid has already decided this is a node.
    let v = parse_shape_data_yaml(yaml_body)?;
    let map = match v.as_mapping() {
        Some(m) => m,
        None => return Ok(()),
    };

    let mut provided_label: Option<String> = None;
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "shape" => {
                let Some(shape) = v.as_str() else { continue };
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }
                if !is_valid_shape_11_12_2(shape) {
                    return Err(format!("No such shape: {shape}."));
                }
                node.shape = Some(shape.to_string());
            }
            "label" => {
                if let Some(label) = yaml_to_string(v) {
                    provided_label = Some(label.clone());
                    node.label = Some(label);
                    node.label_type = TitleKind::Text;
                }
            }
            "icon" => {
                if let Some(icon) = yaml_to_string(v) {
                    node.icon = Some(icon);
                }
            }
            "form" => {
                if let Some(form) = yaml_to_string(v) {
                    node.form = Some(form);
                }
            }
            "pos" => {
                if let Some(pos) = yaml_to_string(v) {
                    node.pos = Some(pos);
                }
            }
            "img" => {
                if let Some(img) = yaml_to_string(v) {
                    node.img = Some(img);
                }
            }
            "constraint" => {
                if let Some(constraint) = yaml_to_string(v) {
                    node.constraint = Some(constraint);
                }
            }
            "w" => {
                if let Some(w) = yaml_to_f64(v) {
                    node.asset_width = Some(w);
                }
            }
            "h" => {
                if let Some(h) = yaml_to_f64(v) {
                    node.asset_height = Some(h);
                }
            }
            _ => {}
        }
    }

    // Mermaid clears the default label when an icon or img is set without an explicit label.
    let has_visual = node.icon.is_some() || node.img.is_some();
    let label_is_empty_or_missing = provided_label
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if has_visual && label_is_empty_or_missing {
        let current_text = node.label.as_deref().unwrap_or(node.id.as_str());
        if current_text == node.id {
            node.label = Some(String::new());
            node.label_type = TitleKind::Text;
        }
    }

    Ok(())
}

fn count_char(ch: char, s: &str) -> usize {
    s.chars().filter(|&c| c == ch).count()
}

fn destruct_start_link(s: &str) -> (&'static str, &'static str) {
    let mut str = s.trim();
    let mut edge_type = "arrow_open";
    if let Some(first) = str.as_bytes().first().copied() {
        match first {
            b'<' => {
                edge_type = "arrow_point";
                str = &str[1..];
            }
            b'x' => {
                edge_type = "arrow_cross";
                str = &str[1..];
            }
            b'o' => {
                edge_type = "arrow_circle";
                str = &str[1..];
            }
            _ => {}
        }
    }

    let mut stroke = "normal";
    if str.contains('=') {
        stroke = "thick";
    }
    if str.contains('.') {
        stroke = "dotted";
    }
    (edge_type, stroke)
}

fn destruct_end_link(s: &str) -> (String, String, usize) {
    let str = s.trim();
    if str.len() < 2 {
        return ("arrow_open".to_string(), "normal".to_string(), 1);
    }
    let mut line = &str[..str.len() - 1];
    let mut edge_type = "arrow_open".to_string();

    match str.as_bytes()[str.len() - 1] {
        b'x' => {
            edge_type = "arrow_cross".to_string();
            if str.as_bytes().first().copied() == Some(b'x') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        b'>' => {
            edge_type = "arrow_point".to_string();
            if str.as_bytes().first().copied() == Some(b'<') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        b'o' => {
            edge_type = "arrow_circle".to_string();
            if str.as_bytes().first().copied() == Some(b'o') {
                edge_type = format!("double_{edge_type}");
                line = &line[1..];
            }
        }
        _ => {}
    }

    let mut stroke = "normal".to_string();
    let mut length = line.len().saturating_sub(1);

    if line.starts_with('=') {
        stroke = "thick".to_string();
    }
    if line.starts_with('~') {
        stroke = "invisible".to_string();
    }

    let dots = count_char('.', line);
    if dots > 0 {
        stroke = "dotted".to_string();
        length = dots;
    }

    (edge_type, stroke, length)
}

fn flow_subgraph_to_json(sg: FlowSubGraph) -> Value {
    json!({
        "id": sg.id,
        "nodes": sg.nodes,
        "title": sg.title,
        "classes": sg.classes,
        "dir": sg.dir,
        "labelType": sg.label_type,
    })
}

#[allow(dead_code)]
fn collect_nodes_and_edges(statements: &[Stmt], nodes: &mut Vec<Node>, edges: &mut Vec<Edge>) {
    for stmt in statements {
        match stmt {
            Stmt::Chain {
                nodes: chain_nodes,
                edges: chain_edges,
            } => {
                nodes.extend(chain_nodes.iter().cloned());
                edges.extend(chain_edges.iter().cloned());
            }
            Stmt::Node(n) => nodes.push(n.clone()),
            Stmt::Subgraph(sg) => collect_nodes_and_edges(&sg.statements, nodes, edges),
            Stmt::Direction(_) => {}
            Stmt::Style(_) => {}
            Stmt::ClassDef(_) => {}
            Stmt::ClassAssign(_) => {}
            Stmt::Click(_) => {}
            Stmt::LinkStyle(_) => {}
            Stmt::ShapeData { .. } => {}
        }
    }
}

#[allow(dead_code)]
fn merge_nodes_and_edges(nodes: Vec<Node>, edges: Vec<Edge>) -> (Vec<Node>, Vec<Edge>) {
    let mut nodes_by_id: HashMap<String, usize> = HashMap::new();
    let mut merged: Vec<Node> = Vec::new();
    for n in nodes {
        if let Some(&idx) = nodes_by_id.get(&n.id) {
            if n.label.is_some() {
                merged[idx].label = n.label;
                merged[idx].label_type = n.label_type.clone();
            }
            if n.shape.is_some() {
                merged[idx].shape = n.shape;
            }
            merged[idx].styles.extend(n.styles);
            merged[idx].classes.extend(n.classes);
            continue;
        }
        let idx = merged.len();
        nodes_by_id.insert(n.id.clone(), idx);
        merged.push(n);
    }
    (merged, edges)
}

fn title_kind_str(kind: &TitleKind) -> &'static str {
    match kind {
        TitleKind::Text => "text",
        TitleKind::String => "string",
        TitleKind::Markdown => "markdown",
    }
}

fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return s[1..s.len() - 1].to_string();
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

#[derive(Debug, Clone)]
enum StatementItem {
    Id(String),
    Dir(String),
}

struct SubgraphBuilder {
    sub_count: usize,
    subgraphs: Vec<FlowSubGraph>,
    inherit_dir: bool,
    global_dir: Option<String>,
}

impl SubgraphBuilder {
    fn new(inherit_dir: bool, global_dir: Option<String>) -> Self {
        Self {
            sub_count: 0,
            subgraphs: Vec::new(),
            inherit_dir,
            global_dir,
        }
    }

    fn eval_statements(&mut self, statements: &[Stmt]) -> Vec<StatementItem> {
        let mut out: Vec<StatementItem> = Vec::new();
        for stmt in statements {
            match stmt {
                Stmt::Chain { nodes, edges } => {
                    // Mermaid FlowDB's subgraph membership list is based on the Jison `vertexStatement.nodes`
                    // shape, which prepends the last node in a chain first (e.g. `a-->b` yields `[b, a]`).
                    //
                    // For node-only group statements (e.g. `A & B`), there are no edges and the list
                    // preserves the input order.
                    if edges.is_empty() {
                        for n in nodes {
                            out.push(StatementItem::Id(n.id.clone()));
                        }
                    } else {
                        for n in nodes.iter().rev() {
                            out.push(StatementItem::Id(n.id.clone()));
                        }
                    }
                }
                Stmt::Node(n) => out.push(StatementItem::Id(n.id.clone())),
                Stmt::Direction(d) => out.push(StatementItem::Dir(d.clone())),
                Stmt::Subgraph(sg) => {
                    let id = self.eval_subgraph(sg);
                    out.push(StatementItem::Id(id));
                }
                Stmt::Style(_) => {}
                Stmt::ClassDef(_) => {}
                Stmt::ClassAssign(_) => {}
                Stmt::Click(_) => {}
                Stmt::LinkStyle(_) => {}
                Stmt::ShapeData { .. } => {}
            }
        }
        out
    }

    fn eval_subgraph(&mut self, sg: &SubgraphBlock) -> String {
        let items = self.eval_statements(&sg.statements);
        let mut seen: HashSet<String> = HashSet::new();
        let mut members: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;

        for item in items {
            match item {
                StatementItem::Dir(d) => dir = Some(d),
                StatementItem::Id(id) => {
                    if id.trim().is_empty() {
                        continue;
                    }
                    if seen.insert(id.clone()) {
                        members.push(id);
                    }
                }
            }
        }

        let dir = dir.or_else(|| {
            if self.inherit_dir {
                self.global_dir.clone()
            } else {
                None
            }
        });

        let raw_id = unquote(&sg.header.raw_id);
        let (title_raw, title_kind) =
            parse_subgraph_title(&sg.header.raw_title, sg.header.id_equals_title);
        let id_raw = strip_wrapping_backticks(raw_id.trim()).0;

        let mut id: Option<String> = {
            let trimmed = id_raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };

        if sg.header.id_equals_title && title_raw.chars().any(|c| c.is_whitespace()) {
            id = None;
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        let title = title_raw.trim().to_string();
        let label_type = match title_kind {
            TitleKind::Text => "text",
            TitleKind::String => "string",
            TitleKind::Markdown => "markdown",
        }
        .to_string();

        self.sub_count += 1;

        members.retain(|m| !subgraphs_exist(&self.subgraphs, m));

        self.subgraphs.push(FlowSubGraph {
            id: id.clone(),
            nodes: members,
            title,
            classes: Vec::new(),
            dir,
            label_type,
        });

        id
    }
}

fn subgraphs_exist(subgraphs: &[FlowSubGraph], node_id: &str) -> bool {
    subgraphs
        .iter()
        .any(|sg| sg.nodes.iter().any(|n| n == node_id))
}

fn parse_subgraph_title(raw_title: &str, id_equals_title: bool) -> (String, TitleKind) {
    let trimmed = raw_title.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    let unquoted = if quoted {
        unquote(trimmed)
    } else {
        trimmed.to_string()
    };

    let (no_backticks, is_markdown) = strip_wrapping_backticks(unquoted.trim());
    if is_markdown {
        return (no_backticks, TitleKind::Markdown);
    }

    if !id_equals_title && quoted {
        return (unquoted, TitleKind::String);
    }

    (unquoted, TitleKind::Text)
}

fn parse_label_text(raw: &str) -> (String, TitleKind) {
    let trimmed = raw.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    let unquoted = if quoted {
        unquote(trimmed)
    } else {
        trimmed.to_string()
    };

    let (no_backticks, is_markdown) = strip_wrapping_backticks(unquoted.trim());
    if is_markdown {
        return (no_backticks, TitleKind::Markdown);
    }
    if quoted {
        return (unquoted, TitleKind::String);
    }
    (unquoted, TitleKind::Text)
}

fn parse_node_label_text(raw: &str) -> std::result::Result<LabeledText, LexError> {
    let trimmed = raw.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    let quote_char = trimmed.as_bytes().first().copied();

    let (text, kind) = parse_label_text(raw);

    match kind {
        TitleKind::Text => {
            // Mermaid Jison-based flowchart lexer treats these as structural tokens (PS/PE/SQE/etc)
            // and will throw parse errors if they appear inside TEXT.
            if text.contains('"')
                || text.contains('(')
                || text.contains(')')
                || text.contains('[')
                || text.contains(']')
                || text.contains('{')
                || text.contains('}')
            {
                return Err(LexError {
                    message:
                        "Invalid text label: contains structural characters; quote it to use them"
                            .to_string(),
                });
            }
        }
        TitleKind::String => {
            // Jison string state terminates at the first matching quote; nested quotes are invalid.
            if quoted {
                if let Some(q) = quote_char {
                    let inner = &trimmed[1..trimmed.len().saturating_sub(1)];
                    let q = q as char;
                    if inner.contains(q) {
                        return Err(LexError {
                            message: "Invalid string label: contains nested quotes".to_string(),
                        });
                    }
                }
            }
        }
        TitleKind::Markdown => {}
    }

    Ok(LabeledText { text, kind })
}

fn parse_rect_border_label(raw: &str) -> (&'static str, &str) {
    // Mermaid supports a special "rect" variant via `[|borders:...|Label]`.
    // We only need the shape name and the actual label payload here.
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix('|') else {
        return ("square", trimmed);
    };
    let Some((prefix, label)) = rest.split_once('|') else {
        return ("square", trimmed);
    };
    if prefix.trim_start().starts_with("borders:") {
        return ("rect", label);
    }
    ("square", trimmed)
}

fn find_unquoted_delim(input: &str, start: usize, delim: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let delim_bytes = delim.as_bytes();
    let mut pos = start;

    while pos + delim_bytes.len() <= len {
        if bytes[pos..pos + delim_bytes.len()] == *delim_bytes {
            return Some(pos);
        }

        // Do not scan across statements.
        if bytes[pos] == b';' || bytes[pos] == b'\n' {
            return None;
        }

        match bytes[pos] {
            b'"' | b'\'' | b'`' => {
                let quote = bytes[pos];
                pos += 1;
                while pos < len {
                    if bytes[pos] == quote && (pos == 0 || bytes[pos - 1] != b'\\') {
                        pos += 1;
                        break;
                    }
                    pos += 1;
                }
            }
            _ => pos += 1,
        }
    }

    None
}

fn strip_wrapping_backticks(s: &str) -> (String, bool) {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('`') && trimmed.ends_with('`') {
        return (trimmed[1..trimmed.len() - 1].to_string(), true);
    }
    (trimmed.to_string(), false)
}

fn apply_semantic_statements(
    statements: &[Stmt],
    nodes: &mut Vec<Node>,
    node_index: &mut HashMap<String, usize>,
    edges: &mut Vec<Edge>,
    subgraphs: &mut Vec<FlowSubGraph>,
    subgraph_index: &mut HashMap<String, usize>,
    class_defs: &mut IndexMap<String, Vec<String>>,
    tooltips: &mut HashMap<String, String>,
    edge_defaults: &mut EdgeDefaults,
    security_level_loose: bool,
    diagram_type: &str,
    config: &MermaidConfig,
) -> Result<()> {
    for stmt in statements {
        match stmt {
            Stmt::Subgraph(sg) => {
                apply_semantic_statements(
                    &sg.statements,
                    nodes,
                    node_index,
                    edges,
                    subgraphs,
                    subgraph_index,
                    class_defs,
                    tooltips,
                    edge_defaults,
                    security_level_loose,
                    diagram_type,
                    config,
                )?;
            }
            Stmt::Style(s) => {
                let idx = ensure_node(nodes, node_index, &s.target);
                nodes[idx].styles.extend(s.styles.iter().cloned());
            }
            Stmt::ClassDef(c) => {
                for id in &c.ids {
                    class_defs.insert(id.clone(), c.styles.clone());
                }
            }
            Stmt::ClassAssign(c) => {
                for target in &c.targets {
                    add_class_to_target(
                        nodes,
                        node_index,
                        subgraphs,
                        subgraph_index,
                        target,
                        &c.class_name,
                    );
                }
            }
            Stmt::Click(c) => {
                for id in &c.ids {
                    if let Some(tt) = &c.tooltip {
                        tooltips.insert(id.clone(), sanitize_text(tt, config));
                    }
                    add_class_to_target(
                        nodes,
                        node_index,
                        subgraphs,
                        subgraph_index,
                        id,
                        "clickable",
                    );

                    match &c.action {
                        ClickAction::Link { href, target } => {
                            if let Some(&idx) = node_index.get(id) {
                                nodes[idx].link = format_url(href, config);
                                nodes[idx].link_target = target.clone();
                            }
                        }
                        ClickAction::Callback { .. } => {
                            if security_level_loose {
                                if let Some(&idx) = node_index.get(id) {
                                    nodes[idx].have_callback = true;
                                }
                            }
                        }
                    }
                }
            }
            Stmt::LinkStyle(ls) => {
                if let Some(algo) = &ls.interpolate {
                    for pos in &ls.positions {
                        match pos {
                            LinkStylePos::Default => edge_defaults.interpolate = Some(algo.clone()),
                            LinkStylePos::Index(i) => {
                                if *i >= edges.len() {
                                    return Err(Error::DiagramParse {
                                        diagram_type: diagram_type.to_string(),
                                        message: format!(
                                            "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                            edges.len().saturating_sub(1)
                                        ),
                                    });
                                }
                                edges[*i].interpolate = Some(algo.clone());
                            }
                        }
                    }
                }

                if !ls.styles.is_empty() {
                    for pos in &ls.positions {
                        match pos {
                            LinkStylePos::Default => edge_defaults.style = ls.styles.clone(),
                            LinkStylePos::Index(i) => {
                                if *i >= edges.len() {
                                    return Err(Error::DiagramParse {
                                        diagram_type: diagram_type.to_string(),
                                        message: format!(
                                            "The index {i} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                                            edges.len().saturating_sub(1)
                                        ),
                                    });
                                }
                                edges[*i].style = ls.styles.clone();
                                if !edges[*i].style.is_empty()
                                    && !edges[*i]
                                        .style
                                        .iter()
                                        .any(|s| s.trim_start().starts_with("fill"))
                                {
                                    edges[*i].style.push("fill:none".to_string());
                                }
                            }
                        }
                    }
                }
            }
            Stmt::ShapeData { target, yaml } => {
                // Mermaid syntax uses the same `@{...}` form for both nodes and edges:
                // - if an edge with the given ID exists, it updates the edge metadata
                // - otherwise it updates (and may create) a node
                let v = parse_shape_data_yaml(yaml).map_err(|e| Error::DiagramParse {
                    diagram_type: diagram_type.to_string(),
                    message: format!("Invalid shapeData: {e}"),
                })?;

                let map = v.as_mapping();
                let is_edge_target = edges
                    .iter()
                    .any(|e| e.id.as_deref() == Some(target.as_str()));
                if is_edge_target {
                    if let Some(map) = map {
                        for e in edges.iter_mut() {
                            if e.id.as_deref() != Some(target.as_str()) {
                                continue;
                            }
                            for (k, v) in map {
                                let Some(key) = k.as_str() else { continue };
                                match key {
                                    "animate" => {
                                        if let Some(b) = yaml_to_bool(v) {
                                            e.animate = Some(b);
                                        }
                                    }
                                    "animation" => {
                                        if let Some(s) = yaml_to_string(v) {
                                            e.animation = Some(s);
                                        }
                                    }
                                    "curve" => {
                                        if let Some(s) = yaml_to_string(v) {
                                            e.interpolate = Some(s);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    continue;
                }

                let idx = ensure_node(nodes, node_index, target);
                apply_shape_data_to_node(&mut nodes[idx], yaml).map_err(|e| {
                    Error::DiagramParse {
                        diagram_type: diagram_type.to_string(),
                        message: e,
                    }
                })?;
            }
            Stmt::Chain { .. } | Stmt::Node(_) | Stmt::Direction(_) => {}
        }
    }
    Ok(())
}

fn add_class_to_target(
    nodes: &mut Vec<Node>,
    node_index: &HashMap<String, usize>,
    subgraphs: &mut Vec<FlowSubGraph>,
    subgraph_index: &HashMap<String, usize>,
    target: &str,
    class_name: &str,
) {
    if let Some(&idx) = node_index.get(target) {
        nodes[idx].classes.push(class_name.to_string());
        return;
    }
    if let Some(&idx) = subgraph_index.get(target) {
        subgraphs[idx].classes.push(class_name.to_string());
    }
}

fn ensure_node(nodes: &mut Vec<Node>, node_index: &mut HashMap<String, usize>, id: &str) -> usize {
    if let Some(&idx) = node_index.get(id) {
        return idx;
    }
    let idx = nodes.len();
    nodes.push(Node {
        id: id.to_string(),
        label: None,
        label_type: TitleKind::Text,
        shape: None,
        shape_data: None,
        icon: None,
        form: None,
        pos: None,
        img: None,
        constraint: None,
        asset_width: None,
        asset_height: None,
        styles: Vec::new(),
        classes: Vec::new(),
        link: None,
        link_target: None,
        have_callback: false,
    });
    node_index.insert(id.to_string(), idx);
    idx
}

fn split_first_word(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let first = &trimmed[..i];
    let rest = &trimmed[i..];
    Some((first, rest))
}

fn parse_styles_list(s: &str) -> Vec<String> {
    // Used by `classDef` / `style` statements. Mermaid normalizes these style tokens by trimming
    // whitespace around each comma-separated entry.
    let placeholder = "\u{0000}";
    let replaced = s.replace("\\,", placeholder);
    replaced
        .split(',')
        .map(|p| p.replace(placeholder, ","))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn parse_linkstyle_styles_list(s: &str) -> Vec<String> {
    // Mermaid's Jison grammar preserves whitespace inside each style token (e.g. `, stroke: ...`
    // becomes `" stroke: ..."`) and downstream FlowDB joins the style list verbatim via
    // `styles.join(';')` (see `flow.jison` + `flowDb.updateLink(...)`).
    //
    // Keep the raw spacing (except for filtering out all-whitespace entries).
    let placeholder = "\u{0000}";
    let replaced = s.replace("\\,", placeholder);
    replaced
        .split(',')
        .map(|p| p.replace(placeholder, ","))
        .filter(|p| !p.trim().is_empty())
        .collect()
}

fn parse_style_stmt(rest: &str) -> std::result::Result<StyleStmt, LexError> {
    let Some((target, styles_raw)) = split_first_word(rest) else {
        return Err(LexError {
            message: "Invalid style statement".to_string(),
        });
    };
    let styles = parse_styles_list(styles_raw);
    Ok(StyleStmt {
        target: target.trim().to_string(),
        styles,
    })
}

fn parse_classdef_stmt(rest: &str) -> std::result::Result<ClassDefStmt, LexError> {
    let Some((ids_raw, styles_raw)) = split_first_word(rest) else {
        return Err(LexError {
            message: "Invalid classDef statement".to_string(),
        });
    };
    let ids = ids_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let styles = parse_styles_list(styles_raw);
    Ok(ClassDefStmt { ids, styles })
}

fn parse_class_assign_stmt(rest: &str) -> std::result::Result<ClassAssignStmt, LexError> {
    let Some((targets_raw, class_raw)) = split_first_word(rest) else {
        return Err(LexError {
            message: "Invalid class statement".to_string(),
        });
    };
    let targets = targets_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let class_name = class_raw.trim().to_string();
    if class_name.is_empty() {
        return Err(LexError {
            message: "Invalid class statement".to_string(),
        });
    }
    Ok(ClassAssignStmt {
        targets,
        class_name,
    })
}

#[derive(Clone)]
struct ClickParse<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> ClickParse<'a> {
    fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }

    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }

    fn take_word(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len() && !self.s.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        Some(self.s[start..self.i].to_string())
    }

    fn take_quoted(&mut self) -> Option<String> {
        self.skip_ws();
        if self.peek()? != b'"' {
            return None;
        }
        self.i += 1;
        let start = self.i;
        while self.i < self.s.len() && self.s.as_bytes()[self.i] != b'"' {
            self.i += 1;
        }
        let out = self.s[start..self.i].to_string();
        if self.i < self.s.len() && self.s.as_bytes()[self.i] == b'"' {
            self.i += 1;
        }
        Some(out)
    }

    fn rest(&self) -> &str {
        &self.s[self.i..]
    }
}

fn parse_click_stmt(rest: &str) -> std::result::Result<ClickStmt, LexError> {
    let mut p = ClickParse::new(rest);
    let Some(id) = p.take_word() else {
        return Err(LexError {
            message: "Invalid click statement".to_string(),
        });
    };
    let ids = vec![id];

    p.skip_ws();
    let tooltip: Option<String>;
    let action: ClickAction;

    if p.rest().starts_with("href")
        && p.rest()
            .as_bytes()
            .get(4)
            .map_or(true, |b| b.is_ascii_whitespace())
    {
        let _ = p.take_word();
        let Some(link) = p.take_quoted() else {
            return Err(LexError {
                message: "Invalid click statement".to_string(),
            });
        };
        let maybe_tt = p.take_quoted();
        let maybe_target = p.take_word().filter(|w| w.starts_with('_'));
        tooltip = maybe_tt;
        action = ClickAction::Link {
            href: link,
            target: maybe_target,
        };
        return Ok(ClickStmt {
            ids,
            tooltip,
            action,
        });
    }

    if p.rest().starts_with("call")
        && p.rest()
            .as_bytes()
            .get(4)
            .map_or(true, |b| b.is_ascii_whitespace())
    {
        let _ = p.take_word();
        p.skip_ws();
        let start = p.i;
        while p.i < p.s.len() {
            let b = p.s.as_bytes()[p.i];
            if b.is_ascii_whitespace() || b == b'(' {
                break;
            }
            p.i += 1;
        }
        if p.i == start {
            return Err(LexError {
                message: "Invalid click statement".to_string(),
            });
        }
        let function_name = p.s[start..p.i].to_string();

        let mut function_args: Option<String> = None;
        p.skip_ws();
        if p.peek() == Some(b'(') {
            p.i += 1;
            let args_start = p.i;
            while p.i < p.s.len() && p.s.as_bytes()[p.i] != b')' {
                p.i += 1;
            }
            let args = p.s[args_start..p.i].to_string();
            if p.peek() == Some(b')') {
                p.i += 1;
            }
            if !args.trim().is_empty() {
                function_args = Some(args);
            }
        }

        tooltip = p.take_quoted();
        action = ClickAction::Callback {
            function_name,
            function_args,
        };
        return Ok(ClickStmt {
            ids,
            tooltip,
            action,
        });
    }

    if let Some(link) = p.take_quoted() {
        let maybe_tt = p.take_quoted();
        let maybe_target = p.take_word().filter(|w| w.starts_with('_'));
        tooltip = maybe_tt;
        action = ClickAction::Link {
            href: link,
            target: maybe_target,
        };
        return Ok(ClickStmt {
            ids,
            tooltip,
            action,
        });
    }

    let Some(function_name) = p.take_word() else {
        return Err(LexError {
            message: "Invalid click statement".to_string(),
        });
    };
    tooltip = p.take_quoted();
    action = ClickAction::Callback {
        function_name,
        function_args: None,
    };
    Ok(ClickStmt {
        ids,
        tooltip,
        action,
    })
}

fn parse_link_style_stmt(rest: &str) -> std::result::Result<LinkStyleStmt, LexError> {
    let mut p = ClickParse::new(rest);
    let Some(pos_raw) = p.take_word() else {
        return Err(LexError {
            message: "Invalid linkStyle statement".to_string(),
        });
    };

    let positions = if pos_raw == "default" {
        vec![LinkStylePos::Default]
    } else {
        pos_raw
            .split(',')
            .map(|s| {
                let idx = s.trim().parse::<usize>().map_err(|_| LexError {
                    message: "Invalid linkStyle statement".to_string(),
                })?;
                Ok(LinkStylePos::Index(idx))
            })
            .collect::<std::result::Result<Vec<_>, LexError>>()?
    };

    p.skip_ws();
    let mut interpolate: Option<String> = None;
    if p.rest().starts_with("interpolate")
        && p.rest()
            .as_bytes()
            .get("interpolate".len())
            .map_or(true, |b| b.is_ascii_whitespace())
    {
        let _ = p.take_word();
        interpolate = p.take_word();
    }

    // Mermaid's `linkStyle ... interpolate <curve> ...` still tokenizes the styles list without the
    // leading whitespace between the curve name and the first style token. Keep the whitespace
    // inside comma-separated tokens (handled by `parse_linkstyle_styles_list`), but drop the
    // leading separator spaces at the list boundary.
    p.skip_ws();
    let styles = parse_linkstyle_styles_list(p.rest());
    Ok(LinkStyleStmt {
        positions,
        interpolate,
        styles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_click_stmt_parses_callback() {
        let stmt = parse_click_stmt("A callback").unwrap();
        assert_eq!(stmt.ids, vec!["A"]);
        assert!(stmt.tooltip.is_none());
        match stmt.action {
            ClickAction::Callback {
                function_name,
                function_args,
            } => {
                assert_eq!(function_name, "callback");
                assert!(function_args.is_none());
            }
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_call_callback_empty_args() {
        let stmt = parse_click_stmt("A call callback()").unwrap();
        assert_eq!(stmt.ids, vec!["A"]);
        assert!(stmt.tooltip.is_none());
        match stmt.action {
            ClickAction::Callback {
                function_name,
                function_args,
            } => {
                assert_eq!(function_name, "callback");
                assert!(function_args.is_none());
            }
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_call_callback_with_args() {
        let stmt = parse_click_stmt("A call callback(\"test0\", test1, test2)").unwrap();
        match stmt.action {
            ClickAction::Callback {
                function_name,
                function_args,
            } => {
                assert_eq!(function_name, "callback");
                assert_eq!(function_args.as_deref(), Some("\"test0\", test1, test2"));
            }
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_link_and_tooltip_and_target() {
        let stmt = parse_click_stmt("A \"click.html\" \"tooltip\" _blank").unwrap();
        assert_eq!(stmt.tooltip.as_deref(), Some("tooltip"));
        match stmt.action {
            ClickAction::Link { href, target } => {
                assert_eq!(href, "click.html");
                assert_eq!(target.as_deref(), Some("_blank"));
            }
            _ => panic!("expected link action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_href_link_and_tooltip_and_target() {
        let stmt = parse_click_stmt("A href \"click.html\" \"tooltip\" _blank").unwrap();
        assert_eq!(stmt.tooltip.as_deref(), Some("tooltip"));
        match stmt.action {
            ClickAction::Link { href, target } => {
                assert_eq!(href, "click.html");
                assert_eq!(target.as_deref(), Some("_blank"));
            }
            _ => panic!("expected link action"),
        }
    }

    #[test]
    fn flowchart_subgraphs_exist_matches_mermaid_flowdb_spec() {
        let subgraphs = vec![
            FlowSubGraph {
                id: "sg0".to_string(),
                nodes: vec![
                    "a".to_string(),
                    "b".to_string(),
                    "c".to_string(),
                    "e".to_string(),
                ],
                title: "".to_string(),
                classes: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg1".to_string(),
                nodes: vec!["f".to_string(), "g".to_string(), "h".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg2".to_string(),
                nodes: vec!["i".to_string(), "j".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg3".to_string(),
                nodes: vec!["k".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
        ];

        assert!(subgraphs_exist(&subgraphs, "a"));
        assert!(subgraphs_exist(&subgraphs, "h"));
        assert!(subgraphs_exist(&subgraphs, "j"));
        assert!(subgraphs_exist(&subgraphs, "k"));

        assert!(!subgraphs_exist(&subgraphs, "a2"));
        assert!(!subgraphs_exist(&subgraphs, "l"));
    }
}
