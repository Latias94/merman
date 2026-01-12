use crate::sanitize::sanitize_text;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
struct Block {
    id: String,
    block_type: String,
    label: Option<String>,
    children: Vec<Block>,

    start: Option<String>,
    end: Option<String>,
    arrow_type_end: Option<String>,
    arrow_type_start: Option<String>,

    width: Option<i64>,
    columns: Option<i64>,
    width_in_columns: Option<i64>,
    directions: Option<Vec<String>>,

    classes: Vec<String>,
    styles: Option<Vec<String>>,

    css: Option<String>,
    style_class: Option<String>,
    styles_str: Option<String>,
}

impl Block {
    fn new(id: String) -> Self {
        Self {
            id,
            block_type: "na".to_string(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ClassDef {
    id: String,
    styles: Vec<String>,
    text_styles: Vec<String>,
}

#[derive(Debug, Default)]
struct BlockDb {
    root_id: String,
    block_database: HashMap<String, Block>,
    block_database_order: Vec<String>,
    blocks: Vec<Block>,
    edges: Vec<Block>,
    edge_count: HashMap<String, i64>,
    classes: HashMap<String, ClassDef>,
    warnings: Vec<String>,
    gen_counter: i64,
}

impl BlockDb {
    fn clear(&mut self) {
        self.root_id = "root".to_string();
        self.block_database.clear();
        self.block_database_order.clear();
        self.blocks.clear();
        self.edges.clear();
        self.edge_count.clear();
        self.classes.clear();
        self.warnings.clear();
        self.gen_counter = 0;

        let root = Block {
            id: self.root_id.clone(),
            block_type: "composite".to_string(),
            children: Vec::new(),
            columns: Some(-1),
            label: Some("".to_string()),
            ..Default::default()
        };
        self.insert_block(self.root_id.clone(), root);
    }

    fn insert_block(&mut self, id: String, block: Block) {
        let existed = self.block_database.contains_key(&id);
        self.block_database.insert(id.clone(), block);
        if !existed {
            self.block_database_order.push(id);
        }
    }

    fn ensure_block_exists(&mut self, id: &str) -> &mut Block {
        if !self.block_database.contains_key(id) {
            self.insert_block(id.to_string(), Block::new(id.to_string()));
        }
        self.block_database
            .get_mut(id)
            .expect("block must exist after ensure_block_exists")
    }

    #[allow(dead_code)]
    fn generate_id(&mut self) -> String {
        self.gen_counter += 1;
        format!("id-{}", self.gen_counter)
    }

    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let entry = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| ClassDef {
                id: id.to_string(),
                styles: Vec::new(),
                text_styles: Vec::new(),
            });

        for raw in style_attributes.split(',') {
            let fixed = raw.splitn(2, ';').next().unwrap_or("").trim().to_string();
            if fixed.is_empty() {
                continue;
            }

            if raw.contains("color") {
                let new_style1 = fixed.replace("fill", "bgFill");
                let new_style2 = new_style1.replace("color", "fill");
                entry.text_styles.push(new_style2);
            }
            entry.styles.push(fixed);
        }
    }

    fn add_style_to_node(&mut self, id: &str, styles: &str) {
        let parts: Vec<String> = styles
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if let Some(block) = self.block_database.get_mut(id) {
            block.styles = Some(parts);
            return;
        }

        let mut placeholder = Block::new(id.to_string());
        placeholder.styles = Some(parts);
        self.insert_block(id.to_string(), placeholder);
    }

    fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for raw_id in item_ids.split(',') {
            let id = raw_id.trim();
            if id.is_empty() {
                continue;
            }

            let entry = self.ensure_block_exists(id);
            entry.classes.push(css_class_name.to_string());
        }
    }

    fn set_hierarchy(&mut self, blocks: Vec<Block>, config: &MermaidConfig) -> Result<()> {
        let root_id = self.root_id.clone();
        self.populate_block_database(blocks, &root_id, config)?;
        let root = self
            .block_database
            .get(&self.root_id)
            .cloned()
            .unwrap_or_default();
        self.blocks = root.children;
        Ok(())
    }

    fn populate_block_database(
        &mut self,
        blocks: Vec<Block>,
        parent_id: &str,
        config: &MermaidConfig,
    ) -> Result<()> {
        let col = blocks
            .iter()
            .find(|b| b.block_type == "column-setting")
            .and_then(|b| b.columns)
            .unwrap_or(-1);

        let mut child_ids: Vec<String> = Vec::new();
        for mut block in blocks {
            if col > 0
                && block.block_type != "column-setting"
                && block.width_in_columns.is_some_and(|w| w > col)
            {
                self.warnings.push(format!(
                    "Block {} width {} exceeds configured column width {}",
                    block.id,
                    block.width_in_columns.unwrap_or(1),
                    col
                ));
            }

            if let Some(label) = &block.label {
                block.label = Some(sanitize_text(label, config));
            }

            match block.block_type.as_str() {
                "classDef" => {
                    let css = block.css.clone().unwrap_or_default();
                    self.add_style_class(&block.id, &css);
                    continue;
                }
                "applyClass" => {
                    let style_class = block.style_class.clone().unwrap_or_default();
                    self.set_css_class(&block.id, &style_class);
                    continue;
                }
                "applyStyles" => {
                    if let Some(styles) = block.styles_str.clone() {
                        self.add_style_to_node(&block.id, &styles);
                    }
                    continue;
                }
                "column-setting" => {
                    if let Some(parent) = self.block_database.get_mut(parent_id) {
                        parent.columns = block.columns;
                    }
                    continue;
                }
                "edge" => {
                    let base_id = block.id.clone();
                    let count = self.edge_count.get(&base_id).copied().unwrap_or(0) + 1;
                    self.edge_count.insert(base_id.clone(), count);
                    block.id = format!("{count}-{base_id}");
                    self.edges.push(block);
                    continue;
                }
                _ => {}
            }

            if block.label.is_none() {
                if block.block_type == "composite" {
                    block.label = Some("".to_string());
                } else {
                    block.label = Some(block.id.clone());
                }
            }

            let parsed_children = std::mem::take(&mut block.children);

            let existed = self.block_database.contains_key(&block.id);
            if !existed {
                self.insert_block(block.id.clone(), block.clone());
            } else {
                let mut existing = self
                    .block_database
                    .get(&block.id)
                    .cloned()
                    .unwrap_or_else(|| Block::new(block.id.clone()));
                if block.block_type != "na" {
                    existing.block_type = block.block_type.clone();
                }
                if let Some(lbl) = &block.label {
                    if lbl != &block.id {
                        existing.label = Some(lbl.clone());
                    }
                }
                if let Some(cols) = block.columns {
                    existing.columns = Some(cols);
                }
                if let Some(w) = block.width_in_columns {
                    existing.width_in_columns = Some(w);
                }
                if let Some(w) = block.width {
                    existing.width = Some(w);
                }
                if let Some(dirs) = &block.directions {
                    existing.directions = Some(dirs.clone());
                }
                self.insert_block(block.id.clone(), existing);
            }

            if !parsed_children.is_empty() {
                self.populate_block_database(parsed_children, &block.id, config)?;
            }

            if block.block_type == "space" {
                let w = block.width.unwrap_or(1).max(0);
                for j in 0..w {
                    let id = format!("{}-{}", block.id, j);
                    let mut new_block = block.clone();
                    new_block.id = id.clone();
                    self.insert_block(id.clone(), new_block);
                    child_ids.push(id);
                }
                continue;
            }

            if !existed {
                child_ids.push(block.id.clone());
            }
        }

        let child_blocks: Vec<Block> = child_ids
            .iter()
            .filter_map(|id| self.block_database.get(id).cloned())
            .collect();
        if let Some(parent) = self.block_database.get_mut(parent_id) {
            parent.children = child_blocks;
        }

        Ok(())
    }

    fn blocks_flat(&self) -> Vec<Block> {
        self.block_database_order
            .iter()
            .filter_map(|id| self.block_database.get(id).cloned())
            .collect()
    }
}

fn block_to_value(b: &Block) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(b.id));
    obj.insert("type".to_string(), json!(b.block_type));
    if let Some(label) = &b.label {
        obj.insert("label".to_string(), json!(label));
    }
    obj.insert(
        "children".to_string(),
        Value::Array(b.children.iter().map(block_to_value).collect()),
    );

    if let Some(v) = &b.start {
        obj.insert("start".to_string(), json!(v));
    }
    if let Some(v) = &b.end {
        obj.insert("end".to_string(), json!(v));
    }
    if let Some(v) = &b.arrow_type_end {
        obj.insert("arrowTypeEnd".to_string(), json!(v));
    }
    if let Some(v) = &b.arrow_type_start {
        obj.insert("arrowTypeStart".to_string(), json!(v));
    }
    if let Some(v) = b.width {
        obj.insert("width".to_string(), json!(v));
    }
    if let Some(v) = b.columns {
        obj.insert("columns".to_string(), json!(v));
    }
    if let Some(v) = b.width_in_columns {
        obj.insert("widthInColumns".to_string(), json!(v));
    }
    if let Some(v) = &b.directions {
        obj.insert("directions".to_string(), json!(v));
    }
    if !b.classes.is_empty() {
        obj.insert("classes".to_string(), json!(b.classes));
    }
    if let Some(v) = &b.styles {
        obj.insert("styles".to_string(), json!(v));
    }
    if let Some(v) = &b.css {
        obj.insert("css".to_string(), json!(v));
    }
    if let Some(v) = &b.style_class {
        obj.insert("styleClass".to_string(), json!(v));
    }
    if let Some(v) = &b.styles_str {
        obj.insert("stylesStr".to_string(), json!(v));
    }

    Value::Object(obj)
}

