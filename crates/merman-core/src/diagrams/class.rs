use crate::common::parse_generic_types;
use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::OnceLock;

lalrpop_util::lalrpop_mod!(class_grammar, "/diagrams/class_grammar.rs");

pub(crate) const LINE_SOLID: i32 = 0;
pub(crate) const LINE_DOTTED: i32 = 1;

pub(crate) const REL_AGGREGATION: i32 = 0;
pub(crate) const REL_EXTENSION: i32 = 1;
pub(crate) const REL_COMPOSITION: i32 = 2;
pub(crate) const REL_DEPENDENCY: i32 = 3;
pub(crate) const REL_LOLLIPOP: i32 = 4;
pub(crate) const REL_NONE: i32 = -1;

const MERMAID_DOM_ID_PREFIX: &str = "classId-";

static METHOD_RE: OnceLock<Regex> = OnceLock::new();
static ACC_DESCR_RE: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    Newline,

    ClassDiagram,

    Direction(String),

    ClassKw,
    NamespaceKw,

    Note,
    NoteFor,

    CssClass,
    StyleKw,
    ClassDefKw,
    ClickKw,
    LinkKw,
    CallbackKw,
    HrefKw,

    StructStart,
    StructStop,

    SquareStart,
    SquareStop,

    AnnotationStart,
    AnnotationStop,

    StyleSeparator,

    Ext,
    Dep,
    Comp,
    Agg,
    Lollipop,
    Line,
    DottedLine,

    Label(String),
    Str(String),
    Name(String),
    Member(String),
    RestOfLine(String),
    LinkTarget(String),
    CallbackName(String),
    CallbackArgs(String),

    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Action {
    SetDirection(String),
    SetAccTitle(String),
    SetAccDescr(String),

    AddNamespace {
        id: String,
    },
    AddClassesToNamespace {
        namespace: String,
        class_ids: Vec<String>,
    },

    AddClass {
        id: String,
    },
    SetClassLabel {
        id: String,
        label: String,
    },
    SetCssClass {
        ids: String,
        css_class: String,
    },
    SetCssStyle {
        id: String,
        raw: String,
    },
    DefineClass {
        id: String,
        raw: String,
    },
    SetLink {
        id: String,
        url: String,
        target: Option<String>,
    },
    SetTooltip {
        id: String,
        tooltip: String,
    },
    SetClickEvent {
        id: String,
        function: String,
        args: Option<String>,
    },
    AddMembers {
        id: String,
        members: Vec<String>,
    },
    AddMember {
        id: String,
        member: String,
    },
    AddAnnotation {
        id: String,
        annotation: String,
    },
    AddRelation {
        data: RelationData,
    },
    AddNote {
        class_id: Option<String>,
        text: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Relation {
    pub type1: i32,
    pub type2: i32,
    pub line_type: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct RelationData {
    pub id1: String,
    pub id2: String,
    pub relation: Relation,
    pub relation_title1: Option<String>,
    pub relation_title2: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
struct ClassMember {
    member_type: String,
    visibility: String,
    id: String,
    classifier: String,
    parameters: String,
    return_type: String,
    display_text: String,
    css_style: String,
}

impl ClassMember {
    fn new(input: &str, member_type: &str, config: &MermaidConfig) -> Self {
        let input = sanitize_text(input, config);
        let mut m = Self {
            member_type: member_type.to_string(),
            visibility: String::new(),
            id: String::new(),
            classifier: String::new(),
            parameters: String::new(),
            return_type: String::new(),
            display_text: String::new(),
            css_style: String::new(),
        };
        m.parse_member(&input, member_type);
        m
    }

    fn parse_method_signature_fast(input: &str) -> Option<(&str, &str, &str, &str, &str)> {
        // Fast-path for the common Mermaid method member forms:
        //
        //   ([#+~-])? <name> "(" <params> ")" <classifier?> <return_type?>
        //
        // where classifier is `$` (underline) or `*` (italic) and can appear either:
        // - immediately after `)` (e.g. `foo()$`)
        // - at the end of the return type payload (e.g. `foo() : i32$`), in which case Mermaid's
        //   upstream parsing treats it as the classifier (see legacy regex logic below).
        //
        // We return borrowed slices and let the caller allocate as needed.
        let s = input.trim();
        if s.is_empty() {
            return None;
        }

        let (visibility, rest) = match s.as_bytes()[0] {
            b'#' | b'+' | b'~' | b'-' => (&s[..1], &s[1..]),
            _ => ("", s),
        };

        let Some(paren_open_rel) = rest.find('(') else {
            return None;
        };
        let Some(paren_close_rel) = rest.rfind(')') else {
            return None;
        };
        if paren_close_rel < paren_open_rel {
            return None;
        }

        let name = rest[..paren_open_rel].trim();
        let params = rest[paren_open_rel + 1..paren_close_rel].trim();
        let after_paren = rest[paren_close_rel + 1..].trim_start();

        let mut classifier = "";
        let mut return_type = after_paren.trim();

        if let Some(first) = after_paren.as_bytes().first().copied() {
            if first == b'$' || first == b'*' {
                classifier = &after_paren[..1];
                return_type = after_paren[1..].trim();
            }
        }

        if classifier.is_empty() {
            if let Some(last) = return_type.as_bytes().last().copied() {
                if last == b'$' || last == b'*' {
                    classifier = &return_type[return_type.len() - 1..];
                    return_type = return_type[..return_type.len() - 1].trim();
                }
            }
        }

        Some((visibility, name, params, classifier, return_type))
    }

    fn parse_member(&mut self, input: &str, member_type: &str) {
        let input = input.trim();
        if member_type == "method" {
            if let Some((visibility, id, params, classifier, return_type)) =
                Self::parse_method_signature_fast(input)
            {
                if matches!(visibility, "#" | "+" | "~" | "-") {
                    self.visibility = visibility.to_string();
                }
                self.id = id.to_string();
                self.parameters = params.to_string();
                self.classifier = classifier.to_string();
                self.return_type = return_type.to_string();
            } else {
                let method_re = METHOD_RE.get_or_init(|| {
                    Regex::new(r"^([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?$")
                        .expect("class method regex must compile")
                });
                if let Some(caps) = method_re.captures(input) {
                    if let Some(v) = caps.get(1).map(|m| m.as_str().trim()) {
                        if matches!(v, "#" | "+" | "~" | "-" | "") {
                            self.visibility = v.to_string();
                        }
                    }
                    self.id = caps
                        .get(2)
                        .map(|m| m.as_str())
                        .unwrap_or_default()
                        .to_string();
                    self.parameters = caps
                        .get(3)
                        .map(|m| m.as_str().trim())
                        .unwrap_or_default()
                        .to_string();
                    let mut classifier = caps
                        .get(4)
                        .map(|m| m.as_str().trim())
                        .unwrap_or_default()
                        .to_string();
                    self.return_type = caps
                        .get(5)
                        .map(|m| m.as_str().trim())
                        .unwrap_or_default()
                        .to_string();

                    if classifier.is_empty() {
                        if let Some(last) = self.return_type.chars().last() {
                            if last == '$' || last == '*' {
                                classifier = last.to_string();
                                self.return_type.pop();
                                self.return_type = self.return_type.trim().to_string();
                            }
                        }
                    }

                    self.classifier = classifier;
                }
            }
        } else {
            let first = input.chars().next().unwrap_or('\0');
            let last = input.chars().last().unwrap_or('\0');
            let mut start = 0usize;
            let mut end = input.len();
            if matches!(first, '#' | '+' | '~' | '-') {
                self.visibility = first.to_string();
                start = first.len_utf8();
            }
            if last == '$' || last == '*' {
                self.classifier = last.to_string();
                end = input.len() - last.len_utf8();
            }
            self.id = input[start..end].to_string();
        }

        if self.id.starts_with(' ') {
            self.id = format!(" {}", self.id.trim());
        } else {
            self.id = self.id.trim().to_string();
        }

        self.css_style = match self.classifier.as_str() {
            "*" => "font-style:italic;".to_string(),
            "$" => "text-decoration:underline;".to_string(),
            _ => String::new(),
        };

        let mut display = format!("{}{}", self.visibility, parse_generic_types(&self.id));
        if member_type == "method" {
            display.push('(');
            display.push_str(&parse_generic_types(self.parameters.trim()));
            display.push(')');
            if !self.return_type.is_empty() {
                display.push_str(" : ");
                display.push_str(&parse_generic_types(self.return_type.trim()));
            }
        }
        self.display_text = display.trim().to_string();
    }

    fn to_json(&self) -> Value {
        json!({
            "memberType": self.member_type,
            "visibility": self.visibility,
            "id": self.id,
            "classifier": self.classifier,
            "parameters": self.parameters,
            "returnType": self.return_type,
            "displayText": self.display_text,
            "cssStyle": self.css_style,
        })
    }
}

#[derive(Debug, Clone)]
struct ClassNode {
    id: String,
    type_param: String,
    label: String,
    text: String,
    css_classes: String,
    methods: Vec<ClassMember>,
    members: Vec<ClassMember>,
    annotations: Vec<String>,
    styles: Vec<String>,
    dom_id: String,
    parent: Option<String>,
    link: Option<String>,
    link_target: Option<String>,
    tooltip: Option<String>,
    have_callback: bool,
    callback: Option<serde_json::Map<String, Value>>,
    callback_effective: bool,
}

#[derive(Debug, Clone)]
struct ClassNote {
    id: String,
    class_id: Option<String>,
    text: String,
}

#[derive(Debug, Clone)]
struct Interface {
    id: String,
    label: String,
    class_id: String,
}

#[derive(Debug, Clone)]
struct Namespace {
    id: String,
    dom_id: String,
    class_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct StyleClass {
    id: String,
    styles: Vec<String>,
    text_styles: Vec<String>,
}

#[derive(Debug, Default)]
struct ClassDb {
    direction: String,
    classes: IndexMap<String, ClassNode>,
    relations: Vec<RelationData>,
    notes: Vec<ClassNote>,
    interfaces: Vec<Interface>,
    namespaces: IndexMap<String, Namespace>,
    style_classes: IndexMap<String, StyleClass>,
    class_counter: usize,
    namespace_counter: usize,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    security_level: Option<String>,
    config: MermaidConfig,
}

impl ClassDb {
    fn new(config: MermaidConfig) -> Self {
        Self {
            direction: "TB".to_string(),
            security_level: config.get_str("securityLevel").map(|s| s.to_string()),
            config,
            ..Default::default()
        }
    }
}

pub fn parse_class(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let actions = class_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut db = ClassDb::new(meta.effective_config.clone());
    for a in actions {
        db.apply(a).map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    }
    Ok(db.to_model(meta))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    AfterClass,
    ClassBody,
    LineNeedId,
    LineRest,
    ClickNeedId,
    ClickAfterId,
    ClickNeedCallbackName,
    ClickAfterCallbackName,
}

struct Lexer<'input> {
    input: &'input str,
    pos: usize,
    pending: VecDeque<(usize, Tok, usize)>,
    mode: Mode,
}

impl<'input> Lexer<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            mode: Mode::Default,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
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

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn starts_with_word(&self, s: &str) -> bool {
        if !self.starts_with(s) {
            return false;
        }
        let after = self.pos + s.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        b.is_ascii_whitespace() || matches!(b, b'{' | b'}' | b'[' | b']' | b'"' | b'`' | b':')
    }

    fn read_to_newline(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn lex_newline(&mut self) -> Option<(usize, Tok, usize)> {
        if self.peek()? != b'\n' {
            return None;
        }
        let start = self.pos;
        while let Some(b'\n') = self.peek() {
            self.pos += 1;
        }
        if self.mode == Mode::AfterClass {
            self.mode = Mode::Default;
        }
        Some((start, Tok::Newline, self.pos))
    }

    fn lex_comment(&mut self) -> bool {
        if self.starts_with("%%") {
            let _ = self.read_to_newline();
            return true;
        }
        false
    }

    fn lex_acc_title(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if !self.starts_with("accTitle") {
            return None;
        }
        let after = self.pos + "accTitle".len();
        let rest = &self.input[after..];
        let colon = rest.find(':')?;
        self.pos = after + colon + 1;
        let value = self.read_to_newline();
        Some((start, Tok::AccTitle(value.trim().to_string()), self.pos))
    }

    fn lex_acc_descr(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with("accDescr") {
            return None;
        }
        let after = self.pos + "accDescr".len();
        let rest = &self.input[after..];
        let rest_trim = rest.trim_start();
        if rest_trim.starts_with('{') {
            let brace_pos = rest.find('{').unwrap();
            self.pos = after + brace_pos + 1;
            let Some(end_rel) = self.input[self.pos..].find('}') else {
                return Some(Err(LexError {
                    message: "Unterminated accDescr block; missing '}'".to_string(),
                }));
            };
            let body = self.input[self.pos..self.pos + end_rel].to_string();
            self.pos = self.pos + end_rel + 1;
            return Some(Ok((
                start,
                Tok::AccDescrMultiline(body.trim().to_string()),
                self.pos,
            )));
        }
        let colon = rest.find(':')?;
        self.pos = after + colon + 1;
        let value = self.read_to_newline();
        Some(Ok((
            start,
            Tok::AccDescr(value.trim().to_string()),
            self.pos,
        )))
    }

    fn lex_keyword(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.starts_with_word("classDiagram-v2") {
            self.pos += "classDiagram-v2".len();
            return Some((start, Tok::ClassDiagram, self.pos));
        }
        if self.starts_with_word("classDiagram") {
            self.pos += "classDiagram".len();
            return Some((start, Tok::ClassDiagram, self.pos));
        }

        if self.starts_with_word("direction") {
            let after = self.pos + "direction".len();
            self.pos = after;
            self.skip_ws();
            let dir = if self.input[self.pos..].starts_with("TB") {
                self.pos += 2;
                "TB"
            } else if self.input[self.pos..].starts_with("BT") {
                self.pos += 2;
                "BT"
            } else if self.input[self.pos..].starts_with("LR") {
                self.pos += 2;
                "LR"
            } else if self.input[self.pos..].starts_with("RL") {
                self.pos += 2;
                "RL"
            } else {
                return None;
            };
            let _ = self.read_to_newline();
            return Some((start, Tok::Direction(dir.to_string()), self.pos));
        }

        if self.starts_with_word("namespace") {
            self.pos += "namespace".len();
            return Some((start, Tok::NamespaceKw, self.pos));
        }
        if self.starts_with_word("class") {
            self.pos += "class".len();
            self.mode = Mode::AfterClass;
            return Some((start, Tok::ClassKw, self.pos));
        }

        if self.starts_with("note for") {
            self.pos += "note for".len();
            return Some((start, Tok::NoteFor, self.pos));
        }
        if self.starts_with_word("note") {
            self.pos += "note".len();
            return Some((start, Tok::Note, self.pos));
        }

        if self.starts_with_word("cssClass") {
            self.pos += "cssClass".len();
            return Some((start, Tok::CssClass, self.pos));
        }
        if self.starts_with_word("style") {
            self.pos += "style".len();
            self.mode = Mode::LineNeedId;
            return Some((start, Tok::StyleKw, self.pos));
        }
        if self.starts_with_word("classDef") {
            self.pos += "classDef".len();
            self.mode = Mode::LineNeedId;
            return Some((start, Tok::ClassDefKw, self.pos));
        }
        if self.starts_with_word("click") {
            self.pos += "click".len();
            self.mode = Mode::ClickNeedId;
            return Some((start, Tok::ClickKw, self.pos));
        }
        if self.starts_with_word("link") {
            self.pos += "link".len();
            return Some((start, Tok::LinkKw, self.pos));
        }
        if self.starts_with_word("callback") {
            self.pos += "callback".len();
            return Some((start, Tok::CallbackKw, self.pos));
        }
        if self.starts_with_word("href") {
            self.pos += "href".len();
            return Some((start, Tok::HrefKw, self.pos));
        }

        None
    }

    fn lex_link_target(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        for t in ["_self", "_blank", "_parent", "_top"] {
            if self.starts_with_word(t) {
                self.pos += t.len();
                return Some((start, Tok::LinkTarget(t.to_string()), self.pos));
            }
        }
        None
    }

    fn lex_click_call(&mut self) -> bool {
        if self.mode != Mode::ClickAfterId {
            return false;
        }
        if self.starts_with_word("call") {
            self.pos += "call".len();
            self.mode = Mode::ClickNeedCallbackName;
            return true;
        }
        false
    }

    fn lex_callback_name(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode != Mode::ClickNeedCallbackName {
            return None;
        }
        let start = self.pos;
        self.skip_ws();
        let bytes = self.input.as_bytes();
        let mut end = self.pos;
        while end < self.input.len() {
            let b = bytes[end];
            if b.is_ascii_whitespace() || b == b'\n' || b == b'(' {
                break;
            }
            end += 1;
        }
        if end == self.pos {
            return None;
        }
        let s = self.input[self.pos..end].to_string();
        self.pos = end;
        self.mode = Mode::ClickAfterCallbackName;
        Some((start, Tok::CallbackName(s), self.pos))
    }

    fn lex_callback_args(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if self.mode != Mode::ClickAfterCallbackName {
            return None;
        }
        let start = self.pos;
        if self.peek()? != b'(' {
            return None;
        }
        self.pos += 1;
        let Some(end_rel) = self.input[self.pos..].find(')') else {
            return Some(Err(LexError {
                message: "Unterminated callback arguments; missing ')'".to_string(),
            }));
        };
        let args = self.input[self.pos..self.pos + end_rel].trim().to_string();
        self.pos = self.pos + end_rel + 1;
        self.mode = Mode::ClickAfterId;
        Some(Ok((start, Tok::CallbackArgs(args), self.pos)))
    }

    fn lex_rest_of_line(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode != Mode::LineRest {
            return None;
        }
        let start = self.pos;
        let s = self.read_to_newline();
        self.mode = Mode::Default;
        Some((start, Tok::RestOfLine(s.trim().to_string()), self.pos))
    }

    fn lex_punct(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'{' => {
                self.pos += 1;
                if self.mode == Mode::AfterClass {
                    self.mode = Mode::ClassBody;
                }
                Some((start, Tok::StructStart, self.pos))
            }
            b'}' => {
                self.pos += 1;
                if self.mode == Mode::ClassBody {
                    self.mode = Mode::Default;
                }
                Some((start, Tok::StructStop, self.pos))
            }
            b'[' => {
                self.pos += 1;
                Some((start, Tok::SquareStart, self.pos))
            }
            b']' => {
                self.pos += 1;
                Some((start, Tok::SquareStop, self.pos))
            }
            b'<' => {
                if self.input[self.pos..].starts_with("<<") {
                    self.pos += 2;
                    return Some((start, Tok::AnnotationStart, self.pos));
                }
                if self.input[self.pos..].starts_with("<|") {
                    self.pos += 2;
                    return Some((start, Tok::Ext, self.pos));
                }
                self.pos += 1;
                Some((start, Tok::Dep, self.pos))
            }
            b'>' => {
                if self.input[self.pos..].starts_with(">>") {
                    self.pos += 2;
                    return Some((start, Tok::AnnotationStop, self.pos));
                }
                self.pos += 1;
                Some((start, Tok::Dep, self.pos))
            }
            b'|' => {
                if self.input[self.pos..].starts_with("|>") {
                    self.pos += 2;
                    return Some((start, Tok::Ext, self.pos));
                }
                None
            }
            b'(' => {
                if self.input[self.pos..].starts_with("()") {
                    self.pos += 2;
                    return Some((start, Tok::Lollipop, self.pos));
                }
                None
            }
            b'*' => {
                self.pos += 1;
                Some((start, Tok::Comp, self.pos))
            }
            b'o' => {
                let next = self.input.as_bytes().get(self.pos + 1).copied();
                if matches!(next, Some(b'-' | b'.' | b' ' | b'\t') | None) {
                    self.pos += 1;
                    Some((start, Tok::Agg, self.pos))
                } else {
                    None
                }
            }
            b'.' => {
                if self.input[self.pos..].starts_with("..") {
                    self.pos += 2;
                    Some((start, Tok::DottedLine, self.pos))
                } else {
                    None
                }
            }
            b'-' => {
                if self.input[self.pos..].starts_with("--") {
                    self.pos += 2;
                    Some((start, Tok::Line, self.pos))
                } else {
                    None
                }
            }
            b':' => {
                if self.input[self.pos..].starts_with(":::") {
                    self.pos += 3;
                    Some((start, Tok::StyleSeparator, self.pos))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn lex_label(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b':' {
            return None;
        }
        if self.input[self.pos..].starts_with(":::") {
            return None;
        }
        self.pos += 1;
        let s = self.read_to_newline();
        Some((start, Tok::Label(format!(":{}", s)), self.pos))
    }

    fn lex_str(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if self.peek()? != b'"' {
            return None;
        }
        self.pos += 1;
        let Some(rel_end) = self.input[self.pos..].find('"') else {
            return Some(Err(LexError {
                message: "Unterminated string literal; missing '\"'".to_string(),
            }));
        };
        let s = self.input[self.pos..self.pos + rel_end].to_string();
        self.pos = self.pos + rel_end + 1;
        Some(Ok((start, Tok::Str(s), self.pos)))
    }

    fn lex_name(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode == Mode::ClassBody {
            return None;
        }
        let start = self.pos;
        if self.peek()? == b'`' {
            self.pos += 1;
            let Some(rel_end) = self.input[self.pos..].find('`') else {
                let s = self.input[self.pos..].to_string();
                self.pos = self.input.len();
                return Some((start, Tok::Name(s), self.pos));
            };
            let s = self.input[self.pos..self.pos + rel_end].to_string();
            self.pos = self.pos + rel_end + 1;
            if self.mode == Mode::LineNeedId {
                self.mode = Mode::LineRest;
            }
            if self.mode == Mode::ClickNeedId {
                self.mode = Mode::ClickAfterId;
            }
            let s = if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                format!("{MERMAID_DOM_ID_PREFIX}{s}")
            } else {
                s
            };
            return Some((start, Tok::Name(s), self.pos));
        }

        let bytes = self.input.as_bytes();
        let mut end = self.pos;
        while end < self.input.len() {
            let b = bytes[end];
            if b.is_ascii_whitespace()
                || b == b'\n'
                || b == b'{'
                || b == b'}'
                || b == b'['
                || b == b']'
                || b == b'"'
                || b == b','
            {
                break;
            }
            if b == b':' {
                break;
            }
            if b == b'<' || b == b'>' {
                break;
            }
            if b == b'.' && end + 1 < bytes.len() && bytes[end + 1] == b'.' {
                break;
            }
            if b == b'-' && end + 1 < bytes.len() && bytes[end + 1] == b'-' {
                break;
            }
            end += 1;
        }
        if end == start {
            return None;
        }
        let mut s = self.input[start..end].to_string();
        if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            s = format!("{MERMAID_DOM_ID_PREFIX}{s}");
        }
        self.pos = end;
        if self.mode == Mode::LineNeedId {
            self.mode = Mode::LineRest;
        }
        if self.mode == Mode::ClickNeedId {
            self.mode = Mode::ClickAfterId;
        }
        Some((start, Tok::Name(s), self.pos))
    }

    fn lex_member(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if self.mode != Mode::ClassBody {
            return None;
        }
        self.skip_ws();
        if self.pos >= self.input.len() {
            return Some(Err(LexError {
                message: "EOF inside class body".to_string(),
            }));
        }
        if self.peek() == Some(b'}') {
            return None;
        }
        if self.peek() == Some(b'{') {
            return Some(Err(LexError {
                message: "Unexpected '{' inside class body".to_string(),
            }));
        }
        // Newlines inside a class body are ignored by Mermaid's lexer.
        while self.peek() == Some(b'\n') {
            self.pos += 1;
            self.skip_ws();
        }
        let start = self.pos;
        let s = self.read_to_newline();
        Some(Ok((start, Tok::Member(s.trim_end().to_string()), self.pos)))
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(Ok(tok));
        }

        loop {
            self.skip_ws();
            if self.pos >= self.input.len() {
                if self.mode == Mode::ClassBody {
                    return Some(Err(LexError {
                        message: "EOF inside class body".to_string(),
                    }));
                }
                return None;
            }

            if self.lex_comment() {
                continue;
            }

            if let Some(tok) = self.lex_rest_of_line() {
                return Some(Ok(tok));
            }

            if self.lex_click_call() {
                continue;
            }

            if self.mode == Mode::ClassBody && self.peek() == Some(b'\n') {
                self.pos += 1;
                continue;
            }

            if let Some(tok) = self.lex_callback_name() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_link_target() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_member() {
                return Some(tok);
            }

            if let Some(tok) = self.lex_newline() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_acc_title() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_acc_descr() {
                return Some(tok);
            }

            if let Some(tok) = self.lex_keyword() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_callback_args() {
                return Some(tok);
            }

            if let Some(tok) = self.lex_punct() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_label() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_str() {
                return Some(tok);
            }

            if let Some(tok) = self.lex_name() {
                return Some(Ok(tok));
            }

            let start = self.pos;
            let _ = self.bump();
            return Some(Err(LexError {
                message: format!("Unexpected character at {start}"),
            }));
        }
    }
}

impl ClassDb {
    fn split_class_name_and_type(&self, id: &str) -> (String, String) {
        let id = sanitize_text(id, &self.config);
        let (left, right) = if let Some((left, right)) = id.split_once('~') {
            (
                left.to_string(),
                right.split('~').next().unwrap_or("").to_string(),
            )
        } else {
            (id, String::new())
        };

        let class_name = sanitize_text(&left, &self.config);
        let type_param = if right.is_empty() {
            right
        } else {
            sanitize_text(&right, &self.config)
        };

        (class_name, type_param)
    }

    fn add_class(&mut self, id: &str) {
        let (class_name, type_param) = self.split_class_name_and_type(id);
        if self.classes.contains_key(&class_name) {
            return;
        }
        let dom_id = format!("{MERMAID_DOM_ID_PREFIX}{class_name}-{}", self.class_counter);
        self.class_counter += 1;
        let text = if type_param.is_empty() {
            class_name.clone()
        } else {
            format!("{class_name}&lt;{type_param}&gt;")
        };
        self.classes.insert(
            class_name.clone(),
            ClassNode {
                id: class_name.clone(),
                type_param: type_param.clone(),
                label: class_name.clone(),
                text,
                css_classes: "default".to_string(),
                methods: Vec::new(),
                members: Vec::new(),
                annotations: Vec::new(),
                styles: Vec::new(),
                dom_id,
                parent: None,
                link: None,
                link_target: None,
                tooltip: None,
                have_callback: false,
                callback: None,
                callback_effective: false,
            },
        );
    }

    fn set_class_label(&mut self, id: &str, label: &str) {
        let (class_name, type_param) = self.split_class_name_and_type(id);
        self.add_class(&class_name);
        let Some(c) = self.classes.get_mut(&class_name) else {
            return;
        };
        let label = sanitize_text(label, &self.config);
        c.label = label.clone();
        c.text = if type_param.is_empty() {
            label
        } else {
            format!("{label}<{type_param}>")
        };
    }

    fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    fn cleanup_label(&self, label: &str) -> String {
        let t = label.trim();
        let t = t.strip_prefix(':').unwrap_or(t);
        sanitize_text(t.trim(), &self.config)
    }

    fn add_member(&mut self, class_name: &str, member: &str) {
        self.add_class(class_name);
        let (class_name, _) = self.split_class_name_and_type(class_name);
        let Some(c) = self.classes.get_mut(&class_name) else {
            return;
        };

        let member_string = member.trim();
        if member_string.is_empty() {
            return;
        }
        if member_string.starts_with("<<") && member_string.ends_with(">>") {
            c.annotations.push(sanitize_text(
                member_string
                    .trim_start_matches("<<")
                    .trim_end_matches(">>"),
                &self.config,
            ));
            return;
        }
        if member_string.contains(')') {
            c.methods
                .push(ClassMember::new(member_string, "method", &self.config));
            return;
        }
        c.members
            .push(ClassMember::new(member_string, "attribute", &self.config));
    }

    fn add_members(&mut self, class_name: &str, mut members: Vec<String>) {
        members.reverse();
        for m in members {
            self.add_member(class_name, &m);
        }
    }

    fn add_annotation(&mut self, class_name: &str, annotation: &str) {
        self.add_class(class_name);
        let (class_name, _) = self.split_class_name_and_type(class_name);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.annotations.push(sanitize_text(annotation, &self.config));
        }
    }

    fn set_css_class(&mut self, ids: &str, css_class: &str) {
        for raw in ids.split(',') {
            let id = raw.trim();
            if id.is_empty() {
                continue;
            }
            let (class_name, _) = self.split_class_name_and_type(id);
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.css_classes.push(' ');
                c.css_classes.push_str(css_class);
            }
        }
    }

    fn set_tooltip(&mut self, id: &str, tooltip: &str) {
        let (class_name, _) = self.split_class_name_and_type(id);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.tooltip = Some(sanitize_text(tooltip, &self.config));
        }
    }

    fn set_link(&mut self, id: &str, url: &str, target: Option<String>) {
        let (class_name, _) = self.split_class_name_and_type(id);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.link = format_url(url, &self.config);

            let final_target = if self.security_level.as_deref() == Some("sandbox") {
                "_top".to_string()
            } else if let Some(t) = target.clone() {
                sanitize_text(&t, &self.config)
            } else {
                "_blank".to_string()
            };
            c.link_target = Some(final_target);
        }
        self.set_css_class(&class_name, "clickable");
    }

    fn set_click_event(&mut self, id: &str, function: &str, args: Option<String>) {
        let (class_name, _) = self.split_class_name_and_type(id);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.have_callback = true;
            let mut map = serde_json::Map::new();
            map.insert("function".to_string(), Value::String(function.to_string()));
            let args = args.and_then(|s| {
                let t = s.trim().to_string();
                if t.is_empty() { None } else { Some(t) }
            });
            if let Some(args) = args.clone() {
                map.insert("args".to_string(), Value::String(args.clone()));
            }
            c.callback = Some(map);
            c.callback_effective = self.security_level.as_deref() == Some("loose");
        }
        self.set_css_class(&class_name, "clickable");
    }

    fn parse_styles(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn set_css_style(&mut self, id: &str, styles: Vec<String>) {
        let Some(c) = self.classes.get_mut(id) else {
            return;
        };
        for s in styles {
            for part in s.split(',') {
                let t = part.trim();
                if !t.is_empty() {
                    c.styles.push(t.to_string());
                }
            }
        }
    }

    fn define_class(&mut self, id: &str, styles: Vec<String>) {
        let entry = self
            .style_classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass {
                id: id.to_string(),
                ..Default::default()
            });

        for s in &styles {
            if s.contains("color") {
                entry.text_styles.push(s.replace("fill", "bgFill"));
            }
            entry.styles.push(s.to_string());
        }

        for c in self.classes.values_mut() {
            if !c.css_classes.contains(id) {
                continue;
            }
            for s in &styles {
                for part in s.split(',') {
                    let t = part.trim();
                    if !t.is_empty() {
                        c.styles.push(t.to_string());
                    }
                }
            }
        }
    }

    fn add_note(&mut self, class_id: Option<String>, text: &str) {
        let note_id = format!("note{}", self.notes.len());
        self.notes.push(ClassNote {
            id: note_id,
            class_id,
            text: text.to_string(),
        });
    }

    fn add_namespace(&mut self, id: &str) {
        if self.namespaces.contains_key(id) {
            return;
        }
        let dom_id = format!("{MERMAID_DOM_ID_PREFIX}{id}-{}", self.namespace_counter);
        self.namespace_counter += 1;
        self.namespaces.insert(
            id.to_string(),
            Namespace {
                id: id.to_string(),
                dom_id,
                class_ids: Vec::new(),
            },
        );
    }

    fn add_classes_to_namespace(&mut self, namespace: &str, class_names: &[String]) {
        if !self.namespaces.contains_key(namespace) {
            return;
        }
        let mut ids = Vec::new();
        for name in class_names {
            let (class_name, _) = self.split_class_name_and_type(name);
            self.add_class(&class_name);
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.parent = Some(namespace.to_string());
            }
            ids.push(class_name);
        }
        if let Some(ns) = self.namespaces.get_mut(namespace) {
            ns.class_ids.extend(ids);
        }
    }

    fn add_relation(&mut self, mut rel: RelationData) {
        let (id1_name, _) = self.split_class_name_and_type(&rel.id1);
        let (id2_name, _) = self.split_class_name_and_type(&rel.id2);

        let invalid_types = [
            REL_LOLLIPOP,
            REL_AGGREGATION,
            REL_COMPOSITION,
            REL_DEPENDENCY,
            REL_EXTENSION,
        ];

        if rel.relation.type1 == REL_LOLLIPOP && !invalid_types.contains(&rel.relation.type2) {
            self.add_class(&id2_name);
            let iface_id = format!("interface{}", self.interfaces.len());
            self.interfaces.push(Interface {
                id: iface_id.clone(),
                label: rel.id1.clone(),
                class_id: id2_name.clone(),
            });
            rel.id1 = iface_id;
        } else if rel.relation.type2 == REL_LOLLIPOP && !invalid_types.contains(&rel.relation.type1)
        {
            self.add_class(&id1_name);
            let iface_id = format!("interface{}", self.interfaces.len());
            self.interfaces.push(Interface {
                id: iface_id.clone(),
                label: rel.id2.clone(),
                class_id: id1_name.clone(),
            });
            rel.id2 = iface_id;
        } else {
            self.add_class(&id1_name);
            self.add_class(&id2_name);
            rel.id1 = id1_name;
            rel.id2 = id2_name;
        }

        self.relations.push(rel);
    }

    fn apply(&mut self, action: Action) -> std::result::Result<(), String> {
        match action {
            Action::SetDirection(d) => {
                self.set_direction(&d);
                Ok(())
            }
            Action::SetAccTitle(t) => {
                self.acc_title = Some(t.trim_start().to_string());
                Ok(())
            }
            Action::SetAccDescr(t) => {
                let trimmed = t.trim().to_string();
                let re = ACC_DESCR_RE.get_or_init(|| {
                    Regex::new(r"\n\s+").expect("class acc descr regex must compile")
                });
                self.acc_descr = Some(re.replace_all(&trimmed, "\n").to_string());
                Ok(())
            }

            Action::AddNamespace { id } => {
                self.add_namespace(&id);
                Ok(())
            }
            Action::AddClassesToNamespace {
                namespace,
                class_ids,
            } => {
                self.add_classes_to_namespace(&namespace, &class_ids);
                Ok(())
            }

            Action::AddClass { id } => {
                self.add_class(&id);
                Ok(())
            }
            Action::SetClassLabel { id, label } => {
                self.set_class_label(&id, &label);
                Ok(())
            }
            Action::SetCssClass { ids, css_class } => {
                self.set_css_class(&ids, &css_class);
                Ok(())
            }
            Action::SetCssStyle { id, raw } => {
                let styles = Self::parse_styles(&raw);
                self.set_css_style(&id, styles);
                Ok(())
            }
            Action::DefineClass { id, raw } => {
                let styles = Self::parse_styles(&raw);
                self.define_class(&id, styles);
                Ok(())
            }
            Action::SetLink { id, url, target } => {
                self.set_link(&id, &url, target);
                Ok(())
            }
            Action::SetTooltip { id, tooltip } => {
                self.set_tooltip(&id, &tooltip);
                Ok(())
            }
            Action::SetClickEvent { id, function, args } => {
                self.set_click_event(&id, &function, args);
                Ok(())
            }
            Action::AddMembers { id, members } => {
                self.add_members(&id, members);
                Ok(())
            }
            Action::AddMember { id, member } => {
                let cleaned = self.cleanup_label(&member);
                self.add_member(&id, &cleaned);
                Ok(())
            }
            Action::AddAnnotation { id, annotation } => {
                self.add_annotation(&id, &annotation);
                Ok(())
            }
            Action::AddRelation { mut data } => {
                if let Some(t) = data.title.take() {
                    data.title = Some(self.cleanup_label(&t));
                }
                if let Some(t) = data.relation_title1.take() {
                    data.relation_title1 = Some(sanitize_text(t.trim(), &self.config));
                }
                if let Some(t) = data.relation_title2.take() {
                    data.relation_title2 = Some(sanitize_text(t.trim(), &self.config));
                }
                self.add_relation(data);
                Ok(())
            }
            Action::AddNote { class_id, text } => {
                self.add_note(class_id, text.trim());
                Ok(())
            }
        }
    }

    fn to_model(&self, meta: &ParseMetadata) -> Value {
        let classes_json: serde_json::Map<String, Value> = self
            .classes
            .iter()
            .map(|(id, c)| {
                let methods: Vec<Value> = c.methods.iter().map(|m| m.to_json()).collect();
                let members: Vec<Value> = c.members.iter().map(|m| m.to_json()).collect();
                (
                    id.clone(),
                    json!({
                        "id": c.id,
                        "type": c.type_param,
                        "label": c.label,
                        "text": c.text,
                        "cssClasses": c.css_classes,
                        "methods": methods,
                        "members": members,
                        "annotations": c.annotations,
                        "styles": c.styles,
                        "domId": c.dom_id,
                        "parent": c.parent,
                        "link": c.link,
                        "linkTarget": c.link_target,
                        "tooltip": c.tooltip,
                        "haveCallback": c.have_callback,
                        "callback": c.callback,
                        "callbackEffective": c.callback_effective,
                    }),
                )
            })
            .collect();

        let relations_json: Vec<Value> = self
            .relations
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                json!({
                    "id": idx.to_string(),
                    "id1": r.id1,
                    "id2": r.id2,
                    "relationTitle1": r.relation_title1.clone().unwrap_or_else(|| "none".to_string()),
                    "relationTitle2": r.relation_title2.clone().unwrap_or_else(|| "none".to_string()),
                    "title": r.title.clone().unwrap_or_default(),
                    "relation": {
                        "type1": r.relation.type1,
                        "type2": r.relation.type2,
                        "lineType": r.relation.line_type,
                    }
                })
            })
            .collect();

        let notes_json: Vec<Value> = self
            .notes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "class": n.class_id,
                    "text": n.text,
                })
            })
            .collect();

        let interfaces_json: Vec<Value> = self
            .interfaces
            .iter()
            .map(|i| json!({"id": i.id, "label": i.label, "classId": i.class_id }))
            .collect();

        let namespaces_json: serde_json::Map<String, Value> = self
            .namespaces
            .iter()
            .map(|(k, ns)| {
                (
                    k.clone(),
                    json!({
                        "id": ns.id,
                        "domId": ns.dom_id,
                        "classIds": ns.class_ids,
                    }),
                )
            })
            .collect();

        let style_classes_json: serde_json::Map<String, Value> = self
            .style_classes
            .iter()
            .map(|(k, sc)| {
                (
                    k.clone(),
                    json!({
                        "id": sc.id,
                        "styles": sc.styles,
                        "textStyles": sc.text_styles,
                    }),
                )
            })
            .collect();

        json!({
            "type": meta.diagram_type,
            "direction": self.direction,
            "accTitle": self.acc_title,
            "accDescr": self.acc_descr,
            "classes": Value::Object(classes_json),
            "relations": relations_json,
            "notes": notes_json,
            "interfaces": interfaces_json,
            "namespaces": Value::Object(namespaces_json),
            "styleClasses": Value::Object(style_classes_json),
            "constants": {
                "lineType": {
                    "line": LINE_SOLID,
                    "dottedLine": LINE_DOTTED,
                },
                "relationType": {
                    "none": REL_NONE,
                    "aggregation": REL_AGGREGATION,
                    "extension": REL_EXTENSION,
                    "composition": REL_COMPOSITION,
                    "dependency": REL_DEPENDENCY,
                    "lollipop": REL_LOLLIPOP,
                }
            }
        })
    }
}
