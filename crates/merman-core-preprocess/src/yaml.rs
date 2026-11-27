//! Bounded YAML parsing and source-order capture for preprocessed text.

#[cfg(feature = "yaml-editor")]
use crate::SourceConfigPath;
use granit_parser::{Event, Parser, ScalarStyle, Tag};
#[cfg(feature = "yaml-editor")]
use granit_parser::{Span, StructureStyle};
use merman_model::json::{
    JSON_OBJECT_ENTRY_CHARGE_BYTES, JSON_VALUE_CHARGE_BYTES, clone_value_nonrecursive,
    drop_value_nonrecursive,
};
use merman_model::{OperationControl, OperationControlResult};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem::size_of;
#[cfg(feature = "yaml-editor")]
use std::sync::Arc;

const YAML_MATERIALIZATION_MIN_BYTES: usize = 64 * 1024;
const YAML_MATERIALIZATION_INPUT_MULTIPLIER: usize = 16;
const YAML_PARSER_MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

#[cfg(test)]
fn parse_yaml_value(input: &str, max_nesting_depth: usize) -> Result<Value, String> {
    let control = OperationControl::new();
    parse_yaml_value_controlled(input, max_nesting_depth, &control)
        .expect("a private parse control cannot be cancelled")
}

pub fn parse_yaml_value_controlled(
    input: &str,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, String>> {
    parse_yaml_value_with_limits_controlled(
        input,
        YAML_PARSER_MAX_INPUT_BYTES,
        max_nesting_depth,
        default_materialization_budget(input),
        control,
    )
}

fn default_materialization_budget(input: &str) -> usize {
    input
        .len()
        .saturating_mul(YAML_MATERIALIZATION_INPUT_MULTIPLIER)
        .max(YAML_MATERIALIZATION_MIN_BYTES)
}

#[cfg(feature = "yaml-editor")]
#[derive(Debug)]
pub struct YamlConfigKeyEvidence {
    pub path: SourceConfigPath,
    pub span: Option<std::ops::Range<usize>>,
    pub rewrite_safe: bool,
}

pub struct YamlValueCapture {
    pub value: Result<Value, String>,
    pub collection_text: YamlCollectionText,
    #[cfg(feature = "yaml-editor")]
    pub keys: Vec<YamlConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    pub rewrite_safe: bool,
}

/// Ordered JSON text for requested top-level collection fields.
pub type YamlCollectionText = BTreeMap<&'static str, String>;

#[derive(Default)]
struct Capture<'a> {
    collection_text_keys: &'a [&'static str],
    root_key_order: Option<&'a mut Vec<String>>,
    #[cfg(feature = "yaml-editor")]
    ordered_document: Option<&'a mut String>,
    #[cfg(feature = "yaml-editor")]
    insert_order: Option<InsertOrderCapture<'a>>,
    #[cfg(feature = "yaml-editor")]
    keys: bool,
}

/// Selects the ordered roots to project for a proposed insertion.
#[cfg(feature = "yaml-editor")]
pub struct YamlInsertOrderRequest<'a> {
    /// Source keys and destination names in the caller's precedence order.
    pub routes: &'a [(&'static str, &'static str)],
    /// Root key whose presence makes the insertion unnecessary.
    pub insert_key: &'static str,
    pub max_bytes: usize,
    pub max_depth: usize,
}

/// Order evidence for a YAML document that admits insertion of the requested key.
#[cfg(feature = "yaml-editor")]
#[derive(Debug, Clone, PartialEq)]
pub enum YamlInsertOrder {
    /// The document is appendable and contains none of the requested routes.
    NoRoutes,
    /// The selected routes were projected into ordered JSON.
    Captured(Arc<str>),
    /// The selected routes exceeded the projection budget.
    ExceededBudget,
}

#[cfg(feature = "yaml-editor")]
struct InsertOrderCapture<'a> {
    request: YamlInsertOrderRequest<'a>,
    output: &'a mut Option<YamlInsertOrder>,
}