fn class_def_map_to_value(classes: &HashMap<String, ClassDef>) -> Value {
    let mut obj = Map::new();
    for (k, v) in classes {
        obj.insert(
            k.clone(),
            json!({
                "id": v.id,
                "styles": v.styles,
                "textStyles": v.text_styles,
            }),
        );
    }
    Value::Object(obj)
}

fn type_str_to_type(type_str: &str) -> String {
    match type_str {
        "[]" => "square",
        "()" => "round",
        "(())" => "circle",
        ">]" => "rect_left_inv_arrow",
        "{}" => "diamond",
        "{{}}" => "hexagon",
        "([])" => "stadium",
        "[[]]" => "subroutine",
        "[()]" => "cylinder",
        "((()))" => "doublecircle",
        "[//]" => "lean_right",
        "[\\\\]" => "lean_left",
        "[/\\]" => "trapezoid",
        "[\\/]" => "inv_trapezoid",
        "<[]>" => "block_arrow",
        _ => "na",
    }
    .to_string()
}

fn edge_str_to_edge_data(type_str: &str) -> String {
    let trimmed = type_str.trim_matches(|c: char| c.is_whitespace() || c == '-');
    match trimmed {
        "x" => "arrow_cross",
        "o" => "arrow_circle",
        ">" => "arrow_point",
        _ => "",
    }
    .to_string()
}

