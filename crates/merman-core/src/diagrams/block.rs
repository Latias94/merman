use crate::diagram::{BLOCK_WIDTH_WARNING_RULE_ID, DiagramWarningFact, legacy_warning_messages};
use crate::sanitize::sanitize_text;
use crate::{
    EditorCompletionCandidate, EditorCompletionVocabulary, EditorExpectedSyntax,
    EditorExpectedSyntaxKind, EditorLexemeKind, EditorLexemeModifier, EditorLexemeModifiers,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, MermaidConfig,
    OperationControl, OperationControlResult, ParseMetadata, Result, SourceSpan,
    editor::EditorLexemeJournal,
};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, hash_map::Entry};

#[cfg(test)]
use std::cell::Cell;

// Block spacing is materialized as one semantic placeholder per occupied column to match Mermaid.
// Bound that expansion before allocation; larger bounded product profiles allow at most this many
// model items, and an unbounded profile must still not turn a tiny source into an effectively
// infinite allocation.
const MAX_BLOCK_SPACE_EXPANSION_ITEMS: i64 = 200_000;

const BLOCK_COMPLETION_DIRECTIONS: &[EditorCompletionCandidate] = &[
    EditorCompletionCandidate::keyword("right", "right"),
    EditorCompletionCandidate::keyword("left", "left"),
    EditorCompletionCandidate::keyword("up", "up"),
    EditorCompletionCandidate::keyword("down", "down"),
    EditorCompletionCandidate::keyword("x", "horizontal"),
    EditorCompletionCandidate::keyword("y", "vertical"),
];

const BLOCK_COMPLETION_VOCABULARY: EditorCompletionVocabulary =
    EditorCompletionVocabulary::new(&[], BLOCK_COMPLETION_DIRECTIONS);

