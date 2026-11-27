//! Block syntax, deterministic ID generation, and bounded semantic construction.

use indexmap::IndexMap;
use merman_markup::sanitize::sanitize_text;
use merman_model::families::block::{
    BlockClassDef, BlockDiagramModel, BlockEdge, BlockEdgePattern, BlockEdgeThickness, BlockNode,
};
#[cfg(feature = "presentation")]
use merman_model::families::block::{
    BlockDiagramRenderModel, BlockFieldPresence, BlockNodeFieldPresence, BlockProjectionFacts,
    BlockVisuals,
};
use merman_model::runtime::OperationContext;
use merman_model::warnings::{BLOCK_WIDTH_WARNING_RULE_ID, DiagramWarningFact};
use merman_model::{MermaidConfig, OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashMap;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;

// Block spacing is materialized as one semantic placeholder per occupied column to match Mermaid.
// Bound total space expansion before allocation; larger bounded product profiles allow at most this many
// model items, and an unbounded profile must still not turn a tiny source into an effectively
// infinite allocation.
pub const MAX_BLOCK_SPACE_EXPANSION_ITEMS: usize = 200_000;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static BLOCK_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_block_syntax_construction_count() {
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn block_syntax_construction_count() -> usize {
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug, Clone, Default)]
struct Block {
    id: String,
    #[cfg(feature = "presentation")]
    color_index: Option<usize>,
    block_type: String,
    label: Option<String>,
    children: Vec<usize>,

    start: Option<String>,
    end: Option<String>,
    arrow_type_end: Option<String>,
    arrow_type_start: Option<String>,
    thickness: BlockEdgeThickness,
    pattern: BlockEdgePattern,

    width: Option<f64>,
    columns: Option<f64>,
    width_in_columns: Option<f64>,
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

#[derive(Debug, Default)]
struct BlockDb {
    root_id: String,
    #[cfg(feature = "presentation")]
    next_color_index: usize,
    block_ids: HashMap<String, usize>,
    blocks: Vec<Block>,
    edges: Vec<Block>,
    edge_count: HashMap<String, i64>,
    classes: IndexMap<String, BlockClassDef>,
    warning_facts: Vec<DiagramWarningFact>,
}

impl BlockDb {
    fn clear(&mut self) {
        *self = Self::default();
        self.root_id = "root".to_string();
        self.insert_block(Block {
            id: self.root_id.clone(),
            block_type: "composite".to_string(),
            columns: Some(-1.0),
            label: Some(String::new()),
            ..Default::default()
        });
    }

    fn insert_block(&mut self, block: Block) -> usize {
        if let Some(&index) = self.block_ids.get(&block.id) {
            self.blocks[index] = block;
            index
        } else {
            let index = self.blocks.len();
            self.block_ids.insert(block.id.clone(), index);
            self.blocks.push(block);
            index
        }
    }

    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let entry = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| BlockClassDef::new(id));

        for raw in style_attributes.split(',') {
            let fixed = raw.split(';').next().unwrap_or("").trim().to_string();
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

        if let Some(&index) = self.block_ids.get(id) {
            let block = &mut self.blocks[index];
            block.styles = Some(parts);
            return;
        }

        let mut placeholder = Block::new(id.to_string());
        placeholder.styles = Some(parts);
        self.insert_block(placeholder);
    }

    fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for raw_id in item_ids.split(',') {
            let id = raw_id.trim();
            if id.is_empty() {
                continue;
            }
            let index = self.block_ids.get(raw_id).copied().unwrap_or_else(|| {
                // Mermaid looks up the authored token before trimming a missing ID. Rebinding
                // the map must not mutate an older object already attached to the hierarchy.
                let index = self.blocks.len();
                self.blocks.push(Block::new(id.to_string()));
                self.block_ids.insert(id.to_string(), index);
                index
            });
            self.blocks[index].classes.push(css_class_name.to_string());
        }
    }

    fn set_hierarchy(
        &mut self,
        document: BlockDocument,
        config: &MermaidConfig,
        _presentation_requested: bool,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut records = document.records.into_iter().map(Some).collect::<Vec<_>>();
        let mut stack = vec![PopulateFrame::new(Some(0), document.blocks, &records)];
        let mut visited_count = 0usize;
        while !stack.is_empty() {
            if visited_count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited_count += 1;
            let next = stack.last_mut().and_then(|frame| {
                frame
                    .blocks
                    .next()
                    .map(|index| (index, frame.parent, frame.col))
            });
            let Some((index, parent, col)) = next else {
                if let Some(frame) = stack.pop()
                    && let Some(parent) = frame.parent
                {
                    self.blocks[parent].children = frame.child_ids;
                }
                continue;
            };
            let Some(mut block) = records[index].take() else {
                continue;
            };
            if col > 0.0
                && block.block_type != "column-setting"
                && block.width_in_columns.is_some_and(|width| width > col)
            {
                self.warning_facts.push(DiagramWarningFact::new(
                    BLOCK_WIDTH_WARNING_RULE_ID,
                    format!(
                        "Block {} width {} exceeds configured column width {}",
                        block.id,
                        format_js_number(block.width_in_columns.unwrap_or(1.0)),
                        format_js_number(col)
                    ),
                ));
            }
            if let Some(label) = &block.label {
                block.label = Some(sanitize_text(label, config));
            }
            match block.block_type.as_str() {
                "classDef" => {
                    self.add_style_class(&block.id, block.css.as_deref().unwrap_or_default());
                    continue;
                }
                "applyClass" => {
                    self.set_css_class(&block.id, block.style_class.as_deref().unwrap_or_default());
                    continue;
                }
                "applyStyles" => {
                    if let Some(styles) = &block.styles_str {
                        self.add_style_to_node(&block.id, styles);
                    }
                    continue;
                }
                "column-setting" => {
                    if let Some(parent) = parent {
                        self.blocks[parent].columns = block.columns;
                    }
                    continue;
                }
                "edge" => {
                    let count = self.edge_count.entry(block.id.clone()).or_default();
                    *count += 1;
                    block.id = format!("{count}-{}", block.id);
                    self.edges.push(block);
                    continue;
                }
                _ => {}
            }
            if block.label.is_none() {
                block.label = Some(if block.block_type == "composite" {
                    String::new()
                } else {
                    block.id.clone()
                });
            }
            let children = std::mem::take(&mut block.children);
            let existed = self.block_ids.get(&block.id).copied();
            let is_space = block.block_type == "space";
            // Authored space widths have already passed finite, bounded expansion admission.
            let space_width = block.width.unwrap_or(1.0) as usize;
            let block_index = if let Some(index) = existed {
                let existing = &mut self.blocks[index];
                // Mermaid only updates type and an explicit label on repeated declarations.
                if block.block_type != "na" {
                    existing.block_type = block.block_type.clone();
                }
                if let Some(label) = &block.label
                    && label != &block.id
                {
                    existing.label = Some(label.clone());
                }
                index
            } else {
                #[cfg(feature = "presentation")]
                if _presentation_requested && block.block_type == "composite" {
                    block.color_index = Some(self.next_color_index);
                    self.next_color_index += 1;
                }
                self.insert_block(block.clone())
            };
            if is_space {
                for offset in 0..space_width {
                    if offset % 128 == 0 {
                        control.checkpoint()?;
                    }
                    let mut space = block.clone();
                    space.id = format!("{}-{offset}", block.id);
                    let index = self.insert_block(space);
                    if let Some(frame) = stack.last_mut()
                        && frame.parent.is_some()
                    {
                        frame.child_ids.push(index);
                    }
                }
            } else if existed.is_none()
                && let Some(frame) = stack.last_mut()
                && frame.parent.is_some()
            {
                frame.child_ids.push(block_index);
            }
            if !children.is_empty() {
                // Upstream replays a repeated composite into its new declaration object,
                // while the first registered object keeps its own children and columns.
                let parent = existed.is_none().then_some(block_index);
                stack.push(PopulateFrame::new(parent, children, &records));
            }
        }
        Ok(())
    }
}