fn is_valid_link_token(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }

    if s.chars().all(|c| c == '~') {
        return s.len() >= 3;
    }

    let (prefix, rest) = match s.chars().next() {
        Some('x') | Some('o') | Some('<') => (&s[..1], &s[1..]),
        _ => ("", s),
    };
    let _ = prefix;

    is_valid_solid_link(rest) || is_valid_thick_link(rest) || is_valid_dotted_link(rest)
}

fn is_valid_solid_link(rest: &str) -> bool {
    if rest.is_empty() || !rest.starts_with('-') {
        return false;
    }

    if rest.chars().all(|c| c == '-') {
        return rest.len() >= 3;
    }

    let (body, tail) = rest.split_at(rest.len() - 1);
    let last = tail.chars().next().unwrap_or('\0');
    if !matches!(last, '-' | 'x' | 'o' | '>') {
        return false;
    }

    let dash_count = body.chars().filter(|c| *c == '-').count();
    dash_count >= 2 && body.chars().all(|c| c == '-')
}

fn is_valid_thick_link(rest: &str) -> bool {
    if rest.is_empty() || !rest.starts_with('=') {
        return false;
    }

    if rest.chars().all(|c| c == '=') {
        return rest.len() >= 3;
    }

    let (body, tail) = rest.split_at(rest.len() - 1);
    let last = tail.chars().next().unwrap_or('\0');
    if !matches!(last, '=' | 'x' | 'o' | '>') {
        return false;
    }

    let eq_count = body.chars().filter(|c| *c == '=').count();
    eq_count >= 2 && body.chars().all(|c| c == '=')
}

fn is_valid_dotted_link(rest: &str) -> bool {
    if rest.is_empty() {
        return false;
    }

    let mut chars = rest.chars().peekable();
    if matches!(chars.peek(), Some('-')) {
        chars.next();
    }

    let mut dot_count = 0usize;
    while matches!(chars.peek(), Some('.')) {
        dot_count += 1;
        chars.next();
    }
    if dot_count == 0 {
        return false;
    }

    if chars.next() != Some('-') {
        return false;
    }

    let tail: String = chars.collect();
    if tail.is_empty() {
        return true;
    }
    if tail.len() == 1 {
        return matches!(tail.chars().next(), Some('x' | 'o' | '>'));
    }
    false
}

struct NodeDelims {
    start: &'static str,
    ends: &'static [&'static str],
}