#[cfg(test)]
thread_local! {
    static BLOCK_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_block_syntax_construction_count() {
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn block_syntax_construction_count() -> usize {
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockDiagramRenderModel {
    #[serde(default, rename = "blocksFlat")]
    pub blocks_flat: Vec<BlockNodeRenderModel>,
    #[serde(default)]
    pub edges: Vec<BlockEdgeRenderModel>,
    #[serde(
        default,
        rename = "warningFacts",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub warning_facts: Vec<DiagramWarningFact>,
    #[serde(default, rename = "classes")]
    pub class_defs: IndexMap<String, BlockClassDefRenderModel>,
    #[serde(skip)]
    compat_root_id: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct BlockClassDefRenderModel {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockNodeRenderModel {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub children: Vec<BlockNodeRenderModel>,
    #[serde(default)]
    pub columns: Option<i64>,
    #[serde(default, rename = "widthInColumns")]
    pub width_in_columns: Option<i64>,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(skip)]
    compatibility: BlockNodeCompatibility,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockEdgeRenderModel {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: Option<String>,
    #[serde(default, rename = "arrowTypeStart")]
    pub arrow_type_start: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(skip)]
    compat_directions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Default)]
enum CompatibilityFieldPresence {
    #[default]
    Omitted,
    Present,
}

impl CompatibilityFieldPresence {
    fn from_option<T>(value: &Option<T>) -> Self {
        if value.is_some() {
            Self::Present
        } else {
            Self::Omitted
        }
    }

    fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct BlockNodeCompatibility {
    styles: CompatibilityFieldPresence,
    directions: CompatibilityFieldPresence,
}

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

fn clone_block_shallow(block: &Block) -> Block {
    Block {
        id: block.id.clone(),
        block_type: block.block_type.clone(),
        label: block.label.clone(),
        children: Vec::new(),
        start: block.start.clone(),
        end: block.end.clone(),
        arrow_type_end: block.arrow_type_end.clone(),
        arrow_type_start: block.arrow_type_start.clone(),
        width: block.width,
        columns: block.columns,
        width_in_columns: block.width_in_columns,
        directions: block.directions.clone(),
        classes: block.classes.clone(),
        styles: block.styles.clone(),
        css: block.css.clone(),
        style_class: block.style_class.clone(),
        styles_str: block.styles_str.clone(),
    }
}

fn clone_block_tree_nonrecursive(
    block: &Block,
    control: &OperationControl,
) -> OperationControlResult<Block> {
    let mut completed: HashMap<*const Block, Block> = HashMap::new();
    let mut stack = vec![(block, false)];
    let mut visited_count = 0usize;

    while let Some((block, visited)) = stack.pop() {
        if visited_count.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited_count += 1;
        if visited {
            let children = block
                .children
                .iter()
                .filter_map(|child| completed.remove(&(child as *const Block)))
                .collect();
            let mut cloned = clone_block_shallow(block);
            cloned.children = children;
            completed.insert(block as *const Block, cloned);
        } else {
            stack.push((block, true));
            for child in block.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    Ok(completed
        .remove(&(block as *const Block))
        .unwrap_or_else(|| clone_block_shallow(block)))
}

#[derive(Debug, Default)]
struct BlockDb {
    root_id: String,
    block_database: HashMap<String, Block>,
    block_database_order: Vec<String>,
    blocks: Vec<Block>,
    edges: Vec<Block>,
    edge_count: HashMap<String, i64>,
    classes: IndexMap<String, BlockClassDefRenderModel>,
    warning_facts: Vec<DiagramWarningFact>,
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
        self.warning_facts.clear();

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
        match self.block_database.entry(id.to_string()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                self.block_database_order.push(id.to_string());
                entry.insert(Block::new(id.to_string()))
            }
        }
    }

    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let entry =
            self.classes
                .entry(id.to_string())
                .or_insert_with(|| BlockClassDefRenderModel {
                    id: id.to_string(),
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                });

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

    fn set_hierarchy(
        &mut self,
        blocks: Vec<Block>,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let root_id = self.root_id.clone();
        self.populate_block_database(blocks, &root_id, config, control)?;
        let root_children = self
            .block_database
            .get(&self.root_id)
            .map(|root| root.children.as_slice())
            .unwrap_or_default();
        let mut blocks = Vec::with_capacity(root_children.len());
        for child in root_children {
            blocks.push(clone_block_tree_nonrecursive(child, control)?);
        }
        self.blocks = blocks;
        Ok(())
    }

    fn populate_block_database(
        &mut self,
        blocks: Vec<Block>,
        parent_id: &str,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut stack = vec![PopulateFrame::new(parent_id.to_string(), blocks)];
        let mut visited_count = 0usize;

        while !stack.is_empty() {
            if visited_count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited_count += 1;
            let next = {
                let Some(frame) = stack.last_mut() else {
                    break;
                };
                frame
                    .blocks
                    .next()
                    .map(|block| (block, frame.parent_id.clone(), frame.col))
            };

            let Some((mut block, parent_id, col)) = next else {
                let Some(frame) = stack.pop() else {
                    break;
                };
                let mut child_blocks = Vec::with_capacity(frame.child_ids.len());
                for id in &frame.child_ids {
                    if let Some(block) = self.block_database.get(id) {
                        child_blocks.push(clone_block_tree_nonrecursive(block, control)?);
                    }
                }
                if let Some(parent) = self.block_database.get_mut(&frame.parent_id) {
                    parent.children = child_blocks;
                }
                continue;
            };

            if col > 0
                && block.block_type != "column-setting"
                && block.width_in_columns.is_some_and(|w| w > col)
            {
                self.warning_facts.push(DiagramWarningFact::new(
                    BLOCK_WIDTH_WARNING_RULE_ID,
                    format!(
                        "Block {} width {} exceeds configured column width {}",
                        block.id,
                        block.width_in_columns.unwrap_or(1),
                        col
                    ),
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
                    if let Some(parent) = self.block_database.get_mut(&parent_id) {
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
            let block_id = block.id.clone();

            let existed = self.block_database.contains_key(&block.id);
            if !existed {
                self.insert_block(block.id.clone(), clone_block_shallow(&block));
            } else {
                let mut existing = self
                    .block_database
                    .get(&block.id)
                    .map(|block| clone_block_tree_nonrecursive(block, control))
                    .transpose()?
                    .unwrap_or_else(|| Block::new(block.id.clone()));
                // Mermaid's blockDB only merges a small subset of fields when a block id is
                // encountered multiple times. In particular, later occurrences do *not* override
                // arrow directions (see upstream cypress BL6), so keep the first-seen properties
                // and only patch in "obviously relevant" updates.
                if block.block_type != "na" {
                    existing.block_type = block.block_type.clone();
                }
                if let Some(lbl) = &block.label
                    && lbl != &block.id
                {
                    existing.label = Some(lbl.clone());
                }
                self.insert_block(block.id.clone(), existing);
            }

            if block.block_type == "space" {
                let w = block.width.unwrap_or(1).max(0);
                for j in 0..w {
                    if j % 128 == 0 {
                        control.checkpoint()?;
                    }
                    let id = format!("{}-{}", block.id, j);
                    let mut new_block = clone_block_shallow(&block);
                    new_block.id = id.clone();
                    self.insert_block(id.clone(), new_block);
                    if let Some(frame) = stack.last_mut() {
                        frame.child_ids.push(id);
                    }
                }
                if !parsed_children.is_empty() {
                    stack.push(PopulateFrame::new(block_id, parsed_children));
                }
                continue;
            }

            if !existed && let Some(frame) = stack.last_mut() {
                frame.child_ids.push(block.id.clone());
            }

            if !parsed_children.is_empty() {
                stack.push(PopulateFrame::new(block_id, parsed_children));
            }
        }

        Ok(())
    }

    fn blocks_flat(&self) -> Vec<&Block> {
        self.block_database_order
            .iter()
            .filter_map(|id| self.block_database.get(id))
            .collect()
    }
}

struct PopulateFrame {
    parent_id: String,
    blocks: std::vec::IntoIter<Block>,
    col: i64,
    child_ids: Vec<String>,
}

impl PopulateFrame {
    fn new(parent_id: String, blocks: Vec<Block>) -> Self {
        let col = blocks
            .iter()
            .find(|b| b.block_type == "column-setting")
            .and_then(|b| b.columns)
            .unwrap_or(-1);
        Self {
            parent_id,
            blocks: blocks.into_iter(),
            col,
            child_ids: Vec::new(),
        }
    }
}

fn block_render_node_to_value_shallow(block: &BlockNodeRenderModel, children: Vec<Value>) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(&block.id));
    obj.insert("type".to_string(), json!(&block.block_type));
    obj.insert("label".to_string(), json!(&block.label));
    obj.insert("children".to_string(), Value::Array(children));

    if let Some(v) = block.width {
        obj.insert("width".to_string(), json!(v));
    }
    if let Some(v) = block.columns {
        obj.insert("columns".to_string(), json!(v));
    }
    if let Some(v) = block.width_in_columns {
        obj.insert("widthInColumns".to_string(), json!(v));
    }
    if block.compatibility.directions.is_present() {
        obj.insert("directions".to_string(), json!(&block.directions));
    }
    if !block.classes.is_empty() {
        obj.insert("classes".to_string(), json!(&block.classes));
    }
    if block.compatibility.styles.is_present() {
        obj.insert("styles".to_string(), json!(&block.styles));
    }

    Value::Object(obj)
}

fn block_render_node_to_value(block: &BlockNodeRenderModel) -> Value {
    let mut stack: Vec<(&BlockNodeRenderModel, bool)> = vec![(block, false)];
    let mut completed: HashMap<*const BlockNodeRenderModel, Value> = HashMap::new();

    while let Some((block, visited)) = stack.pop() {
        if visited {
            let children = block
                .children
                .iter()
                .filter_map(|child| completed.remove(&(child as *const BlockNodeRenderModel)))
                .collect();
            completed.insert(
                block as *const BlockNodeRenderModel,
                block_render_node_to_value_shallow(block, children),
            );
        } else {
            stack.push((block, true));
            for child in block.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    completed
        .remove(&(block as *const BlockNodeRenderModel))
        .unwrap_or_else(|| block_render_node_to_value_shallow(block, Vec::new()))
}

fn block_render_edge_to_value(edge: &BlockEdgeRenderModel) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(&edge.id));
    obj.insert("type".to_string(), json!("edge"));
    obj.insert("label".to_string(), json!(&edge.label));
    obj.insert("children".to_string(), json!([]));
    obj.insert("start".to_string(), json!(&edge.start));
    obj.insert("end".to_string(), json!(&edge.end));
    if let Some(value) = &edge.arrow_type_end {
        obj.insert("arrowTypeEnd".to_string(), json!(value));
    }
    if let Some(value) = &edge.arrow_type_start {
        obj.insert("arrowTypeStart".to_string(), json!(value));
    }
    if let Some(directions) = &edge.compat_directions {
        obj.insert("directions".to_string(), json!(directions));
    }
    Value::Object(obj)
}

fn block_compat_classes_to_value(classes: &IndexMap<String, BlockClassDefRenderModel>) -> Value {
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

fn block_to_render_node_shallow(
    b: &Block,
    children: Vec<BlockNodeRenderModel>,
) -> BlockNodeRenderModel {
    BlockNodeRenderModel {
        id: b.id.clone(),
        label: b.label.clone().unwrap_or_default(),
        block_type: b.block_type.clone(),
        children,
        columns: b.columns,
        width_in_columns: b.width_in_columns,
        width: b.width,
        classes: b.classes.clone(),
        styles: b.styles.clone().unwrap_or_default(),
        directions: b.directions.clone().unwrap_or_default(),
        compatibility: BlockNodeCompatibility {
            styles: CompatibilityFieldPresence::from_option(&b.styles),
            directions: CompatibilityFieldPresence::from_option(&b.directions),
        },
    }
}

fn block_to_render_node(b: &Block) -> BlockNodeRenderModel {
    let mut stack: Vec<(&Block, bool)> = vec![(b, false)];
    let mut completed: HashMap<*const Block, BlockNodeRenderModel> = HashMap::new();

    while let Some((block, visited)) = stack.pop() {
        if visited {
            let children = block
                .children
                .iter()
                .filter_map(|child| completed.remove(&(child as *const Block)))
                .collect();
            completed.insert(
                block as *const Block,
                block_to_render_node_shallow(block, children),
            );
        } else {
            stack.push((block, true));
            for child in block.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    completed
        .remove(&(b as *const Block))
        .unwrap_or_else(|| block_to_render_node_shallow(b, Vec::new()))
}

fn block_to_render_edge(b: &Block) -> BlockEdgeRenderModel {
    BlockEdgeRenderModel {
        id: b.id.clone(),
        start: b.start.clone().unwrap_or_default(),
        end: b.end.clone().unwrap_or_default(),
        arrow_type_end: b.arrow_type_end.clone(),
        arrow_type_start: b.arrow_type_start.clone(),
        label: b.label.clone().unwrap_or_default(),
        compat_directions: b.directions.clone(),
    }
}

fn block_db_to_render_model(db: &BlockDb) -> BlockDiagramRenderModel {
    BlockDiagramRenderModel {
        blocks_flat: db
            .blocks_flat()
            .into_iter()
            .map(block_to_render_node)
            .collect(),
        edges: db.edges.iter().map(block_to_render_edge).collect(),
        warning_facts: db.warning_facts.clone(),
        class_defs: db.classes.clone(),
        compat_root_id: db.root_id.clone(),
    }
}

struct BlockSemanticSource {
    db: BlockDb,
    editor_facts: EditorSemanticFacts,
}

struct BlockParseFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
    span: SourceSpan,
}

impl BlockParseFailure {
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        let mut facts = *self.editor_facts;
        facts.mark_recovered_from_parse_error(
            format!("block parser recovered after parse error: {}", self.error),
            Some(self.span),
        );
        (*self.error, facts)
    }
}

fn construct_block_semantic_source(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<BlockSemanticSource, BlockParseFailure>> {
    #[cfg(test)]
    BLOCK_SYNTAX_CONSTRUCTION_COUNT.set(BLOCK_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    control.checkpoint()?;
    let mut parser = Parser::new(code, control);
    if let Err(error) = parser.parse_header() {
        let (error, span) = block_error_with_fallback_span(error, parser.current_token_span());
        return Ok(Err(BlockParseFailure {
            error: Box::new(error),
            editor_facts: Box::new(parser.into_editor_facts()),
            span,
        }));
    }

    let document = match parser.parse_document(false)? {
        Ok(document) => document,
        Err(error) => {
            let (error, span) = block_error_with_fallback_span(error, parser.current_token_span());
            return Ok(Err(BlockParseFailure {
                error: Box::new(error),
                editor_facts: Box::new(parser.into_editor_facts()),
                span,
            }));
        }
    };
    let editor_facts = parser.into_editor_facts();

    if let Some(failure) = document.failure {
        return Ok(Err(BlockParseFailure {
            error: Box::new(failure.error),
            editor_facts: Box::new(editor_facts),
            span: failure.span,
        }));
    }

    control.checkpoint()?;
    let mut db = BlockDb::default();
    db.clear();
    db.set_hierarchy(document.blocks, &meta.effective_config, control)?;

    control.checkpoint()?;
    Ok(Ok(BlockSemanticSource { db, editor_facts }))
}

pub(crate) fn parse_block_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<BlockDiagramRenderModel> {
    let source = construct_block_semantic_source(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
        .map_err(|failure| *failure.error)?;
    Ok(block_db_to_render_model(&source.db))
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

#[derive(Debug, Clone)]
struct BlockSpannedText {
    text: String,
    span: SourceSpan,
}

fn validate_block_space_width(width: i64, span: SourceSpan) -> Result<()> {
    if width > MAX_BLOCK_SPACE_EXPANSION_ITEMS {
        return Err(Error::diagram_parse_exact(
            "block",
            format!(
                "block space width {width} exceeds the materialization limit of \
                 {MAX_BLOCK_SPACE_EXPANSION_ITEMS}"
            ),
            span,
        ));
    }
    Ok(())
}

fn push_block_entity(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_block_outline(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::outline(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_block_payload(
    facts: &mut EditorSemanticFacts,
    text: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_block_id_list(
    facts: &mut EditorSemanticFacts,
    ids: BlockSpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if ids.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::IdList,
        ids.span,
    ));

    let mut cursor = 0usize;
    while cursor <= ids.text.len() {
        let next_comma = ids.text[cursor..]
            .find(',')
            .map(|offset| cursor + offset)
            .unwrap_or(ids.text.len());
        let raw = &ids.text[cursor..next_comma];
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trailing = raw.trim_end().len();
        if leading < trailing {
            push_block_entity(
                facts,
                BlockSpannedText {
                    text: ids.text[cursor + leading..cursor + trailing].to_string(),
                    span: SourceSpan::new(
                        ids.span.start + cursor + leading,
                        ids.span.start + cursor + trailing,
                    ),
                },
                detail,
                kind,
            );
        }

        if next_comma == ids.text.len() {
            break;
        }
        cursor = next_comma + 1;
    }
}

pub(crate) fn parse_block_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_block_semantic_source(code, meta, control)?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |source| {
            let model = block_db_to_render_model(&source.db);
            (
                render_model_to_compat_json(&model, meta),
                source.editor_facts,
            )
        },
        BlockParseFailure::into_error_and_editor_facts,
    );
    control.checkpoint()?;
    Ok(parsed)
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
    children: Vec<Block>,
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

fn block_document_frame_error() -> Error {
    Error::diagram_parse_fallback(
        "block".to_string(),
        "internal block document frame stack is empty".to_string(),
    )
}

fn current_document_frame_mut(frames: &mut [DocumentFrame]) -> Result<&mut DocumentFrame> {
    frames.last_mut().ok_or_else(block_document_frame_error)
}

fn push_document_child(frames: &mut [DocumentFrame], block: Block) -> Result<()> {
    current_document_frame_mut(frames)?.children.push(block);
    Ok(())
}

struct BlockStatementFailure {
    error: Error,
    span: SourceSpan,
}

struct BlockDocument {
    blocks: Vec<Block>,
    failure: Option<BlockStatementFailure>,
}

fn block_error_with_fallback_span(error: Error, fallback: SourceSpan) -> (Error, SourceSpan) {
    match error {
        Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            if let Some(span) = diagnostic.span() {
                (
                    Error::diagram_parse_diagnostic(diagram_type, diagnostic),
                    span,
                )
            } else {
                let message = diagnostic.message().to_string();
                (
                    Error::diagram_parse_exact(diagram_type, message, fallback),
                    fallback,
                )
            }
        }
        other => (other, fallback),
    }
}

struct Parser<'input, 'control> {
    input: &'input str,
    control: &'control OperationControl,
    pos: usize,
    gen_counter: i64,
    editor_facts: EditorSemanticFacts,
    lexemes: EditorLexemeJournal<'input>,
}

impl<'input, 'control> Parser<'input, 'control> {
    fn new(input: &'input str, control: &'control OperationControl) -> Self {
        Self {
            input,
            control,
            pos: 0,
            gen_counter: 0,
            editor_facts: EditorSemanticFacts::new()
                .with_completion_vocabulary(BLOCK_COMPLETION_VOCABULARY),
            lexemes: EditorLexemeJournal::family_parser(input),
        }
    }

    fn into_editor_facts(mut self) -> EditorSemanticFacts {
        self.editor_facts
            .replace_family_lexemes(self.lexemes.finish());
        self.editor_facts
    }

    fn record_lexeme(&mut self, kind: EditorLexemeKind, span: SourceSpan) {
        self.record_lexeme_with_modifiers(kind, EditorLexemeModifiers::NONE, span);
    }

    fn record_lexeme_with_modifier(
        &mut self,
        kind: EditorLexemeKind,
        modifier: EditorLexemeModifier,
        span: SourceSpan,
    ) {
        self.record_lexeme_with_modifiers(
            kind,
            EditorLexemeModifiers::from_modifier(modifier),
            span,
        );
    }

    fn record_lexeme_with_modifiers(
        &mut self,
        kind: EditorLexemeKind,
        modifiers: EditorLexemeModifiers,
        span: SourceSpan,
    ) {
        if span.start < span.end {
            self.lexemes.push(kind, modifiers, span);
        }
    }

    fn record_keyword(&mut self, start: usize, keyword: &str) {
        if let Some(keyword) = keyword.strip_suffix(':') {
            self.record_lexeme(
                EditorLexemeKind::Keyword,
                SourceSpan::new(start, start + keyword.len()),
            );
            self.record_lexeme(
                EditorLexemeKind::Delimiter,
                SourceSpan::new(start + keyword.len(), start + keyword.len() + 1),
            );
        } else {
            self.record_lexeme(
                EditorLexemeKind::Keyword,
                SourceSpan::new(start, start + keyword.len()),
            );
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
        let rest = &self.input[start..];
        let line_len = rest.find(['\n', '\r']).unwrap_or(rest.len());
        let raw = &rest[..line_len];
        SourceSpan::new(start, start + raw.trim_end().len())
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
        let rand =
            crate::runtime::generated_id_hex("block.generated-id", self.gen_counter as u64, 12);
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
        let start = self.pos;
        self.pos += kw.len();
        self.record_keyword(start, kw);
        true
    }

    fn consume_keyword_same_line(&mut self, kw: &str) -> bool {
        // Like `consume_keyword`, but does not skip newlines/comments. This is used for
        // statement-local infix tokens (e.g. `id1 space id2`), where treating the next line's
        // `space` statement as an infix separator would be incorrect.
        while self.peek_char().is_some_and(|c| c == ' ' || c == '\t') {
            self.bump();
        }
        if self.starts_with("%%") {
            return false;
        }
        if !self.starts_with(kw) {
            return false;
        }
        if kw.ends_with(':') {
            let start = self.pos;
            self.pos += kw.len();
            self.record_keyword(start, kw);
            return true;
        }
        let after = &self.input[self.pos + kw.len()..];
        if after
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == ':')
        {
            let start = self.pos;
            self.pos += kw.len();
            self.record_keyword(start, kw);
            return true;
        }
        false
    }

    fn consume_exact(&mut self, s: &str) -> bool {
        self.skip_ws_and_comments();
        if !self.starts_with(s) {
            return false;
        }
        let start = self.pos;
        self.pos += s.len();
        self.record_lexeme(
            EditorLexemeKind::Delimiter,
            SourceSpan::new(start, self.pos),
        );
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
        if self.is_eof() {
            Err(Error::diagram_parse_insertion_point(
                "block",
                "expected block header",
                self.pos,
            ))
        } else {
            Err(Error::diagram_parse_exact(
                "block",
                "expected block header",
                self.statement_span(self.pos),
            ))
        }
    }

    fn parse_document(
        &mut self,
        stop_on_end: bool,
    ) -> OperationControlResult<Result<BlockDocument>> {
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

            let result = self.parse_document_statement(&mut frames, stop_on_end);
            match result {
                Ok(true) => break,
                Ok(false) => continue,
                Err(error) => {
                    let fallback = self.statement_span(statement_start);
                    let (error, span) = block_error_with_fallback_span(error, fallback);
                    if first_failure.is_none() {
                        first_failure = Some(BlockStatementFailure { error, span });
                    }
                    self.recover_to_next_statement(statement_start);
                }
            }
        }

        if frames.len() > 1 && first_failure.is_none() {
            let span = SourceSpan::new(self.input.len(), self.input.len());
            first_failure = Some(BlockStatementFailure {
                error: Error::diagram_parse_insertion_point(
                    "block",
                    "expected end for nested block",
                    self.input.len(),
                ),
                span,
            });
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
            blocks: frame.children,
            failure: first_failure,
        }))
    }

    fn parse_document_statement(
        &mut self,
        frames: &mut Vec<DocumentFrame>,
        stop_on_end: bool,
    ) -> Result<bool> {
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
            let mut stm =
                self.parse_node_statement("block composite", EditorSemanticKind::Namespace)?;
            let header = stm
                .drain(..)
                .find(|b| b.block_type != "edge")
                .unwrap_or_else(|| Block::new(self.generate_id()));
            frames.push(DocumentFrame::id_block(header));
            return Ok(false);
        }

        if self.peek_keyword("block-beta") || self.peek_keyword("block") {
            if !(self.consume_keyword("block-beta") || self.consume_keyword("block")) {
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "expected block".to_string(),
                ));
            }
            frames.push(DocumentFrame::anonymous_block());
            return Ok(false);
        }

        if self.peek_keyword("columns") {
            let block = self.parse_columns_statement()?;
            push_document_child(frames, block)?;
            return Ok(false);
        }
        if self.peek_keyword("space") {
            let block = self.parse_space_statement()?;
            push_document_child(frames, block)?;
            return Ok(false);
        }
        if self.peek_keyword("classDef") {
            let block = self.parse_classdef_statement()?;
            push_document_child(frames, block)?;
            return Ok(false);
        }
        if self.peek_keyword("class") {
            let block = self.parse_apply_class_statement()?;
            push_document_child(frames, block)?;
            return Ok(false);
        }
        if self.peek_keyword("style") {
            let block = self.parse_style_statement()?;
            push_document_child(frames, block)?;
            return Ok(false);
        }

        let mut blocks = self.parse_node_statement("block node", EditorSemanticKind::Object)?;
        current_document_frame_mut(frames)?
            .children
            .append(&mut blocks);
        Ok(false)
    }

    fn finish_document_frame(&mut self, frames: &mut Vec<DocumentFrame>) -> Result<()> {
        let Some(frame) = frames.pop() else {
            return Err(block_document_frame_error());
        };
        let block = frame.into_block(self);
        current_document_frame_mut(frames)?.children.push(block);
        Ok(())
    }

    fn parse_columns_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("columns") {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected columns".to_string(),
            ));
        }
        self.skip_ws_and_comments();
        let value = if self.consume_keyword("auto") {
            -1
        } else {
            let (value, value_fact) = self.parse_int()?;
            push_block_payload(
                &mut self.editor_facts,
                value_fact,
                "block columns",
                EditorSemanticKind::Property,
            );
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

    fn parse_space_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("space") {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected space".to_string(),
            ));
        }
        let mut width = 1;
        self.skip_ws_and_comments();
        if self.consume_exact(":") {
            let (value, value_fact) = self.parse_int()?;
            validate_block_space_width(value, value_fact.span)?;
            push_block_payload(
                &mut self.editor_facts,
                value_fact,
                "block space width",
                EditorSemanticKind::Property,
            );
            width = value;
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
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected classDef".to_string(),
            ));
        }
        self.editor_facts.push_directive_prefix("classDef");
        let id = self.parse_classdef_id()?;
        let css = self.take_rest_of_line_trimmed();
        if id.text == "default" {
            self.record_lexeme(EditorLexemeKind::Keyword, id.span);
        } else {
            self.record_lexeme_with_modifier(
                EditorLexemeKind::Identifier,
                EditorLexemeModifier::Definition,
                id.span,
            );
        }
        self.record_lexeme(EditorLexemeKind::Style, css.span);
        push_block_outline(
            &mut self.editor_facts,
            id.clone(),
            "block class definition",
            EditorSemanticKind::Class,
        );
        push_block_payload(
            &mut self.editor_facts,
            css.clone(),
            "block class style",
            EditorSemanticKind::String,
        );
        let mut b = Block::new(id.text);
        b.block_type = "classDef".to_string();
        b.css = Some(css.text);
        Ok(b)
    }

    fn parse_apply_class_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("class") {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected class".to_string(),
            ));
        }
        self.editor_facts.push_directive_prefix("class");
        let ids = self.parse_identifier_list(EditorLexemeModifier::Reference)?;
        let style_class = self.take_rest_of_line_trimmed();
        self.record_lexeme_with_modifier(
            EditorLexemeKind::Identifier,
            EditorLexemeModifier::Reference,
            style_class.span,
        );
        push_block_id_list(
            &mut self.editor_facts,
            ids.clone(),
            "block class target",
            EditorSemanticKind::Object,
        );
        push_block_payload(
            &mut self.editor_facts,
            style_class.clone(),
            "block class name",
            EditorSemanticKind::Class,
        );
        let mut b = Block::new(ids.text);
        b.block_type = "applyClass".to_string();
        b.style_class = Some(style_class.text);
        Ok(b)
    }

    fn parse_style_statement(&mut self) -> Result<Block> {
        self.skip_ws_and_comments();
        if !self.consume_keyword("style") {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected style".to_string(),
            ));
        }
        self.editor_facts.push_directive_prefix("style");
        let ids = self.parse_identifier_list(EditorLexemeModifier::Reference)?;
        let styles_str = self.take_rest_of_line_trimmed();
        self.record_lexeme(EditorLexemeKind::Style, styles_str.span);
        push_block_id_list(
            &mut self.editor_facts,
            ids.clone(),
            "block style target",
            EditorSemanticKind::Object,
        );
        push_block_payload(
            &mut self.editor_facts,
            styles_str.clone(),
            "block style",
            EditorSemanticKind::String,
        );
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
        detail: &str,
        kind: EditorSemanticKind,
    ) -> Result<Vec<Block>> {
        let mut left = self.parse_node(detail, kind)?;
        if self.consume_keyword_same_line("space") {
            let mut width = 1;
            while self.peek_char().is_some_and(|c| c == ' ' || c == '\t') {
                self.bump();
            }
            if self.peek_char() == Some(':') {
                let delimiter_start = self.pos;
                self.bump();
                self.record_lexeme(
                    EditorLexemeKind::Delimiter,
                    SourceSpan::new(delimiter_start, self.pos),
                );
                while self.peek_char().is_some_and(|c| c == ' ' || c == '\t') {
                    self.bump();
                }
                let start = self.pos;
                while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                    self.bump();
                }
                if self.pos == start {
                    return Err(Error::diagram_parse_fallback(
                        "block".to_string(),
                        "expected integer width after space:".to_string(),
                    ));
                }
                let text = self.input[start..self.pos].to_string();
                width = text.parse::<i64>().map_err(|_| {
                    Error::diagram_parse_exact(
                        "block",
                        "block space width is outside the supported integer range",
                        SourceSpan::new(start, self.pos),
                    )
                })?;
                validate_block_space_width(width, SourceSpan::new(start, self.pos))?;
                self.record_lexeme(EditorLexemeKind::Number, SourceSpan::new(start, self.pos));
                push_block_payload(
                    &mut self.editor_facts,
                    BlockSpannedText {
                        text,
                        span: SourceSpan::new(start, self.pos),
                    },
                    "block space width",
                    EditorSemanticKind::Property,
                );
            }
            let mut space = Block::new(self.generate_id());
            space.block_type = "space".to_string();
            space.label = Some("".to_string());
            space.width = Some(width);

            left.width_in_columns.get_or_insert(1);
            while self.peek_char().is_some_and(|c| c == ' ' || c == '\t') {
                self.bump();
            }
            if self.starts_with("%%") || matches!(self.peek_char(), None | Some('\n' | '\r')) {
                return Ok(vec![left, space]);
            }

            let mut right = self.parse_node("block node", EditorSemanticKind::Object)?;
            right.width_in_columns.get_or_insert(1);
            return Ok(vec![left, space, right]);
        }

        self.skip_ws_and_comments();
        if let Some((label, edge_marker)) = self.parse_link()? {
            let mut right = self.parse_node("block edge endpoint", EditorSemanticKind::Object)?;
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
            let (w, width_fact) = self.parse_int()?;
            push_block_payload(
                &mut self.editor_facts,
                width_fact,
                "block width",
                EditorSemanticKind::Property,
            );
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
        let mut partial_start_marker = None;
        if let Some(start_marker) = self.try_read_link_start_marker() {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('"') {
                self.record_lexeme(EditorLexemeKind::Operator, start_marker.span);
                let label = self.parse_string_literal()?;
                self.skip_ws_and_comments();
                if let Some(edge_marker) = self.try_read_link_full_marker() {
                    self.record_lexeme(EditorLexemeKind::Operator, edge_marker.span);
                    push_block_payload(
                        &mut self.editor_facts,
                        label.clone(),
                        "block edge label",
                        EditorSemanticKind::String,
                    );
                    return Ok(Some((label.text, edge_marker.text)));
                }
                self.pos = snapshot;
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "expected edge marker after block edge label".to_string(),
                ));
            }
            partial_start_marker = Some(start_marker);
            self.pos = snapshot;
        }

        if let Some(edge_marker) = self.try_read_link_full_marker() {
            self.record_lexeme(EditorLexemeKind::Operator, edge_marker.span);
            return Ok(Some(("".to_string(), edge_marker.text)));
        }
        if let Some(start_marker) = partial_start_marker {
            self.record_lexeme(EditorLexemeKind::Operator, start_marker.span);
            self.pos = snapshot;
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected block edge label or complete edge marker".to_string(),
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

    fn parse_node(&mut self, detail: &str, kind: EditorSemanticKind) -> Result<Block> {
        self.skip_ws_and_comments();
        let id = self.parse_node_id()?;
        push_block_entity(&mut self.editor_facts, id.clone(), detail, kind);
        let mut b = Block::new(id.text);
        b.label = None;
        b.block_type = "na".to_string();

        self.skip_ws_and_comments();

        if self.starts_with("<[") {
            let delimiter_start = self.pos;
            self.pos += 2;
            self.record_lexeme(
                EditorLexemeKind::Delimiter,
                SourceSpan::new(delimiter_start, self.pos),
            );
            self.skip_ws_and_comments();
            let label = self.parse_string_literal()?;
            push_block_payload(
                &mut self.editor_facts,
                label.clone(),
                "block arrow label",
                EditorSemanticKind::String,
            );
            self.skip_ws_and_comments();
            if !self.consume_exact("]>") {
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "expected ]> in block arrow".to_string(),
                ));
            }
            self.skip_ws_and_comments();
            if !self.consume_exact("(") {
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "expected '(' in block arrow".to_string(),
                ));
            }
            let dirs = self.parse_direction_list()?;
            if !self.consume_exact(")") {
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "expected ')' in block arrow".to_string(),
                ));
            }

            b.label = Some(label.text);
            b.block_type = "block_arrow".to_string();
            b.directions = Some(dirs);
            b.width_in_columns = Some(1);
            return Ok(b);
        }

        if let Some(delims) = node_delims_at_start(&self.input[self.pos..]) {
            let start_delim = delims.start;
            let delimiter_start = self.pos;
            self.pos += start_delim.len();
            self.record_lexeme(
                EditorLexemeKind::Delimiter,
                SourceSpan::new(delimiter_start, self.pos),
            );
            self.skip_ws_and_comments();
            let label = self.parse_string_literal_or_md()?;
            push_block_payload(
                &mut self.editor_facts,
                label.clone(),
                "block label",
                EditorSemanticKind::String,
            );
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
                    return Err(Error::diagram_parse_fallback(
                        "block".to_string(),
                        "unterminated node delimiter".to_string(),
                    ));
                }
            };
            if end_delim.is_empty() {
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "unterminated node delimiter".to_string(),
                ));
            }

            let type_str = format!("{start_delim}{end_delim}");
            b.label = Some(label.text);
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
            let direction = self.parse_direction()?;
            push_block_payload(
                &mut self.editor_facts,
                direction.clone(),
                "block arrow direction",
                EditorSemanticKind::Property,
            );
            out.push(direction.text);
            self.skip_ws_and_comments();
            if self.consume_exact(",") {
                continue;
            }
            break;
        }
        Ok(out)
    }

    fn parse_direction(&mut self) -> Result<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == ',' || c == ')' {
                break;
            }
            self.bump();
        }
        self.editor_facts
            .push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::DirectionValue,
                SourceSpan::new(start, self.pos),
            ));
        if self.pos == start {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected direction".to_string(),
            ));
        }
        let dir = self.input[start..self.pos].trim().to_string();
        match dir.as_str() {
            "right" | "left" | "x" | "y" | "up" | "down" => {
                self.record_lexeme(EditorLexemeKind::Keyword, SourceSpan::new(start, self.pos));
                Ok(BlockSpannedText {
                    text: dir,
                    span: SourceSpan::new(start, self.pos),
                })
            }
            _ => {
                self.record_lexeme(EditorLexemeKind::Literal, SourceSpan::new(start, self.pos));
                Err(Error::diagram_parse_exact(
                    "block",
                    format!("invalid direction: {dir}"),
                    SourceSpan::new(start, self.pos),
                ))
            }
        }
    }

    fn parse_node_id(&mut self) -> Result<BlockSpannedText> {
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
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected node id".to_string(),
            ));
        }
        let id = BlockSpannedText {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(start, self.pos),
        };
        self.record_lexeme_with_modifier(
            EditorLexemeKind::Identifier,
            EditorLexemeModifier::Definition,
            id.span,
        );
        Ok(id)
    }

    fn parse_classdef_id(&mut self) -> Result<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
        if self.pos == start {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected identifier".to_string(),
            ));
        }
        let span = SourceSpan::new(start, self.pos);
        let text = self.input[start..self.pos].trim().to_string();
        if !text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(Error::diagram_parse_exact(
                "block",
                format!("invalid classDef identifier: {text}"),
                span,
            ));
        }
        Ok(BlockSpannedText { text, span })
    }

    fn parse_identifier_list(
        &mut self,
        modifier: EditorLexemeModifier,
    ) -> Result<BlockSpannedText> {
        self.skip_ws_and_comments();
        let start = self.pos;
        let mut identifier_start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == '\n' || c == '\r' {
                break;
            }
            if c == ',' {
                self.record_lexeme_with_modifier(
                    EditorLexemeKind::Identifier,
                    modifier,
                    SourceSpan::new(identifier_start, self.pos),
                );
                let delimiter_start = self.pos;
                self.bump();
                self.record_lexeme(
                    EditorLexemeKind::Delimiter,
                    SourceSpan::new(delimiter_start, self.pos),
                );
                identifier_start = self.pos;
            } else {
                self.bump();
            }
        }
        if self.pos == start {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected identifier".to_string(),
            ));
        }
        self.record_lexeme_with_modifier(
            EditorLexemeKind::Identifier,
            modifier,
            SourceSpan::new(identifier_start, self.pos),
        );
        Ok(BlockSpannedText {
            text: self.input[start..self.pos].to_string(),
            span: SourceSpan::new(start, self.pos),
        })
    }

    fn parse_int(&mut self) -> Result<(i64, BlockSpannedText)> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        if self.pos == start {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected integer".to_string(),
            ));
        }
        let text = self.input[start..self.pos].to_string();
        let value = text
            .parse::<i64>()
            .map_err(|e| Error::diagram_parse_fallback("block".to_string(), e.to_string()))?;
        self.record_lexeme(EditorLexemeKind::Number, SourceSpan::new(start, self.pos));
        Ok((
            value,
            BlockSpannedText {
                text,
                span: SourceSpan::new(start, self.pos),
            },
        ))
    }

    fn parse_string_literal_or_md(&mut self) -> Result<BlockSpannedText> {
        self.skip_ws_and_comments();
        if self.starts_with("\"`") {
            let opening_start = self.pos;
            self.pos += 2;
            self.record_lexeme(
                EditorLexemeKind::Delimiter,
                SourceSpan::new(opening_start, self.pos),
            );
            let start = self.pos;
            while self.pos < self.input.len() && !self.input[self.pos..].starts_with("`\"") {
                self.bump();
            }
            if self.pos >= self.input.len() {
                self.record_lexeme(EditorLexemeKind::String, SourceSpan::new(start, self.pos));
                return Err(Error::diagram_parse_fallback(
                    "block".to_string(),
                    "unterminated markdown string".to_string(),
                ));
            }
            let end = self.pos;
            let inner = self.input[start..end].to_string();
            self.pos += 2;
            self.record_lexeme(EditorLexemeKind::String, SourceSpan::new(start, end));
            self.record_lexeme(EditorLexemeKind::Delimiter, SourceSpan::new(end, self.pos));
            return Ok(BlockSpannedText {
                text: inner,
                span: SourceSpan::new(start, end),
            });
        }
        self.parse_string_literal()
    }

    fn parse_string_literal(&mut self) -> Result<BlockSpannedText> {
        self.skip_ws_and_comments();
        if self.peek_char() != Some('"') {
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "expected string literal".to_string(),
            ));
        }
        let opening_start = self.pos;
        self.bump();
        self.record_lexeme(
            EditorLexemeKind::Delimiter,
            SourceSpan::new(opening_start, self.pos),
        );
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '"' {
                break;
            }
            self.bump();
        }
        if self.peek_char() != Some('"') {
            self.record_lexeme(EditorLexemeKind::String, SourceSpan::new(start, self.pos));
            return Err(Error::diagram_parse_fallback(
                "block".to_string(),
                "unterminated string literal".to_string(),
            ));
        }
        let end = self.pos;
        let inner = self.input[start..end].to_string();
        self.bump();
        self.record_lexeme(EditorLexemeKind::String, SourceSpan::new(start, end));
        self.record_lexeme(EditorLexemeKind::Delimiter, SourceSpan::new(end, self.pos));
        Ok(BlockSpannedText {
            text: inner,
            span: SourceSpan::new(start, end),
        })
    }
}

