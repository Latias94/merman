#[cfg(feature = "presentation")]
use crate::ParseMetadata;
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
use crate::diagrams::langium_common::{LangiumCommonField, parse_langium_common};
use crate::diagrams::scan::{leading_whitespace_len, physical_line_at, split_ascii_indent};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::family;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, Result, SourceSpan};
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::{Map, json};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapNode {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<usize>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<crate::json_value::ManagedJson>,
    #[serde(
        default,
        rename = "classSelector",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_selector: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapDiagramModel {
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    pub root: TreemapNode,
    /// Flat records; child IDs are indices into this vector.
    pub nodes: Vec<TreemapNode>,
    pub classes: std::collections::BTreeMap<String, TreemapClassDef>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapClassDef {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

impl TreemapDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[cfg(feature = "presentation")]
pub type TreemapNodeRenderModel = TreemapNode;
#[cfg(feature = "presentation")]
pub type TreemapClassDefRenderModel = TreemapClassDef;

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Default)]
pub struct TreemapVisuals {
    pub root_styles: Option<Vec<String>>,
    /// Explicit full-model overrides, indexed by canonical node ID.
    pub node_styles: Vec<Option<Vec<String>>>,
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Default)]
pub struct TreemapDiagramRenderModel {
    pub semantic: TreemapDiagramModel,
    pub visuals: TreemapVisuals,
    #[cfg(feature = "compat-json")]
    compatibility_empty: bool,
}

#[cfg(feature = "presentation")]
impl std::ops::Deref for TreemapDiagramRenderModel {
    type Target = TreemapDiagramModel;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation")]
impl std::ops::DerefMut for TreemapDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

#[cfg(feature = "presentation")]
impl TreemapDiagramRenderModel {
    pub fn from_semantic_model(semantic: TreemapDiagramModel) -> Self {
        Self {
            semantic,
            ..Self::default()
        }
    }