fn node_delims_at_start(input: &str) -> Option<NodeDelims> {
    let delims: &[NodeDelims] = &[
        NodeDelims {
            start: "([",
            ends: &["])"],
        },
        NodeDelims {
            start: "[[",
            ends: &["]]"],
        },
        NodeDelims {
            start: "[(",
            ends: &[")]"],
        },
        NodeDelims {
            start: "(((",
            ends: &[")))"],
        },
        NodeDelims {
            start: "((",
            ends: &["))", ")"],
        },
        NodeDelims {
            start: "{{",
            ends: &["}}"],
        },
        NodeDelims {
            start: "[/",
            ends: &["/]", "\\]"],
        },
        NodeDelims {
            start: "[\\",
            ends: &["\\]", "/]"],
        },
        NodeDelims {
            start: "[",
            ends: &["]"],
        },
        NodeDelims {
            start: "(",
            ends: &[")"],
        },
        NodeDelims {
            start: "{",
            ends: &["}"],
        },
        NodeDelims {
            start: ">",
            ends: &["]"],
        },
    ];

    for d in delims {
        if input.starts_with(d.start) {
            return Some(NodeDelims {
                start: d.start,
                ends: d.ends,
            });
        }
    }

    None
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    gen_counter: i64,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            gen_counter: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn generate_id(&mut self) -> String {
        self.gen_counter += 1;
        format!("id-{}", self.gen_counter)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while self.peek_char().is_some_and(|c| c.is_whitespace()) {
                self.bump();
            }

            if self.starts_with("%%") {
                while let Some(c) = self.bump() {
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }

            break;
        }
    }

    fn peek_keyword(&mut self, kw: &str) -> bool {
        self.skip_ws_and_comments();
        if !self.starts_with(kw) {
            return false;
        }
        if kw.ends_with(':') {
            return true;
        }
        let after = &self.input[self.pos + kw.len()..];
        after
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == ':')
    }

    fn consume_keyword(&mut self, kw: &str) -> bool {
        if !self.peek_keyword(kw) {
            return false;
        }
        self.pos += kw.len();
        true
    }

    fn consume_exact(&mut self, s: &str) -> bool {
        self.skip_ws_and_comments();
        if !self.starts_with(s) {
            return false;
        }
        self.pos += s.len();
        true
    }

    fn parse_header(&mut self) -> Result<()> {
        self.skip_ws_and_comments();
        if self.consume_keyword("block-beta") {
            return Ok(());
        }
        if self.consume_keyword("block") {
            return Ok(());
        }
        Err(Error::DiagramParse {
            diagram_type: "block".to_string(),
            message: "expected block header".to_string(),
        })
    }

    fn parse_document(&mut self, stop_on_end: bool) -> Result<Vec<Block>> {
        let mut out = Vec::<Block>::new();
        loop {
            self.skip_ws_and_comments();
            if self.is_eof() {
                break;
            }
            if stop_on_end && self.peek_keyword("end") {
                self.consume_keyword("end");
                break;
            }

            if self.peek_keyword("block:") {
                out.push(self.parse_id_block()?);
                continue;
            }
            if self.peek_keyword("block-beta") || self.peek_keyword("block") {
                out.push(self.parse_anonymous_block()?);
                continue;
            }
            if self.peek_keyword("columns") {
                out.push(self.parse_columns_statement()?);
                continue;
            }
            if self.peek_keyword("space") {
                out.push(self.parse_space_statement()?);
                continue;
            }
            if self.peek_keyword("classDef") {
                out.push(self.parse_classdef_statement()?);
                continue;
            }
            if self.peek_keyword("class") {
                out.push(self.parse_apply_class_statement()?);
                continue;
            }
            if self.peek_keyword("style") {
                out.push(self.parse_style_statement()?);
                continue;
            }

            let mut blocks = self.parse_node_statement()?;
            out.append(&mut blocks);
        }
        Ok(out)
    }

    fn parse_anonymous_block(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !(self.consume_keyword("block-beta") || self.consume_keyword("block")) {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected block".to_string(),
            });
        }
        let children = self.parse_document(true)?;
        let mut b = Block::new(self.generate_id());
        b.block_type = "composite".to_string();
        b.label = Some("".to_string());
        b.children = children;
        Ok(b)
    }

    fn parse_id_block(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("block:") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected block:".to_string(),
            });
        }
        let mut stm = self.parse_node_statement()?;
        let header = stm
            .drain(..)
            .find(|b| b.block_type != "edge")
            .unwrap_or_else(|| Block::new(self.generate_id()));
        let children = self.parse_document(true)?;

        let mut out = header;
        out.block_type = "composite".to_string();
        out.children = children;
        Ok(out)
    }

    fn parse_columns_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("columns") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected columns".to_string(),
            });
        }
        self.skip_ws_and_comments();
        let value = if self.consume_keyword("auto") {
            -1
        } else {
            self.parse_int()?
        };

        let mut b = Block::new(self.generate_id());
        b.block_type = "column-setting".to_string();
        b.columns = Some(value);
        Ok(b)
    }

    fn parse_space_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("space") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected space".to_string(),
            });
        }
        let mut width = 1;
        self.skip_ws_and_comments();
        if self.consume_exact(":") {
            width = self.parse_int()? as i64;
        }
        let mut b = Block::new(self.generate_id());
        b.block_type = "space".to_string();
        b.label = Some("".to_string());
        b.width = Some(width);
        Ok(b)
    }

    fn parse_classdef_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("classDef") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected classDef".to_string(),
            });
        }
        self.skip_ws_and_comments();
        let id = self.parse_identifier_like()?;
        let css = self.take_rest_of_line_trimmed();
        let mut b = Block::new(id);
        b.block_type = "classDef".to_string();
        b.css = Some(css);
        Ok(b)
    }

    fn parse_apply_class_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("class") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected class".to_string(),
            });
        }
        self.skip_ws_and_comments();
        let ids = self.parse_identifier_like()?;
        let style_class = self.take_rest_of_line_trimmed();
        let mut b = Block::new(ids);
        b.block_type = "applyClass".to_string();
        b.style_class = Some(style_class);
        Ok(b)
    }

    fn parse_style_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("style") {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected style".to_string(),
            });
        }
        self.skip_ws_and_comments();
        let ids = self.parse_identifier_like()?;
        let styles_str = self.take_rest_of_line_trimmed();
        let mut b = Block::new(ids);
        b.block_type = "applyStyles".to_string();
        b.styles_str = Some(styles_str);
        Ok(b)
    }

    fn take_rest_of_line_trimmed(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
        self.input[start..self.pos].trim().to_string()
    }

    fn parse_node_statement(&mut self) -> Result<Vec<Block>> {
        let mut left = self.parse_node()?;
        self.skip_ws_and_comments();

        if let Some((label, edge_marker)) = self.parse_link()? {
            let mut right = self.parse_node()?;
            let arrow_type_end = edge_str_to_edge_data(&edge_marker);
            let edge_id = format!("{}-{}", left.id, right.id);
            let edge = Block {
                id: edge_id,
                block_type: "edge".to_string(),
                label: Some(label),
                children: Vec::new(),
                start: Some(left.id.clone()),
                end: Some(right.id.clone()),
                arrow_type_end: Some(arrow_type_end),
                arrow_type_start: Some("arrow_open".to_string()),
                directions: right.directions.clone(),
                ..Default::default()
            };

            left.width_in_columns.get_or_insert(1);
            right.width_in_columns.get_or_insert(1);
            return Ok(vec![left, edge, right]);
        }

        self.skip_ws_and_comments();
        if self.consume_exact(":") {
            let w = self.parse_int()? as i64;
            left.width_in_columns = Some(w);
        } else {
            left.width_in_columns.get_or_insert(1);
        }

        Ok(vec![left])
    }

    fn parse_link(&mut self) -> Result<Option<(String, String)>> {
        self.skip_ws_and_comments();
        if self.is_eof() {
            return Ok(None);
        }

        let snapshot = self.pos;
        if self.try_read_link_start_marker().is_some() {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('"') {
                let label = self.parse_string_literal()?;
                self.skip_ws_and_comments();
                if let Some(edge_marker) = self.try_read_link_full_marker() {
                    return Ok(Some((label, edge_marker)));
                }
                self.pos = snapshot;
                return Ok(None);
            }
            self.pos = snapshot;
        }

        if let Some(edge_marker) = self.try_read_link_full_marker() {
            return Ok(Some(("".to_string(), edge_marker)));
        }

        Ok(None)
    }

    fn try_read_link_start_marker(&mut self) -> Option<String> {
        self.skip_ws_and_comments();
        let start = self.pos;
        if self
            .peek_char()
            .is_some_and(|c| c == 'x' || c == 'o' || c == '<')
        {
            self.bump()?;
        }
        if self.starts_with("--") || self.starts_with("==") || self.starts_with("-.") {
            self.bump()?;
            self.bump()?;
            return Some(self.input[start..self.pos].to_string());
        }
        self.pos = start;
        None
    }

    fn try_read_link_full_marker(&mut self) -> Option<String> {
        self.skip_ws_and_comments();
        let start = self.pos;

        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                break;
            }
            self.bump();
        }

        if self.pos == start {
            return None;
        }

        let token = &self.input[start..self.pos];
        if !is_valid_link_token(token) {
            self.pos = start;
            return None;
        }
        Some(token.to_string())
    }

    fn parse_node(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        let id = self.parse_node_id()?;
        let mut b = Block::new(id);
        b.label = None;
        b.block_type = "na".to_string();

        self.skip_ws_and_comments();

        if self.starts_with("<[") {
            self.pos += 2;
            self.skip_ws_and_comments();
            let label = self.parse_string_literal()?;
            self.skip_ws_and_comments();
            if !self.consume_exact("]>") {
                return Err(Error::DiagramParse {
                    diagram_type: "block".to_string(),
                    message: "expected ]> in block arrow".to_string(),
                });
            }
            self.skip_ws_and_comments();
            if !self.consume_exact("(") {
                return Err(Error::DiagramParse {
                    diagram_type: "block".to_string(),
                    message: "expected '(' in block arrow".to_string(),
                });
            }
            let dirs = self.parse_direction_list()?;
            if !self.consume_exact(")") {
                return Err(Error::DiagramParse {
                    diagram_type: "block".to_string(),
                    message: "expected ')' in block arrow".to_string(),
                });
            }

            b.label = Some(label);
            b.block_type = "block_arrow".to_string();
            b.directions = Some(dirs);
            b.width_in_columns = Some(1);
            return Ok(b);
        }

        if let Some(delims) = node_delims_at_start(&self.input[self.pos..]) {
            let start_delim = delims.start;
            self.pos += start_delim.len();
            self.skip_ws_and_comments();
            let label = self.parse_string_literal_or_md()?;
            self.skip_ws_and_comments();
            let mut matched_end: Option<&'static str> = None;
            for end in delims.ends {
                if self.consume_exact(end) {
                    matched_end = Some(end);
                    break;
                }
            }
            let end_delim = match matched_end {
                Some(e) => e,
                None => {
                    return Err(Error::DiagramParse {
                        diagram_type: "block".to_string(),
                        message: "unterminated node delimiter".to_string(),
                    });
                }
            };
            if end_delim.is_empty() {
                return Err(Error::DiagramParse {
                    diagram_type: "block".to_string(),
                    message: "unterminated node delimiter".to_string(),
                });
            }

            let type_str = format!("{start_delim}{end_delim}");
            b.label = Some(label);
            b.block_type = type_str_to_type(&type_str);
            b.width_in_columns = Some(1);
            return Ok(b);
        }

        Ok(b)
    }

    fn parse_direction_list(&mut self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_comments();
            let w = self.parse_direction()?;
            out.push(w);
            self.skip_ws_and_comments();
            if self.consume_exact(",") {
                continue;
            }
            break;
        }
        Ok(out)
    }

    fn parse_direction(&mut self) -> Result<String> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == ',' || c == ')' {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected direction".to_string(),
            });
        }
        let dir = self.input[start..self.pos].trim().to_string();
        match dir.as_str() {
            "right" | "left" | "x" | "y" | "up" | "down" => Ok(dir),
            _ => Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: format!("invalid direction: {dir}"),
            }),
        }
    }

    fn parse_node_id(&mut self) -> Result<String> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace()
                || matches!(
                    c,
                    '(' | '[' | '\n' | '-' | ')' | '{' | '}' | '<' | '>' | ':'
                )
            {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected node id".to_string(),
            });
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_identifier_like(&mut self) -> Result<String> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected identifier".to_string(),
            });
        }
        Ok(self.input[start..self.pos].trim().to_string())
    }

    fn parse_int(&mut self) -> Result<i64> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        if self.pos == start {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected integer".to_string(),
            });
        }
        self.input[start..self.pos]
            .parse::<i64>()
            .map_err(|e| Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: e.to_string(),
            })
    }

    fn parse_string_literal_or_md(&mut self) -> Result<String> {
        self.skip_ws_and_comments();
        if self.starts_with("\"`") {
            self.pos += 2;
            let start = self.pos;
            while self.pos < self.input.len() && !self.input[self.pos..].starts_with("`\"") {
                self.bump();
            }
            if self.pos >= self.input.len() {
                return Err(Error::DiagramParse {
                    diagram_type: "block".to_string(),
                    message: "unterminated markdown string".to_string(),
                });
            }
            let inner = self.input[start..self.pos].to_string();
            self.pos += 2;
            return Ok(inner);
        }
        self.parse_string_literal()
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        self.skip_ws_and_comments();
        if self.peek_char() != Some('"') {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "expected string literal".to_string(),
            });
        }
        self.bump();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '"' {
                break;
            }
            self.bump();
        }
        if self.peek_char() != Some('"') {
            return Err(Error::DiagramParse {
                diagram_type: "block".to_string(),
                message: "unterminated string literal".to_string(),
            });
        }
        let inner = self.input[start..self.pos].to_string();
        self.bump();
        Ok(inner)
    }
}

