use super::StateParsedDocument;
use super::document::{StateDocumentScratch, assign_divider_ids};
#[cfg(feature = "presentation")]
use super::render_model::{StateNodePresentation, into_render_model};
use indexmap::IndexMap;
use merman_markup::sanitize::{sanitize_text, sanitize_text_or_array};
#[cfg(feature = "presentation")]
use merman_model::families::state::StateDiagramRenderModel;
use merman_model::families::state::{
    CSS_EDGE_NOTE_EDGE, StateDiagramModel, StateDocumentError, StateDocumentId, StateEdge,
    StateLink, StateLinks, StateNode, StateNote, StateRecord, StateRelation, StateStatement,
    StateStatementNote, StateStatementState, StateStyleClass,
};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{MermaidConfig, OperationCancelled, OperationControl, OperationControlResult};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// Database/model construction failures after State syntax has been accepted.
#[derive(Debug)]
pub enum StateModelError {
    /// The source declares a graph that cannot be constructed.
    Graph(String),
    /// The constructed flat document failed an ownership invariant.
    Document(StateDocumentError),
}

/// Accepted State syntax with divider numbering and database extraction complete.
#[derive(Debug)]
pub struct StateSource(StateDb);

impl StateSource {
    pub(super) fn from_document(
        document: StateParsedDocument,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        let mut document = StateDocumentScratch::from_parsed_parts(document.builder, document.root);
        let mut divider_cnt = 0usize;
        assign_divider_ids(&mut document, &mut divider_cnt, control)?;

        let mut db = StateDb::new();
        db.set_root_doc(document, control)?;
        control.checkpoint()?;
        Ok(Self(db))
    }

    /// Builds the canonical State graph without collecting render-only fields.
    pub fn into_semantic_model_controlled(
        self,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateDiagramModel, StateModelError>> {
        Ok(self
            .0
            .into_canonical_model_controlled(
                config,
                #[cfg(feature = "presentation")]
                false,
                control,
            )?
            .map(|construction| construction.model))
    }

    /// Builds the typed State render model with the effective presentation config.
    #[cfg(feature = "presentation")]
    pub fn into_render_model_controlled(
        self,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateDiagramRenderModel, StateModelError>> {
        match self
            .0
            .into_canonical_model_controlled(config, true, control)?
        {
            Ok(construction) => Ok(Ok(into_render_model(construction, config, control)?)),
            Err(error) => Ok(Err(error)),
        }
    }
}

pub(super) struct StateModelConstruction {
    pub(super) model: StateDiagramModel,
    #[cfg(feature = "presentation")]
    pub(super) presentation: Vec<StateNodePresentation>,
}

#[derive(Debug, Clone, Default)]
struct StyleClass {
    id: String,
    styles: Vec<String>,
    text_styles: Vec<String>,
}

#[derive(Debug, Clone)]
struct RelationEdge {
    id1: String,
    id2: String,
    relation_title: Option<String>,
}

#[derive(Debug, Clone)]
struct StateDbRecord {
    id: String,
    ty: String,
    descriptions: Vec<String>,
    note: Option<StateStatementNote>,
    classes: Vec<String>,
    styles: Vec<String>,
    text_styles: Vec<String>,
    start: Option<bool>,
}

#[derive(Debug, Default)]
struct StateDb {
    document: StateDocumentScratch,
    states: HashMap<String, StateDbRecord>,
    state_order: Vec<String>,
    relations: Vec<RelationEdge>,
    style_classes: IndexMap<String, StyleClass>,
    direction: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    generated_id_cnt: usize,
    links: HashMap<String, Vec<Link>>,
}

impl StateDb {
    fn new() -> Self {
        Self::default()
    }

    fn generate_id(&mut self) -> String {
        self.generated_id_cnt += 1;
        let cnt = self.generated_id_cnt as u64;

        // Mermaid `@11.12.2` uses `Math.random().toString(36).substr(2, 12)` plus a monotonically
        // increasing counter. We keep the same `id-<base36>-<n>` shape, but generate the middle
        // segment deterministically to keep snapshots stable.
        let mut x = cnt ^ 0x9e37_79b9_7f4a_7c15u64;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9u64);
        x ^= x >> 32;
        x = x.wrapping_mul(0x94d0_49bb_1331_11ebu64);
        x ^= x >> 32;

        fn to_base36(mut v: u64) -> String {
            const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
            if v == 0 {
                return "0".to_string();
            }
            let mut buf = [0u8; 32];
            let mut i = buf.len();
            while v > 0 {
                let rem = (v % 36) as usize;
                v /= 36;
                i -= 1;
                buf[i] = DIGITS[rem];
            }
            String::from_utf8_lossy(&buf[i..]).to_string()
        }

        let mut mid = to_base36(x);
        if mid.len() < 12 {
            let pad = "0".repeat(12 - mid.len());
            mid = format!("{pad}{mid}");
        } else if mid.len() > 12 {
            mid = mid[mid.len() - 12..].to_string();
        }

        format!("id-{mid}-{}", self.generated_id_cnt)
    }

    fn set_root_doc(
        &mut self,
        mut document: StateDocumentScratch,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.translate_doc(&mut document, control)?;
        self.extract(document.root_statements(), control)?;
        self.document = document;
        Ok(())
    }

    fn translate_state_ref(parent_id: &str, s: &mut StateStatementState, first: bool) {
        if s.id.trim_matches(is_ecmascript_whitespace) == "[*]" {
            s.id = format!("{}_{}", parent_id, if first { "start" } else { "end" });
            s.start = Some(first);
        } else {
            s.id = s.id.trim_matches(is_ecmascript_whitespace).to_string();
        }
    }

    fn translate_state_concurrency_split(
        &mut self,
        document: &mut StateDocumentScratch,
        doc: StateDocumentId,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let old = std::mem::take(document.get_mut(doc));
        let mut out = Vec::new();
        let mut current = Vec::new();
        let mut saw_divider = false;
        for (index, stmt) in old.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            match stmt {
                StateStatement::State(mut state) if state.ty == "divider" => {
                    saw_divider = true;
                    state.doc = Some(document.push(std::mem::take(&mut current)));
                    out.push(StateStatement::State(state));
                }
                stmt => current.push(stmt),
            }
        }
        if saw_divider && !current.is_empty() {
            let mut divider = StateStatementState::new_typed(self.generate_id(), "divider");
            divider.doc = Some(document.push(std::mem::take(&mut current)));
            out.push(StateStatement::State(divider));
        }
        *document.get_mut(doc) = if saw_divider { out } else { current };
        Ok(())
    }