    pub fn compiled_styles(&self, index: Option<usize>) -> Option<&[String]> {
        match index {
            Some(index) => self
                .visuals
                .node_styles
                .get(index)
                .and_then(Option::as_deref),
            None => self.visuals.root_styles.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemType {
    Section,
    Leaf,
}

#[derive(Debug, Clone)]
struct ClassDefStatement {
    class_name: SpannedText,
    style_text: Option<SpannedText>,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct ItemRow {
    indent: usize,
    name: SpannedText,
    item_type: ItemType,
    value: Option<SpannedValue>,
    class_selector: Option<SpannedText>,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone)]
enum TreemapRow {
    Item(ItemRow),
    ClassDef(ClassDefStatement),
}

type StyleClassDef = TreemapClassDef;

// The compatibility shape contains both a nested `root` tree and a flattened `nodes` array. Keep a
// hard cap on the complete serialized shape so model limits cannot be bypassed by that duplication
// or by cloning the effective config into the output.
#[cfg(feature = "compat-json")]
const TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Arena {
    nodes: Vec<TreemapNode>,
}

impl Arena {
    fn push(&mut self, node: TreemapNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }
}

struct TreemapParsedInput {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    rows: Vec<TreemapRow>,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[derive(Debug, Clone)]
struct TreemapParseIssue {
    message: String,
    span: Option<SourceSpan>,
}

struct TreemapParseOutcome {
    #[cfg(feature = "editor-facts")]
    editor_demand: EditorFactsDemand,
    parsed: TreemapParsedInput,
    first_issue: Option<TreemapParseIssue>,
}

impl TreemapParseOutcome {
    fn record_issue(&mut self, message: impl Into<String>, span: Option<SourceSpan>) {
        let issue = TreemapParseIssue {
            message: message.into(),
            span,
        };
        #[cfg(feature = "editor-facts")]
        if self.editor_demand == EditorFactsDemand::Capture {
            self.parsed
                .editor_facts
                .mark_recovered_from_parse_error(issue.message.clone(), issue.span);
        }
        if self.first_issue.is_none() {
            self.first_issue = Some(issue);
        }
    }

    fn into_strict(self, diagram_type: &str) -> Result<TreemapParsedInput> {
        match self.first_issue {
            Some(issue) => Err(treemap_error(diagram_type, issue.message, issue.span)),
            None => Ok(self.parsed),
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn into_combined_controlled(
        self,
        meta: &ParseMetadata,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<family::CombinedSemanticParse> {
        let Self {
            parsed,
            first_issue,
            ..
        } = self;
        let construction = match first_issue {
            Some(issue) => Err(family::CombinedSemanticFailure::new(
                treemap_error(&meta.diagram_type, issue.message, issue.span),
                Some(parsed.editor_facts),
            )),
            None => {
                let mut source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
                let editor_facts = std::mem::take(&mut source.editor_facts);
                let model = source.into_render_model_controlled(control)?;
                match treemap_compatibility_output_preflight(&model, meta, control)? {
                    Ok(()) => Ok((model, editor_facts)),
                    Err(error) => Err(family::CombinedSemanticFailure::new(
                        error,
                        Some(editor_facts),
                    )),
                }
            }
        };
        let combined = family::CombinedSemanticParse::from_construction_controlled(
            construction,
            control,
            |(model, editor_facts)| {
                render_model_to_compat_json_unchecked(&model, meta, control)
                    .map(|model| (model, editor_facts))
            },
            family::CombinedSemanticFailure::into_parts,
        )?;
        control.checkpoint()?;
        Ok(combined)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedValue {
    value: Value,
    #[cfg(feature = "editor-facts")]
    source: SpannedText,
}

struct TreemapSemanticSource {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    class_defs: std::collections::HashMap<String, StyleClassDef>,
    arena: Arena,
    roots: Vec<usize>,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[cfg(feature = "editor-facts")]
fn push_treemap_entity(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
    statement_span: SourceSpan,
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
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        statement_span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_treemap_payload(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
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
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_treemap_row_editor_facts(facts: &mut EditorSemanticFacts, row: &TreemapRow) {
    match row {
        TreemapRow::ClassDef(class_def) => {
            facts.push_symbol(EditorSemanticSymbol::class_definition(
                class_def.class_name.text.clone(),
                Some("treemap class definition".to_string()),
                EditorSemanticKind::Class,
                class_def.span,
                class_def.class_name.span,
            ));
            if let Some(style) = class_def.style_text.as_ref() {
                push_treemap_payload(
                    facts,
                    style,
                    "treemap class style",
                    EditorSemanticKind::String,
                );
            }
        }
        TreemapRow::Item(item) => {
            let (detail, kind) = match item.item_type {
                ItemType::Section => ("treemap section", EditorSemanticKind::Namespace),
                ItemType::Leaf => ("treemap leaf", EditorSemanticKind::Variable),
            };
            push_treemap_entity(facts, &item.name, item.span, detail, kind);
            if let Some(class_selector) = item.class_selector.as_ref() {
                push_treemap_payload(
                    facts,
                    class_selector,
                    "treemap class selector",
                    EditorSemanticKind::String,
                );
            }
            if let Some(value) = item.value.as_ref() {
                push_treemap_payload(
                    facts,
                    &value.source,
                    "treemap value",
                    EditorSemanticKind::String,
                );
            }
        }
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_treemap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_treemap_semantic_source(code, meta)?
        .into_render_model_controlled(&crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled");
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_treemap_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?
    .into_combined_controlled(meta, control)?;
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_treemap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapDiagramRenderModel> {
    Ok(parse_treemap_semantic_source(code, meta)?
        .into_render_model_controlled(&crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled"))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_treemap_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<TreemapDiagramRenderModel>> {
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let parsed = match parsed.into_strict(&meta.diagram_type) {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
    control.checkpoint()?;
    Ok(Ok(source.into_render_model_controlled(control)?))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    render_model_to_compat_json_controlled(model, meta, &crate::OperationControl::new())
        .expect("a private compatibility projection control cannot be cancelled")
        .map(crate::ManagedSemanticJson::into_unmanaged_value)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<crate::ManagedSemanticJson>> {
    control.checkpoint()?;
    match treemap_compatibility_output_preflight(model, meta, control)? {
        Ok(()) => {}
        Err(error) => return Ok(Err(error)),
    }
    render_model_to_compat_json_unchecked(model, meta, control)
}

#[cfg(feature = "compat-json")]
fn render_model_to_compat_json_unchecked(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<crate::ManagedSemanticJson>> {
    control.checkpoint()?;
    if model.compatibility_empty || model.root.children.is_none() {
        return Ok(Ok(crate::ManagedSemanticJson::from_value(json!({}))));
    }
    let root = match model.project_root_controlled(control)? {
        Ok(root) => root,
        Err(message) => return Ok(Err(treemap_error(&meta.diagram_type, message, None))),
    };
    let mut nodes =
        crate::ManagedSemanticJson::from_value(Value::Array(Vec::with_capacity(model.nodes.len())));
    let mut stack = model
        .root
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .map(|&id| (id, 0_i64))
        .collect::<Vec<_>>();
    while let Some((id, level)) = stack.pop() {
        control.checkpoint()?;
        let node = &model.nodes[id];
        let mut value = render_node_to_json_controlled(
            node,
            model.compiled_styles(Some(id)),
            &model.classes,
            control,
        )?;
        value
            .as_value_mut()
            .as_object_mut()
            .expect("node projection is an object")
            .insert("level".to_string(), Value::Number(level.into()));
        nodes
            .as_value_mut()
            .as_array_mut()
            .expect("flat node projection is an array")
            .push(value.into_unmanaged_value());
        if let Some(children) = &node.children {
            for &child in children.iter().rev() {
                control.checkpoint()?;
                stack.push((child, level.saturating_add(1)));
            }
        }
    }
    let classes = render_classes_to_value(&model.classes, control)?;
    let config = crate::ManagedSemanticJson::from_value(
        crate::json_value::clone_value_nonrecursive_with_control(
            meta.effective_config.as_value(),
            control,
        )?,
    );
    let mut out = Map::new();
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("root".to_string(), root.into_unmanaged_value());
    out.insert("nodes".to_string(), nodes.into_unmanaged_value());
    out.insert("classes".to_string(), classes);
    out.insert("config".to_string(), config.into_unmanaged_value());
    let out = crate::ManagedSemanticJson::from_value(Value::Object(out));
    control.checkpoint()?;
    Ok(Ok(out))
}

/// Estimates the complete compatibility JSON shape before allocating its output `Value` tree.
/// The estimate includes the nested and flattened node representations, class definitions, source
/// values, and effective config. It is deliberately conservative around JSON punctuation and
/// escaping so a later projection cannot bypass the hard output cap. Cancellation remains on the
/// outer [`crate::OperationControlResult`] channel.
#[cfg(feature = "compat-json")]
fn treemap_compatibility_output_preflight(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<()>> {
    control.checkpoint()?;
    if model.compatibility_empty || model.root.children.is_none() {
        return Ok(Ok(()));
    }

    if let Err(message) = model.validate_child_relationships_controlled(control)? {
        return Ok(Err(treemap_error(&meta.diagram_type, message, None)));
    }
    let class_style_bytes = estimate_class_style_arrays(&model.classes, control)?;
    let estimated_bytes =
        estimate_compatibility_json_bytes(model, meta, &class_style_bytes, control)?;
    if estimated_bytes > TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES {
        return Ok(Err(treemap_compatibility_output_budget_error(
            meta,
            estimated_bytes,
        )));
    }
    Ok(Ok(()))
}

#[cfg(feature = "compat-json")]
fn estimate_compatibility_json_bytes(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = 2usize; // top-level object braces
    let field_count = 8usize;

    total = total.saturating_add(json_field_bytes(
        "type",
        json_string_serialized_bytes(&meta.diagram_type),
    ));
    total = total.saturating_add(json_field_bytes(
        "title",
        json_optional_string_bytes(model.title.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "accTitle",
        json_optional_string_bytes(model.acc_title.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "accDescr",
        json_optional_string_bytes(model.acc_descr.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "root",
        estimate_nested_node_tree_bytes(model, class_style_bytes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "nodes",
        estimate_flat_node_array_bytes(model, class_style_bytes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "classes",
        estimate_classes_value_bytes(&model.classes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "config",
        json_value_serialized_bytes(meta.effective_config.as_value(), control)?,
    ));
    total = total.saturating_add(field_count.saturating_sub(1));
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn estimate_nested_node_tree_bytes(
    model: &TreemapDiagramRenderModel,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = 0usize;
    let mut stack = vec![(&model.root, None)];
    let mut visited = 0usize;
    while let Some((node, id)) = stack.pop() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        total = total.saturating_add(estimate_node_map_bytes(
            node,
            model.compiled_styles(id),
            true,
            None,
            class_style_bytes,
            control,
        )?);
        if let Some(children) = node.children.as_ref() {
            stack.extend(
                children
                    .iter()
                    .rev()
                    .map(|&id| (&model.nodes[id], Some(id))),
            );
        }
    }
    control.checkpoint()?;
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn estimate_flat_node_array_bytes(
    model: &TreemapDiagramRenderModel,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let children = model.root.children.as_deref().unwrap_or_default();
    let mut total = 2usize; // array brackets; commas are added per emitted item
    let mut stack = children
        .iter()
        .rev()
        .map(|&id| (id, 0usize))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some((id, level)) = stack.pop() {
        let node = &model.nodes[id];
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        if visited > 1 {
            total = total.saturating_add(1);
        }
        total = total.saturating_add(estimate_node_map_bytes(
            node,
            model.compiled_styles(Some(id)),
            false,
            Some(level),
            class_style_bytes,
            control,
        )?);
        if let Some(children) = node.children.as_ref() {
            stack.extend(
                children
                    .iter()
                    .rev()
                    .map(|&id| (id, level.saturating_add(1))),
            );
        }
    }
    control.checkpoint()?;
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn estimate_node_map_bytes(
    node: &TreemapNode,
    compiled_styles: Option<&[String]>,
    include_children: bool,
    level: Option<usize>,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = 2usize; // object braces
    let mut fields = 1usize; // name
    total = total.saturating_add(json_field_bytes(
        "name",
        json_string_serialized_bytes(&node.name),
    ));

    if include_children && let Some(children) = node.children.as_ref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "children",
            json_array_overhead(children.len()),
        ));
    }
    if let Some(value) = node.value.as_ref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "value",
            json_value_serialized_bytes(value, control)?,
        ));
    }
    if let Some(class_selector) = node.class_selector.as_deref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "classSelector",
            json_string_serialized_bytes(class_selector),
        ));
    }
    if let Some(styles) = compiled_styles {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "cssCompiledStyles",
            estimate_string_array_bytes(styles, control)?,
        ));
    } else if let Some(selector) = node.class_selector.as_deref()
        && let Some(&styles_bytes) = class_style_bytes.get(selector)
    {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes("cssCompiledStyles", styles_bytes));
    }
    if let Some(level) = level {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes("level", level.to_string().len()));
    }

    Ok(total.saturating_add(fields.saturating_sub(1)))
}

#[cfg(feature = "compat-json")]
fn estimate_class_style_arrays<'a>(
    classes: &'a std::collections::BTreeMap<String, TreemapClassDef>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::collections::HashMap<&'a str, usize>> {
    let mut class_style_bytes = std::collections::HashMap::with_capacity(classes.len());
    for (index, (id, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if !class_def.styles.is_empty() {
            class_style_bytes.insert(
                id.as_str(),
                json_array_overhead(1).saturating_add(
                    joined_style_json_content_bytes(&class_def.styles, control)?.saturating_add(2),
                ),
            );
        }
    }
    Ok(class_style_bytes)
}

#[cfg(feature = "compat-json")]
fn estimate_classes_value_bytes(
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = json_object_overhead(classes.len());
    for (index, (name, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let mut class_total = 2usize; // class definition object braces
        class_total = class_total.saturating_add(json_field_bytes(
            "id",
            json_string_serialized_bytes(&class_def.id),
        ));
        class_total = class_total.saturating_add(json_field_bytes(
            "styles",
            estimate_string_array_bytes(&class_def.styles, control)?,
        ));
        class_total = class_total.saturating_add(json_field_bytes(
            "textStyles",
            estimate_string_array_bytes(&class_def.text_styles, control)?,
        ));
        class_total = class_total.saturating_add(2); // three class fields
        total = total.saturating_add(json_string_serialized_bytes(name));
        total = total.saturating_add(1); // map entry colon
        total = total.saturating_add(class_total);
    }
    control.checkpoint()?;
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn estimate_string_array_bytes(
    values: &[String],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = json_array_overhead(values.len());
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        total = total.saturating_add(json_string_serialized_bytes(value));
    }
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn joined_style_json_content_bytes(
    styles: &[String],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    let mut total = 0usize;
    for (index, style) in styles.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if index != 0 {
            total = total.saturating_add(json_string_content_bytes(";"));
        }
        total = total.saturating_add(json_string_content_bytes(style));
    }
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn json_value_serialized_bytes(
    value: &Value,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<usize> {
    enum Task<'a> {
        Value(&'a Value),
        Array {
            values: &'a [Value],
            index: usize,
        },
        Object {
            entries: serde_json::map::Iter<'a>,
            remaining: usize,
        },
    }

    let mut total = 0usize;
    let mut tasks = vec![Task::Value(value)];
    let mut visited = 0usize;
    while let Some(task) = tasks.pop() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        match task {
            Task::Value(value) => match value {
                Value::Null => total = total.saturating_add(4),
                Value::Bool(value) => {
                    total = total.saturating_add(if *value { 4 } else { 5 });
                }
                Value::Number(value) => total = total.saturating_add(value.to_string().len()),
                Value::String(value) => {
                    total = total.saturating_add(json_string_serialized_bytes(value));
                }
                Value::Array(values) => {
                    total = total.saturating_add(json_array_overhead(values.len()));
                    tasks.push(Task::Array { values, index: 0 });
                }
                Value::Object(values) => {
                    total = total.saturating_add(json_object_overhead(values.len()));
                    tasks.push(Task::Object {
                        entries: values.iter(),
                        remaining: values.len(),
                    });
                }
            },
            Task::Array { values, index } => {
                if let Some(value) = values.get(index) {
                    tasks.push(Task::Array {
                        values,
                        index: index.saturating_add(1),
                    });
                    tasks.push(Task::Value(value));
                }
            }
            Task::Object {
                mut entries,
                remaining,
            } => {
                if remaining != 0
                    && let Some((key, value)) = entries.next()
                {
                    total = total
                        .saturating_add(json_string_serialized_bytes(key))
                        .saturating_add(1);
                    tasks.push(Task::Object {
                        entries,
                        remaining: remaining.saturating_sub(1),
                    });
                    tasks.push(Task::Value(value));
                }
            }
        }
    }
    Ok(total)
}

#[cfg(feature = "compat-json")]
fn json_string_content_bytes(value: &str) -> usize {
    value.bytes().fold(0usize, |total, byte| {
        total.saturating_add(match byte {
            b'"' | b'\\' | 0x08 | 0x09 | 0x0a | 0x0c | 0x0d => 2,
            0x00..=0x1f => 6,
            _ => 1,
        })
    })
}

#[cfg(feature = "compat-json")]
fn json_string_serialized_bytes(value: &str) -> usize {
    json_string_content_bytes(value).saturating_add(2)
}

#[cfg(feature = "compat-json")]
fn json_optional_string_bytes(value: Option<&str>) -> usize {
    value.map_or(4, json_string_serialized_bytes)
}

#[cfg(feature = "compat-json")]
fn json_field_bytes(key: &str, value_bytes: usize) -> usize {
    json_string_serialized_bytes(key)
        .saturating_add(1)
        .saturating_add(value_bytes)
}

#[cfg(feature = "compat-json")]
fn json_array_overhead(item_count: usize) -> usize {
    2usize.saturating_add(item_count.saturating_sub(1))
}

#[cfg(feature = "compat-json")]
fn json_object_overhead(field_count: usize) -> usize {
    2usize.saturating_add(field_count.saturating_sub(1))
}

#[cfg(feature = "compat-json")]
fn treemap_compatibility_output_budget_error(
    meta: &ParseMetadata,
    estimated_bytes: usize,
) -> Error {
    Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        format!(
            "treemap compatibility JSON output budget exceeded: estimated compatibility JSON requires {estimated_bytes} bytes, maximum is {TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES} bytes"
        ),
    )
}

#[cfg(feature = "compat-json")]
fn render_node_to_json_controlled(
    node: &TreemapNode,
    compiled_styles: Option<&[String]>,
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::ManagedSemanticJson> {
    control.checkpoint()?;
    let mut out = crate::ManagedSemanticJson::from_value(Value::Object(Map::new()));
    let fields = out.as_value_mut().as_object_mut().expect("node object");
    fields.insert("name".to_string(), Value::String(node.name.clone()));
    if let Some(value) = &node.value {
        fields.insert(
            "value".to_string(),
            crate::json_value::clone_value_nonrecursive_with_control(value, control)?,
        );
    }
    if let Some(class_selector) = &node.class_selector {
        fields.insert(
            "classSelector".to_string(),
            Value::String(class_selector.clone()),
        );
    }
    if let Some(styles) = compiled_styles {
        fields.insert(
            "cssCompiledStyles".to_string(),
            Value::Array(clone_string_array_with_control(styles, control)?),
        );
    } else if let Some(selector) = node.class_selector.as_deref()
        && let Some(class_def) = classes.get(selector)
    {
        let styles = join_styles_with_control(&class_def.styles, control)?;
        if !styles.is_empty() {
            fields.insert(
                "cssCompiledStyles".to_string(),
                Value::Array(vec![Value::String(styles)]),
            );
        }
    }
    Ok(out)
}

#[cfg(feature = "compat-json")]
impl TreemapDiagramRenderModel {
    fn validate_child_relationships_controlled(
        &self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<std::result::Result<(), &'static str>> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![&self.root];
        while let Some(node) = stack.pop() {
            control.checkpoint()?;
            if let Some(children) = &node.children {
                for &child in children.iter().rev() {
                    control.checkpoint()?;
                    let Some(record) = self.nodes.get(child) else {
                        return Ok(Err("treemap child ID is out of range"));
                    };
                    if std::mem::replace(&mut seen[child], true) {
                        return Ok(Err("cyclic or repeated treemap child relationship"));
                    }
                    stack.push(record);
                }
            }
        }
        Ok(Ok(()))
    }

    fn project_root_controlled(
        &self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<std::result::Result<crate::ManagedSemanticJson, &'static str>>
    {
        let root_id = self.nodes.len();
        let mut completed = vec![None::<crate::ManagedSemanticJson>; root_id + 1];
        let mut seen = vec![false; root_id + 1];
        let mut stack = vec![(root_id, false)];
        while let Some((id, expanded)) = stack.pop() {
            control.checkpoint()?;
            let node = if id == root_id {
                &self.root
            } else {
                &self.nodes[id]
            };
            if expanded {
                let mut projected = render_node_to_json_controlled(
                    node,
                    self.compiled_styles((id != root_id).then_some(id)),
                    &self.classes,
                    control,
                )?;
                let mut children = node.children.as_ref().map(|_| Vec::new());
                if let Some(ids) = &node.children {
                    for &child in ids {
                        control.checkpoint()?;
                        let Some(value) = completed[child].take() else {
                            return Ok(Err("invalid treemap child relationship"));
                        };
                        children.as_mut().expect("children slot exists").push(value);
                    }
                }
                if let Some(children) = children {
                    projected
                        .as_value_mut()
                        .as_object_mut()
                        .expect("node projection is an object")
                        .insert(
                            "children".to_string(),
                            Value::Array(
                                children
                                    .into_iter()
                                    .map(crate::ManagedSemanticJson::into_unmanaged_value)
                                    .collect(),
                            ),
                        );
                }
                completed[id] = Some(projected);
            } else {
                if std::mem::replace(&mut seen[id], true) {
                    return Ok(Err("cyclic or repeated treemap child relationship"));
                }
                stack.push((id, true));
                if let Some(children) = &node.children {
                    for &child in children.iter().rev() {
                        control.checkpoint()?;
                        if child >= root_id {
                            return Ok(Err("treemap child ID is out of range"));
                        }
                        stack.push((child, false));
                    }
                }
            }
        }
        control.checkpoint()?;
        Ok(Ok(completed[root_id]
            .take()
            .expect("root projection completed")))
    }
}

#[cfg(feature = "compat-json")]
fn render_classes_to_value(
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Value> {
    let mut out = Map::new();
    for (index, (name, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let mut class = Map::new();
        class.insert("id".to_string(), Value::String(class_def.id.clone()));
        class.insert(
            "styles".to_string(),
            Value::Array(clone_string_array_with_control(&class_def.styles, control)?),
        );
        class.insert(
            "textStyles".to_string(),
            Value::Array(clone_string_array_with_control(
                &class_def.text_styles,
                control,
            )?),
        );
        out.insert(name.clone(), Value::Object(class));
    }
    control.checkpoint()?;
    Ok(Value::Object(out))
}

#[cfg(feature = "compat-json")]
fn clone_string_array_with_control(
    values: &[String],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Vec<Value>> {
    let mut cloned = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        cloned.push(Value::String(value.clone()));
    }
    Ok(cloned)
}

#[cfg(feature = "compat-json")]
fn join_styles_with_control(
    styles: &[String],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<String> {
    let mut joined = String::new();
    for (index, style) in styles.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if index != 0 {
            joined.push(';');
        }
        joined.push_str(style);
    }
    Ok(joined)
}

impl TreemapSemanticSource {
    fn into_semantic_model_controlled(
        self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<TreemapDiagramModel> {
        let mut classes = std::collections::BTreeMap::new();
        for (id, class) in self.class_defs {
            control.checkpoint()?;
            classes.insert(id, class);
        }
        control.checkpoint()?;
        Ok(TreemapDiagramModel {
            title: self.title,
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            root: TreemapNode {
                children: Some(self.roots),
                ..Default::default()
            },
            nodes: self.arena.nodes,
            classes,
        })
    }

    #[cfg(feature = "presentation")]
    fn into_render_model_controlled(
        self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<TreemapDiagramRenderModel> {
        #[cfg(feature = "compat-json")]
        let compatibility_empty = !self.present;
        let semantic = self.into_semantic_model_controlled(control)?;
        Ok(TreemapDiagramRenderModel {
            semantic,
            visuals: TreemapVisuals::default(),
            #[cfg(feature = "compat-json")]
            compatibility_empty,
        })
    }
}

#[cfg(feature = "presentation")]
fn parse_treemap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapSemanticSource> {
    construct_treemap_semantic_source(code, meta)
}

#[cfg(feature = "presentation")]
fn construct_treemap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapSemanticSource> {
    let control = crate::OperationControl::new();
    let parsed = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &control,
    )
    .expect("a private parse control cannot be cancelled")
    .into_strict(&meta.diagram_type)?;
    Ok(
        treemap_semantic_source_from_parsed_controlled(parsed, &control)
            .expect("a private parse control cannot be cancelled"),
    )
}

pub(crate) fn parse_treemap_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<TreemapDiagramModel>> {
    let TreemapParseOutcome {
        parsed,
        first_issue,
        ..
    } = parse_treemap_outcome_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let mut parsed = parsed;
    #[cfg(feature = "editor-facts")]
    let editor_facts = matches!(editor_demand, EditorFactsDemand::Capture)
        .then(|| std::mem::take(&mut parsed.editor_facts));
    let model = match first_issue {
        Some(issue) => Err(treemap_error(&meta.diagram_type, issue.message, issue.span)),
        None => {
            let source = treemap_semantic_source_from_parsed_controlled(parsed, control)?;
            let mut model = source.into_semantic_model_controlled(control)?;
            model.sanitize_common_db_fields(&meta.semantic_config);
            Ok(model)
        }
    };
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn treemap_semantic_source_from_parsed_controlled(
    parsed: TreemapParsedInput,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<TreemapSemanticSource> {
    let class_defs = class_defs_from_rows_controlled(&parsed.rows, control)?;
    let flat_items = flat_items_from_rows_controlled(&parsed.rows, control)?;
    let (arena, roots) = build_hierarchy_controlled(&flat_items, control)?;
    control.checkpoint()?;
    Ok(TreemapSemanticSource {
        present: parsed.present,
        title: parsed.title,
        acc_title: parsed.acc_title,
        acc_descr: parsed.acc_descr,
        class_defs,
        arena,
        roots,
        #[cfg(feature = "editor-facts")]
        editor_facts: parsed.editor_facts,
    })
}

fn parse_treemap_outcome_controlled(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<TreemapParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("treemap");

    let mut outcome = TreemapParseOutcome {
        #[cfg(feature = "editor-facts")]
        editor_demand,
        parsed: TreemapParsedInput {
            present: false,
            title: None,
            acc_title: None,
            acc_descr: None,
            rows: Vec::new(),
            #[cfg(feature = "editor-facts")]
            editor_facts: EditorSemanticFacts::new(),
        },
        first_issue: None,
    };
    let body = match treemap_body_start_controlled(code, control)? {
        Ok(Some(body)) => body,
        Ok(None) => return Ok(outcome),
        Err(issue) => {
            outcome.record_issue(issue.message, issue.span);
            return Ok(outcome);
        }
    };
    outcome.parsed.present = true;

    let mut offset = body.offset;
    let mut saw_statement = false;
    let mut trailing_whitespace_span = None;

    while offset < code.len() {
        control.checkpoint()?;
        if let Some(parsed) = parse_langium_common(code, offset) {
            saw_statement = true;
            trailing_whitespace_span = parsed.trailing_whitespace_span;
            let field = parsed.fact.field;
            let value = parsed.fact.value.clone();
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                push_langium_common_editor_fact(
                    &mut outcome.parsed.editor_facts,
                    &parsed.fact,
                    "treemap",
                );
            }
            match field {
                LangiumCommonField::Title => outcome.parsed.title = Some(value),
                LangiumCommonField::AccTitle => outcome.parsed.acc_title = Some(value),
                LangiumCommonField::AccDescr => outcome.parsed.acc_descr = Some(value),
            }
            if let Some(diagnostic) = parsed.diagnostic {
                outcome.record_issue(diagnostic.message, Some(diagnostic.span));
            }
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line_at(code, offset);
        if line.trim_start().starts_with("%%") {
            offset = next_offset;
            continue;
        }
        let visible = strip_inline_comment_aware(line);
        if visible.trim().is_empty() {
            if saw_statement && !visible.is_empty() {
                trailing_whitespace_span = Some(SourceSpan::new(offset, offset + line.len()));
            }
            offset = next_offset;
            continue;
        }
        saw_statement = true;
        trailing_whitespace_span = None;

        let (indent, rest_with_trailing) = split_ascii_indent(visible);
        let rest = rest_with_trailing.trim_end();
        let statement_start = offset + visible.len().saturating_sub(rest_with_trailing.len());
        let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());

        match parse_class_def(rest, statement_start) {
            Ok(Some(class_def)) => {
                let style_issue = class_def.style_text.as_ref().and_then(|style| {
                    validate_class_def_style(&style.text)
                        .err()
                        .map(|message| (message, style.span))
                });
                let row = TreemapRow::ClassDef(class_def);
                #[cfg(feature = "editor-facts")]
                if editor_demand == EditorFactsDemand::Capture {
                    push_treemap_row_editor_facts(&mut outcome.parsed.editor_facts, &row);
                }
                outcome.parsed.rows.push(row);
                if let Some((message, span)) = style_issue {
                    outcome.record_issue(message, Some(span));
                }
            }
            Ok(None) => match parse_item_row(indent, rest, statement_start) {
                Ok(item) => {
                    let row = TreemapRow::Item(item);
                    #[cfg(feature = "editor-facts")]
                    if editor_demand == EditorFactsDemand::Capture {
                        push_treemap_row_editor_facts(&mut outcome.parsed.editor_facts, &row);
                    }
                    outcome.parsed.rows.push(row);
                }
                Err(message) => {
                    outcome.record_issue(message, Some(statement_span));
                }
            },
            Err(message) => {
                outcome.record_issue(message, Some(statement_span));
            }
        }
        offset = next_offset;
    }

    if let Some(span) = trailing_whitespace_span {
        outcome.record_issue("unexpected trailing whitespace-only line", Some(span));
    }

    control.checkpoint()?;
    Ok(outcome)
}

#[derive(Debug, Clone, Copy)]
struct TreemapBodyStart {
    offset: usize,
}

fn treemap_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::result::Result<Option<TreemapBodyStart>, TreemapParseIssue>>
{
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line_at(code, offset);
        let visible = strip_inline_comment_aware(line);
        let trimmed = visible.trim();
        if trimmed.is_empty() {
            offset = next_offset;
            continue;
        }
        let leading = visible.len().saturating_sub(visible.trim_start().len());
        let span = SourceSpan::new(offset + leading, offset + leading + trimmed.len());
        if !is_treemap_header(trimmed) {
            return Ok(Err(TreemapParseIssue {
                message: "expected treemap".to_string(),
                span: Some(span),
            }));
        }
        return Ok(Ok(Some(TreemapBodyStart {
            offset: next_offset,
        })));
    }
    Ok(Ok(None))
}

fn treemap_error(
    diagram_type: &str,
    message: impl Into<String>,
    span: Option<SourceSpan>,
) -> Error {
    let message = message.into();
    match span {
        Some(span) => Error::diagram_parse_exact(diagram_type, &message, span),
        None => Error::diagram_parse_fallback(diagram_type, &message),
    }
}

fn class_defs_from_rows_controlled(
    rows: &[TreemapRow],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::collections::HashMap<String, StyleClassDef>> {
    let mut class_defs: std::collections::HashMap<String, StyleClassDef> =
        std::collections::HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let TreemapRow::ClassDef(c) = row else {
            continue;
        };
        add_class(
            &mut class_defs,
            &c.class_name.text,
            c.style_text
                .as_ref()
                .map(|style| style.text.as_str())
                .unwrap_or(""),
        );
    }

    control.checkpoint()?;
    Ok(class_defs)
}

fn flat_items_from_rows_controlled(
    rows: &[TreemapRow],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Vec<FlatItem>> {
    let mut flat_items: Vec<FlatItem> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let TreemapRow::Item(item) = row else {
            continue;
        };

        flat_items.push(FlatItem {
            level: item.indent,
            name: item.name.text.clone(),
            item_type: item.item_type,
            value: item.value.as_ref().map(|value| value.value.clone()),
            class_selector: item
                .class_selector
                .as_ref()
                .map(|selector| selector.text.clone()),
        });
    }

    control.checkpoint()?;
    Ok(flat_items)
}

#[derive(Debug, Clone)]
struct FlatItem {
    level: usize,
    name: String,
    item_type: ItemType,
    value: Option<Value>,
    class_selector: Option<String>,
}

#[cfg(test)]
fn build_hierarchy(items: &[FlatItem]) -> (Arena, Vec<usize>) {
    build_hierarchy_controlled(items, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

fn build_hierarchy_controlled(
    items: &[FlatItem],
    control: &crate::OperationControl,
) -> crate::OperationControlResult<(Arena, Vec<usize>)> {
    if items.is_empty() {
        return Ok((Arena { nodes: Vec::new() }, Vec::new()));
    }

    let mut arena = Arena { nodes: Vec::new() };
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (node_idx, item.level)

    for (index, item) in items.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let mut node = TreemapNode {
            name: item.name.clone(),
            value: None,
            class_selector: item.class_selector.clone(),
            children: match item.item_type {
                ItemType::Leaf => None,
                ItemType::Section => Some(Vec::new()),
            },
        };
        if item.item_type == ItemType::Leaf {
            node.value = item
                .value
                .as_ref()
                .map(crate::json_value::ManagedJson::from);
        }

        let idx = arena.push(node);

        while stack.last().is_some_and(|(_, lvl)| *lvl >= item.level) {
            control.checkpoint()?;
            stack.pop();
        }

        if stack.is_empty() {
            roots.push(idx);
        } else {
            let parent_idx = stack[stack.len() - 1].0;
            let parent = &mut arena.nodes[parent_idx];
            if let Some(children) = parent.children.as_mut() {
                children.push(idx);
            } else {
                parent.children = Some(vec![idx]);
            }
        }

        if item.item_type != ItemType::Leaf {
            stack.push((idx, item.level));
        }
    }

    control.checkpoint()?;
    Ok((arena, roots))
}

fn add_class(
    classes: &mut std::collections::HashMap<String, StyleClassDef>,
    id: &str,
    style: &str,
) {
    let style_class = classes
        .entry(id.to_string())
        .or_insert_with(|| StyleClassDef {
            id: id.to_string(),
            styles: Vec::new(),
            text_styles: Vec::new(),
        });

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for s in replaced.split(';') {
        if is_label_style_bug_compatible(s) {
            style_class.text_styles.push(s.to_string());
        }
        style_class.styles.push(s.to_string());
    }
}

fn validate_class_def_style(style: &str) -> std::result::Result<(), String> {
    let style = style.trim().trim_end_matches(';').trim();
    if style.is_empty() {
        return Ok(());
    }

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for raw in replaced.split(';') {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        let Some((k, v)) = s.split_once(':') else {
            return Err(format!("invalid classDef style token `{s}`"));
        };
        if k.trim().is_empty() || v.trim().is_empty() {
            return Err(format!("invalid classDef style token `{s}`"));
        }
    }

    Ok(())
}

fn is_label_style_bug_compatible(s: &str) -> bool {
    matches!(
        s.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_quote: Option<char> = None;

    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            continue;
        }

        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }

    line
}

fn is_treemap_header(line: &str) -> bool {
    let t = line.trim_start();
    t == "treemap" || t == "treemap-beta"
}

fn parse_class_def(
    line: &str,
    line_start: usize,
) -> std::result::Result<Option<ClassDefStatement>, String> {
    let Some(after_keyword) = line.strip_prefix("classDef") else {
        return Ok(None);
    };
    if after_keyword
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return Ok(None);
    }
    if after_keyword.is_empty() {
        return Err("expected class name".to_string());
    }

    let class_input = after_keyword.trim_start();
    let class_start = line.len().saturating_sub(class_input.len());
    let (class_name, tail) =
        parse_id2(class_input).ok_or_else(|| "expected class name".to_string())?;
    let class_name_span = SourceSpan::new(
        line_start + class_start,
        line_start + class_start + class_name.len(),
    );

    let style_input = tail.trim_start();
    let style_input_start = line.len().saturating_sub(style_input.len());
    let semicolon = style_input.find(';');
    let (style_raw, trailing) = match semicolon {
        Some(semi) => (&style_input[..semi], &style_input[semi + 1..]),
        None => (style_input, ""),
    };
    let style = style_raw.trim();
    let style_text = if style.is_empty() {
        None
    } else {
        let leading = leading_whitespace_len(style_raw);
        let start = line_start + style_input_start + leading;
        Some(SpannedText {
            text: style.to_string(),
            span: SourceSpan::new(start, start + style.len()),
        })
    };
    if !trailing.trim().is_empty() {
        return Err("unexpected tokens after classDef".to_string());
    }

    Ok(Some(ClassDefStatement {
        class_name: SpannedText {
            text: class_name,
            span: class_name_span,
        },
        style_text,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(line_start, line_start + line.len()),
    }))
}

fn parse_item_row(
    indent: usize,
    line: &str,
    line_start: usize,
) -> std::result::Result<ItemRow, String> {
    let mut p = Parser::new(line);
    p.skip_ws();
    let name_token_start = p.pos;
    let name_text = p
        .parse_string2()
        .ok_or_else(|| "expected quoted string".to_string())?;
    let name_token_end = p.pos;
    let quote_len = line[name_token_start..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or_default();
    let name = SpannedText {
        text: name_text,
        span: SourceSpan::new(
            line_start + name_token_start + quote_len,
            line_start + name_token_end.saturating_sub(quote_len),
        ),
    };
    p.skip_ws();

    // Section: "Name" (:::class)?
    if p.try_consume_str(":::") {
        p.skip_ws();
        let selector_start = p.pos;
        let (cls, _) = parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
        p.pos += cls.len();
        let selector_span = SourceSpan::new(
            line_start + selector_start,
            line_start + selector_start + cls.len(),
        );
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected tokens after section".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: Some(SpannedText {
                span: selector_span,
                text: cls,
            }),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    // Leaf: "Name" : 10 (:::class)?
    if p.try_consume(':') || p.try_consume(',') {
        p.skip_ws();
        #[cfg(feature = "editor-facts")]
        let value_start = p.pos;
        let token = p
            .parse_number2_token()
            .ok_or_else(|| "expected number".to_string())?;
        #[cfg(feature = "editor-facts")]
        let value_span = SourceSpan::new(
            line_start + value_start,
            line_start + value_start + token.len(),
        );
        let Some(value) = parse_number2_value(&token) else {
            return Err("expected number".to_string());
        };
        p.skip_ws();
        let mut class_selector = None;
        if p.try_consume_str(":::") {
            p.skip_ws();
            let selector_start = p.pos;
            let (cls, _) =
                parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
            p.pos += cls.len();
            let selector_span = SourceSpan::new(
                line_start + selector_start,
                line_start + selector_start + cls.len(),
            );
            class_selector = Some(SpannedText {
                span: selector_span,
                text: cls,
            });
            p.skip_ws();
        }
        if !p.eof() {
            return Err("unexpected tokens after leaf".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Leaf,
            value: Some(SpannedValue {
                value,
                #[cfg(feature = "editor-facts")]
                source: SpannedText {
                    text: token.clone(),
                    span: value_span,
                },
            }),
            class_selector,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    if p.eof() {
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: None,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    Err("expected ':' or ':::' or end of line".to_string())
}

fn parse_id2(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut idx = first.len_utf8();
    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    Some((input[..idx].to_string(), &input[idx..]))
}

fn parse_number2_value(token: &str) -> Option<Value> {
    let no_commas: String = token.chars().filter(|c| *c != ',').collect();
    let mut saw_dot = false;
    let mut cut = 0usize;
    for ch in no_commas.chars() {
        if ch.is_ascii_digit() {
            cut += ch.len_utf8();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            cut += 1;
            continue;
        }
        break;
    }
    if cut == 0 {
        return None;
    }
    let prefix = &no_commas[..cut];

    if saw_dot {
        let frac = prefix.split_once('.').map(|(_, b)| b).unwrap_or("");
        if frac.is_empty() || frac.chars().all(|c| c == '0') {
            let int_part = prefix.split_once('.').map(|(a, _)| a).unwrap_or(prefix);
            let i: i64 = int_part.parse().ok()?;
            return Some(Value::Number(i.into()));
        }
        let f: f64 = prefix.parse().ok()?;
        let n = serde_json::Number::from_f64(f)?;
        return Some(Value::Number(n));
    }

    let i: i64 = prefix.parse().ok()?;
    Some(Value::Number(i.into()))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
                continue;
            }
            break;
        }
    }

    fn try_consume(&mut self, ch: char) -> bool {
        if self.rest().starts_with(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn try_consume_str(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_string2(&mut self) -> Option<String> {
        let rest = self.rest();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut idx = 1usize;
        for ch in rest[1..].chars() {
            idx += ch.len_utf8();
            if ch == quote {
                let inner = &rest[1..idx - 1];
                self.pos += idx;
                return Some(inner.to_string());
            }
        }
        None
    }

    fn parse_number2_token(&mut self) -> Option<String> {
        let mut idx = 0usize;
        for ch in self.rest().chars() {
            if ch.is_ascii_digit() || ch == '_' || ch == '.' || ch == ',' {
                idx += ch.len_utf8();
                continue;
            }
            break;
        }
        if idx == 0 {
            return None;
        }
        let token = &self.rest()[..idx];
        self.pos += idx;
        Some(token.to_string())
    }
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) mod tests {
    use super::*;
    use crate::{
        Engine, ManagedSemanticJson, MermaidConfig, ParseMetadata, ParseOptions,
        RenderSemanticModel,
    };

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "treemap".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn treemap_ordinary_projections_do_not_collect_editor_facts() {
        let source = r#"treemap
"Leaf": 1
"#;
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_treemap_model_for_render_controlled(source, &meta(), &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let projected = crate::ManagedSemanticJson::from(parse_treemap(source, &meta()).unwrap());
        assert!(projected.as_value().is_object());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn treemap_accepts_treemap_beta_header() {
        let model = parse("treemap-beta\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_accepts_treemap_header() {
        let model = parse("treemap\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = Engine::new();
        let input = r#"treemap
title Treemap Title
accTitle: Treemap accTitle
accDescr: Treemap accDescr
"Root"
  "Leaf": 42
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.metadata().diagram_type, "treemap");
        match parsed.model() {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(model.title.as_deref(), Some("Treemap Title"));
                assert_eq!(model.acc_title.as_deref(), Some("Treemap accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Treemap accDescr"));
                assert_eq!(model.root.name, "");
                let root_children = model.root.children.as_ref().unwrap();
                assert_eq!(root_children.len(), 1);
                let root = &model.nodes[root_children[0]];
                assert_eq!(root.name, "Root");
                assert_eq!(model.nodes[root.children.as_ref().unwrap()[0]].name, "Leaf");
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("treemap"));
        assert_eq!(parsed_json.model["title"], json!("Treemap Title"));
        assert_eq!(parsed_json.model["accTitle"], json!("Treemap accTitle"));
        assert_eq!(
            parsed_json.model["root"]["children"][0]["name"],
            json!("Root")
        );
        assert_eq!(
            parsed_json.model["root"]["children"][0]["children"][0]["value"],
            json!(42)
        );
        assert!(parsed_json.model.get("config").is_some());
    }

    fn parse_error(text: &str) -> String {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        err.to_string()
    }

    fn deep_treemap_chain(depth: usize) -> String {
        let mut input = String::from("treemap\n");
        for level in 0..depth {
            input.push_str(&" ".repeat(level));
            input.push('"');
            input.push_str(&format!("section{level}"));
            input.push_str("\"\n");
        }
        input.push_str(&" ".repeat(depth));
        input.push_str("\"leaf\": 1\n");
        input
    }

    #[test]
    fn treemap_deep_chain_semantic_and_render_model_use_heap_traversal() {
        const DEPTH: usize = 1200;
        let input = deep_treemap_chain(DEPTH);

        let model = parse(&input);
        let nodes = model["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), DEPTH + 1);
        assert_eq!(nodes[0]["name"], json!("section0"));
        assert_eq!(
            nodes
                .last()
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str),
            Some("leaf")
        );

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        match parsed.model() {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(model.nodes.len() + 1, DEPTH + 2);
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }
    }

    #[test]
    fn treemap_errors_on_trailing_whitespace_only_line() {
        let msg = parse_error("treemap\n\"A\": 1\n    \n");
        assert!(
            msg.contains("unexpected trailing whitespace-only line"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_trailing_comment_is_not_a_whitespace_only_line() {
        let model = parse("treemap\n\"A\": 1\n    %% trailing comment\n");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_rejects_header_with_colon() {
        let msg = parse_error("treemap:\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_rejects_header_with_suffix_tokens() {
        let msg = parse_error("treemap utilities\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_allows_whitespace_only_lines_in_the_middle() {
        let model = parse("treemap\n\"A\": 1\n    \n\"B\": 2\n");
        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn treemap_does_not_treat_unicode_nbsp_as_indentation() {
        let model = parse("treemap\n\"Root\"\n\u{00A0}\"Leaf\": 1\n");
        let children = model["root"]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["name"], json!("Root"));
        assert_eq!(children[1]["name"], json!("Leaf"));
    }

    #[test]
    fn treemap_parses_basic_hierarchy_from_docs() {
        let model = parse(
            r#"treemap-beta
"Section 1"
    "Leaf 1.1": 12
    "Section 1.2"
      "Leaf 1.2.1": 12
"Section 2"
    "Leaf 2.1": 20
    "Leaf 2.2": 25
"#,
        );

        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
        assert_eq!(model["root"]["children"][0]["name"], json!("Section 1"));
        assert_eq!(
            model["root"]["children"][0]["children"][0]["name"],
            json!("Leaf 1.1")
        );
        assert_eq!(
            model["root"]["children"][0]["children"][0]["value"],
            json!(12)
        );
        assert_eq!(model["root"]["children"][1]["name"], json!("Section 2"));
        assert_eq!(
            model["root"]["children"][1]["children"][1]["value"],
            json!(25)
        );
    }

    #[test]
    fn treemap_classdef_applies_compiled_styles() {
        let model = parse(
            r#"treemap-beta
"Main":::important
  "A": 20

classDef important fill:#f96,stroke:#333,stroke-width:2px;
"#,
        );
        assert_eq!(
            model["classes"]["important"]["styles"][0],
            json!("fill:#f96")
        );
        assert_eq!(
            model["root"]["children"][0]["cssCompiledStyles"][0],
            json!("fill:#f96;stroke:#333;stroke-width:2px")
        );
    }

    #[test]
    fn treemap_duplicate_classdefs_append_styles_in_source_order() {
        let model = parse(
            r#"treemap
"Root":::important
classDef important fill:#f96
classDef important stroke:#333
"Leaf": 1:::important
"#,
        );

        assert_eq!(
            model["classes"]["important"]["styles"],
            json!(["fill:#f96", "stroke:#333"])
        );
        assert_eq!(
            model["root"]["children"][0]["cssCompiledStyles"],
            json!(["fill:#f96;stroke:#333"])
        );
    }

    fn model_with_shared_class_styles(
        node_count: usize,
        style_bytes: usize,
    ) -> (TreemapDiagramRenderModel, String) {
        let style = format!("fill:{}", "a".repeat(style_bytes.saturating_sub(5)));
        let nodes = (0..node_count)
            .map(|index| TreemapNode {
                name: format!("node-{index}"),
                value: Some(json!(1).into()),
                class_selector: Some("shared".to_string()),
                ..Default::default()
            })
            .collect();
        (
            TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
                root: TreemapNode {
                    children: Some((0..node_count).collect()),
                    ..Default::default()
                },
                nodes,
                classes: std::collections::BTreeMap::from([(
                    "shared".to_string(),
                    TreemapClassDef {
                        id: "shared".to_string(),
                        styles: vec![style.clone()],
                        text_styles: Vec::new(),
                    },
                )]),
                ..Default::default()
            }),
            style,
        )
    }

    #[test]
    fn treemap_compatibility_projection_accepts_bounded_shared_class_styles() {
        let (model, style) = model_with_shared_class_styles(512, 256);
        let output = render_model_to_compat_json(&model, &meta())
            .expect("bounded repeated classDef styles should remain compatible");

        assert_eq!(output["nodes"].as_array().unwrap().len(), 512);
        assert_eq!(output["nodes"][0]["cssCompiledStyles"], json!([style]));
        assert_eq!(
            output["root"]["children"][511]["cssCompiledStyles"],
            output["nodes"][511]["cssCompiledStyles"]
        );
    }

    #[test]
    fn treemap_compatibility_preflight_estimate_covers_serialized_output() {
        let model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            title: Some("title \"with\" controls\n".to_string()),
            acc_title: Some("acc".to_string()),
            acc_descr: Some("description".to_string()),
            root: TreemapNode {
                name: "root".to_string(),
                children: Some(vec![0]),
                value: None,
                class_selector: None,
            },
            nodes: vec![TreemapNode {
                name: "leaf \"\u{0001}".to_string(),
                children: None,
                value: Some(json!({"nested": ["value", true, null]}).into()),
                class_selector: Some("shared".to_string()),
            }],
            classes: std::collections::BTreeMap::from([(
                "shared".to_string(),
                TreemapClassDef {
                    id: "shared".to_string(),
                    styles: vec!["fill:\"quoted\"".to_string(), "stroke:#123".to_string()],
                    text_styles: vec!["font-size:12px".to_string()],
                },
            )]),
        });
        let meta = meta();
        let control = crate::OperationControl::new();
        let class_style_bytes = estimate_class_style_arrays(&model.classes, &control).unwrap();
        let estimate =
            estimate_compatibility_json_bytes(&model, &meta, &class_style_bytes, &control).unwrap();
        let output = render_model_to_compat_json(&model, &meta).unwrap();
        let actual = serde_json::to_vec(&output).unwrap().len();

        assert!(
            estimate >= actual,
            "preflight estimate {estimate} must cover serialized output {actual}"
        );
    }

    #[test]
    fn treemap_compatibility_projection_rejects_repeated_class_styles_before_value_build() {
        let (model, _) = model_with_shared_class_styles(2_048, 4_096);
        let error = render_model_to_compat_json(&model, &meta())
            .expect_err("repeated classDef styles beyond the output budget must fail");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured compatibility output budget error");
        };
        assert!(
            diagnostic
                .message()
                .contains("treemap compatibility JSON output budget exceeded"),
            "{}",
            diagnostic.message()
        );
        assert!(diagnostic.message().contains(&format!(
            "maximum is {TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES}"
        )));
    }

    #[test]
    fn treemap_compatibility_projection_rejects_large_nodes_before_value_build() {
        const NODE_COUNT: usize = 64_000;
        const NAME_BYTES: usize = 128;
        let name_suffix = "x".repeat(NAME_BYTES.saturating_sub(12));
        let nodes = (0..NODE_COUNT)
            .map(|index| TreemapNode {
                name: format!("node-{index:06}-{name_suffix}"),
                value: Some(json!(1).into()),
                ..Default::default()
            })
            .collect();
        let model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some((0..NODE_COUNT).collect()),
                ..Default::default()
            },
            nodes,
            ..Default::default()
        });

        let error = render_model_to_compat_json(&model, &meta())
            .expect_err("large nested and flat node output must fail before Value materialization");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured compatibility output budget error");
        };
        assert!(
            diagnostic
                .message()
                .contains("treemap compatibility JSON output budget exceeded"),
            "{}",
            diagnostic.message()
        );
        assert!(diagnostic.message().contains("estimated "));
    }

    #[test]
    fn treemap_compatibility_preflight_observes_cancellation() {
        let (model, _) = model_with_shared_class_styles(512, 256);
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            render_model_to_compat_json_controlled(&model, &meta(), &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn treemap_compatibility_projection_observes_cancellation_after_preflight() {
        let (model, _) = model_with_shared_class_styles(512, 256);
        let control = crate::OperationControl::new();
        treemap_compatibility_output_preflight(&model, &meta(), &control)
            .expect("preflight control should remain active")
            .expect("bounded model should pass preflight");
        control.cancel();

        assert!(matches!(
            render_model_to_compat_json_unchecked(&model, &meta(), &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn treemap_style_byte_preflight_observes_cancellation_inside_one_class() {
        let styles = (0..512)
            .map(|index| format!("fill:{index}"))
            .collect::<Vec<_>>();
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(1);

        assert!(matches!(
            joined_style_json_content_bytes(&styles, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_combined_compatibility_budget_error_preserves_editor_facts() {
        const NODE_COUNT: usize = 2_048;
        const STYLE_BYTES: usize = 4_096;
        let style = format!("fill:{}", "a".repeat(STYLE_BYTES.saturating_sub(5)));
        let mut text = String::from("treemap\n");
        for index in 0..NODE_COUNT {
            text.push_str(&format!("\"node-{index}\": 1:::shared\n"));
        }
        text.push_str("classDef shared ");
        text.push_str(&style);
        text.push('\n');

        let parsed =
            parse_treemap_json_and_editor_facts(&text, &meta(), &crate::OperationControl::new())
                .expect("budget rejection must remain a semantic result");
        let (model, facts, _) = parsed.into_parts();
        let error = model.expect_err("oversized compatibility output must not be truncated");
        assert!(
            error
                .to_string()
                .contains("treemap compatibility JSON output budget exceeded")
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "shared" && symbol.detail.as_deref() == Some("treemap class definition")
        }));
    }

    #[test]
    fn treemap_explicit_styles_override_shared_class_styles_in_both_projections() {
        let (mut model, _) = model_with_shared_class_styles(1, 32);
        for styles in [Vec::new(), vec!["fill:#123".to_string()]] {
            model.visuals.node_styles = vec![Some(styles.clone())];
            let output = render_model_to_compat_json(&model, &meta()).unwrap();
            assert_eq!(
                output["root"]["children"][0]["cssCompiledStyles"],
                json!(styles)
            );
            assert_eq!(output["nodes"][0]["cssCompiledStyles"], json!(styles));
            assert_eq!(output["nodes"][0]["classSelector"], json!("shared"));
        }
    }

    #[test]
    fn treemap_compatibility_preflight_rejects_repeated_relationships() {
        let (mut model, _) = model_with_shared_class_styles(1, 32);
        model.root.children = Some(vec![0, 0]);
        let error = treemap_compatibility_output_preflight(
            &model,
            &meta(),
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cyclic or repeated treemap child relationship")
        );
    }

    #[test]
    fn treemap_classdef_rejects_bare_label_style_tokens_like_mermaid_parser() {
        let msg = parse_error(
            r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#,
        );
        assert!(
            msg.contains("invalid classDef style token `color`"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_build_hierarchy_matches_upstream_utils_test() {
        let items = vec![
            FlatItem {
                level: 0,
                name: "Root".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
            },
            FlatItem {
                level: 4,
                name: "Branch 1".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 1.1".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(10)),
                class_selector: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 1.2".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(15)),
                class_selector: None,
            },
            FlatItem {
                level: 4,
                name: "Branch 2".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.1".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(20)),
                class_selector: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.2".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(25)),
                class_selector: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.3".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(30)),
                class_selector: None,
            },
        ];

        let (arena, roots) = build_hierarchy(&items);
        let model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some(roots),
                ..Default::default()
            },
            nodes: arena.nodes,
            ..Default::default()
        });
        let projected = render_model_to_compat_json(&model, &meta()).unwrap();
        let root_value = projected["root"]["children"].as_array().unwrap();
        assert_eq!(
            root_value,
            &vec![json!({
                "name": "Root",
                "children": [
                    {
                        "name": "Branch 1",
                        "children": [
                            { "name": "Leaf 1.1", "value": 10 },
                            { "name": "Leaf 1.2", "value": 15 },
                        ]
                    },
                    {
                        "name": "Branch 2",
                        "children": [
                            { "name": "Leaf 2.1", "value": 20 },
                            { "name": "Leaf 2.2", "value": 25 },
                            { "name": "Leaf 2.3", "value": 30 },
                        ]
                    }
                ]
            })]
        );
    }

    #[cfg(feature = "all-diagrams")]
    pub(crate) fn run_lifecycle_child(test: &'static str, worker: impl FnOnce() + Send + 'static) {
        const CASE_ENV: &str = "MERMAN_CORE_TREE_UNIT_LIFECYCLE_CASE";
        if std::env::var(CASE_ENV).as_deref() == Ok(test) {
            std::thread::Builder::new()
                .stack_size(2 * 1024 * 1024)
                .spawn(move || {
                    println!("phase=tree-worker status=begin case={test} stack_bytes=2097152");
                    worker();
                    println!("phase=tree-worker status=done case={test}");
                })
                .expect("spawn 2 MiB tree worker")
                .join()
                .expect("tree lifecycle worker completes");
            return;
        }
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(CASE_ENV, test)
            .spawn()
            .expect("spawn tree lifecycle child");
        let started = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("tree child observation failed: {error}");
                }
            }
            if started.elapsed() > std::time::Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("tree lifecycle child timed out: {test}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "tree lifecycle child failed: {test}, status={status}"
        );
    }

    #[cfg(feature = "all-diagrams")]
    pub(crate) fn assert_in_progress_export_deadline(json: &crate::ManagedSemanticJson) {
        struct Writer {
            control: crate::OperationControl,
            written: usize,
        }
        impl std::io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.written += bytes.len();
                if self.written >= 1_024 {
                    self.control.set_deadline(std::time::Duration::ZERO);
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let control = crate::OperationControl::new();
        let mut writer = Writer {
            control: control.clone(),
            written: 0,
        };
        assert!(matches!(
            json.write_json_controlled(&mut writer, &control),
            Err(crate::OperationCancelled {
                reason: crate::CancelReason::DeadlineExceeded,
                ..
            })
        ));
        assert!(
            writer.written >= 1_024,
            "deadline is installed after useful export work"
        );
    }

    #[cfg(feature = "all-diagrams")]
    #[test]
    fn treemap_deep_clone_drop_export_child() {
        run_lifecycle_child(
            "diagrams::treemap::tests::treemap_deep_clone_drop_export_child",
            || {
                let source = deep_treemap_chain(5_000);
                let parsed = Engine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let RenderSemanticModel::Treemap(model) = parsed.model() else {
                    panic!("typed treemap");
                };
                assert_eq!(model.nodes.len(), 5_001);
                let json =
                    ManagedSemanticJson::from(render_model_to_compat_json(model, &meta()).unwrap());
                let mut output = Vec::new();
                json.write_json(&mut output).unwrap();
                assert_in_progress_export_deadline(&json);
                assert!(output.windows(10).any(|bytes| bytes == b"\"value\":1}"));
                drop(json.clone());
                drop(json);
                drop(parsed.clone());
                drop(parsed);
                assert!(
                    Engine::new()
                        .parse_diagram_sync("treemap\n\"leaf\": 1\n", ParseOptions::strict())
                        .is_ok()
                );
            },
        );
    }

    #[cfg(feature = "all-diagrams")]
    #[test]
    fn treemap_projection_cancels_after_deep_completed_subtree_child() {
        run_lifecycle_child(
            "diagrams::treemap::tests::treemap_projection_cancels_after_deep_completed_subtree_child",
            || {
                const DEPTH: usize = 3_000;
                let mut source = deep_treemap_chain(DEPTH);
                source.push_str("\"sibling\": 2\n");
                let model = parse_treemap_model_for_render(&source, &meta()).unwrap();
                let control = crate::OperationControl::new();
                control.cancel_after_checkpoints(4 * DEPTH + 5);
                assert!(matches!(
                    model.project_root_controlled(&control),
                    Err(crate::OperationCancelled { .. })
                ));
                let projection =
                    render_model_to_compat_json(&model, &meta()).map(ManagedSemanticJson::from);
                assert!(projection.is_ok());
                let mut invalid = model.clone();
                let sibling = invalid.root.children.as_ref().unwrap()[1];
                invalid.nodes[sibling].children = Some(vec![usize::MAX]);
                assert!(render_model_to_compat_json(&invalid, &meta()).is_err());
                drop(invalid);
                drop(model);
            },
        );
    }

    #[test]
    fn treemap_projection_handles_deep_canonical_records() {
        let boundary = parse_treemap_model_for_render(&deep_treemap_chain(62), &meta()).unwrap();
        let json =
            ManagedSemanticJson::from(render_model_to_compat_json(&boundary, &meta()).unwrap());
        let mut output = Vec::new();
        json.write_json(&mut output).unwrap();
        assert!(!output.is_empty());
        let empty = TreemapDiagramRenderModel::default();
        assert_eq!(
            render_model_to_compat_json(&empty, &meta()).unwrap(),
            json!({})
        );
    }

    #[test]
    fn treemap_projection_rejects_invalid_ids_and_cycles() {
        let mut model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some(vec![0]),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(render_model_to_compat_json(&model, &meta()).is_err());
        model.nodes.push(TreemapNode {
            name: "cycle".to_string(),
            children: Some(vec![0]),
            ..Default::default()
        });
        assert!(render_model_to_compat_json(&model, &meta()).is_err());
    }

    #[test]
    fn treemap_parser_can_cancel_between_rows() {
        let mut text = String::from("treemap\n");
        for index in 0..512 {
            text.push_str(&format!("\"item-{index}\": 1\n"));
        }
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);

        assert!(matches!(
            parse_treemap_outcome_controlled(
                &text,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_combined_parse_constructs_source_once_and_preserves_projections() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "accTitle: Product areas\n",
            "accDescr: Product hierarchy\n",
            "\"Root\":::important\n",
            "  \"Leaf\": 42\n",
            "classDef important fill:#f96,stroke:#333;\n",
        );
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_treemap_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1,
            "one combined request must construct Treemap syntax once"
        );

        let standalone_json = parse_treemap(text, &meta).unwrap();
        assert_eq!(combined_json, standalone_json);
        assert!(!combined_editor.symbols.is_empty());
    }

    #[test]
    fn treemap_typed_and_json_projections_share_the_same_semantics() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "\"Root\":::important\n",
            "  \"Leaf\": 42\n",
            "classDef important fill:#f96,stroke:#333;\n",
            "classDef unused fill:#abc;\n",
        );
        let meta = meta();

        let compat = parse_treemap(text, &meta).unwrap();
        let typed = parse_treemap_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        let first = typed.root.children.as_ref().unwrap()[0];
        assert!(typed.compiled_styles(Some(first)).is_none());
        assert_eq!(
            typed.nodes[first].class_selector.as_deref(),
            Some("important")
        );
        assert_eq!(
            compat["root"]["children"][0]["cssCompiledStyles"][0],
            json!("fill:#f96;stroke:#333")
        );
        assert_eq!(compat["type"], json!("treemap"));
        assert!(compat["config"].is_object());
        assert_eq!(compat["accTitle"], Value::Null);
        assert_eq!(compat["accDescr"], Value::Null);
        assert!(compat["classes"].get("unused").is_some());
        assert!(typed.classes.contains_key("unused"));

        let empty = parse_treemap_model_for_render("", &meta).unwrap();
        assert_eq!(
            render_model_to_compat_json(&empty, &meta).unwrap(),
            json!({})
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn treemap_editor_projection_uses_exact_statement_spans() {
        let text = "treemap\n\"Leaf\": 42 :::hot\nclassDef hot fill:#f96;\n";
        let facts = crate::family::test_support::editor_facts(
            parse_treemap_json_and_editor_facts,
            text,
            &meta(),
        );

        for (name, detail) in [
            ("Leaf", "treemap leaf"),
            ("42", "treemap value"),
            ("hot", "treemap class selector"),
            ("fill:#f96", "treemap class style"),
        ] {
            let start = text.find(name).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection == SourceSpan::new(start, start + name.len())
            }));
        }

        let class_definition_start = text.rfind("hot").expect("classDef name");
        let class_definition = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "hot"
                    && symbol.detail.as_deref() == Some("treemap class definition")
                    && symbol.selection.start == class_definition_start
            })
            .expect("treemap class definition fact");
        assert_eq!(
            class_definition.role,
            crate::EditorSemanticRole::ClassDefinition
        );
    }

    #[test]
    fn treemap_incomplete_statement_recovers_prior_facts_with_exact_error_span() {
        let text = "treemap\n\"Root\"\n  \"Broken\":\n";
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must reject the leaf");
        assert!(error.to_string().contains("expected number"));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                text,
                &meta,
            );

            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Root"));
            let diagnostic = facts
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.message.contains("expected number"))
                .expect("recovery diagnostic");
            assert!(diagnostic.message.contains("expected number"));
            let start = text.find("\"Broken\":").unwrap();
            assert_eq!(
                diagnostic.span,
                Some(SourceSpan::new(start, start + "\"Broken\":".len()))
            );
        }
    }

    #[test]
    fn treemap_recoverable_outcome_keeps_error_prefix_and_later_safe_lines() {
        let text = concat!(
            "treemap\r\n",
            "\"Before\": 1\r\n",
            "  \"Broken\": 12 :::\r\n",
            "classDef hot fill:#f00; trailing\r\n",
            "\"After\": 2\r\n",
        );
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must use the first error");
        assert!(error.to_string().contains("expected class selector"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                text,
                &meta,
            );
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| { diagnostic.message.contains("expected class selector") })
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("unexpected tokens after classDef")
            }));

            assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
        }
    }

    #[test]
    fn treemap_multiline_acc_descr_uses_common_syntax_and_recovers_when_unterminated() {
        let complete = "treemap\naccDescr {\nline one\nline two\n}\n\"Root\"\n";
        let meta = meta();
        #[cfg(feature = "editor-facts")]
        let (json, facts) = crate::family::test_support::into_result(
            parse_treemap_json_and_editor_facts(complete, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        #[cfg(not(feature = "editor-facts"))]
        let json = parse_treemap(complete, &meta).unwrap();
        assert_eq!(json["accDescr"], json!("line one\nline two"));
        #[cfg(feature = "editor-facts")]
        {
            let payload_start = complete.find("line one").unwrap();
            let payload = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "line one\nline two")
                .expect("multiline accessibility payload");
            assert_eq!(
                payload.selection,
                SourceSpan::new(payload_start, payload_start + "line one\nline two".len())
            );
        }

        let incomplete = "treemap\naccDescr {\nline one\n";
        assert!(parse_treemap(incomplete, &meta).is_err());
        #[cfg(feature = "editor-facts")]
        {
            let recovered = crate::family::test_support::editor_facts(
                parse_treemap_json_and_editor_facts,
                incomplete,
                &meta,
            );
            assert_eq!(
                recovered.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(recovered.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("unterminated accDescr block")
                    && diagnostic.span == Some(SourceSpan::new(incomplete.len(), incomplete.len()))
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "treemap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn treemap_semantics_preserve_hierarchy_and_authored_styles_without_editor_work() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_treemap_semantic_model_controlled(
            r#"treemap
title Inventory
"Group"
  "Leaf": 42:::hot
classDef hot fill:red
"#,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Inventory"));
        assert_eq!(model.root.children.as_deref(), Some([0usize].as_slice()));
        assert_eq!(model.nodes[0].name, "Group");
        assert_eq!(
            model.nodes[0].children.as_deref(),
            Some([1usize].as_slice())
        );
        assert_eq!(model.nodes[1].value.as_ref().unwrap().as_i64(), Some(42));
        assert_eq!(model.nodes[1].class_selector.as_deref(), Some("hot"));
        assert_eq!(model.classes["hot"].styles, ["fill:red"]);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn treemap_semantics_keep_empty_state_and_exact_errors_without_projection() {
        for source in [
            "", "treemap
",
        ] {
            let model = parse_treemap_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.root.children.as_deref(), Some([].as_slice()));
            assert!(model.nodes.is_empty());
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_treemap_semantic_model_controlled(
            "wrong
",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("structured error");
        };
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(0, 5)));
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            parse_treemap_semantic_model_controlled(
                r#"treemap
"Leaf": 1
"#,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }
}