struct PopulateFrame {
    parent: Option<usize>,
    blocks: std::vec::IntoIter<usize>,
    col: f64,
    child_ids: Vec<usize>,
}

impl PopulateFrame {
    fn new(parent: Option<usize>, blocks: Vec<usize>, records: &[Option<Block>]) -> Self {
        let col = blocks
            .iter()
            .filter_map(|&index| records[index].as_ref())
            .find(|block| block.block_type == "column-setting")
            .and_then(|block| block.columns)
            .unwrap_or(-1.0);
        Self {
            parent,
            blocks: blocks.into_iter(),
            col,
            child_ids: Vec::new(),
        }
    }
}

fn block_to_semantic_node(b: Block) -> BlockNode {
    BlockNode {
        id: b.id,
        label: b.label.unwrap_or_default(),
        block_type: b.block_type,
        children: b.children,
        columns: b.columns,
        width_in_columns: b.width_in_columns,
        width: b.width,
        classes: b.classes,
        styles: b.styles.unwrap_or_default(),
        directions: b.directions.unwrap_or_default(),
    }
}

fn block_to_semantic_edge(b: Block) -> BlockEdge {
    BlockEdge {
        id: b.id,
        start: b.start.unwrap_or_default(),
        end: b.end.unwrap_or_default(),
        arrow_type_end: b.arrow_type_end,
        arrow_type_start: b.arrow_type_start,
        label: b.label.unwrap_or_default(),
        thickness: b.thickness,
        pattern: b.pattern,
    }
}

struct BlockModelOutput {
    semantic: BlockDiagramModel,
    #[cfg(feature = "presentation")]
    visuals: BlockVisuals,
    #[cfg(feature = "presentation")]
    projection_facts: BlockProjectionFacts,
}

fn block_db_to_model(
    db: BlockDb,
    _presentation_requested: bool,
    control: &OperationControl,
) -> OperationControlResult<BlockModelOutput> {
    let root = db.block_ids.get(&db.root_id).copied();
    let mut blocks_flat = Vec::with_capacity(db.blocks.len());
    #[cfg(feature = "presentation")]
    let mut visuals = BlockVisuals::default();
    #[cfg(feature = "presentation")]
    let mut node_presence = Vec::new();
    #[cfg(feature = "presentation")]
    let mut edge_directions = Vec::new();
    for (index, block) in db.blocks.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        #[cfg(feature = "presentation")]
        if _presentation_requested {
            visuals.color_indices.push(block.color_index);
        }
        #[cfg(feature = "presentation")]
        if _presentation_requested {
            node_presence.push(BlockNodeFieldPresence {
                styles: BlockFieldPresence::from_option(&block.styles),
                directions: BlockFieldPresence::from_option(&block.directions),
            });
        }
        blocks_flat.push(block_to_semantic_node(block));
    }
    let mut edges = Vec::with_capacity(db.edges.len());
    for (index, edge) in db.edges.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        #[cfg(feature = "presentation")]
        let mut edge = edge;
        #[cfg(feature = "presentation")]
        if _presentation_requested {
            edge_directions.push(edge.directions.take());
        }
        edges.push(block_to_semantic_edge(edge));
    }
    control.checkpoint()?;
    Ok(BlockModelOutput {
        semantic: BlockDiagramModel {
            blocks_flat,
            edges,
            warning_facts: db.warning_facts,
            class_defs: db.classes,
            root,
        },
        #[cfg(feature = "presentation")]
        visuals,
        #[cfg(feature = "presentation")]
        projection_facts: if _presentation_requested {
            BlockProjectionFacts::Explicit {
                nodes: node_presence,
                edge_directions,
            }
        } else {
            BlockProjectionFacts::Model
        },
    })
}

#[cfg(feature = "presentation")]
fn block_db_to_render_model(
    db: BlockDb,
    control: &OperationControl,
) -> OperationControlResult<BlockDiagramRenderModel> {
    let output = block_db_to_model(db, true, control)?;
    Ok(BlockDiagramRenderModel::from_parts(
        output.semantic,
        output.visuals,
        output.projection_facts,
    ))
}