    fn translate_doc(
        &mut self,
        document: &mut StateDocumentScratch,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let Some(root) = document.root else {
            return Ok(());
        };
        let mut stack = vec![("root".to_string(), root, 0usize)];
        let mut inspected = 0usize;
        while let Some((parent, doc, index)) = stack.last_mut() {
            if inspected.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let Some(stmt) = document.get_mut(*doc).get_mut(*index) else {
                stack.pop();
                continue;
            };
            *index += 1;
            let child = match stmt {
                StateStatement::Relation(relation) => {
                    Self::translate_state_ref(parent, &mut relation.state1, true);
                    Self::translate_state_ref(parent, &mut relation.state2, false);
                    None
                }
                StateStatement::State(state) => {
                    Self::translate_state_ref(parent, state, true);
                    state.doc.map(|doc| (state.id.clone(), doc))
                }
                _ => None,
            };
            if let Some((parent, child)) = child {
                self.translate_state_concurrency_split(document, child, control)?;
                stack.push((parent, child, 0));
            }
            inspected = inspected.saturating_add(1);
        }
        control.checkpoint()
    }

    fn extract(
        &mut self,
        root_doc: &[StateStatement],
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.states.clear();
        self.state_order.clear();
        self.relations.clear();
        self.style_classes.clear();
        self.direction = None;
        self.acc_title = None;
        self.acc_descr = None;
        self.generated_id_cnt = 0;
        self.links.clear();

        for (index, stmt) in root_doc.iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            match stmt {
                StateStatement::State(s) => self.add_state(s),
                StateStatement::Relation(relation) => self.add_relation(
                    &relation.state1,
                    &relation.state2,
                    relation.description.as_deref(),
                ),
                StateStatement::ClassDef { id, classes } => self.add_style_class(id, classes),
                StateStatement::ApplyClass { ids, class_name } => {
                    self.set_css_class(ids, class_name)
                }
                StateStatement::Style { ids, styles } => self.handle_style_def(ids, styles),
                StateStatement::Direction(dir) => self.direction = Some(dir.clone()),
                StateStatement::AccTitle(t) => self.acc_title = Some(t.clone()),
                StateStatement::AccDescr(d) => {
                    // Grammar trims first; commonDb sanitizes before folding newline whitespace.
                    self.acc_descr = Some(d.trim_matches(is_ecmascript_whitespace).to_string());
                }
                StateStatement::Click(c) => self.add_link(&c.id, &c.url, &c.tooltip),
                StateStatement::Noop => {}
            }
        }
        control.checkpoint()
    }

    fn add_link(&mut self, state_id: &str, url: &str, tooltip: &str) {
        // Mermaid's Jison parser creates a fresh idStatement object for every click directive.
        // Group by the normalized state id, but retain declaration order for wrapper replay.
        self.links
            .entry(state_id.to_string())
            .or_default()
            .push(Link {
                url: url.to_string(),
                tooltip: tooltip.to_string(),
            });
    }

    fn ensure_state(&mut self, id: &str) -> &mut StateDbRecord {
        let id = id.trim_matches(is_ecmascript_whitespace).to_string();
        match self.states.entry(id.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                self.state_order.push(id.clone());
                entry.insert(StateDbRecord {
                    id,
                    ty: "default".to_string(),
                    descriptions: Vec::new(),
                    note: None,
                    classes: Vec::new(),
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                    start: None,
                })
            }
        }
    }

    fn add_description(&mut self, id: &str, descr: &str) {
        let clean = descr
            .trim_matches(is_ecmascript_whitespace)
            .trim_start_matches(':')
            .trim_matches(is_ecmascript_whitespace)
            .to_string();
        if clean.is_empty() {
            return;
        }
        self.ensure_state(id).descriptions.push(clean);
    }

    fn add_state(&mut self, state: &StateStatementState) {
        let id = state.id.trim_matches(is_ecmascript_whitespace);
        let st = self.ensure_state(id);
        if st.ty == "default" && state.ty != "default" {
            st.ty = state.ty.clone();
        }
        if st.start.is_none() {
            st.start = state.start;
        }
        if state.note.is_some() {
            st.note = state.note.clone();
        }

        if let Some(d) = state.description.as_deref() {
            self.add_description(id, d);
        }
        for d in &state.descriptions {
            self.add_description(id, d);
        }
        for c in &state.classes {
            self.set_css_class(id, c);
        }
        for s in &state.styles {
            self.set_style(id, s);
        }
        for ts in &state.text_styles {
            self.set_text_style(id, ts);
        }
    }

    fn add_relation(
        &mut self,
        s1: &StateStatementState,
        s2: &StateStatementState,
        title: Option<&str>,
    ) {
        let id1 = s1.id.trim_matches(is_ecmascript_whitespace);
        let id2 = s2.id.trim_matches(is_ecmascript_whitespace);

        self.add_state(s1);
        self.add_state(s2);

        let relation_title = title
            .map(|t| t.trim_matches(is_ecmascript_whitespace).to_string())
            .filter(|s| !s.is_empty());

        // Mermaid `@11.12.2` self-loops are special-cased during rendering/layout (fixed
        // `*-cyclic-special-*` ids). Multiple self-loop statements on the same node effectively
        // overwrite; keep the latest label/title.
        if id1 == id2
            && let Some(existing) = self
                .relations
                .iter_mut()
                .find(|r| r.id1 == id1 && r.id2 == id2)
        {
            existing.relation_title = relation_title;
            return;
        }

        self.relations.push(RelationEdge {
            id1: id1.to_string(),
            id2: id2.to_string(),
            relation_title,
        });
    }

    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let entry = self
            .style_classes
            .entry(id.trim_matches(is_ecmascript_whitespace).to_string())
            .or_insert_with(|| StyleClass {
                id: id.trim_matches(is_ecmascript_whitespace).to_string(),
                styles: Vec::new(),
                text_styles: Vec::new(),
            });

        for attrib in style_attributes.split(',') {
            let fixed = attrib
                .split_once(';')
                .map(|(a, _)| a)
                .unwrap_or(attrib)
                .trim_matches(is_ecmascript_whitespace)
                .to_string();
            if fixed.is_empty() {
                continue;
            }
            if attrib.contains("color") {
                let t1 = fixed.replace("fill", "bgFill");
                let t2 = t1.replace("color", "fill");
                entry.text_styles.push(t2);
            }
            entry.styles.push(fixed);
        }
    }

    fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for id in item_ids.split(',') {
            let trimmed = id.trim_matches(is_ecmascript_whitespace);
            if trimmed.is_empty() {
                continue;
            }
            self.ensure_state(trimmed).classes.push(
                css_class_name
                    .trim_matches(is_ecmascript_whitespace)
                    .to_string(),
            );
        }
    }

    fn set_style(&mut self, item_id: &str, style_text: &str) {
        self.ensure_state(item_id).styles.push(
            style_text
                .trim_matches(is_ecmascript_whitespace)
                .to_string(),
        );
    }

    fn set_text_style(&mut self, item_id: &str, style_text: &str) {
        self.ensure_state(item_id).text_styles.push(
            style_text
                .trim_matches(is_ecmascript_whitespace)
                .to_string(),
        );
    }

    fn handle_style_def(&mut self, ids: &str, styles: &str) {
        let styles_vec: Vec<String> = styles
            .split(',')
            .map(|s| {
                s.replace(';', "")
                    .trim_matches(is_ecmascript_whitespace)
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        for id in ids.split(',') {
            let trimmed = id.trim_matches(is_ecmascript_whitespace);
            if trimmed.is_empty() {
                continue;
            }
            self.ensure_state(trimmed).styles = styles_vec.clone();
        }
    }

    fn into_canonical_model_controlled(
        self,
        config: &MermaidConfig,
        #[cfg(feature = "presentation")] collect_presentation: bool,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateModelConstruction, StateModelError>> {
        let StateGraphConstruction {
            nodes,
            edges,
            #[cfg(feature = "presentation")]
            presentation,
        } = match build_state_graph(
            &self.document,
            &self.states,
            &self.style_classes,
            config,
            #[cfg(feature = "presentation")]
            collect_presentation,
            control,
        ) {
            Ok(data) => data,
            Err(StateBuildError::Cancelled(error)) => return Err(error),
            Err(StateBuildError::Semantic(message)) => {
                return Ok(Err(StateModelError::Graph(message)));
            }
        };
        let Self {
            document,
            states: mut state_records,
            state_order,
            relations: relation_records,
            style_classes: class_records,
            direction,
            acc_title,
            acc_descr,
            links: link_records,
            ..
        } = self;
        let compact = |values: Vec<String>| values.into_boxed_slice().into_vec();
        let mut doc_by_state_id = HashMap::new();
        for (index, stmt) in document.root_statements().iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if let StateStatement::State(state) = stmt
                && let Some(doc) = state.doc
            {
                doc_by_state_id.entry(state.id.as_str()).or_insert(doc);
            }
        }
        let mut states = HashMap::with_capacity(state_records.len());
        for (index, key) in state_order.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let Some(state) = state_records.remove(&key) else {
                continue;
            };
            let doc = doc_by_state_id.remove(state.id.as_str());
            let note = state.note.map(|note| StateNote {
                position: note.position,
                text: note.text,
            });
            states.insert(
                key,
                StateRecord {
                    id: state.id,
                    state_type: state.ty,
                    descriptions: compact(state.descriptions),
                    doc,
                    note,
                    classes: compact(state.classes),
                    styles: compact(state.styles),
                    text_styles: compact(state.text_styles),
                    start: state.start,
                },
            );
        }
        drop(doc_by_state_id);
        let mut relations = Vec::with_capacity(relation_records.len());
        for (index, relation) in relation_records.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            relations.push(StateRelation {
                id1: relation.id1,
                id2: relation.id2,
                relation_title: relation.relation_title,
            });
        }
        let mut style_classes = IndexMap::with_capacity(class_records.len());
        for (index, (key, class)) in class_records.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            style_classes.insert(
                key,
                StateStyleClass {
                    id: class.id,
                    styles: compact(class.styles),
                    text_styles: compact(class.text_styles),
                },
            );
        }
        let mut links = HashMap::with_capacity(link_records.len());
        for (index, (key, declarations)) in link_records.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let declarations = match <[Link; 1]>::try_from(declarations) {
                Ok([link]) => StateLinks::One(StateLink {
                    url: link.url,
                    tooltip: link.tooltip,
                }),
                Err(declarations) => {
                    let mut out = Vec::with_capacity(declarations.len());
                    for (index, link) in declarations.into_iter().enumerate() {
                        if index.is_multiple_of(128) {
                            control.checkpoint()?;
                        }
                        out.push(StateLink {
                            url: link.url,
                            tooltip: link.tooltip,
                        });
                    }
                    StateLinks::Many(out)
                }
            };
            links.insert(key, declarations);
        }

        let document = match document.finish_controlled(control)? {
            Ok(document) => document,
            Err(error) => return Ok(Err(StateModelError::Document(error))),
        };
        control.checkpoint()?;
        Ok(Ok(StateModelConstruction {
            model: StateDiagramModel {
                document,
                direction: direction.unwrap_or_else(|| "TB".to_string()),
                acc_title,
                acc_descr,
                nodes,
                edges,
                relations,
                links,
                states,
                style_classes,
            },
            #[cfg(feature = "presentation")]
            presentation,
        }))
    }
}