pub fn parse_block(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut parser = Parser::new(code);
    parser.parse_header()?;
    let blocks = parser.parse_document(false)?;

    let mut db = BlockDb::default();
    db.clear();
    db.gen_counter = parser.gen_counter;
    db.set_hierarchy(blocks, &meta.effective_config)?;

    Ok(json!({
        "type": meta.diagram_type,
        "blocks": db.blocks.iter().map(block_to_value).collect::<Vec<_>>(),
        "edges": db.edges.iter().map(block_to_value).collect::<Vec<_>>(),
        "blocksFlat": db.blocks_flat().iter().map(block_to_value).collect::<Vec<_>>(),
        "classes": class_def_map_to_value(&db.classes),
        "warnings": db.warnings,
        "config": meta.effective_config.as_value().clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn blocks(model: &Value) -> Vec<Value> {
        model["blocks"].as_array().cloned().unwrap_or_default()
    }

    fn edges(model: &Value) -> Vec<Value> {
        model["edges"].as_array().cloned().unwrap_or_default()
    }

    fn columns_for_id(model: &Value, id: &str) -> Option<i64> {
        for b in model["blocksFlat"].as_array()? {
            if b["id"].as_str()? == id {
                return b.get("columns").and_then(|v| v.as_i64());
            }
        }
        None
    }

    #[test]
    fn block_diagram_with_node() {
        let model = parse("block-beta\n  id\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["id"].as_str().unwrap(), "id");
        assert_eq!(blocks[0]["label"].as_str().unwrap(), "id");
    }

    #[test]
    fn node_with_square_shape_and_label() {
        let model = parse("block\n  id[\"A label\"]\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["id"].as_str().unwrap(), "id");
        assert_eq!(blocks[0]["label"].as_str().unwrap(), "A label");
        assert_eq!(blocks[0]["type"].as_str().unwrap(), "square");
    }

    #[test]
    fn multiple_nodes() {
        let model = parse("block\n  id1\n  id2\n  id3\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0]["id"].as_str().unwrap(), "id1");
        assert_eq!(blocks[1]["id"].as_str().unwrap(), "id2");
        assert_eq!(blocks[2]["id"].as_str().unwrap(), "id3");
    }

    #[test]
    fn nodes_with_edge_basic() {
        let model = parse("block\n  id1[\"first\"]  -->   id2[\"second\"]\n");
        let blocks = blocks(&model);
        let edges = edges(&model);
        assert_eq!(blocks.len(), 2);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["start"].as_str().unwrap(), "id1");
        assert_eq!(edges[0]["end"].as_str().unwrap(), "id2");
        assert_eq!(edges[0]["arrowTypeEnd"].as_str().unwrap(), "arrow_point");
    }

    #[test]
    fn nodes_with_edge_label() {
        let model = parse("block\n  id1[\"first\"]  -- \"a label\" -->   id2[\"second\"]\n");
        let edges = edges(&model);
        assert_eq!(edges[0]["label"].as_str().unwrap(), "a label");
    }

    #[test]
    fn diagram_with_column_statements() {
        let model = parse("block\n  columns 2\n  block1[\"Block 1\"]\n");
        assert_eq!(columns_for_id(&model, "root").unwrap(), 2);
        assert_eq!(blocks(&model).len(), 1);
    }

    #[test]
    fn diagram_without_column_statements() {
        let model = parse("block\n  block1[\"Block 1\"]\n");
        assert_eq!(columns_for_id(&model, "root").unwrap(), -1);
        assert_eq!(blocks(&model).len(), 1);
    }

    #[test]
    fn diagram_with_auto_column_statements() {
        let model = parse("block\n  columns auto\n  block1[\"Block 1\"]\n");
        assert_eq!(columns_for_id(&model, "root").unwrap(), -1);
        assert_eq!(blocks(&model).len(), 1);
    }

    #[test]
    fn blocks_next_to_each_other() {
        let model = parse("block\n  columns 2\n  block1[\"Block 1\"]\n  block2[\"Block 2\"]\n");
        assert_eq!(columns_for_id(&model, "root").unwrap(), 2);
        assert_eq!(blocks(&model).len(), 2);
    }

    #[test]
    fn blocks_on_top_of_each_other() {
        let model = parse("block\n  columns 1\n  block1[\"Block 1\"]\n  block2[\"Block 2\"]\n");
        assert_eq!(columns_for_id(&model, "root").unwrap(), 1);
        assert_eq!(blocks(&model).len(), 2);
    }

    #[test]
    fn compound_blocks() {
        let model =
            parse("block\n  block\n    aBlock[\"ABlock\"]\n    bBlock[\"BBlock\"]\n  end\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"].as_str().unwrap(), "composite");
        assert_eq!(blocks[0]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn compound_blocks_of_compound_blocks() {
        let model = parse(
            "block\n  block\n    aBlock[\"ABlock\"]\n    block\n      bBlock[\"BBlock\"]\n    end\n  end\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        let first = &blocks[0];
        assert_eq!(first["children"].as_array().unwrap().len(), 2);
        let a_block = &first["children"][0];
        assert_eq!(a_block["label"].as_str().unwrap(), "ABlock");
        let second_composite = &first["children"][1];
        assert_eq!(second_composite["type"].as_str().unwrap(), "composite");
        assert_eq!(second_composite["children"].as_array().unwrap().len(), 1);
        let b_block = &second_composite["children"][0];
        assert_eq!(b_block["label"].as_str().unwrap(), "BBlock");
    }

    #[test]
    fn compound_blocks_with_title() {
        let model = parse(
            "block\n  block:compoundBlock[\"Compound block\"]\n    columns 1\n    block2[\"Block 2\"]\n  end\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        let compound = &blocks[0];
        assert_eq!(compound["id"].as_str().unwrap(), "compoundBlock");
        assert_eq!(compound["label"].as_str().unwrap(), "Compound block");
        assert_eq!(compound["type"].as_str().unwrap(), "composite");
        assert_eq!(compound["children"].as_array().unwrap().len(), 1);
        assert_eq!(compound["children"][0]["id"].as_str().unwrap(), "block2");
    }

    #[test]
    fn blocks_mixed_with_compound_blocks() {
        let model = parse(
            "block\n  columns 1\n  block1[\"Block 1\"]\n\n  block\n    columns 2\n    block2[\"Block 2\"]\n    block3[\"Block 3\"]\n  end\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 2);
        let compound = &blocks[1];
        assert_eq!(compound["type"].as_str().unwrap(), "composite");
        assert_eq!(compound["children"].as_array().unwrap().len(), 2);
        assert_eq!(compound["children"][0]["id"].as_str().unwrap(), "block2");
    }

    #[test]
    fn arrow_blocks() {
        let model = parse(
            "block\n  columns 3\n  block1[\"Block 1\"]\n  blockArrow<[\"&nbsp;&nbsp;&nbsp;\"]>(right)\n  block2[\"Block 2\"]\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1]["type"].as_str().unwrap(), "block_arrow");
        assert!(
            blocks[1]["directions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("right"))
        );
    }

    #[test]
    fn arrow_blocks_with_multiple_points() {
        let model = parse(
            "block\n  columns 1\n  A\n  blockArrow<[\"&nbsp;&nbsp;&nbsp;\"]>(up, down)\n  block\n    columns 3\n    B\n    C\n    D\n  end\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 3);
        let arrow = &blocks[1];
        assert_eq!(arrow["type"].as_str().unwrap(), "block_arrow");
        let dirs: Vec<&str> = arrow["directions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(dirs.contains(&"up"));
        assert!(dirs.contains(&"down"));
        assert!(!dirs.contains(&"right"));
    }

    #[test]
    fn blocks_with_different_widths() {
        let model = parse("block\n  columns 3\n  one[\"One Slot\"]\n  two[\"Two slots\"]:2\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1]["widthInColumns"].as_i64().unwrap(), 2);
    }

    #[test]
    fn empty_blocks_space() {
        let model = parse("block\n  columns 3\n  space\n  middle[\"In the middle\"]\n  space\n");
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0]["type"].as_str().unwrap(), "space");
        assert_eq!(blocks[2]["type"].as_str().unwrap(), "space");
        assert_eq!(blocks[1]["label"].as_str().unwrap(), "In the middle");
    }

    #[test]
    fn classdef_and_apply_class() {
        let model = parse(
            "block\n  classDef black color:#ffffff, fill:#000000;\n  mc[\"Memcache\"]\n  class mc black\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0]["classes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("black"))
        );
        let classes = model["classes"].as_object().unwrap();
        let black = classes.get("black").unwrap();
        assert_eq!(black["id"].as_str().unwrap(), "black");
        assert_eq!(black["styles"][0].as_str().unwrap(), "color:#ffffff");
    }

    #[test]
    fn style_statement_applied() {
        let model = parse(
            "block\n  columns 1\n  B[\"A wide one in the middle\"]\n  style B fill:#f9F,stroke:#333,stroke-width:4px\n",
        );
        let blocks = blocks(&model);
        assert_eq!(blocks.len(), 1);
        let styles: Vec<&str> = blocks[0]["styles"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(styles.contains(&"fill:#f9F"));
    }

    #[test]
    fn warns_when_block_width_exceeds_column_width() {
        let model = parse("block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n");
        let warnings: Vec<&str> = model["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(
            warnings
                .iter()
                .any(|w| *w == "Block B width 2 exceeds configured column width 1")
        );
    }

    #[test]
    fn prototype_property_ids_do_not_crash() {
        for prop in ["__proto__", "constructor"] {
            let text = format!("block\n{prop}\n");
            let _ = parse(&text);
            let text =
                format!("block\nA\nclassDef {prop} color:#ffffff,fill:#000000;\nclass A {prop}\n");
            let _ = parse(&text);
            let text =
                format!("block\nA; classDef {prop} color:#ffffff,fill:#000000; class A {prop}");
            let _ = parse(&text);
        }
    }
}