fn format_js_number(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

/// An admitted document whose database can be consumed by either model projection.
pub struct ParsedBlock(BlockDb);

impl ParsedBlock {
    pub fn into_semantic_model(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<BlockDiagramModel> {
        Ok(block_db_to_model(self.0, false, control)?.semantic)
    }

    #[cfg(feature = "presentation")]
    pub fn into_render_model(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<BlockDiagramRenderModel> {
        block_db_to_render_model(self.0, control)
    }
}

#[derive(Debug)]
pub struct BlockSourceIssue {
    pub message: String,
    pub location: BlockIssueLocation,
}

#[derive(Debug, Clone, Copy)]
pub enum BlockIssueLocation {
    Exact(SourceSpan),
    InsertionPoint(usize),
    Fallback,
}

impl BlockSourceIssue {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            location: BlockIssueLocation::Exact(span),
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            location: BlockIssueLocation::InsertionPoint(offset),
        }
    }

    fn fallback(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: BlockIssueLocation::Fallback,
        }
    }

    fn with_fallback_span(mut self, span: SourceSpan) -> Self {
        if matches!(self.location, BlockIssueLocation::Fallback) {
            self.location = BlockIssueLocation::Exact(span);
        }
        self
    }
}

type ParseResult<T> = Result<T, BlockSourceIssue>;

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum BlockFieldKind {
    Namespace,
    Object,
    Property,
    Class,
    String,
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum BlockFieldSyntax {
    TargetList,
    StyleValue,
    ClassName,
    Direction,
    Payload,
}

#[cfg(feature = "editor-facts")]
pub enum BlockObservation<'a> {
    Directive {
        keyword: &'static str,
        span: SourceSpan,
    },
    Expected {
        kind: BlockFieldSyntax,
        span: SourceSpan,
    },
    TrailingField {
        kind: BlockFieldSyntax,
        start: usize,
        end: usize,
    },
    Node {
        text: &'a BlockSpannedText,
        detail: &'a str,
        kind: BlockFieldKind,
        is_entity: bool,
    },
    ClassDefinition(&'a BlockSpannedText),
    Payload {
        text: &'a BlockSpannedText,
        detail: &'a str,
        kind: BlockFieldKind,
        syntax: BlockFieldSyntax,
    },
    Targets {
        text: &'a BlockSpannedText,
        detail: &'a str,
    },
}

#[cfg(feature = "editor-facts")]
type BlockObserver<'a> = dyn FnMut(BlockObservation<'_>) + 'a;

/// Parses and builds a single preprocessed document under the supplied operation context.
pub fn parse_block_source_controlled(
    code: &str,
    config: &MermaidConfig,
    presentation_requested: bool,
    runtime: &OperationContext,
    control: &OperationControl,
) -> OperationControlResult<ParseResult<ParsedBlock>> {
    parse_block_source(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        None,
        presentation_requested,
        runtime,
        control,
    )
}

/// Reports borrowed observations from the same parsing pass before building the document.
#[cfg(feature = "editor-facts")]
pub fn parse_block_source_with_observer_controlled<'a>(
    code: &str,
    config: &MermaidConfig,
    observe: &'a mut BlockObserver<'a>,
    presentation_requested: bool,
    runtime: &'a OperationContext,
    control: &'a OperationControl,
) -> OperationControlResult<ParseResult<ParsedBlock>> {
    parse_block_source(
        code,
        config,
        Some(observe),
        presentation_requested,
        runtime,
        control,
    )
}

#[cfg_attr(
    not(feature = "editor-facts"),
    allow(
        clippy::needless_lifetimes,
        reason = "The editor observer shares the operation borrow."
    )
)]
fn parse_block_source<'a>(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] observe: Option<&'a mut BlockObserver<'a>>,
    presentation_requested: bool,
    runtime: &'a OperationContext,
    control: &'a OperationControl,
) -> OperationControlResult<ParseResult<ParsedBlock>> {
    #[cfg(any(test, feature = "test-support"))]
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.set(BLOCK_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    control.checkpoint()?;
    let mut parser = Parser::new(
        code,
        #[cfg(feature = "editor-facts")]
        observe,
        presentation_requested,
        runtime,
        control,
    );
    if let Err(issue) = parser.parse_header() {
        return Ok(Err(issue.with_fallback_span(parser.current_token_span())));
    }
    let document = match parser.parse_document(false)? {
        Ok(document) => document,
        Err(issue) => return Ok(Err(issue.with_fallback_span(parser.current_token_span()))),
    };
    if let Some(issue) = document.failure {
        return Ok(Err(issue));
    }

    control.checkpoint()?;
    let mut db = BlockDb::default();
    db.clear();
    db.set_hierarchy(document, config, presentation_requested, control)?;
    control.checkpoint()?;
    Ok(Ok(ParsedBlock(db)))
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
    match type_str.trim().chars().next_back() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('>') => "arrow_point",
        _ => "",
    }
    .to_string()
}

fn edge_str_to_start_data(type_str: &str) -> String {
    match type_str.trim().chars().next() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('<') => "arrow_point",
        _ => "arrow_open",
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

#[derive(Debug, Clone)]
pub struct BlockSpannedText {
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug)]
struct ParsedBlockNode {
    block: Block,
    #[cfg(feature = "editor-facts")]
    id: BlockSpannedText,
    #[cfg(feature = "editor-facts")]
    definition_emitted: bool,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy)]
enum BlockNodeOccurrence {
    Definition,
    RelationEndpoint,
}