#[derive(Debug, Clone)]
struct Link {
    url: String,
    tooltip: String,
}

const DEFAULT_NESTED_DOC_DIR: &str = "TB";

#[cfg(feature = "presentation")]
const DOMID_STATE: &str = "state";
#[cfg(feature = "presentation")]
const DOMID_TYPE_SPACER: &str = "----";

#[cfg(feature = "presentation")]
const NOTE: &str = "note";
#[cfg(feature = "presentation")]
const PARENT: &str = "parent";
const NOTE_ID: &str = "----note";
const PARENT_ID: &str = "----parent";

const SHAPE_STATE: &str = "rect";
const SHAPE_STATE_WITH_DESC: &str = "rectWithTitle";
const SHAPE_START: &str = "stateStart";
const SHAPE_END: &str = "stateEnd";
const SHAPE_DIVIDER: &str = "divider";
const SHAPE_GROUP: &str = "roundedWithTitle";
const SHAPE_NOTE: &str = "note";
const SHAPE_NOTEGROUP: &str = "noteGroup";

const CSS_EDGE: &str = "transition";
const CSS_DIAGRAM_STATE: &str = "statediagram-state";
const CSS_DIAGRAM_NOTE: &str = "statediagram-note";
const CSS_DIAGRAM_CLUSTER: &str = "statediagram-cluster";
const CSS_DIAGRAM_CLUSTER_ALT: &str = "statediagram-cluster-alt";