pub fn parse_yaml_with_capture_controlled(
    input: &str,
    max_nesting_depth: usize,
    collection_text_keys: &[&'static str],
    control: &OperationControl,
) -> OperationControlResult<YamlValueCapture> {
    parse_yaml_controlled(
        input,
        YAML_PARSER_MAX_INPUT_BYTES,
        max_nesting_depth,
        default_materialization_budget(input),
        Capture {
            collection_text_keys,
            ..Capture::default()
        },
        control,
    )
}

#[cfg(feature = "yaml-editor")]
pub fn parse_yaml_frontmatter_capture_controlled(
    input: &str,
    max_nesting_depth: usize,
    collection_text_keys: &[&'static str],
    insert_order_request: Option<YamlInsertOrderRequest<'_>>,
    control: &OperationControl,
) -> OperationControlResult<(YamlValueCapture, Option<YamlInsertOrder>)> {
    let Some(request) = insert_order_request else {
        let captured = parse_yaml_with_capture_controlled(
            input,
            max_nesting_depth,
            collection_text_keys,
            control,
        )?;
        return Ok((captured, None));
    };
    let mut insert_order = None;
    let captured = parse_yaml_controlled(
        input,
        YAML_PARSER_MAX_INPUT_BYTES,
        max_nesting_depth,
        default_materialization_budget(input),
        Capture {
            collection_text_keys,
            keys: true,
            insert_order: Some(InsertOrderCapture {
                request,
                output: &mut insert_order,
            }),
            ..Capture::default()
        },
        control,
    )?;
    Ok((captured, insert_order))
}

pub fn parse_yaml_value_with_root_order_controlled(
    input: &str,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<(Value, Vec<String>), String>> {
    let mut root_key_order = Vec::new();
    let captured = parse_yaml_controlled(
        input,
        YAML_PARSER_MAX_INPUT_BYTES,
        max_nesting_depth,
        default_materialization_budget(input),
        Capture {
            root_key_order: Some(&mut root_key_order),
            ..Capture::default()
        },
        control,
    )?;
    Ok(captured.value.map(|value| (value, root_key_order)))
}

pub fn parse_yaml_value_with_limits_controlled(
    input: &str,
    max_input_bytes: usize,
    max_nesting_depth: usize,
    materialization_budget: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, String>> {
    Ok(parse_yaml_controlled(
        input,
        max_input_bytes,
        max_nesting_depth,
        materialization_budget,
        Capture::default(),
        control,
    )?
    .value)
}

#[cfg(feature = "yaml-editor")]
pub fn parse_yaml_ordered_document_controlled(
    input: &str,
    max_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let mut ordered_document = String::new();
    let captured = parse_yaml_controlled(
        input,
        max_bytes,
        max_nesting_depth,
        max_bytes,
        Capture {
            ordered_document: Some(&mut ordered_document),
            ..Capture::default()
        },
        control,
    )?;
    Ok(captured.value.ok().map(|_| ordered_document))
}

fn parse_yaml_controlled(
    input: &str,
    max_input_bytes: usize,
    max_nesting_depth: usize,
    materialization_budget: usize,
    capture: Capture<'_>,
    control: &OperationControl,
) -> OperationControlResult<YamlValueCapture> {
    control.checkpoint()?;
    if input.len() > max_input_bytes {
        return Ok(YamlValueCapture {
            value: Err(format!(
                "YAML input exceeds the safe parser budget of {max_input_bytes} bytes"
            )),
            collection_text: YamlCollectionText::new(),
            #[cfg(feature = "yaml-editor")]
            keys: Vec::new(),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: false,
        });
    }
    let mut builder = YamlValueBuilder::new(
        max_nesting_depth,
        materialization_budget,
        #[cfg(feature = "yaml-editor")]
        capture.keys,
    );
    #[cfg(feature = "yaml-editor")]
    {
        builder.frontmatter_appendable = capture.insert_order.is_some();
    }

    // Feature unification may enable comment support for semantic-only callers.
    // Emit comments only when collecting editor evidence.
    let options = granit_parser::options! {
        #[cfg(feature = "yaml-editor")]
        emit_comments: capture.keys,
        #[cfg(not(feature = "yaml-editor"))]
        emit_comments: false,
    };
    for (index, event) in Parser::new_from_str_with_options(input, options).enumerate() {
        if index % 64 == 0 {
            control.checkpoint()?;
        }
        let (event, _span) = match event {
            Ok(event) => event,
            Err(error) => {
                #[cfg(feature = "yaml-editor")]
                if capture.keys && error.kind() == &granit_parser::ErrorKind::TooManyComments {
                    // Optional editor evidence must not narrow semantic source acceptance.
                    // Discard partial evidence and retry once without comment buffering,
                    // preserving the caller's control and all semantic resource limits.
                    drop(builder);
                    let mut semantic = parse_yaml_controlled(
                        input,
                        max_input_bytes,
                        max_nesting_depth,
                        materialization_budget,
                        Capture {
                            keys: false,
                            ..capture
                        },
                        control,
                    )?;
                    semantic.rewrite_safe = false;
                    return Ok(semantic);
                }
                #[cfg(feature = "yaml-editor")]
                let (keys, _) = builder.take_capture();
                return Ok(YamlValueCapture {
                    value: Err(error.to_string()),
                    collection_text: YamlCollectionText::new(),
                    #[cfg(feature = "yaml-editor")]
                    keys,
                    #[cfg(feature = "yaml-editor")]
                    rewrite_safe: false,
                });
            }
        };
        if let Err(error) = builder.on_event(
            #[cfg(feature = "yaml-editor")]
            input,
            event,
            #[cfg(feature = "yaml-editor")]
            _span,
        ) {
            #[cfg(feature = "yaml-editor")]
            let (keys, _) = builder.take_capture();
            return Ok(YamlValueCapture {
                value: Err(error),
                collection_text: YamlCollectionText::new(),
                #[cfg(feature = "yaml-editor")]
                keys,
                #[cfg(feature = "yaml-editor")]
                rewrite_safe: false,
            });
        }
    }

    control.checkpoint()?;
    #[cfg(feature = "yaml-editor")]
    let (keys, rewrite_safe) = builder.take_capture();
    let value = builder.finish_controlled(control)?;
    #[cfg(feature = "yaml-editor")]
    let insert_order = capture
        .insert_order
        .filter(|_| builder.frontmatter_appendable);
    let captured = (|| {
        if !value.as_ref().is_ok_and(Value::is_object) {
            #[cfg(feature = "yaml-editor")]
            if let Some(insert_order) = insert_order
                && input.is_empty()
                && builder.root.is_none()
                && matches!(&value, Ok(Value::Null))
            {
                *insert_order.output = Some(YamlInsertOrder::NoRoutes);
            }
            return Ok(YamlCollectionText::new());
        }
        #[cfg(feature = "yaml-editor")]
        if let Some(document) = capture.ordered_document
            && let Some(root) = builder.root
        {
            *document = write_ordered_yaml_json(&builder.arena, root, control)?;
        }
        let collection_text = if capture.collection_text_keys.is_empty() {
            YamlCollectionText::new()
        } else {
            builder.capture_collection_text(capture.collection_text_keys, control)?
        };
        #[cfg(feature = "yaml-editor")]
        if let Some(insert_order) = insert_order
            && let Ok(Value::Object(fields)) = &value
            && !fields.contains_key(insert_order.request.insert_key)
        {
            let request = insert_order.request;
            let order = if request
                .routes
                .iter()
                .any(|(key, _)| fields.contains_key(*key))
            {
                match builder.frontmatter_insert_order(
                    request.routes,
                    request.max_bytes,
                    request.max_depth,
                    control,
                )? {
                    Some(json) => YamlInsertOrder::Captured(json.into()),
                    None => YamlInsertOrder::ExceededBudget,
                }
            } else {
                YamlInsertOrder::NoRoutes
            };
            *insert_order.output = Some(order);
        }
        if let Some(order) = capture.root_key_order {
            builder.take_root_key_order(order, control)?;
        }
        Ok(collection_text)
    })();
    let collection_text = match captured {
        Ok(text) => text,
        Err(cancelled) => {
            if let Ok(value) = value {
                drop_value_nonrecursive(value);
            }
            return Err(cancelled);
        }
    };
    Ok(YamlValueCapture {
        value,
        collection_text,
        #[cfg(feature = "yaml-editor")]
        keys,
        #[cfg(feature = "yaml-editor")]
        rewrite_safe,
    })
}

#[cfg(feature = "yaml-editor")]
fn yaml_key_span(input: &str, span: Span, style: ScalarStyle) -> Option<std::ops::Range<usize>> {
    let mut range = span.byte_range()?;
    if matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted) {
        let source = input.get(range.clone())?;
        let quote = match style {
            ScalarStyle::SingleQuoted => '\'',
            ScalarStyle::DoubleQuoted => '"',
            ScalarStyle::Plain | ScalarStyle::Literal | ScalarStyle::Folded => unreachable!(),
        };
        if !source.starts_with(quote) || !source.ends_with(quote) || source.len() < 2 {
            return None;
        }
        range.start = range.start.saturating_add(quote.len_utf8());
        range.end = range.end.saturating_sub(quote.len_utf8());
    }
    input.get(range.clone()).map(|_| range)
}

#[cfg(feature = "yaml-editor")]
fn yaml_key_span_is_rewrite_safe(
    input: &str,
    span: Span,
    style: ScalarStyle,
    decoded: &str,
) -> bool {
    yaml_key_span(input, span, style)
        .and_then(|range| input.get(range))
        .is_some_and(|source| source == decoded)
}

struct YamlValueBuilder {
    stack: Vec<Frame>,
    arena: Vec<YamlNode>,
    root: Option<NodeId>,
    anchors: HashMap<usize, NodeId>,
    max_nesting_depth: usize,
    materialization_budget: usize,
    #[cfg(feature = "yaml-editor")]
    capture_keys: bool,
    #[cfg(feature = "yaml-editor")]
    key_evidence: Vec<YamlConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    rewrite_safe: bool,
    #[cfg(feature = "yaml-editor")]
    frontmatter_appendable: bool,
}

impl YamlValueBuilder {
    fn new(
        max_nesting_depth: usize,
        materialization_budget: usize,
        #[cfg(feature = "yaml-editor")] capture_keys: bool,
    ) -> Self {
        Self {
            stack: Vec::new(),
            arena: Vec::new(),
            root: None,
            anchors: HashMap::new(),
            max_nesting_depth,
            materialization_budget,
            #[cfg(feature = "yaml-editor")]
            capture_keys,
            #[cfg(feature = "yaml-editor")]
            key_evidence: Vec::new(),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: true,
            #[cfg(feature = "yaml-editor")]
            frontmatter_appendable: false,
        }
    }

    fn on_event(
        &mut self,
        #[cfg(feature = "yaml-editor")] input: &str,
        event: Event<'_>,
        #[cfg(feature = "yaml-editor")] span: Span,
    ) -> Result<(), String> {
        match event {
            Event::StreamStart | Event::StreamEnd | Event::DocumentStart(..) => Ok(()),
            Event::DocumentEnd => {
                #[cfg(feature = "yaml-editor")]
                if self.frontmatter_appendable && !span.is_empty() {
                    self.frontmatter_appendable = false;
                }
                Ok(())
            }
            #[cfg(feature = "yaml-editor")]
            Event::Comment(_, _) => {
                self.rewrite_safe = false;
                Ok(())
            }
            Event::Alias(anchor_id) => {
                #[cfg(feature = "yaml-editor")]
                {
                    self.rewrite_safe = false;
                }
                let role = self.reserve_role()?;
                let node = self
                    .anchors
                    .get(&anchor_id)
                    .copied()
                    .ok_or_else(|| "unsupported forward YAML alias".to_string())?;
                self.complete_node(
                    node,
                    role,
                    0,
                    #[cfg(feature = "yaml-editor")]
                    false,
                )
            }
            Event::Scalar(raw, style, anchor_id, tag) => {
                #[cfg(feature = "yaml-editor")]
                if self.frontmatter_appendable
                    && matches!(style, ScalarStyle::Literal | ScalarStyle::Folded)
                    && span.end.byte_offset() == Some(input.len())
                {
                    self.frontmatter_appendable = false;
                }
                #[cfg(feature = "yaml-editor")]
                if anchor_id != 0
                    || tag.is_some()
                    || matches!(style, ScalarStyle::Literal | ScalarStyle::Folded)
                {
                    self.rewrite_safe = false;
                }
                let role = self.reserve_role()?;
                let value = scalar_to_value(raw.as_ref(), style, tag.as_deref())?;
                #[cfg(feature = "yaml-editor")]
                let key_source = self.capture_keys.then(|| YamlKeySource {
                    span: yaml_key_span(input, span, style),
                    rewrite_safe: yaml_key_span_is_rewrite_safe(input, span, style, raw.as_ref()),
                });
                let node = self.push_node(YamlNode::Scalar {
                    value,
                    #[cfg(feature = "yaml-editor")]
                    key_source,
                });
                self.complete_node(
                    node,
                    role,
                    anchor_id,
                    #[cfg(feature = "yaml-editor")]
                    true,
                )
            }
            Event::SequenceStart(_, anchor_id, _tag) => {
                #[cfg(feature = "yaml-editor")]
                if anchor_id != 0 || _tag.is_some() {
                    self.rewrite_safe = false;
                }
                let role = self.reserve_role()?;
                #[cfg(feature = "yaml-editor")]
                let path = self.path_for_role(&role);
                self.push_frame(Frame {
                    container: Container::Sequence(Vec::new()),
                    role,
                    anchor_id,
                    #[cfg(feature = "yaml-editor")]
                    path,
                })
            }
            Event::MappingStart(_style, anchor_id, _tag) => {
                #[cfg(feature = "yaml-editor")]
                if anchor_id != 0 || _tag.is_some() {
                    self.rewrite_safe = false;
                }
                let role = self.reserve_role()?;
                #[cfg(feature = "yaml-editor")]
                if self.frontmatter_appendable && matches!(role, Role::Root) {
                    self.frontmatter_appendable =
                        _style == StructureStyle::Block && span.start.col() == 0;
                }
                #[cfg(feature = "yaml-editor")]
                let path = self.path_for_role(&role);
                self.push_frame(Frame {
                    container: Container::Mapping {
                        entries: Vec::new(),
                        keys: HashSet::new(),
                        pending_key: None,
                    },
                    role,
                    anchor_id,
                    #[cfg(feature = "yaml-editor")]
                    path,
                })
            }
            end_event @ (Event::SequenceEnd | Event::MappingEnd) => {
                let frame = self
                    .stack
                    .pop()
                    .ok_or_else(|| "unexpected YAML collection end".to_string())?;
                let node = match (end_event, frame.container) {
                    (Event::SequenceEnd, Container::Sequence(items)) => {
                        self.push_node(YamlNode::Sequence(items))
                    }
                    (
                        Event::MappingEnd,
                        Container::Mapping {
                            entries,
                            keys: _,
                            pending_key: _,
                        },
                    ) => self.push_node(YamlNode::Mapping(entries)),
                    (Event::SequenceEnd, _) | (Event::MappingEnd, _) => {
                        return Err("mismatched YAML collection end".to_string());
                    }
                    _ => unreachable!(),
                };
                self.complete_node(
                    node,
                    frame.role,
                    frame.anchor_id,
                    #[cfg(feature = "yaml-editor")]
                    true,
                )
            }
            _ => Err("unsupported YAML parser event".to_string()),
        }
    }

    #[cfg(feature = "yaml-editor")]
    fn take_capture(&mut self) -> (Vec<YamlConfigKeyEvidence>, bool) {
        (std::mem::take(&mut self.key_evidence), self.rewrite_safe)
    }

    #[cfg(feature = "yaml-editor")]
    fn path_for_role(&self, role: &Role) -> Option<SourceConfigPath> {
        if !self.capture_keys {
            return None;
        }
        match role {
            Role::MappingValue(MappingKey::String { path, .. }) => path.clone(),
            Role::Root => Some(SourceConfigPath::root()),
            // Collection-valued mapping keys are discarded by materialization. Their descendants
            // must not publish paths as if they belonged to the surrounding object.
            Role::MappingKey => None,
            Role::SequenceItem | Role::MappingValue(MappingKey::Ignored) => None,
        }
    }

    fn push_node(&mut self, node: YamlNode) -> NodeId {
        let id = NodeId(self.arena.len());
        self.arena.push(node);
        id
    }

    fn push_frame(&mut self, frame: Frame) -> Result<(), String> {
        if self.stack.len() >= self.max_nesting_depth {
            return Err(format!(
                "config nesting exceeds {} levels",
                self.max_nesting_depth
            ));
        }
        self.stack.push(frame);
        Ok(())
    }

    fn reserve_role(&mut self) -> Result<Role, String> {
        let Some(parent) = self.stack.last_mut() else {
            if self.root.is_some() {
                return Err("multiple YAML documents are not supported".to_string());
            }
            return Ok(Role::Root);
        };

        match &mut parent.container {
            Container::Sequence(_) => Ok(Role::SequenceItem),
            Container::Mapping { pending_key, .. } => match pending_key.take() {
                Some(key) => Ok(Role::MappingValue(key)),
                None => Ok(Role::MappingKey),
            },
        }
    }

    fn complete_node(
        &mut self,
        node: NodeId,
        role: Role,
        anchor_id: usize,
        #[cfg(feature = "yaml-editor")] source_addressable: bool,
    ) -> Result<(), String> {
        if anchor_id != 0 {
            self.anchors.insert(anchor_id, node);
        }

        match role {
            Role::Root => {
                self.root = Some(node);
                Ok(())
            }
            Role::SequenceItem => {
                let Some(Frame {
                    container: Container::Sequence(items),
                    ..
                }) = self.stack.last_mut()
                else {
                    return Err("YAML sequence item had no parent sequence".to_string());
                };
                items.push(node);
                Ok(())
            }
            Role::MappingKey => {
                let key = node_to_mapping_key(&self.arena, node);
                #[cfg(feature = "yaml-editor")]
                let mut key = key;
                #[cfg(feature = "yaml-editor")]
                if self.capture_keys {
                    let parent_path = self.stack.last().and_then(|frame| frame.path.clone());
                    match (&mut key, source_addressable, parent_path) {
                        (
                            MappingKey::String {
                                name,
                                span,
                                rewrite_safe,
                                path,
                            },
                            true,
                            Some(parent_path),
                        ) => {
                            let key_path = parent_path.child(name.clone());
                            self.key_evidence.push(YamlConfigKeyEvidence {
                                path: key_path.clone(),
                                span: span.clone(),
                                rewrite_safe: *rewrite_safe,
                            });
                            *path = Some(key_path);
                            if name.as_str() == "<<" || !*rewrite_safe {
                                self.rewrite_safe = false;
                            }
                        }
                        (MappingKey::Ignored, true, _) | (_, false, _) => {
                            self.rewrite_safe = false;
                        }
                        (MappingKey::String { .. }, true, None) => {}
                    }
                }
                let Some(Frame {
                    container: Container::Mapping { pending_key, .. },
                    ..
                }) = self.stack.last_mut()
                else {
                    return Err("YAML mapping key had no parent mapping".to_string());
                };
                *pending_key = Some(key);
                Ok(())
            }
            Role::MappingValue(key) => {
                let Some(Frame {
                    container: Container::Mapping { entries, keys, .. },
                    ..
                }) = self.stack.last_mut()
                else {
                    return Err("YAML mapping value had no parent mapping".to_string());
                };
                match key {
                    MappingKey::String { name, .. } => {
                        if !keys.insert(name.clone()) {
                            return Err("duplicated mapping key".to_string());
                        }
                        entries.push((name, node));
                    }
                    MappingKey::Ignored => {}
                }
                Ok(())
            }
        }
    }

    fn finish_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Value, String>> {
        if !self.stack.is_empty() {
            return Ok(Err("incomplete YAML document".to_string()));
        }

        let Some(root) = self.root else {
            return Ok(Ok(Value::Null));
        };
        materialize_yaml_controlled(
            &self.arena,
            root,
            self.max_nesting_depth,
            self.materialization_budget,
            control,
        )
    }

    fn capture_collection_text(
        &self,
        allowed_keys: &[&'static str],
        control: &OperationControl,
    ) -> OperationControlResult<YamlCollectionText> {
        let mut text = YamlCollectionText::new();
        let Some(YamlNode::Mapping(entries)) = self.root.map(|root| &self.arena[root.0]) else {
            return Ok(text);
        };
        for (index, (key, node)) in entries.iter().enumerate() {
            if index.is_multiple_of(64) {
                control.checkpoint()?;
            }
            let Some(key) = allowed_keys
                .iter()
                .copied()
                .find(|allowed| *allowed == key.as_str())
            else {
                continue;
            };
            if matches!(
                self.arena[node.0],
                YamlNode::Sequence(_) | YamlNode::Mapping(_)
            ) {
                // JSON text is externally visible for these fields. Use the already validated
                // YAML arena to retain source order through alias expansion and nested objects.
                text.insert(key, write_ordered_yaml_json(&self.arena, *node, control)?);
            }
        }
        Ok(text)
    }

    #[cfg(feature = "yaml-editor")]
    fn frontmatter_insert_order(
        &self,
        routes: &[(&'static str, &'static str)],
        max_bytes: usize,
        max_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<String>> {
        let Some(YamlNode::Mapping(entries)) = self.root.map(|root| &self.arena[root.0]) else {
            return Ok(None);
        };
        let mut selected = BTreeMap::new();
        for (index, (key, node)) in entries.iter().enumerate() {
            if index.is_multiple_of(64) {
                control.checkpoint()?;
            }
            if routes.iter().any(|(source, _)| *source == key.as_str()) {
                selected.insert(key.as_str(), *node);
            }
        }
        let mut projected: Vec<(String, NodeId)> = Vec::new();
        for &(source, namespace) in routes {
            let Some(&node) = selected.get(source) else {
                continue;
            };
            if let Some((_, previous)) = projected
                .iter_mut()
                .find(|(key, _)| key.as_str() == namespace)
            {
                *previous = node;
            } else {
                projected.push((namespace.to_owned(), node));
            }
        }
        let mut json = String::from("{");
        let written = append_ordered_yaml_json::<true>(
            &self.arena,
            JsonTask::Mapping(projected.iter(), false),
            &mut json,
            max_bytes,
            max_depth,
            control,
        )?;
        Ok(written.then_some(json))
    }

    fn take_root_key_order(
        &mut self,
        order: &mut Vec<String>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let Some(YamlNode::Mapping(entries)) = self.root.map(|root| &mut self.arena[root.0]) else {
            return Ok(());
        };
        order.reserve(entries.len());
        for (index, (key, _)) in entries.drain(..).enumerate() {
            if index.is_multiple_of(64) {
                control.checkpoint()?;
            }
            order.push(key);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeId(usize);

enum YamlNode {
    Scalar {
        value: Value,
        #[cfg(feature = "yaml-editor")]
        key_source: Option<YamlKeySource>,
    },
    Sequence(Vec<NodeId>),
    Mapping(Vec<(String, NodeId)>),
}

enum JsonTask<'a> {
    Visit(NodeId),
    Sequence(std::slice::Iter<'a, NodeId>, bool),
    Mapping(std::slice::Iter<'a, (String, NodeId)>, bool),
}

fn write_ordered_yaml_json(
    arena: &[YamlNode],
    root: NodeId,
    control: &OperationControl,
) -> OperationControlResult<String> {
    let mut output = String::new();
    append_ordered_yaml_json::<false>(
        arena,
        JsonTask::Visit(root),
        &mut output,
        usize::MAX,
        usize::MAX,
        control,
    )?;
    Ok(output)
}

fn append_ordered_yaml_json<'a, const BOUNDED: bool>(
    arena: &'a [YamlNode],
    first: JsonTask<'a>,
    output: &mut String,
    max_bytes: usize,
    max_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    let mut tasks = vec![first];
    let mut steps = 0usize;
    while let Some(task) = tasks.pop() {
        if BOUNDED
            && tasks.len() >= max_depth
            && matches!(&task, JsonTask::Mapping(..) | JsonTask::Sequence(..))
        {
            return Ok(false);
        }
        if steps.is_multiple_of(64) {
            control.checkpoint()?;
        }
        steps += 1;
        match task {
            JsonTask::Visit(node) => match &arena[node.0] {
                YamlNode::Scalar {
                    value: Value::String(value),
                    ..
                } => {
                    if !append_json_string::<BOUNDED>(output, value, max_bytes, control)? {
                        return Ok(false);
                    }
                }
                YamlNode::Scalar { value, .. } => {
                    output.push_str(&value.to_string());
                }
                YamlNode::Sequence(items) => {
                    output.push('[');
                    tasks.push(JsonTask::Sequence(items.iter(), false));
                }
                YamlNode::Mapping(entries) => {
                    output.push('{');
                    tasks.push(JsonTask::Mapping(entries.iter(), false));
                }
            },
            JsonTask::Sequence(mut items, comma) => {
                if let Some(item) = items.next() {
                    if comma {
                        output.push(',');
                    }
                    tasks.push(JsonTask::Sequence(items, true));
                    tasks.push(JsonTask::Visit(*item));
                } else {
                    output.push(']');
                }
            }
            JsonTask::Mapping(mut entries, comma) => {
                if let Some((key, value)) = entries.next() {
                    if comma {
                        output.push(',');
                    }
                    if !append_json_string::<BOUNDED>(output, key, max_bytes, control)? {
                        return Ok(false);
                    }
                    output.push(':');
                    tasks.push(JsonTask::Mapping(entries, true));
                    tasks.push(JsonTask::Visit(*value));
                } else {
                    output.push('}');
                }
            }
        }
        if BOUNDED && output.len() > max_bytes {
            return Ok(false);
        }
    }
    control.checkpoint()?;
    Ok(true)
}

fn append_json_string<const BOUNDED: bool>(
    output: &mut String,
    mut text: &str,
    max_bytes: usize,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    const CHUNK_BYTES: usize = 4 * 1024;
    output.push('"');
    while !text.is_empty() {
        let end = text.floor_char_boundary(CHUNK_BYTES);
        let (chunk, remaining) = text.split_at(end);
        let quoted = serde_json::to_string(chunk).expect("strings are serializable to JSON");
        let escaped = &quoted[1..quoted.len() - 1];
        if BOUNDED && escaped.len() > max_bytes.saturating_sub(output.len()) {
            return Ok(false);
        }
        output.push_str(escaped);
        if !remaining.is_empty() {
            control.checkpoint()?;
        }
        text = remaining;
    }
    output.push('"');
    Ok(true)
}

struct Frame {
    container: Container,
    role: Role,
    anchor_id: usize,
    #[cfg(feature = "yaml-editor")]
    path: Option<SourceConfigPath>,
}

#[cfg(feature = "yaml-editor")]
struct YamlKeySource {
    span: Option<std::ops::Range<usize>>,
    rewrite_safe: bool,
}

enum Container {
    Sequence(Vec<NodeId>),
    Mapping {
        entries: Vec<(String, NodeId)>,
        keys: HashSet<String>,
        pending_key: Option<MappingKey>,
    },
}

enum Role {
    Root,
    SequenceItem,
    MappingKey,
    MappingValue(MappingKey),
}

enum MappingKey {
    String {
        name: String,
        #[cfg(feature = "yaml-editor")]
        span: Option<std::ops::Range<usize>>,
        #[cfg(feature = "yaml-editor")]
        rewrite_safe: bool,
        #[cfg(feature = "yaml-editor")]
        path: Option<SourceConfigPath>,
    },
    Ignored,
}

fn node_to_mapping_key(arena: &[YamlNode], node: NodeId) -> MappingKey {
    match &arena[node.0] {
        YamlNode::Scalar {
            value: Value::String(key),
            #[cfg(feature = "yaml-editor")]
            key_source,
        } => MappingKey::String {
            name: key.clone(),
            #[cfg(feature = "yaml-editor")]
            span: key_source.as_ref().and_then(|source| source.span.clone()),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: key_source.as_ref().is_none_or(|source| source.rewrite_safe),
            #[cfg(feature = "yaml-editor")]
            path: None,
        },
        YamlNode::Scalar {
            value: Value::Number(key),
            #[cfg(feature = "yaml-editor")]
            key_source,
        } => MappingKey::String {
            name: key.to_string(),
            #[cfg(feature = "yaml-editor")]
            span: key_source.as_ref().and_then(|source| source.span.clone()),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: false,
            #[cfg(feature = "yaml-editor")]
            path: None,
        },
        YamlNode::Scalar {
            value: Value::Bool(true),
            #[cfg(feature = "yaml-editor")]
            key_source,
        } => MappingKey::String {
            name: "true".to_string(),
            #[cfg(feature = "yaml-editor")]
            span: key_source.as_ref().and_then(|source| source.span.clone()),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: false,
            #[cfg(feature = "yaml-editor")]
            path: None,
        },
        YamlNode::Scalar {
            value: Value::Bool(false),
            #[cfg(feature = "yaml-editor")]
            key_source,
        } => MappingKey::String {
            name: "false".to_string(),
            #[cfg(feature = "yaml-editor")]
            span: key_source.as_ref().and_then(|source| source.span.clone()),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: false,
            #[cfg(feature = "yaml-editor")]
            path: None,
        },
        YamlNode::Scalar {
            value: Value::Null,
            #[cfg(feature = "yaml-editor")]
            key_source,
        } => MappingKey::String {
            name: "null".to_string(),
            #[cfg(feature = "yaml-editor")]
            span: key_source.as_ref().and_then(|source| source.span.clone()),
            #[cfg(feature = "yaml-editor")]
            rewrite_safe: false,
            #[cfg(feature = "yaml-editor")]
            path: None,
        },
        YamlNode::Scalar {
            value: Value::Array(_) | Value::Object(_),
            ..
        }
        | YamlNode::Sequence(_)
        | YamlNode::Mapping(_) => MappingKey::Ignored,
    }
}

enum MaterializeTask {
    Visit { node: NodeId, depth: usize },
    CompleteSequence { len: usize },
    CompleteMapping { node: NodeId, len: usize },
}

fn materialize_yaml_controlled(
    arena: &[YamlNode],
    root: NodeId,
    max_nesting_depth: usize,
    materialization_budget: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, String>> {
    control.checkpoint()?;
    let mut tasks = vec![MaterializeTask::Visit {
        node: root,
        depth: 0,
    }];
    let mut values = Vec::new();
    let mut materialized_bytes = 0usize;
    let mut visited = 0usize;

    while let Some(task) = tasks.pop() {
        if visited.is_multiple_of(64)
            && let Err(cancelled) = control.checkpoint()
        {
            drop_materialized_values(values);
            return Err(cancelled);
        }
        visited = visited.saturating_add(1);
        match task {
            MaterializeTask::Visit { node, depth } => {
                if depth > max_nesting_depth {
                    drop_materialized_values(values);
                    return Ok(Err(format!(
                        "config nesting exceeds {max_nesting_depth} levels after resolving YAML aliases"
                    )));
                }

                let yaml_node = &arena[node.0];
                materialized_bytes =
                    materialized_bytes.saturating_add(yaml_node_materialized_cost(yaml_node));
                if materialized_bytes > materialization_budget {
                    drop_materialized_values(values);
                    return Ok(Err(
                        "YAML aliases expand beyond the safe materialization budget".to_string(),
                    ));
                }

                match yaml_node {
                    YamlNode::Scalar { value, .. } => {
                        values.push(clone_value_nonrecursive(value));
                    }
                    YamlNode::Sequence(items) => {
                        tasks.push(MaterializeTask::CompleteSequence { len: items.len() });
                        for (index, child) in items.iter().rev().enumerate() {
                            if index % 64 == 0
                                && let Err(cancelled) = control.checkpoint()
                            {
                                drop_materialized_values(values);
                                return Err(cancelled);
                            }
                            tasks.push(MaterializeTask::Visit {
                                node: *child,
                                depth: depth.saturating_add(1),
                            });
                        }
                    }
                    YamlNode::Mapping(entries) => {
                        tasks.push(MaterializeTask::CompleteMapping {
                            node,
                            len: entries.len(),
                        });
                        for (index, (_, child)) in entries.iter().rev().enumerate() {
                            if index % 64 == 0
                                && let Err(cancelled) = control.checkpoint()
                            {
                                drop_materialized_values(values);
                                return Err(cancelled);
                            }
                            tasks.push(MaterializeTask::Visit {
                                node: *child,
                                depth: depth.saturating_add(1),
                            });
                        }
                    }
                }
            }
            MaterializeTask::CompleteSequence { len } => {
                let start = values.len().saturating_sub(len);
                let items = values.split_off(start);
                values.push(Value::Array(items));
            }
            MaterializeTask::CompleteMapping { node, len } => {
                let start = values.len().saturating_sub(len);
                let children = values.split_off(start);
                let YamlNode::Mapping(entries) = &arena[node.0] else {
                    drop_materialized_values(values);
                    drop_materialized_values(children);
                    return Ok(Err("invalid YAML materialization state".to_string()));
                };
                let mut map = Map::with_capacity(len);
                let mut children = children.into_iter();
                for (index, (key, _)) in entries.iter().enumerate() {
                    if index % 64 == 0
                        && let Err(cancelled) = control.checkpoint()
                    {
                        drop_materialized_values(values);
                        drop_materialized_values(children.collect());
                        drop_value_nonrecursive(Value::Object(map));
                        return Err(cancelled);
                    }
                    let Some(value) = children.next() else {
                        drop_materialized_values(values);
                        drop_materialized_values(children.collect());
                        drop_value_nonrecursive(Value::Object(map));
                        return Ok(Err("invalid YAML materialization state".to_string()));
                    };
                    map.insert(key.clone(), value);
                }
                values.push(Value::Object(map));
            }
        }
    }

    if values.len() != 1 {
        drop_materialized_values(values);
        return Ok(Err("invalid YAML materialization state".to_string()));
    }
    if let Err(cancelled) = control.checkpoint() {
        drop_materialized_values(values);
        return Err(cancelled);
    }
    Ok(Ok(values.pop().unwrap_or(Value::Null)))
}

fn yaml_node_materialized_cost(node: &YamlNode) -> usize {
    match node {
        YamlNode::Scalar {
            value: Value::String(value),
            ..
        } => JSON_VALUE_CHARGE_BYTES.saturating_add(value.len()),
        YamlNode::Scalar {
            value:
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_),
            ..
        } => JSON_VALUE_CHARGE_BYTES,
        YamlNode::Sequence(items) => JSON_VALUE_CHARGE_BYTES
            .saturating_add(size_of::<Vec<Value>>())
            .saturating_add(items.len().saturating_mul(JSON_VALUE_CHARGE_BYTES)),
        YamlNode::Mapping(entries) => entries.iter().fold(
            (2 * JSON_VALUE_CHARGE_BYTES)
                .saturating_add(entries.len().saturating_mul(JSON_OBJECT_ENTRY_CHARGE_BYTES)),
            |total, (key, _)| total.saturating_add(key.len()),
        ),
    }
}

fn drop_materialized_values(values: Vec<Value>) {
    for value in values {
        drop_value_nonrecursive(value);
    }
}

fn scalar_to_value(raw: &str, style: ScalarStyle, tag: Option<&Tag>) -> Result<Value, String> {
    if let Some(core_suffix) = tag.and_then(Tag::core_suffix) {
        return scalar_to_tagged_value(raw, core_suffix);
    }

    if style != ScalarStyle::Plain {
        return Ok(Value::String(raw.to_string()));
    }

    if is_yaml_null(raw) {
        return Ok(Value::Null);
    }
    if let Some(value) = parse_yaml_bool(raw) {
        return Ok(Value::Bool(value));
    }
    if let Some(number) = parse_yaml_int(raw) {
        return Ok(Value::Number(number));
    }
    if let Some(number) = parse_yaml_float(raw) {
        return Ok(Value::Number(number));
    }

    Ok(Value::String(raw.to_string()))
}

fn scalar_to_tagged_value(raw: &str, core_suffix: &str) -> Result<Value, String> {
    match core_suffix {
        "str" => Ok(Value::String(raw.to_string())),
        "null" if is_yaml_null(raw) || raw.is_empty() => Ok(Value::Null),
        "bool" => parse_yaml_bool(raw)
            .map(Value::Bool)
            .ok_or_else(|| format!("invalid YAML bool scalar: {raw:?}")),
        "int" => parse_yaml_int(raw)
            .map(Value::Number)
            .ok_or_else(|| format!("invalid YAML integer scalar: {raw:?}")),
        "float" => parse_yaml_float(raw)
            .map(Value::Number)
            .ok_or_else(|| format!("invalid YAML float scalar: {raw:?}")),
        _ => Ok(Value::String(raw.to_string())),
    }
}

fn is_yaml_null(raw: &str) -> bool {
    matches!(raw, "" | "~" | "null" | "Null" | "NULL")
}

fn parse_yaml_bool(raw: &str) -> Option<bool> {
    match raw {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

fn parse_yaml_int(raw: &str) -> Option<Number> {
    let cleaned = raw.replace('_', "");
    let (negative, body) = match cleaned.as_bytes().first()? {
        b'-' => (true, &cleaned[1..]),
        b'+' => (false, &cleaned[1..]),
        _ => (false, cleaned.as_str()),
    };
    if body.is_empty() {
        return None;
    }

    if body == "0" {
        return Some(Number::from(0));
    }

    if let Some(digits) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        return parse_nondecimal_int(digits, 2, negative);
    }
    if let Some(digits) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        return parse_nondecimal_int(digits, 8, negative);
    }
    if let Some(digits) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        return parse_nondecimal_int(digits, 16, negative);
    }

    if !body.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    parse_decimal_int(body, negative)
}

fn parse_yaml_float(raw: &str) -> Option<Number> {
    let cleaned = raw.replace('_', "");
    let (negative, body) = match cleaned.as_bytes().first()? {
        b'-' => (true, &cleaned[1..]),
        b'+' => (false, &cleaned[1..]),
        _ => (false, cleaned.as_str()),
    };
    if body.is_empty() {
        return None;
    }

    let value = match body {
        ".inf" | ".Inf" | ".INF" => Some(f64::INFINITY),
        ".nan" | ".NaN" | ".NAN" => Some(f64::NAN),
        _ => {
            if !is_yaml_float_body(body) {
                return None;
            }
            body.parse::<f64>().ok()
        }
    }?;

    let value = if negative { -value } else { value };
    Number::from_f64(value)
}

fn parse_decimal_int(body: &str, negative: bool) -> Option<Number> {
    let unsigned = body.parse::<u128>().ok()?;
    if negative {
        let signed = i128::try_from(unsigned).ok()?.checked_neg()?;
        if let Ok(value) = i64::try_from(signed) {
            Some(Number::from(value))
        } else {
            Number::from_f64(signed as f64)
        }
    } else if let Ok(value) = u64::try_from(unsigned) {
        Some(Number::from(value))
    } else {
        Number::from_f64(unsigned as f64)
    }
}

fn parse_nondecimal_int(digits: &str, radix: u32, negative: bool) -> Option<Number> {
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
        return None;
    }
    let unsigned = u128::from_str_radix(digits, radix).ok()?;
    if negative {
        let signed = i128::try_from(unsigned).ok()?.checked_neg()?;
        if let Ok(value) = i64::try_from(signed) {
            Some(Number::from(value))
        } else {
            Number::from_f64(signed as f64)
        }
    } else if let Ok(value) = u64::try_from(unsigned) {
        Some(Number::from(value))
    } else {
        Number::from_f64(unsigned as f64)
    }
}

fn is_yaml_float_body(body: &str) -> bool {
    let mut chars = body.chars().peekable();
    let mut saw_digit = false;

    let mut int_digits = 0usize;
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        chars.next();
        saw_digit = true;
        int_digits += 1;
    }

    let mut frac_digits = 0usize;
    if chars.peek() == Some(&'.') {
        chars.next();
        while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            chars.next();
            saw_digit = true;
            frac_digits += 1;
        }
        if int_digits == 0 && frac_digits == 0 {
            return false;
        }
    } else if int_digits == 0 && chars.peek() != Some(&'.') {
        return false;
    }

    if let Some(&ch) = chars.peek()
        && (ch == 'e' || ch == 'E')
    {
        chars.next();
        if matches!(chars.peek(), Some('+') | Some('-')) {
            chars.next();
        }
        let mut exp_digits = 0usize;
        while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            chars.next();
            exp_digits += 1;
        }
        if exp_digits == 0 {
            return false;
        }
        saw_digit = true;
    }

    saw_digit && chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::OperationCancelled;
    use serde_json::json;

    #[cfg(feature = "yaml-editor")]
    fn capture_yaml(input: &str) -> YamlValueCapture {
        parse_yaml_controlled(
            input,
            YAML_PARSER_MAX_INPUT_BYTES,
            16,
            default_materialization_budget(input),
            Capture {
                keys: true,
                ..Capture::default()
            },
            &OperationControl::new(),
        )
        .expect("a private parse control cannot be cancelled")
    }

    #[test]
    fn controlled_yaml_parse_stops_before_consuming_events() {
        let control = OperationControl::new();
        control.cancel();

        let result = parse_yaml_value_controlled("value: 1\n", 16, &control);

        assert!(matches!(result, Err(OperationCancelled { .. })));
    }

    #[test]
    fn root_key_order_keeps_authored_sequence_in_the_value_parse() {
        let control = OperationControl::new();
        let (value, order) = parse_yaml_value_with_root_order_controlled(
            "z: &items [first, second]\n'b': *items\na: 3\n",
            16,
            &control,
        )
        .unwrap()
        .unwrap();
        assert_eq!(order, ["z", "b", "a"]);
        assert_eq!(
            value,
            json!({"z": ["first", "second"], "b": ["first", "second"], "a": 3})
        );
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn yaml_insert_order_distinguishes_absence_and_budget_exhaustion() {
        let routes = [
            ("legacy", "flowchart"),
            ("journey", "journey"),
            ("flowchart", "flowchart"),
        ];
        let control = OperationControl::new();
        let parse = |input, max_bytes| {
            parse_yaml_frontmatter_capture_controlled(
                input,
                16,
                &[],
                Some(YamlInsertOrderRequest {
                    routes: &routes,
                    insert_key: "config",
                    max_bytes,
                    max_depth: 16,
                }),
                &control,
            )
            .unwrap()
        };
        let source = concat!(
            "metadata: &options {z: [one, two], a: 2}\n",
            "flowchart: *options\n",
            "journey: {width: 10}\n",
            "legacy: {stale: 0}\n",
        );
        let expected = r#"{"flowchart":{"z":["one","two"],"a":2},"journey":{"width":10}}"#;
        let (captured, order) = parse(source, expected.len());
        let value = captured.value.unwrap();
        assert_eq!(value["metadata"], value["flowchart"]);
        assert_eq!(order, Some(YamlInsertOrder::Captured(expected.into())));
        let (captured, order) = parse(source, expected.len() - 1);
        assert_eq!(captured.value.unwrap(), value);
        assert_eq!(order, Some(YamlInsertOrder::ExceededBudget));

        assert_eq!(parse("", 0).1, Some(YamlInsertOrder::NoRoutes));
        assert_eq!(
            parse("title: title\n", 0).1,
            Some(YamlInsertOrder::NoRoutes)
        );
        assert_eq!(parse("config: null\n", expected.len()).1, None);
        assert_eq!(parse("{legacy: 1}\n", expected.len()).1, None);
    }

    #[test]
    fn ordered_json_string_chunks_preserve_utf8_and_observe_cancellation() {
        let value = "界\n\"\\".repeat(3000);
        let mut output = String::new();
        append_json_string::<false>(&mut output, &value, usize::MAX, &OperationControl::new())
            .unwrap();
        assert_eq!(output, serde_json::to_string(&value).unwrap());

        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        output.clear();
        assert!(append_json_string::<false>(&mut output, &value, usize::MAX, &control).is_err());
    }

    #[test]
    fn parses_nested_yaml_without_recursion() {
        let value = parse_yaml_value(
            r#"
# Configuration metadata.
config:
  theme: base # Inline comment.
  flowchart:
    htmlLabels: true
  values: [1, 0x10, false, null]
"#,
            16,
        )
        .expect("yaml parses");

        assert_eq!(
            value,
            json!({
                "config": {
                    "theme": "base",
                    "flowchart": {
                        "htmlLabels": true
                    },
                    "values": [1, 16, false, null]
                }
            })
        );
    }

    #[test]
    fn folds_minimally_indented_quoted_scalar_continuations() {
        for input in ["title: \"one\n two\"", "title: 'one\n two'"] {
            let value = parse_yaml_value(input, 16).expect("quoted continuation parses");

            assert_eq!(value, json!({ "title": "one two" }));
        }
    }

    #[test]
    fn literal_scalar_chomping_ignores_indentation_only_line_at_eof() {
        for (indicator, expected) in [("|", "a\n"), ("|-", "a"), ("|+", "a\n")] {
            let input = format!("title: {indicator}\n  a\n  ");
            let value = parse_yaml_value(&input, 16).expect("literal scalar parses");

            assert_eq!(value, json!({ "title": expected }), "{indicator}");
        }
    }

    #[test]
    fn ignores_complex_mapping_keys() {
        let value = parse_yaml_value(
            r#"
? [non, string, key]
: ignored
plain: retained
"#,
            16,
        )
        .expect("yaml parses");

        assert_eq!(value, json!({ "plain": "retained" }));
    }

    #[test]
    fn aliases_share_parser_storage_but_materialize_independent_json_values() {
        let value = parse_yaml_value(
            r#"
base: &base
  theme: forest
  flowchart:
    htmlLabels: true
first: *base
second: *base
"#,
            16,
        )
        .expect("yaml aliases parse");

        assert_eq!(value["first"], value["base"]);
        assert_eq!(value["second"], value["base"]);
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn comment_capture_limit_preserves_semantic_values_and_resource_limits() {
        let mut input = String::from("key: # first comment\n");
        input.push_str(&"# buffered comment\n".repeat(128));
        input.push_str("next:\n  value: true\n");
        assert_eq!(
            parse_yaml_value(&input, 16).expect("comments do not change YAML values"),
            json!({ "key": null, "next": { "value": true } })
        );

        for (depth, budget) in [(16, 64 * 1024), (1, 64 * 1024), (16, 8)] {
            let control = OperationControl::new();
            let expected =
                parse_yaml_value_with_limits_controlled(&input, 4096, depth, budget, &control)
                    .expect("active control");
            let captured = parse_yaml_controlled(
                &input,
                4096,
                depth,
                budget,
                Capture {
                    keys: true,
                    ..Capture::default()
                },
                &control,
            )
            .expect("active control");

            assert_eq!(captured.value, expected, "depth={depth}, budget={budget}");
            assert!(
                captured.keys.is_empty(),
                "partial evidence must be discarded"
            );
            assert!(
                !captured.rewrite_safe,
                "comments must not be lost by a rewrite"
            );
        }
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn comment_capture_limit_does_not_hide_later_yaml_syntax_errors() {
        let mut input = String::from("key: # first comment\n");
        input.push_str(&"# buffered comment\n".repeat(128));
        input.push_str("next: [\n");
        let expected = parse_yaml_value(&input, 16).expect_err("unclosed sequence");
        let captured = capture_yaml(&input);

        assert_eq!(captured.value, Err(expected));
        assert!(captured.keys.is_empty());
        assert!(!captured.rewrite_safe);
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn key_evidence_does_not_flatten_mapping_paths_through_sequences() {
        let captured = capture_yaml(
            r#"
config:
  - lazyLoadedDiagrams: true
plain:
  lazyLoadedDiagrams: false
"#,
        );
        captured.value.expect("yaml parses");

        assert!(
            captured
                .keys
                .iter()
                .any(|key| key.path.matches(&["plain", "lazyLoadedDiagrams"]))
        );
        assert!(!captured.keys.iter().any(|key| {
            key.path.matches(&["config", "lazyLoadedDiagrams"])
                || key.path.matches(&["lazyLoadedDiagrams"])
        }));
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn complex_mapping_keys_do_not_publish_descendant_config_paths() {
        let captured = capture_yaml(
            r#"
config:
  ? { flowchart: { htmlLabels: false } }
  : ignored
plain:
  htmlLabels: true
"#,
        );
        captured.value.expect("yaml parses");

        assert!(
            captured
                .keys
                .iter()
                .any(|key| key.path.matches(&["plain", "htmlLabels"]))
        );
        assert!(!captured.keys.iter().any(|key| {
            key.path.matches(&["config", "flowchart"])
                || key.path.matches(&["config", "flowchart", "htmlLabels"])
        }));
        assert!(!captured.rewrite_safe);
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn alias_keys_and_values_do_not_publish_anchor_provenance_at_the_use_site() {
        let captured = capture_yaml(
            r#"
name: &key lazyLoadedDiagrams
? *key
: true
defaults: &defaults
  htmlLabels: false
config: *defaults
"#,
        );
        captured.value.expect("yaml aliases parse");

        assert!(captured.keys.iter().any(|key| key.path.matches(&["name"])));
        assert!(
            captured
                .keys
                .iter()
                .any(|key| key.path.matches(&["defaults", "htmlLabels"]))
        );
        assert!(
            captured
                .keys
                .iter()
                .any(|key| key.path.matches(&["config"]))
        );
        assert!(!captured.keys.iter().any(|key| {
            key.path.matches(&["lazyLoadedDiagrams"]) || key.path.matches(&["config", "htmlLabels"])
        }));
        assert!(!captured.rewrite_safe);
    }

    #[test]
    fn rejects_exponential_alias_materialization() {
        let mut yaml = String::from("a0: &a0 [x]\n");
        for level in 1..=18 {
            yaml.push_str(&format!(
                "a{level}: &a{level} [*a{}, *a{}]\n",
                level - 1,
                level - 1
            ));
        }
        yaml.push_str("root: *a18\n");

        let error = parse_yaml_value(&yaml, 64).unwrap_err();
        assert!(
            error.contains("safe materialization budget"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn caller_can_apply_a_stricter_yaml_materialization_budget() {
        let control = OperationControl::new();
        let input = "name: [one, two]\n";
        let bytes = 8 * JSON_VALUE_CHARGE_BYTES + 10 * size_of::<usize>() + 10;
        let parse = |budget| {
            parse_yaml_value_with_limits_controlled(input, 1024, 16, budget, &control)
                .expect("active control")
        };
        assert_eq!(parse(bytes), Ok(json!({ "name": ["one", "two"] })));
        let error = parse(bytes - 1).expect_err("the caller budget must reject materialization");
        assert!(error.contains("safe materialization budget"), "{error}");
    }

    #[test]
    fn caller_can_bound_the_third_party_yaml_parser_input() {
        let control = OperationControl::new();
        let result = parse_yaml_value_with_limits_controlled(
            "value: a-very-long-single-token\n",
            8,
            16,
            64 * 1024,
            &control,
        )
        .expect("active control");

        let error = result.expect_err("the parser-input budget must reject the token");
        assert!(error.contains("safe parser budget"));
    }

    #[test]
    fn alias_expansion_respects_the_same_nesting_limit_as_source_collections() {
        let mut yaml = String::from("a0: &a0 [x]\n");
        for level in 1..=8 {
            yaml.push_str(&format!("a{level}: &a{level} [*a{}]\n", level - 1));
        }
        yaml.push_str("root: *a8\n");

        let error = parse_yaml_value(&yaml, 4).unwrap_err();
        assert!(
            error.contains("after resolving YAML aliases"),
            "unexpected error: {error}"
        );
    }
}