fn validate_block_space_width(width: f64, span: SourceSpan) -> ParseResult<()> {
    if width > MAX_BLOCK_SPACE_EXPANSION_ITEMS as f64 {
        return Err(BlockSourceIssue::exact(
            format!(
                "block space width {width} exceeds the materialization limit of \
                 {MAX_BLOCK_SPACE_EXPANSION_ITEMS}"
            ),
            span,
        ));
    }
    Ok(())
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
            // Upstream Mermaid's block lexer ends NODE state on `]` as a fallback, even when the
            // node started with a more specific delimiter like `[/` (see cypress BL21).
            // Accepting `]` here matches that behavior (and yields an unknown typeStr like `[/]`,
            // which upstream maps to the default `na` type).
            ends: &["/]", "\\]", "]"],
        },
        NodeDelims {
            start: "[\\",
            // Same as `[/`: accept `]` as a fallback end delimiter for parity with upstream.
            ends: &["\\]", "/]", "]"],
        },
        NodeDelims {
            start: "[",
            // Upstream ends NODE state on `\]` and `/]` before falling back to `]`.
            ends: &["\\]", "/]", "]"],
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

enum DocumentFrameKind {
    Root,
    IdBlock(Box<Block>),
    AnonymousBlock,
}

struct DocumentFrame {
    kind: DocumentFrameKind,
    children: Vec<usize>,
}

impl DocumentFrame {
    fn root() -> Self {
        Self {
            kind: DocumentFrameKind::Root,
            children: Vec::new(),
        }
    }

    fn id_block(header: Block) -> Self {
        Self {
            kind: DocumentFrameKind::IdBlock(Box::new(header)),
            children: Vec::new(),
        }
    }

    fn anonymous_block() -> Self {
        Self {
            kind: DocumentFrameKind::AnonymousBlock,
            children: Vec::new(),
        }
    }

    fn into_block(self, parser: &mut Parser<'_, '_>) -> Block {
        match self.kind {
            DocumentFrameKind::Root => {
                let mut b = Block::new(parser.generate_id());
                b.block_type = "composite".to_string();
                b.label = Some("".to_string());
                b.children = self.children;
                b
            }
            DocumentFrameKind::IdBlock(header) => {
                let mut header = *header;
                header.block_type = "composite".to_string();
                header.children = self.children;
                header
            }
            DocumentFrameKind::AnonymousBlock => {
                let mut b = Block::new(parser.generate_id());
                b.block_type = "composite".to_string();
                b.label = Some("".to_string());
                b.children = self.children;
                b
            }
        }
    }
}

fn block_document_frame_error() -> BlockSourceIssue {
    BlockSourceIssue::fallback("internal block document frame stack is empty".to_string())
}

fn current_document_frame_mut(frames: &mut [DocumentFrame]) -> ParseResult<&mut DocumentFrame> {
    frames.last_mut().ok_or_else(block_document_frame_error)
}

fn push_document_child(frames: &mut [DocumentFrame], index: usize) -> ParseResult<()> {
    current_document_frame_mut(frames)?.children.push(index);
    Ok(())
}

struct BlockDocument {
    records: Vec<Block>,
    blocks: Vec<usize>,
    failure: Option<BlockSourceIssue>,
}

struct Parser<'input, 'control> {
    input: &'input str,
    control: &'control OperationControl,
    runtime: &'control OperationContext,
    pos: usize,
    gen_counter: i64,
    space_expansion_items: usize,
    records: Vec<Block>,
    #[cfg(feature = "editor-facts")]
    declared_entities: HashSet<String>,
    #[cfg(feature = "editor-facts")]
    observe: Option<&'control mut BlockObserver<'control>>,
    #[cfg(feature = "presentation")]
    presentation_requested: bool,
}

impl<'input, 'control> Parser<'input, 'control> {
    fn new(
        input: &'input str,
        #[cfg(feature = "editor-facts")] observe: Option<&'control mut BlockObserver<'control>>,
        _presentation_requested: bool,
        runtime: &'control OperationContext,
        control: &'control OperationControl,
    ) -> Self {
        Self {
            input,
            control,
            runtime,
            pos: 0,
            gen_counter: 0,
            space_expansion_items: 0,
            records: Vec::new(),
            #[cfg(feature = "editor-facts")]
            declared_entities: HashSet::new(),
            #[cfg(feature = "editor-facts")]
            observe,
            #[cfg(feature = "presentation")]
            presentation_requested: _presentation_requested,
        }
    }

    #[cfg(feature = "editor-facts")]
    fn emit(&mut self, observation: BlockObservation<'_>) {
        if let Some(observe) = self.observe.as_deref_mut() {
            observe(observation);
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

    fn current_token_span(&self) -> SourceSpan {
        if self.pos >= self.input.len() {
            return SourceSpan::new(self.input.len(), self.input.len());
        }

        let len = self.input[self.pos..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or_default();
        SourceSpan::new(self.pos, self.pos + len)
    }

    fn statement_span(&self, start: usize) -> SourceSpan {
        let line_end = self.statement_line_end(start);
        let raw = &self.input[start..line_end];
        SourceSpan::new(start, start + raw.trim_end().len())
    }

    fn statement_line_end(&self, start: usize) -> usize {
        let rest = &self.input[start..];
        start + rest.find(['\n', '\r']).unwrap_or(rest.len())
    }

    fn recover_to_next_statement(&mut self, statement_start: usize) {
        if self.pos <= statement_start {
            self.pos = statement_start;
            self.bump();
        }
        while let Some(ch) = self.bump() {
            if ch == '\n' || ch == '\r' {
                break;
            }
        }
    }

    fn generate_id(&mut self) -> String {
        self.gen_counter += 1;
        let rand = self
            .runtime
            .derive_hex("block.generated-id", self.gen_counter as u64, 12);
        format!("id-{rand}-{}", self.gen_counter)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            let mut last_checkpoint = self.pos;
            while self.peek_char().is_some_and(|c| c.is_whitespace()) {
                self.bump();
                if self.pos.saturating_sub(last_checkpoint) >= 4096 {
                    if self.control.is_cancelled() {
                        return;
                    }
                    last_checkpoint = self.pos;
                }
            }

            if self.starts_with("%%") {
                let mut last_checkpoint = self.pos;
                while let Some(c) = self.bump() {
                    if self.pos.saturating_sub(last_checkpoint) >= 4096 {
                        if self.control.is_cancelled() {
                            return;
                        }
                        last_checkpoint = self.pos;
                    }
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }

            break;
        }
    }

    fn skip_inline_whitespace(&mut self) {
        while self.peek_char().is_some_and(|ch| matches!(ch, ' ' | '\t')) {
            self.bump();
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

    fn parse_header(&mut self) -> ParseResult<()> {
        self.skip_ws_and_comments();
        if self.consume_keyword("block-beta") {
            return Ok(());
        }
        if self.consume_keyword("block") {
            return Ok(());
        }
        if self.is_eof() {
            Err(BlockSourceIssue::insertion_point(
                "expected block header",
                self.pos,
            ))
        } else {
            Err(BlockSourceIssue::exact(
                "expected block header",
                self.statement_span(self.pos),
            ))
        }
    }

    fn parse_document(
        &mut self,
        stop_on_end: bool,
    ) -> OperationControlResult<ParseResult<BlockDocument>> {
        let mut frames = vec![DocumentFrame::root()];
        let mut first_failure = None;

        loop {
            self.control.checkpoint()?;
            self.skip_ws_and_comments();
            self.control.checkpoint()?;
            if self.is_eof() {
                break;
            }

            let statement_start = self.pos;
            let space_expansion_items = self.space_expansion_items;

            let result = self.parse_document_statement(&mut frames, stop_on_end);
            match result {
                Ok(true) => break,
                Ok(false) => continue,
                Err(error) => {
                    self.space_expansion_items = space_expansion_items;
                    let fallback = self.statement_span(statement_start);
                    first_failure.get_or_insert_with(|| error.with_fallback_span(fallback));
                    self.recover_to_next_statement(statement_start);
                }
            }
        }

        if frames.len() > 1 && first_failure.is_none() {
            first_failure = Some(BlockSourceIssue::insertion_point(
                "expected end for nested block",
                self.input.len(),
            ));
        }

        while frames.len() > 1 {
            self.control.checkpoint()?;
            if let Err(error) = self.finish_document_frame(&mut frames) {
                return Ok(Err(error));
            }
        }

        let Some(frame) = frames.pop() else {
            return Ok(Err(block_document_frame_error()));
        };
        self.control.checkpoint()?;
        Ok(Ok(BlockDocument {
            records: std::mem::take(&mut self.records),
            blocks: frame.children,
            failure: first_failure,
        }))
    }

    fn parse_document_statement(
        &mut self,
        frames: &mut Vec<DocumentFrame>,
        stop_on_end: bool,
    ) -> ParseResult<bool> {
        let current_is_root = frames.len() == 1;
        if ((!current_is_root) || stop_on_end) && self.peek_keyword("end") {
            self.consume_keyword("end");
            if current_is_root {
                return Ok(true);
            }
            self.finish_document_frame(frames)?;
            return Ok(false);
        }

        if self.peek_keyword("block:") {
            self.consume_keyword("block:");
            let mut stm = self.parse_node_statement(
                #[cfg(feature = "editor-facts")]
                "block composite",
                #[cfg(feature = "editor-facts")]
                BlockFieldKind::Namespace,
            )?;
            let header = stm
                .drain(..)
                .find(|b| b.block_type != "edge")
                .unwrap_or_else(|| Block::new(self.generate_id()));
            frames.push(DocumentFrame::id_block(header));
            return Ok(false);
        }

        if self.peek_keyword("block-beta") || self.peek_keyword("block") {
            if !(self.consume_keyword("block-beta") || self.consume_keyword("block")) {
                return Err(BlockSourceIssue::fallback("expected block"));
            }
            frames.push(DocumentFrame::anonymous_block());
            return Ok(false);
        }

        if let Some(value_start) = self.columns_value_start() {
            let block = self.parse_columns_statement(value_start)?;
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
            return Ok(false);
        }
        if self.starts_with("space") {
            let block = self.parse_space_statement()?;
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
            return Ok(false);
        }
        if self.peek_keyword("classDef") {
            let block = self.parse_classdef_statement()?;
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
            return Ok(false);
        }
        if self.peek_keyword("class") {
            let block = self.parse_apply_class_statement()?;
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
            return Ok(false);
        }
        if self.peek_keyword("style") {
            let block = self.parse_style_statement()?;
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
            return Ok(false);
        }

        let blocks = self.parse_node_statement(
            #[cfg(feature = "editor-facts")]
            "block node",
            #[cfg(feature = "editor-facts")]
            BlockFieldKind::Object,
        )?;
        for block in blocks {
            let index = self.insert_record(block);
            push_document_child(frames, index)?;
        }
        Ok(false)
    }

    fn finish_document_frame(&mut self, frames: &mut Vec<DocumentFrame>) -> ParseResult<()> {
        let Some(frame) = frames.pop() else {
            return Err(block_document_frame_error());
        };
        let block = frame.into_block(self);
        let index = self.insert_record(block);
        push_document_child(frames, index)
    }

    fn insert_record(&mut self, block: Block) -> usize {
        let index = self.records.len();
        self.records.push(block);
        index
    }

    fn columns_value_start(&self) -> Option<usize> {
        let rest = self.input[self.pos..].strip_prefix("columns")?;
        let whitespace = rest
            .chars()
            .take_while(|&ch| merman_model::text::is_ecmascript_whitespace(ch))
            .map(char::len_utf8)
            .sum::<usize>();
        if whitespace == 0 {
            return None;
        }
        let start = self.pos + "columns".len() + whitespace;
        let value = &self.input[start..];
        (value.starts_with("auto") || value.as_bytes().first().is_some_and(u8::is_ascii_digit))
            .then_some(start)
    }

    fn parse_columns_statement(&mut self, value_start: usize) -> ParseResult<Block> {
        // Jison owns the whole `columns\s+(auto|\d+)` token. Other suffixes leave `columns`
        // available as a node identifier; comments do not join its keyword and value.
        self.pos = value_start;
        let value = if self.starts_with("auto") {
            self.pos += "auto".len();
            -1.0
        } else {
            let (value, _value_fact) = self.parse_int_token()?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &_value_fact,
                    detail: "block columns",
                    kind: BlockFieldKind::Property,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            value
        };

        // Mermaid does not require a unique id for column-setting statements (they are not part of
        // the rendered block list); avoid consuming a generated id so generated composite ids
        // match upstream counters.
        let mut b = Block::new("columns".to_string());
        b.block_type = "column-setting".to_string();
        b.columns = Some(value);
        Ok(b)
    }

    fn parse_space_statement(&mut self) -> ParseResult<Block> {
        self.skip_ws_and_comments();
        let start = self.pos;
        self.pos += "space".len();
        let mut width = 1.0;
        if self.peek_char() == Some(':') {
            self.bump();
            let (value, value_fact) = self.parse_int_token()?;
            self.admit_space_width(value, value_fact.span)?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &value_fact,
                    detail: "block space width",
                    kind: BlockFieldKind::Property,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            width = value;
        } else {
            self.admit_space_width(width, SourceSpan::new(start, self.pos))?;
        }
        let mut b = Block::new(self.generate_id());
        b.block_type = "space".to_string();
        b.label = Some("".to_string());
        b.width = Some(width);
        Ok(b)
    }

    fn admit_space_width(&mut self, width: f64, span: SourceSpan) -> ParseResult<()> {
        validate_block_space_width(width, span)?;
        let total = self.space_expansion_items + width as usize;
        if total > MAX_BLOCK_SPACE_EXPANSION_ITEMS {
            return Err(BlockSourceIssue::exact(
                format!(
                    "total block space width {total} exceeds the materialization limit of \
                     {MAX_BLOCK_SPACE_EXPANSION_ITEMS}"
                ),
                span,
            ));
        }
        self.space_expansion_items = total;
        Ok(())
    }

    fn parse_classdef_statement(&mut self) -> ParseResult<Block> {
        self.skip_ws_and_comments();
        #[cfg(feature = "editor-facts")]
        let directive_start = self.pos;
        if !self.consume_keyword("classDef") {
            return Err(BlockSourceIssue::fallback("expected classDef"));
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            let line_end = self.statement_line_end(directive_start);
            self.emit(BlockObservation::Directive {
                keyword: "classDef",
                span: SourceSpan::new(directive_start, line_end),
            });
        }
        let id = self.parse_classdef_id()?;
        #[cfg(feature = "editor-facts")]
        let rest_start = self.pos;
        let css = self.take_rest_of_line_trimmed();
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::ClassDefinition(&id));
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::TrailingField {
                kind: BlockFieldSyntax::StyleValue,
                start: rest_start,
                end: self.pos,
            });
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Payload {
                text: &css,
                detail: "block class style",
                kind: BlockFieldKind::String,
                syntax: BlockFieldSyntax::Payload,
            });
        }
        let mut b = Block::new(id.text);
        b.block_type = "classDef".to_string();
        b.css = Some(css.text);
        Ok(b)
    }

    fn parse_apply_class_statement(&mut self) -> ParseResult<Block> {
        self.skip_ws_and_comments();
        #[cfg(feature = "editor-facts")]
        let directive_start = self.pos;
        if !self.consume_keyword("class") {
            return Err(BlockSourceIssue::fallback("expected class"));
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            let line_end = self.statement_line_end(directive_start);
            self.emit(BlockObservation::Directive {
                keyword: "class",
                span: SourceSpan::new(directive_start, line_end),
            });
        }
        let ids = self.parse_identifier_list();
        #[cfg(feature = "editor-facts")]
        let ids = ids.inspect_err(|_| {
            if self.observe.is_some() {
                let line_end = self.statement_line_end(directive_start);
                self.emit(BlockObservation::Expected {
                    kind: BlockFieldSyntax::TargetList,
                    span: SourceSpan::new(line_end, line_end),
                });
            }
        });
        let ids = ids?;
        #[cfg(feature = "editor-facts")]
        let rest_start = self.pos;
        let style_class = self.take_rest_of_line_trimmed();
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Targets {
                text: &ids,
                detail: "block class target",
            });
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() && style_class.text.is_empty() {
            self.emit(BlockObservation::TrailingField {
                kind: BlockFieldSyntax::ClassName,
                start: rest_start,
                end: self.pos,
            });
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Payload {
                text: &style_class,
                detail: "block class name",
                kind: BlockFieldKind::Class,
                syntax: BlockFieldSyntax::ClassName,
            });
        }
        let mut b = Block::new(ids.text);
        b.block_type = "applyClass".to_string();
        b.style_class = Some(style_class.text);
        Ok(b)
    }

    fn parse_style_statement(&mut self) -> ParseResult<Block> {
        self.skip_ws_and_comments();
        #[cfg(feature = "editor-facts")]
        let directive_start = self.pos;
        if !self.consume_keyword("style") {
            return Err(BlockSourceIssue::fallback("expected style"));
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            let line_end = self.statement_line_end(directive_start);
            self.emit(BlockObservation::Directive {
                keyword: "style",
                span: SourceSpan::new(directive_start, line_end),
            });
        }
        let ids = self.parse_identifier_list();
        #[cfg(feature = "editor-facts")]
        let ids = ids.inspect_err(|_| {
            if self.observe.is_some() {
                let line_end = self.statement_line_end(directive_start);
                self.emit(BlockObservation::Expected {
                    kind: BlockFieldSyntax::TargetList,
                    span: SourceSpan::new(line_end, line_end),
                });
            }
        });
        let ids = ids?;
        #[cfg(feature = "editor-facts")]
        let rest_start = self.pos;
        let styles_str = self.take_rest_of_line_trimmed();
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Targets {
                text: &ids,
                detail: "block style target",
            });
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::TrailingField {
                kind: BlockFieldSyntax::StyleValue,
                start: rest_start,
                end: self.pos,
            });
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Payload {
                text: &styles_str,
                detail: "block style",
                kind: BlockFieldKind::String,
                syntax: BlockFieldSyntax::Payload,
            });
        }
        let mut b = Block::new(ids.text);
        b.block_type = "applyStyles".to_string();
        b.styles_str = Some(styles_str.text);
        Ok(b)
    }

    fn take_rest_of_line_trimmed(&mut self) -> BlockSpannedText {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
        let raw = &self.input[start..self.pos];
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trailing = raw.trim_end().len();
        BlockSpannedText {
            text: raw[leading.min(trailing)..trailing].to_string(),
            span: SourceSpan::new(start + leading.min(trailing), start + trailing),
        }
    }

    fn parse_node_statement(
        &mut self,
        #[cfg(feature = "editor-facts")] detail: &str,
        #[cfg(feature = "editor-facts")] kind: BlockFieldKind,
    ) -> ParseResult<Vec<Block>> {
        let mut left = self.parse_node(
            #[cfg(feature = "editor-facts")]
            detail,
            #[cfg(feature = "editor-facts")]
            kind,
        )?;
        self.skip_ws_and_comments();
        let link = match self.parse_link() {
            Ok(link) => link,
            Err(error) => {
                #[cfg(feature = "editor-facts")]
                if !left.definition_emitted {
                    #[cfg(feature = "editor-facts")]
                    self.push_block_node_occurrence(
                        &left.id,
                        detail,
                        kind,
                        BlockNodeOccurrence::Definition,
                    );
                }
                return Err(error);
            }
        };
        if let Some((label, edge_marker)) = link {
            #[cfg(feature = "editor-facts")]
            if !left.definition_emitted {
                #[cfg(feature = "editor-facts")]
                self.push_block_node_occurrence(
                    &left.id,
                    detail,
                    kind,
                    BlockNodeOccurrence::RelationEndpoint,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = label.as_ref()
                && self.observe.is_some()
            {
                self.emit(BlockObservation::Payload {
                    text: label,
                    detail: "block edge label",
                    kind: BlockFieldKind::String,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            let mut right = self.parse_node(
                #[cfg(feature = "editor-facts")]
                "block edge endpoint",
                #[cfg(feature = "editor-facts")]
                BlockFieldKind::Object,
            )?;
            let arrow_type_end = edge_str_to_edge_data(&edge_marker);
            let edge_id = format!("{}-{}", left.block.id, right.block.id);
            let edge = Block {
                id: edge_id,
                block_type: "edge".to_string(),
                label: Some(label.map(|label| label.text).unwrap_or_default()),
                children: Vec::new(),
                start: Some(left.block.id.clone()),
                end: Some(right.block.id.clone()),
                arrow_type_end: Some(arrow_type_end),
                arrow_type_start: Some(edge_str_to_start_data(&edge_marker)),
                thickness: if edge_marker.contains("==") {
                    BlockEdgeThickness::Thick
                } else {
                    BlockEdgeThickness::Normal
                },
                pattern: if edge_marker.contains(".-") {
                    BlockEdgePattern::Dotted
                } else {
                    BlockEdgePattern::Solid
                },
                #[cfg(feature = "presentation")]
                directions: self
                    .presentation_requested
                    .then(|| right.block.directions.clone())
                    .flatten(),
                ..Default::default()
            };

            #[cfg(feature = "editor-facts")]
            if !right.definition_emitted {
                #[cfg(feature = "editor-facts")]
                self.push_block_node_occurrence(
                    &right.id,
                    "block edge endpoint",
                    BlockFieldKind::Object,
                    BlockNodeOccurrence::RelationEndpoint,
                );
            }
            left.block.width_in_columns.get_or_insert(1.0);
            right.block.width_in_columns.get_or_insert(1.0);
            return Ok(vec![left.block, edge, right.block]);
        }

        self.skip_ws_and_comments();
        if self.consume_exact(":") {
            let (w, _width_fact) = self.parse_int_token()?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &_width_fact,
                    detail: "block width",
                    kind: BlockFieldKind::Property,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            left.block.width_in_columns = Some(w);
        } else {
            left.block.width_in_columns.get_or_insert(1.0);
        }

        #[cfg(feature = "editor-facts")]
        if !left.definition_emitted {
            #[cfg(feature = "editor-facts")]
            self.push_block_node_occurrence(
                &left.id,
                detail,
                kind,
                BlockNodeOccurrence::Definition,
            );
        }
        Ok(vec![left.block])
    }

    fn parse_link(&mut self) -> ParseResult<Option<(Option<BlockSpannedText>, String)>> {
        self.skip_ws_and_comments();
        if self.is_eof() {
            return Ok(None);
        }

        let snapshot = self.pos;
        let mut saw_partial_start_marker = false;
        if self.try_read_link_start_marker().is_some() {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('"') {
                let label = self.parse_string_literal()?;
                self.skip_ws_and_comments();
                if let Some(edge_marker) = self.try_read_link_full_marker() {
                    return Ok(Some((Some(label), edge_marker.text)));
                }
                self.pos = snapshot;
                return Err(BlockSourceIssue::fallback(
                    "expected edge marker after block edge label",
                ));
            }
            saw_partial_start_marker = true;
            self.pos = snapshot;
        }

        if let Some(edge_marker) = self.try_read_link_full_marker() {
            return Ok(Some((None, edge_marker.text)));
        }
        if saw_partial_start_marker {
            self.pos = snapshot;
            return Err(BlockSourceIssue::fallback(
                "expected block edge label or complete edge marker",
            ));
        }

        Ok(None)
    }

    fn try_read_link_start_marker(&mut self) -> Option<BlockSpannedText> {
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
            return Some(BlockSpannedText {
                text: self.input[start..self.pos].to_string(),
                span: SourceSpan::new(start, self.pos),
            });
        }
        self.pos = start;
        None
    }

    fn try_read_link_full_marker(&mut self) -> Option<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;

        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                break;
            }
            // Mermaid block edge markers can be directly adjacent to node ids
            // (e.g. `a-->b`). Stop once we hit a non-marker character so we don't consume the
            // right-hand node into the marker token.
            if !matches!(c, '-' | '=' | '.' | 'x' | 'o' | '<' | '>' | '~') {
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
        Some(BlockSpannedText {
            text: token.to_string(),
            span: SourceSpan::new(start, self.pos),
        })
    }

    fn parse_node(
        &mut self,
        #[cfg(feature = "editor-facts")] detail: &str,
        #[cfg(feature = "editor-facts")] kind: BlockFieldKind,
    ) -> ParseResult<ParsedBlockNode> {
        self.skip_ws_and_comments();
        let id = self.parse_node_id()?;
        let mut b = Block::new(id.text.clone());
        b.label = None;
        b.block_type = "na".to_string();

        self.skip_ws_and_comments();

        if self.starts_with("<[") {
            #[cfg(feature = "editor-facts")]
            self.push_block_node_occurrence(&id, detail, kind, BlockNodeOccurrence::Definition);
            self.pos += 2;
            self.skip_ws_and_comments();
            let label = self.parse_string_literal()?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &label,
                    detail: "block arrow label",
                    kind: BlockFieldKind::String,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            self.skip_ws_and_comments();
            if !self.consume_exact("]>") {
                return Err(BlockSourceIssue::fallback("expected ]> in block arrow"));
            }
            self.skip_ws_and_comments();
            if !self.consume_exact("(") {
                return Err(BlockSourceIssue::fallback("expected '(' in block arrow"));
            }
            let dirs = self.parse_direction_list()?;
            if !self.consume_exact(")") {
                return Err(BlockSourceIssue::fallback("expected ')' in block arrow"));
            }

            b.label = Some(label.text);
            b.block_type = "block_arrow".to_string();
            b.directions = Some(dirs);
            b.width_in_columns = Some(1.0);
            return Ok(ParsedBlockNode {
                block: b,
                #[cfg(feature = "editor-facts")]
                id,
                #[cfg(feature = "editor-facts")]
                definition_emitted: true,
            });
        }

        if let Some(delims) = node_delims_at_start(&self.input[self.pos..]) {
            #[cfg(feature = "editor-facts")]
            self.push_block_node_occurrence(&id, detail, kind, BlockNodeOccurrence::Definition);
            let start_delim = delims.start;
            self.pos += start_delim.len();
            self.skip_ws_and_comments();
            let label = self.parse_string_literal_or_md()?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &label,
                    detail: "block label",
                    kind: BlockFieldKind::String,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
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
                    return Err(BlockSourceIssue::fallback("unterminated node delimiter"));
                }
            };
            if end_delim.is_empty() {
                return Err(BlockSourceIssue::fallback("unterminated node delimiter"));
            }

            let type_str = format!("{start_delim}{end_delim}");
            b.label = Some(label.text);
            b.block_type = type_str_to_type(&type_str);
            b.width_in_columns = Some(1.0);
            return Ok(ParsedBlockNode {
                block: b,
                #[cfg(feature = "editor-facts")]
                id,
                #[cfg(feature = "editor-facts")]
                definition_emitted: true,
            });
        }

        Ok(ParsedBlockNode {
            block: b,
            #[cfg(feature = "editor-facts")]
            id,
            #[cfg(feature = "editor-facts")]
            definition_emitted: false,
        })
    }

    #[cfg(feature = "editor-facts")]
    fn push_block_node_occurrence(
        &mut self,
        id: &BlockSpannedText,
        detail: &str,
        kind: BlockFieldKind,
        occurrence: BlockNodeOccurrence,
    ) {
        if self.observe.is_none() {
            return;
        }
        let is_entity = match occurrence {
            BlockNodeOccurrence::Definition => {
                self.declared_entities.insert(id.text.clone());
                true
            }
            BlockNodeOccurrence::RelationEndpoint => self.declared_entities.insert(id.text.clone()),
        };
        self.emit(BlockObservation::Node {
            text: id,
            detail,
            kind,
            is_entity,
        });
    }

    fn parse_direction_list(&mut self) -> ParseResult<Vec<String>> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_comments();
            let direction = self.parse_direction()?;
            #[cfg(feature = "editor-facts")]
            if self.observe.is_some() {
                self.emit(BlockObservation::Payload {
                    text: &direction,
                    detail: "block arrow direction",
                    kind: BlockFieldKind::Property,
                    syntax: BlockFieldSyntax::Payload,
                });
            }
            out.push(direction.text);
            self.skip_ws_and_comments();
            if self.consume_exact(",") {
                continue;
            }
            break;
        }
        Ok(out)
    }

    fn parse_direction(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == ',' || c == ')' {
                break;
            }
            self.bump();
        }
        #[cfg(feature = "editor-facts")]
        if self.observe.is_some() {
            self.emit(BlockObservation::Expected {
                kind: BlockFieldSyntax::Direction,
                span: SourceSpan::new(start, self.pos),
            });
        }
        if self.pos == start {
            return Err(BlockSourceIssue::fallback("expected direction"));
        }
        let dir = self.input[start..self.pos].trim().to_string();
        match dir.as_str() {
            "right" | "left" | "x" | "y" | "up" | "down" => Ok(BlockSpannedText {
                text: dir,
                span: SourceSpan::new(start, self.pos),
            }),
            _ => Err(BlockSourceIssue::exact(
                format!("invalid direction: {dir}"),
                SourceSpan::new(start, self.pos),
            )),
        }
    }

    fn parse_node_id(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace()
                || matches!(
                    c,
                    '(' | '[' | '\n' | '-' | '=' | ')' | '{' | '}' | '<' | '>' | ':'
                )
            {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(BlockSourceIssue::fallback("expected node id"));
        }
        let id = BlockSpannedText {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(start, self.pos),
        };
        Ok(id)
    }

    fn parse_classdef_id(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_inline_whitespace();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(BlockSourceIssue::fallback("expected identifier"));
        }
        let span = SourceSpan::new(start, self.pos);
        let text = self.input[start..self.pos].trim().to_string();
        if !text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(BlockSourceIssue::exact(
                format!("invalid classDef identifier: {text}"),
                span,
            ));
        }
        Ok(BlockSpannedText { text, span })
    }

    fn parse_identifier_list(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_inline_whitespace();
        let start = self.pos;
        while self
            .peek_char()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.bump();
        }
        if self.pos == start {
            return Err(BlockSourceIssue::fallback("expected identifier"));
        }
        loop {
            let comma = self.pos;
            if self.peek_char() != Some(',') {
                break;
            }
            self.bump();
            while self.peek_char().is_some_and(char::is_whitespace) {
                self.bump();
            }
            let word_start = self.pos;
            while self
                .peek_char()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                self.bump();
            }
            if self.pos == word_start {
                self.pos = comma;
                break;
            }
        }
        Ok(BlockSpannedText {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(start, self.pos),
        })
    }

    fn parse_int_token(&mut self) -> ParseResult<(f64, BlockSpannedText)> {
        let start = self.pos;
        while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        if self.pos == start {
            return Err(BlockSourceIssue::fallback("expected integer"));
        }
        let text = self.input[start..self.pos].to_string();
        let value = text
            .parse::<f64>()
            .map_err(|e| BlockSourceIssue::fallback(e.to_string()))?;
        Ok((
            value,
            BlockSpannedText {
                text,
                span: SourceSpan::new(start, self.pos),
            },
        ))
    }

    fn parse_string_literal_or_md(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_ws_and_comments();
        if self.starts_with("\"`") {
            self.pos += 2;
            let start = self.pos;
            while self.pos < self.input.len() && !self.input[self.pos..].starts_with("`\"") {
                self.bump();
            }
            if self.pos >= self.input.len() {
                return Err(BlockSourceIssue::fallback("unterminated markdown string"));
            }
            let end = self.pos;
            let inner = self.input[start..end].to_string();
            self.pos += 2;
            return Ok(BlockSpannedText {
                text: inner,
                span: SourceSpan::new(start, end),
            });
        }
        self.parse_string_literal()
    }

    fn parse_string_literal(&mut self) -> ParseResult<BlockSpannedText> {
        self.skip_ws_and_comments();
        if self.peek_char() != Some('"') {
            return Err(BlockSourceIssue::fallback("expected string literal"));
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
            return Err(BlockSourceIssue::fallback("unterminated string literal"));
        }
        let end = self.pos;
        let inner = self.input[start..end].to_string();
        self.bump();
        Ok(BlockSpannedText {
            text: inner,
            span: SourceSpan::new(start, end),
        })
    }
}

#[cfg(all(test, feature = "presentation"))]
mod presentation_data_tests;

#[cfg(test)]
mod tests;