#[cfg(all(test, feature = "presentation"))]
thread_local! {
    static STATE_DOM_ID_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(all(test, feature = "presentation"))]
fn reset_dom_id_count() {
    STATE_DOM_ID_COUNT.set(0);
}

#[cfg(all(test, feature = "presentation"))]
fn dom_id_count() -> usize {
    STATE_DOM_ID_COUNT.get()
}

#[cfg(feature = "presentation")]
fn state_dom_id(item_id: &str, counter: usize, ty: Option<&str>) -> String {
    #[cfg(test)]
    STATE_DOM_ID_COUNT.set(STATE_DOM_ID_COUNT.get() + 1);
    let type_str = ty
        .filter(|t| !t.is_empty())
        .map(|t| format!("{DOMID_TYPE_SPACER}{t}"))
        .unwrap_or_default();
    format!("{DOMID_STATE}-{item_id}{type_str}-{counter}")
}

#[derive(Debug, Clone)]
struct NodeScratch {
    id: String,
    shape: String,
    label: Value,
    css_classes: String,
    css_styles: Vec<String>,
    node_type: Option<String>,
    dir: Option<String>,
    explicit_dir: Option<bool>,
    is_group: bool,
    parent_id: Option<String>,
    #[cfg(feature = "presentation")]
    color_index: Option<usize>,
}