pub(crate) fn render_model_to_compat_json(
    model: &BlockDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let warnings = legacy_warning_messages(&model.warning_facts);
    let blocks = model
        .blocks_flat
        .iter()
        .find(|block| block.id == model.compat_root_id)
        .map(|root| {
            root.children
                .iter()
                .map(block_render_node_to_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let edges = model
        .edges
        .iter()
        .map(block_render_edge_to_value)
        .collect::<Vec<_>>();
    let blocks_flat = model
        .blocks_flat
        .iter()
        .map(block_render_node_to_value)
        .collect::<Vec<_>>();
    let classes = block_compat_classes_to_value(&model.class_defs);
    let mut out = Map::new();
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("blocks".to_string(), Value::Array(blocks));
    out.insert("edges".to_string(), Value::Array(edges));
    out.insert("blocksFlat".to_string(), Value::Array(blocks_flat));
    out.insert("classes".to_string(), classes);
    out.insert("warningFacts".to_string(), json!(&model.warning_facts));
    out.insert("warnings".to_string(), json!(warnings));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

pub(crate) fn parse_block(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = construct_block_semantic_source(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
        .map_err(|failure| *failure.error)?;
    let model = block_db_to_render_model(&source.db);
    render_model_to_compat_json(&model, meta)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorLexemeProducerKind, EditorSemanticCompleteness, Engine, ParseDiagnosticSpanKind,
        ParseOptions, RenderSemanticModel,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "block".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        }
    }

    #[test]
    fn block_space_materialization_observes_cancellation() {
        let mut space = Block::new("space".to_string());
        space.block_type = "space".to_string();
        space.label = Some(String::new());
        space.width = Some(1_024);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let mut db = BlockDb::default();
        db.clear();

        assert!(matches!(
            db.set_hierarchy(vec![space], &MermaidConfig::default(), &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(db.block_database.len() > 2);
        assert!(db.block_database.len() < 1_024);
    }

    #[test]
    fn block_space_width_is_bounded_before_materialization() {
        let width = MAX_BLOCK_SPACE_EXPANSION_ITEMS + 1;
        for text in [
            format!("block\nspace:{width}\n"),
            format!("block\nA space:{width} B\n"),
        ] {
            let error = parse_block(&text, &meta()).expect_err("oversized space must be rejected");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected structured Block parse error");
            };
            let start = text.find(&width.to_string()).expect("width in fixture");
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + width.to_string().len()))
            );
            assert!(
                diagnostic
                    .message()
                    .contains("exceeds the materialization limit")
            );
        }
    }

    fn deep_block_chain(depth: usize) -> String {
        let mut input = String::from("block\n");
        for level in 0..depth {
            input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
        }
        input.push_str("leaf[\"leaf\"]\n");
        for _ in 0..depth {
            input.push_str("end\n");
        }
        input
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
    fn block_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = Engine::new();
        let input = "block-beta\n  A[\"first\"] --> B[\"second\"]\n";

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "block");
        match parsed.model() {
            RenderSemanticModel::Block(model) => {
                let a = model
                    .blocks_flat
                    .iter()
                    .find(|block| block.id == "A")
                    .unwrap();
                assert_eq!(a.label, "first");
                assert_eq!(model.edges.len(), 1);
                assert_eq!(model.edges[0].start, "A");
                assert_eq!(model.edges[0].end, "B");
                assert_eq!(
                    model.edges[0].arrow_type_end.as_deref(),
                    Some("arrow_point")
                );
            }
            other => panic!("block render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("block"));
        assert_eq!(parsed_json.model["blocks"][0]["id"], json!("A"));
        assert_eq!(parsed_json.model["edges"][0]["start"], json!("A"));
        assert!(parsed_json.model.get("config").is_some());
    }

    #[test]
    fn block_combined_parse_constructs_once_and_preserves_projections() {
        let text = r#"block-beta
columns 2
A["Alpha"] --"calls"--> B["Beta"]
classDef important fill:#f96,stroke:#333
class A,B important
style B stroke-width:3px
C<["Route"]>(left,down)
"#;
        let meta = meta();

        reset_block_syntax_construction_count();
        let (combined_json, combined_facts) = crate::family::test_support::into_result(
            parse_block_json_and_editor_facts(text, &meta, &OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            block_syntax_construction_count(),
            1,
            "one combined request must construct Block syntax once"
        );

        assert_eq!(combined_json, parse_block(text, &meta).unwrap());
        assert!(!combined_facts.symbols.is_empty());
        let typed = parse_block_model_for_render(text, &meta).unwrap();
        assert_eq!(
            render_model_to_compat_json(&typed, &meta).unwrap(),
            combined_json
        );
        assert_eq!(combined_json["type"], json!("block"));
        assert!(combined_json["config"].is_object());
        assert!(combined_json["warningFacts"].is_array());
        assert!(combined_json["warnings"].is_array());
    }

    #[test]
    fn block_typed_and_json_projections_preserve_semantic_order() {
        let text = "block\ncolumns 1\nA[\"Alpha\"] --> B[\"Beta\"]\n";
        let meta = meta();
        let compat = parse_block(text, &meta).unwrap();
        let typed = parse_block_model_for_render(text, &meta).unwrap();

        let compat_ids = compat["blocksFlat"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|block| block["id"].as_str())
            .collect::<Vec<_>>();
        let typed_ids = typed
            .blocks_flat
            .iter()
            .map(|block| block.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(compat_ids, typed_ids);
        assert_eq!(compat["warningFacts"], json!(typed.warning_facts));
        assert_eq!(compat["edges"].as_array().unwrap().len(), typed.edges.len());
        assert_eq!(compat["edges"][0]["start"], json!(typed.edges[0].start));
        assert_eq!(compat["edges"][0]["end"], json!(typed.edges[0].end));
    }

    #[test]
    fn block_editor_projection_uses_parser_token_spans() {
        let text = r#"block
A["Alpha"] --"calls"--> B["Beta"]
classDef important fill:red
class A,B important
C<["Route"]>(left,down)
"#;
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );

        for (name, detail) in [
            ("Alpha", "block label"),
            ("calls", "block edge label"),
            ("important", "block class definition"),
            ("left", "block arrow direction"),
            ("down", "block arrow direction"),
        ] {
            let start = text.find(name).unwrap();
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                .unwrap_or_else(|| panic!("missing {detail} fact for {name}"));
            assert_eq!(symbol.span, SourceSpan::new(start, start + name.len()));
            assert_eq!(symbol.selection, symbol.span);
        }

        let class_ids_start = text.find("A,B important").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::IdList
                && expected.span == SourceSpan::new(class_ids_start, class_ids_start + 3)
        }));
    }

    #[test]
    fn block_parser_emits_exact_lexemes_for_the_complete_grammar_surface() {
        let text = concat!(
            "block-beta\r\n",
            "  columns 3\r\n",
            "  block:容器[\"组\"]\r\n",
            "    columns auto\r\n",
            "    user((\"用户\")):2\r\n",
            "    route<[\"流\"]>(right, down)\r\n",
            "    user -- \"发送\" --> api[\"接口\"]\r\n",
            "  end\r\n",
            "  classDef hot fill:#f00,stroke:#111\r\n",
            "  class user,api hot\r\n",
            "  style api fill:#0f0\r\n",
            "  space:2\r\n",
        );
        parse_block(text, &meta()).expect("grammar-surface fixture must stay render-compatible");
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert_eq!(facts.lexeme_failure(), None);
        assert!(!facts.lexemes().is_empty());
        assert!(
            facts.lexemes().iter().all(|lexeme| {
                lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            })
        );
        assert!(
            facts
                .lexemes()
                .windows(2)
                .all(|pair| pair[0].span().end <= pair[1].span().start)
        );

        let assert_lexeme = |needle: &str, kind: EditorLexemeKind| {
            let start = text
                .find(needle)
                .unwrap_or_else(|| panic!("missing fixture slice {needle:?}"));
            let span = SourceSpan::new(start, start + needle.len());
            assert!(
                facts
                    .lexemes()
                    .iter()
                    .any(|lexeme| lexeme.kind() == kind && lexeme.span() == span),
                "missing {kind:?} lexeme for {needle:?} at {span:?}"
            );
        };

        assert_lexeme("block-beta", EditorLexemeKind::Keyword);
        assert_lexeme("3", EditorLexemeKind::Number);
        assert_lexeme("容器", EditorLexemeKind::Identifier);
        assert_lexeme("组", EditorLexemeKind::String);
        assert_lexeme("auto", EditorLexemeKind::Keyword);
        assert_lexeme("<[", EditorLexemeKind::Delimiter);
        assert_lexeme("流", EditorLexemeKind::String);
        assert_lexeme("right", EditorLexemeKind::Keyword);
        assert_lexeme(",", EditorLexemeKind::Delimiter);
        assert_lexeme("--", EditorLexemeKind::Operator);
        assert_lexeme("发送", EditorLexemeKind::String);
        assert_lexeme("-->", EditorLexemeKind::Operator);
        assert_lexeme("fill:#f00,stroke:#111", EditorLexemeKind::Style);

        let class_targets = text.find("user,api hot").unwrap();
        for (start, end) in [
            (class_targets, class_targets + "user".len()),
            (
                class_targets + "user,".len(),
                class_targets + "user,api".len(),
            ),
        ] {
            let lexeme = facts
                .lexemes()
                .iter()
                .find(|lexeme| {
                    lexeme.kind() == EditorLexemeKind::Identifier
                        && lexeme.span() == SourceSpan::new(start, end)
                })
                .expect("class target must be emitted as its own identifier");
            assert!(lexeme.modifiers().contains(EditorLexemeModifier::Reference));
        }

        let unicode_span = facts
            .lexemes()
            .iter()
            .find(|lexeme| {
                lexeme.kind() == EditorLexemeKind::String
                    && &text[lexeme.span().start..lexeme.span().end] == "接口"
            })
            .expect("Unicode label must retain caller-source byte coordinates")
            .span();
        assert_eq!(unicode_span.end - unicode_span.start, "接口".len());
    }

    #[test]
    fn block_recovery_keeps_confirmed_prefix_and_later_parser_lexemes() {
        let text = concat!(
            "block-beta\r\n",
            "  A<[\"方向\"]>(right, sideways)\r\n",
            "  后续[\"完成\"]\r\n",
        );
        let invalid_start = text.find("sideways").unwrap();
        let invalid_span = SourceSpan::new(invalid_start, invalid_start + "sideways".len());

        let error =
            parse_block(text, &meta()).expect_err("strict parsing must reject bad direction");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Block parse error");
        };
        assert_eq!(diagnostic.span(), Some(invalid_span));

        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert_eq!(facts.lexeme_failure(), None);
        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
        }));

        for (needle, kind) in [
            ("A", EditorLexemeKind::Identifier),
            ("方向", EditorLexemeKind::String),
            ("right", EditorLexemeKind::Keyword),
            ("sideways", EditorLexemeKind::Literal),
            ("后续", EditorLexemeKind::Identifier),
            ("完成", EditorLexemeKind::String),
        ] {
            let start = text.find(needle).unwrap();
            assert!(facts.lexemes().iter().any(|lexeme| {
                lexeme.kind() == kind
                    && lexeme.span() == SourceSpan::new(start, start + needle.len())
            }));
        }
    }

    #[test]
    fn block_recovery_keeps_an_incomplete_labeled_edge_prefix() {
        let text = "block\nA o-- \"label\"\nB[\"later\"]\n";
        parse_block(text, &meta()).expect_err("strict parsing must reject incomplete labeled edge");
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert_eq!(facts.lexeme_failure(), None);
        for (needle, kind) in [
            ("o--", EditorLexemeKind::Operator),
            ("label", EditorLexemeKind::String),
            ("B", EditorLexemeKind::Identifier),
            ("later", EditorLexemeKind::String),
        ] {
            let start = text.find(needle).unwrap();
            assert!(facts.lexemes().iter().any(|lexeme| {
                lexeme.kind() == kind
                    && lexeme.span() == SourceSpan::new(start, start + needle.len())
            }));
        }
    }

    #[test]
    fn block_malformed_statement_recovers_with_exact_parser_span() {
        let text = "block\nA<[\"Move\"]>(sideways)\nB[\"Later\"]\n";
        let invalid_start = text.find("sideways").unwrap();
        let invalid_span = SourceSpan::new(invalid_start, invalid_start + "sideways".len());

        let error = parse_block(text, &meta()).expect_err("strict parse must reject direction");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Block parse error");
        };
        assert_eq!(diagnostic.span(), Some(invalid_span));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

        reset_block_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(block_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(invalid_span)
                && diagnostic.message.contains("invalid direction")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "B" && symbol.detail.as_deref() == Some("block node")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Later" && symbol.detail.as_deref() == Some("block label")
        }));
    }

    #[test]
    fn block_unclosed_nested_block_reports_eof_insertion_and_partial_facts() {
        let text = "block\nblock:group[\"Group\"]\nA[\"Inside\"]\n";
        let eof = SourceSpan::new(text.len(), text.len());

        let error = parse_block(text, &meta()).expect_err("nested block requires end");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Block parse error");
        };
        assert_eq!(diagnostic.span(), Some(eof));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );

        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(eof) && diagnostic.message.contains("expected end")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "group"
                && symbol.detail.as_deref() == Some("block composite")
                && symbol.kind == EditorSemanticKind::Namespace
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Inside" && symbol.detail.as_deref() == Some("block label")
        }));
    }

    #[test]
    fn block_deep_chain_semantic_and_render_model_use_heap_traversal() {
        const DEPTH: usize = 1200;
        let input = deep_block_chain(DEPTH);

        let model = parse(&input);
        let blocks_flat = model["blocksFlat"].as_array().expect("blocksFlat array");
        assert_eq!(blocks_flat.len(), DEPTH + 2);
        assert_eq!(blocks_flat[0]["id"].as_str(), Some("root"));
        assert_eq!(
            blocks_flat
                .last()
                .and_then(|block| block.get("id"))
                .and_then(Value::as_str),
            Some("leaf")
        );

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        match parsed.model() {
            RenderSemanticModel::Block(model) => {
                assert_eq!(model.blocks_flat.len(), DEPTH + 2);
                assert_eq!(model.blocks_flat[0].id, "root");
                assert_eq!(
                    model.blocks_flat.last().map(|block| block.id.as_str()),
                    Some("leaf")
                );
            }
            other => panic!("block render parse should return typed model, got {other:?}"),
        }
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
    fn generated_block_ids_are_deterministic_for_default_engine() {
        fn generated_ids(model: &Value) -> Vec<String> {
            model["blocksFlat"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|block| block["id"].as_str())
                .filter(|id| id.starts_with("id-"))
                .map(ToString::to_string)
                .collect()
        }

        let first = generated_ids(&parse("block\n  columns 2\n  space\n  space\n"));
        let second = generated_ids(&parse("block\n  columns 2\n  space\n  space\n"));

        assert_eq!(first, second);
        assert!(first.len() >= 2);
        let unique = first.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), first.len());
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
    fn classdef_ids_follow_mermaid_word_identifier_vocabulary() {
        let text = "block\nA\nclassDef foo.bar fill:#f00\nclass A foo.bar\n";
        let error = parse_block(text, &meta()).expect_err("dotted classDef id must be rejected");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Block parse error");
        };
        let start = text.find("foo.bar").expect("fixture classDef id");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "foo.bar".len()))
        );
        assert!(diagnostic.message().contains("invalid classDef identifier"));
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
        let text = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
        let meta = meta();
        let model = parse_block(text, &meta).unwrap();
        let typed = parse_block_model_for_render(text, &meta).unwrap();
        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), model);
        let warnings: Vec<&str> = model["warningFacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.get("message").and_then(|message| message.as_str()))
            .collect();
        assert!(warnings.contains(&"Block B width 2 exceeds configured column width 1"));
        assert_eq!(model["warnings"], json!(warnings));
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