fn apply_state_descriptions(
    entry: &mut NodeScratch,
    item_id: &str,
    primary_description: Option<&str>,
    additional_descriptions: &[String],
    config: &MermaidConfig,
) {
    // Mermaid's compact form can produce a primary display label plus an additional description:
    // `state "Some long name" as S1: The description`.
    let mut descriptions = primary_description
        .into_iter()
        .chain(additional_descriptions.iter().map(String::as_str))
        .filter(|description| {
            !description
                .trim_matches(is_ecmascript_whitespace)
                .is_empty()
        })
        .peekable();
    if descriptions.peek().is_none() {
        return;
    }

    let base_label = sanitize_text(item_id, config);
    for description in descriptions {
        match &mut entry.label {
            Value::Array(labels) => {
                entry.shape = SHAPE_STATE_WITH_DESC.to_string();
                labels.push(Value::String(description.to_string()));
            }
            Value::String(label) if !label.is_empty() => {
                entry.shape = SHAPE_STATE_WITH_DESC.to_string();
                if *label == base_label {
                    entry.label = Value::Array(vec![Value::String(description.to_string())]);
                } else {
                    entry.label = Value::Array(vec![
                        Value::String(label.clone()),
                        Value::String(description.to_string()),
                    ]);
                }
            }
            _ => {
                entry.shape = SHAPE_STATE.to_string();
                entry.label = Value::String(description.to_string());
            }
        }
    }

    entry.label = sanitize_text_or_array(&entry.label, config);

    if entry.shape == SHAPE_STATE_WITH_DESC
        && let Some(labels) = entry.label.as_array()
        && labels.len() == 1
    {
        entry.shape = if entry.node_type.as_deref() == Some("group") {
            SHAPE_GROUP.to_string()
        } else {
            SHAPE_STATE.to_string()
        };
    }
}

fn get_dir_for_doc(doc: &[StateStatement], default_dir: &str) -> String {
    let mut dir = default_dir.to_string();
    for stmt in doc {
        if let StateStatement::Direction(d) = stmt {
            dir = d.clone();
        }
    }
    dir
}

fn compiled_styles(css_classes: &str, classes: &IndexMap<String, StyleClass>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for class_name in css_classes.split_whitespace() {
        if let Some(c) = classes.get(class_name) {
            out.extend(c.styles.iter().cloned());
        }
    }
    out
}

struct ConstructedStateNode {
    model: StateNode,
    #[cfg(feature = "presentation")]
    presentation: Option<StateNodePresentation>,
}

struct StateGraphConstruction {
    nodes: Vec<StateNode>,
    edges: Vec<StateEdge>,
    #[cfg(feature = "presentation")]
    presentation: Vec<StateNodePresentation>,
}

fn upsert_node_typed(
    nodes: &mut Vec<StateNode>,
    #[cfg(feature = "presentation")] presentation: &mut Vec<StateNodePresentation>,
    index: &mut HashMap<String, usize>,
    node: ConstructedStateNode,
) {
    match index.entry(node.model.id.clone()) {
        Entry::Occupied(o) => {
            if let Some(dst) = nodes.get_mut(*o.get()) {
                *dst = node.model;
                #[cfg(feature = "presentation")]
                if let Some(fields) = node.presentation {
                    presentation[*o.get()] = fields;
                }
            }
        }
        Entry::Vacant(v) => {
            v.insert(nodes.len());
            nodes.push(node.model);
            #[cfg(feature = "presentation")]
            if let Some(fields) = node.presentation {
                presentation.push(fields);
            }
        }
    }
}

fn insert_generated_node_typed(
    nodes: &mut Vec<StateNode>,
    #[cfg(feature = "presentation")] presentation: &mut Vec<StateNodePresentation>,
    index: &mut HashMap<String, usize>,
    authored_states: &HashMap<String, StateDbRecord>,
    node: ConstructedStateNode,
    generated_kind: &str,
    owner_id: &str,
) -> std::result::Result<(), String> {
    if authored_states.contains_key(&node.model.id) {
        return Err(format!(
            "Authored state id `{}` conflicts with generated {generated_kind} id for state `{owner_id}`",
            node.model.id
        ));
    }

    match index.entry(node.model.id.clone()) {
        Entry::Occupied(_) => Err(format!(
            "Generated {generated_kind} id `{}` for state `{owner_id}` conflicts with an existing render node",
            node.model.id
        )),
        Entry::Vacant(entry) => {
            entry.insert(nodes.len());
            nodes.push(node.model);
            #[cfg(feature = "presentation")]
            if let Some(fields) = node.presentation {
                presentation.push(fields);
            }
            Ok(())
        }
    }
}

#[cfg(feature = "presentation")]
fn value_as_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

#[derive(Debug)]
enum StateBuildError {
    Cancelled(OperationCancelled),
    Semantic(String),
}

impl From<String> for StateBuildError {
    fn from(message: String) -> Self {
        Self::Semantic(message)
    }
}

impl From<OperationCancelled> for StateBuildError {
    fn from(error: OperationCancelled) -> Self {
        Self::Cancelled(error)
    }
}

fn build_state_graph(
    document: &StateDocumentScratch,
    states: &HashMap<String, StateDbRecord>,
    classes: &IndexMap<String, StyleClass>,
    config: &MermaidConfig,
    #[cfg(feature = "presentation")] collect_presentation: bool,
    control: &OperationControl,
) -> std::result::Result<StateGraphConstruction, StateBuildError> {
    let mut nodes: Vec<StateNode> = Vec::new();
    let mut edges: Vec<StateEdge> = Vec::new();
    #[cfg(feature = "presentation")]
    let mut presentation = Vec::new();
    let mut node_index: HashMap<String, usize> = HashMap::new();

    let mut node_db: HashMap<String, NodeScratch> = HashMap::new();
    let mut graph_item_count: usize = 0;

    struct StateGraphContext<'a> {
        document: &'a StateDocumentScratch,
        control: &'a OperationControl,
        states: &'a HashMap<String, StateDbRecord>,
        classes: &'a IndexMap<String, StyleClass>,
        config: &'a MermaidConfig,
        #[cfg(feature = "presentation")]
        collect_presentation: bool,
        nodes: &'a mut Vec<StateNode>,
        #[cfg(feature = "presentation")]
        presentation: &'a mut Vec<StateNodePresentation>,
        node_index: &'a mut HashMap<String, usize>,
        edges: &'a mut Vec<StateEdge>,
        node_db: &'a mut HashMap<String, NodeScratch>,
        graph_item_count: &'a mut usize,
        #[cfg(feature = "presentation")]
        next_color_index: usize,
        #[cfg(feature = "presentation")]
        container_color_index: HashMap<String, Option<usize>>,
    }

    fn setup_doc(
        ctx: &mut StateGraphContext<'_>,
        parent: Option<&StateStatementState>,
        doc: StateDocumentId,
        alt_flag: bool,
    ) -> std::result::Result<(), StateBuildError> {
        struct DocFrame<'a> {
            parent: Option<&'a StateStatementState>,
            doc: StateDocumentId,
            index: usize,
            alt_flag: bool,
        }

        let mut stack = vec![DocFrame {
            parent,
            doc,
            index: 0,
            alt_flag,
        }];

        let mut inspected = 0usize;
        while let Some(frame) = stack.last_mut() {
            if inspected.is_multiple_of(128) {
                ctx.control.checkpoint()?;
            }
            inspected = inspected.saturating_add(1);
            let Some(item) = ctx
                .document
                .get(frame.doc)
                .expect("scratch document exists")
                .get(frame.index)
            else {
                stack.pop();
                continue;
            };
            frame.index += 1;
            let parent = frame.parent;
            let alt_flag = frame.alt_flag;

            match item {
                StateStatement::State(s) => {
                    data_fetcher(ctx, parent, s, alt_flag)?;
                    if let Some(doc) = s.doc {
                        stack.push(DocFrame {
                            parent: Some(s),
                            doc,
                            index: 0,
                            alt_flag: !alt_flag,
                        });
                    }
                }
                StateStatement::Relation(relation) => {
                    let relation = relation.as_ref();
                    data_fetcher(ctx, parent, &relation.state1, alt_flag)?;
                    data_fetcher(ctx, parent, &relation.state2, alt_flag)?;

                    let edge_label_raw = relation.description.clone().unwrap_or_default();
                    let edge_label = sanitize_text(&edge_label_raw, ctx.config);
                    ctx.edges.push(StateEdge {
                        id: format!("edge{}", *ctx.graph_item_count),
                        start: relation.state1.id.clone(),
                        end: relation.state2.id.clone(),
                        classes: CSS_EDGE.to_string(),
                        label: edge_label,
                    });
                    *ctx.graph_item_count += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[cfg(feature = "presentation")]
    fn config_state_number(config: &MermaidConfig, key: &str) -> Option<f64> {
        config
            .as_value()
            .get("state")
            .and_then(Value::as_object)
            .and_then(|state| state.get(key))
            .and_then(value_as_f64)
    }

    #[cfg(feature = "presentation")]
    fn color_slot_for(
        next_color_index: &mut usize,
        container_color_index: &mut HashMap<String, Option<usize>>,
        shape: &str,
        item_id: &str,
        parent: Option<&StateStatementState>,
        user_styled: bool,
    ) -> Option<usize> {
        if shape == SHAPE_DIVIDER
            && let Some(parent_id) = parent.map(|parent| parent.id.as_str())
            && let Some(inherited) = container_color_index.get(parent_id).copied()
        {
            container_color_index.insert(item_id.to_string(), inherited);
            return inherited;
        }

        let slot = *next_color_index;
        *next_color_index += 1;
        let effective = (!user_styled).then_some(slot);
        container_color_index.insert(item_id.to_string(), effective);
        effective
    }

    fn data_fetcher(
        ctx: &mut StateGraphContext<'_>,
        parent: Option<&StateStatementState>,
        parsed_item: &StateStatementState,
        alt_flag: bool,
    ) -> std::result::Result<(), StateBuildError> {
        let item_id = parsed_item.id.clone();
        if item_id == "root" || item_id.is_empty() {
            return Ok(());
        }

        let states = ctx.states;
        let classes = ctx.classes;
        let config = ctx.config;

        let db_state = states.get(&item_id);
        let class_str = db_state.map(|s| s.classes.join(" ")).unwrap_or_default();
        let styles = db_state.map(|s| s.styles.clone()).unwrap_or_default();
        #[cfg(feature = "presentation")]
        let user_styled =
            !class_str.trim_matches(is_ecmascript_whitespace).is_empty() || !styles.is_empty();
        #[cfg(feature = "presentation")]
        let (wrapping_width, min_width) = if ctx.collect_presentation {
            (
                config_state_number(config, "wrappingWidth"),
                config_state_number(config, "minNodeWidth"),
            )
        } else {
            (None, None)
        };
        #[cfg(feature = "presentation")]
        let needs_color_slot = ctx.collect_presentation
            && parsed_item.doc.is_some()
            && !ctx.container_color_index.contains_key(&item_id);

        let entry = ctx.node_db.entry(item_id.clone()).or_insert_with(|| {
            let mut css_classes = String::new();
            if !class_str.trim_matches(is_ecmascript_whitespace).is_empty() {
                css_classes.push_str(class_str.trim_matches(is_ecmascript_whitespace));
                css_classes.push(' ');
            }
            css_classes.push_str(CSS_DIAGRAM_STATE);

            let mut shape = SHAPE_STATE.to_string();
            if parsed_item.start == Some(true) {
                shape = SHAPE_START.to_string();
            } else if parsed_item.start == Some(false) {
                shape = SHAPE_END.to_string();
            }
            if parsed_item.ty != "default" {
                shape = parsed_item.ty.clone();
            }

            NodeScratch {
                id: item_id.clone(),
                shape,
                label: json!(sanitize_text(&item_id, config)),
                css_classes,
                css_styles: styles.clone(),
                node_type: None,
                dir: None,
                explicit_dir: None,
                is_group: false,
                parent_id: None,
                #[cfg(feature = "presentation")]
                color_index: None,
            }
        });

        apply_state_descriptions(
            entry,
            &item_id,
            parsed_item.description.as_deref(),
            &parsed_item.descriptions,
            config,
        );

        // Group handling (composite states)
        if entry.node_type.is_none()
            && let Some(doc_id) = parsed_item.doc
        {
            let doc = ctx.document.get(doc_id).expect("scratch document exists");
            entry.node_type = Some("group".to_string());
            entry.is_group = true;
            let dir = get_dir_for_doc(doc, DEFAULT_NESTED_DOC_DIR);
            entry.dir = Some(dir);
            entry.explicit_dir = Some(
                doc.iter()
                    .any(|stmt| matches!(stmt, StateStatement::Direction(_))),
            );
            entry.shape = if parsed_item.ty == "divider" {
                SHAPE_DIVIDER.to_string()
            } else {
                SHAPE_GROUP.to_string()
            };
            let mut css = entry.css_classes.clone();
            css.push(' ');
            css.push_str(CSS_DIAGRAM_CLUSTER);
            if alt_flag {
                css.push(' ');
                css.push_str(CSS_DIAGRAM_CLUSTER_ALT);
            }
            entry.css_classes = css;
        }

        if let Some(p) = parent
            && p.id != "root"
        {
            entry.parent_id = Some(p.id.clone());
        }

        #[cfg(feature = "presentation")]
        let entry = if needs_color_slot {
            let shape = entry.shape.clone();
            let _ = entry;
            let color_index = color_slot_for(
                &mut ctx.next_color_index,
                &mut ctx.container_color_index,
                &shape,
                &item_id,
                parent,
                user_styled,
            );
            let entry = ctx
                .node_db
                .get_mut(&item_id)
                .expect("state node inserted before color assignment");
            entry.color_index = color_index;
            entry
        } else {
            entry
        };

        let mut label = Some(entry.label.clone());
        if entry.shape == SHAPE_DIVIDER {
            label = Some(Value::String(String::new()));
        }

        let mut model = StateNode {
            id: entry.id.clone(),
            label,
            description: None,
            is_group: entry.is_group,
            node_type: entry.node_type.clone(),
            parent_id: entry.parent_id.clone(),
            css_classes: entry.css_classes.clone(),
            css_compiled_styles: Vec::new(),
            css_styles: entry.css_styles.clone(),
            dir: entry.dir.clone(),
            explicit_dir: entry.explicit_dir,
            shape: entry.shape.clone(),
            position: None,
        };
        model.css_compiled_styles = compiled_styles(&model.css_classes, classes);
        let node = ConstructedStateNode {
            model,
            #[cfg(feature = "presentation")]
            presentation: ctx.collect_presentation.then(|| StateNodePresentation {
                dom_id: state_dom_id(&item_id, *ctx.graph_item_count, None),
                padding: Some(8.0),
                rx: Some(10.0),
                ry: Some(10.0),
                color_index: entry.color_index,
                wrapping_width,
                min_width: (!entry.is_group).then_some(min_width).flatten(),
            }),
        };

        // Notes create a note node + note group + note edge
        if let Some(mut n) = parsed_item.note.clone() {
            #[cfg(feature = "presentation")]
            let flowchart_padding = ctx
                .collect_presentation
                .then(|| {
                    config
                        .as_value()
                        .as_object()
                        .and_then(|o| o.get("flowchart"))
                        .and_then(|v| v.as_object())
                        .and_then(|o| o.get("padding"))
                        .and_then(value_as_f64)
                })
                .flatten();

            n.text = sanitize_text(&n.text, config);

            let note_id = format!("{item_id}{NOTE_ID}-{}", *ctx.graph_item_count);
            let parent_base_id = format!("{item_id}{PARENT_ID}");
            let parent_node_id = if ctx.node_index.contains_key(&parent_base_id)
                || ctx.states.contains_key(&parent_base_id)
            {
                format!("{parent_base_id}-{}", *ctx.graph_item_count)
            } else {
                parent_base_id
            };
            let mut group_model = StateNode {
                id: parent_node_id.clone(),
                label: Some(Value::String(n.text.clone())),
                description: None,
                is_group: true,
                node_type: Some("group".to_string()),
                parent_id: None,
                css_classes: node.model.css_classes.clone(),
                css_compiled_styles: Vec::new(),
                css_styles: Vec::new(),
                dir: None,
                explicit_dir: None,
                shape: SHAPE_NOTEGROUP.to_string(),
                position: n.position.clone(),
            };
            group_model.css_compiled_styles = compiled_styles(&group_model.css_classes, classes);
            let group_node = ConstructedStateNode {
                model: group_model,
                #[cfg(feature = "presentation")]
                presentation: ctx.collect_presentation.then(|| StateNodePresentation {
                    dom_id: state_dom_id(&item_id, *ctx.graph_item_count, Some(PARENT)),
                    padding: Some(16.0),
                    rx: None,
                    ry: None,
                    color_index: None,
                    wrapping_width,
                    min_width: None,
                }),
            };

            let mut note_model = StateNode {
                id: note_id.clone(),
                label: Some(Value::String(n.text.clone())),
                description: None,
                // Mermaid's generated note node is always a leaf, even when the
                // annotated state is a composite.
                is_group: false,
                node_type: Some("node".to_string()),
                parent_id: Some(parent_node_id.clone()),
                css_classes: CSS_DIAGRAM_NOTE.to_string(),
                css_compiled_styles: Vec::new(),
                css_styles: Vec::new(),
                dir: None,
                explicit_dir: None,
                shape: SHAPE_NOTE.to_string(),
                position: n.position.clone(),
            };
            note_model.css_compiled_styles = compiled_styles(&note_model.css_classes, classes);
            let note_node = ConstructedStateNode {
                model: note_model,
                #[cfg(feature = "presentation")]
                presentation: ctx.collect_presentation.then(|| StateNodePresentation {
                    dom_id: state_dom_id(&item_id, *ctx.graph_item_count, Some(NOTE)),
                    padding: flowchart_padding,
                    rx: None,
                    ry: None,
                    color_index: None,
                    wrapping_width,
                    min_width,
                }),
            };

            insert_generated_node_typed(
                ctx.nodes,
                #[cfg(feature = "presentation")]
                ctx.presentation,
                ctx.node_index,
                ctx.states,
                group_node,
                "note group",
                &item_id,
            )?;
            insert_generated_node_typed(
                ctx.nodes,
                #[cfg(feature = "presentation")]
                ctx.presentation,
                ctx.node_index,
                ctx.states,
                note_node,
                "note",
                &item_id,
            )?;
            upsert_node_typed(
                ctx.nodes,
                #[cfg(feature = "presentation")]
                ctx.presentation,
                ctx.node_index,
                node,
            );

            let (mut from, mut to) = (item_id.clone(), note_id);
            if n.position.as_deref() == Some("left of") {
                std::mem::swap(&mut from, &mut to);
            }

            ctx.edges.push(StateEdge {
                id: format!("{from}-{to}"),
                start: from,
                end: to,
                classes: CSS_EDGE_NOTE_EDGE.to_string(),
                label: String::new(),
            });
            *ctx.graph_item_count += 1;
        } else {
            upsert_node_typed(
                ctx.nodes,
                #[cfg(feature = "presentation")]
                ctx.presentation,
                ctx.node_index,
                node,
            );
        }

        Ok(())
    }

    {
        let mut ctx = StateGraphContext {
            document,
            control,
            states,
            classes,
            config,
            #[cfg(feature = "presentation")]
            collect_presentation,
            nodes: &mut nodes,
            #[cfg(feature = "presentation")]
            presentation: &mut presentation,
            node_index: &mut node_index,
            edges: &mut edges,
            node_db: &mut node_db,
            graph_item_count: &mut graph_item_count,
            #[cfg(feature = "presentation")]
            next_color_index: 0,
            #[cfg(feature = "presentation")]
            container_color_index: HashMap::new(),
        };
        if let Some(root) = document.root {
            setup_doc(&mut ctx, None, root, false)?;
        }
    }

    // Post-process label arrays into (label, description) like Mermaid's StateDB.extract().
    for (index, node) in nodes.iter_mut().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(label_val) = node.label.clone() else {
            continue;
        };
        let Some(arr) = label_val.as_array() else {
            continue;
        };
        if arr.is_empty() {
            continue;
        }
        let label0 = arr[0].clone();
        let rest: Vec<String> = arr
            .iter()
            .skip(1)
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if node.is_group && !rest.is_empty() {
            return Err("Group nodes can only have label".to_string().into());
        }
        node.label = Some(label0);
        node.description = Some(rest);
    }

    Ok(StateGraphConstruction {
        nodes,
        edges,
        #[cfg(feature = "presentation")]
        presentation,
    })
}

#[cfg(all(test, feature = "presentation"))]
mod tests;
