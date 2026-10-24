//! Native Mermaid 12 Usecase grammar and transactional semantic construction.
//!
//! Grammar, declaration merging, and deferred resolution follow the pinned upstream
//! `usecase.parser.ts`, `usecase.visitor.ts`, and `usecaseModelBuilder.ts`.

#[cfg(feature = "editor-facts")]
use crate::EditorSemanticFacts;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::{Error, OperationControl, OperationControlResult, SourceSpan, family};
#[cfg(feature = "presentation")]
use crate::{PresentationMetadata, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::json;
use std::collections::BTreeMap;

mod json_string;
mod lexer;
mod ordered_json;
mod parse;
mod resolve;

/// Whether an unquoted spelling is one complete identifier in the pinned grammar.
#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_identifier(candidate: &str) -> bool {
    if candidate.is_empty()
        || is_forbidden_statement_identifier(candidate)
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return false;
    }
    let Ok(Ok(tokens)) = lexer::lex(candidate, &OperationControl::new()) else {
        return false;
    };
    tokens.len() == 2 && tokens[0].kind == lexer::TokenKind::Identifier
}

fn is_forbidden_statement_identifier(candidate: &str) -> bool {
    [
        "allowmixing",
        "newpage",
        "package",
        "rectangle",
        "skinparam",
    ]
    .iter()
    .any(|keyword| candidate.eq_ignore_ascii_case(keyword))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsecaseNodeKind {
    Actor,
    UseCaseEllipse,
    UseCaseRect,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseLabelType {
    #[default]
    Text,
    Markdown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseActorType {
    #[default]
    Normal,
    Hollow,
    Awesome,
    Icon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseNode {
    pub id: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
    pub kind: UsecaseNodeKind,
    pub actor_type: UsecaseActorType,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    pub business: bool,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseBoundary {
    pub id: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
    pub members: Vec<String>,
    pub package: bool,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseRelationshipType {
    Association,
    Include,
    Extend,
    Generalization,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub enum UsecaseArrowType {
    Point,
    ReversedPoint,
    Line,
    Circle,
    Cross,
    ReversedCircle,
    ReversedCross,
}

impl From<UsecaseArrowType> for u8 {
    fn from(value: UsecaseArrowType) -> Self {
        value as u8
    }
}
impl TryFrom<u8> for UsecaseArrowType {
    type Error = &'static str;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Point),
            1 => Ok(Self::ReversedPoint),
            2 => Ok(Self::Line),
            3 => Ok(Self::Circle),
            4 => Ok(Self::Cross),
            5 => Ok(Self::ReversedCircle),
            6 => Ok(Self::ReversedCross),
            _ => Err("invalid Usecase arrow type"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseAnimation {
    Fast,
    Slow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseRelationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    pub relationship_type: UsecaseRelationshipType,
    /// Upstream ARROW_TYPE: point, reversed point, line, circle, cross,
    /// reversed circle, and reversed cross, respectively.
    pub arrow_type: UsecaseArrowType,
    pub label: Option<String>,
    pub label_type: Option<UsecaseLabelType>,
    pub dotted: bool,
    pub minlen: usize,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<UsecaseAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseNote {
    pub id: String,
    pub target: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
}

/// JavaScript JSON numbers whose binary64 value overflows. Compatibility JSON
/// serializes these as null, while Usecase table labels retain their sign.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsecaseJsonInfinity {
    Positive,
    Negative,
}

/// Lossless JSON string storage for values containing unpaired UTF-16 surrogates.
/// In this mode every object key and string value is a complete canonical JSON
/// string literal. Decode explicitly; these strings are not ordinary display text.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UsecaseJsonStringEncoding {
    #[serde(rename = "json-utf16")]
    JsonUtf16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseJsonNode {
    pub id: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_encoding: Option<UsecaseJsonStringEncoding>,
    /// Source property order, keyed by RFC 6901 JSON pointers, including the root.
    pub property_order: BTreeMap<String, Vec<String>>,
    /// Non-finite table values keyed by RFC 6901 JSON pointers. Their entries in
    /// `value` are null, matching JavaScript JSON.stringify serialization.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub non_finite_numbers: BTreeMap<String, UsecaseJsonInfinity>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

impl UsecaseJsonNode {
    /// Returns UTF-8 display text, replacing each unpaired surrogate only at the
    /// rendering boundary, as Mermaid CLI's TextEncoder does for SVG output.
    pub fn display_string<'a>(
        &self,
        value: &'a str,
    ) -> std::result::Result<std::borrow::Cow<'a, str>, serde_json::Error> {
        if self.string_encoding.is_none() {
            return Ok(std::borrow::Cow::Borrowed(value));
        }
        Ok(std::borrow::Cow::Owned(String::from_utf16_lossy(
            &json_string::code_units(value)?,
        )))
    }

    /// Returns the original JavaScript UTF-16 code units without replacement.
    pub fn string_code_units(
        &self,
        value: &str,
    ) -> std::result::Result<Vec<u16>, serde_json::Error> {
        if self.string_encoding.is_none() {
            return Ok(value.encode_utf16().collect());
        }
        json_string::code_units(value)
    }

    /// Exports ordinary JavaScript JSON, restoring encoded keys and strings.
    /// Non-finite values remain null, matching JSON.stringify. Property order
    /// follows the source-order metadata; malformed encoded strings return errors.
    pub fn to_javascript_json(&self) -> std::result::Result<String, serde_json::Error> {
        fn write_value(
            node: &UsecaseJsonNode,
            value: &Value,
            pointer: &str,
            depth: usize,
            output: &mut String,
        ) -> std::result::Result<(), serde_json::Error> {
            if depth > 128 {
                return Err(<serde_json::Error as serde::de::Error>::custom(
                    "Usecase JSON nesting depth exceeds 128",
                ));
            }
            match value {
                Value::String(value) => {
                    if node.string_encoding.is_some() {
                        output.push_str(&json_string::canonical(value)?);
                    } else {
                        output.push_str(&serde_json::to_string(value)?);
                    }
                }
                Value::Array(values) => {
                    output.push('[');
                    for (index, value) in values.iter().enumerate() {
                        if index > 0 {
                            output.push(',');
                        }
                        write_value(
                            node,
                            value,
                            &format!("{pointer}/{index}"),
                            depth + 1,
                            output,
                        )?;
                    }
                    output.push(']');
                }
                Value::Object(values) => {
                    output.push('{');
                    let order = node.property_order.get(pointer);
                    let mut seen = std::collections::HashSet::with_capacity(values.len());
                    let mut first = true;
                    for key in order.into_iter().flatten().chain(values.keys()) {
                        let Some(value) = values.get(key) else {
                            continue;
                        };
                        if !seen.insert(key) {
                            continue;
                        }
                        if !first {
                            output.push(',');
                        }
                        first = false;
                        if node.string_encoding.is_some() {
                            output.push_str(&json_string::canonical(key)?);
                        } else {
                            output.push_str(&serde_json::to_string(key)?);
                        }
                        output.push(':');
                        let escaped = key.replace('~', "~0").replace('/', "~1");
                        write_value(
                            node,
                            value,
                            &format!("{pointer}/{escaped}"),
                            depth + 1,
                            output,
                        )?;
                    }
                    output.push('}');
                }
                _ => output.push_str(&serde_json::to_string(value)?),
            }
            Ok(())
        }
        let mut output = String::new();
        write_value(self, &self.value, "", 0, &mut output)?;
        Ok(output)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsecaseClassDef {
    pub id: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Resolved use cases, actors, boundaries, relationships, and source-authored JSON records.
pub struct UsecaseDiagramModel {
    pub direction: String,
    pub nodes: Vec<UsecaseNode>,
    pub boundaries: Vec<UsecaseBoundary>,
    pub relationships: Vec<UsecaseRelationship>,
    pub notes: Vec<UsecaseNote>,
    pub json_nodes: Vec<UsecaseJsonNode>,
    pub class_defs: Vec<UsecaseClassDef>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
}

impl UsecaseDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_description, config);
    }
}

#[derive(Debug)]
struct ParseIssue {
    message: String,
    span: SourceSpan,
}

impl ParseIssue {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

pub(crate) fn parse_usecase_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<family::SemanticConstruction<UsecaseDiagramModel>> {
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        metadata.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = None;
    let model = match construction {
        Ok(source) => {
            let mut model = source.model;
            model.sanitize_common_db_fields(&metadata.semantic_config);
            #[cfg(feature = "editor-facts")]
            {
                editor_facts = editor_demand
                    .needs_recovery()
                    .then_some(source.editor_facts);
            }
            Ok(model)
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_parts();
                editor_facts = Some(facts);
                return Ok(family::SemanticConstruction {
                    model: Err(error),
                    editor_facts,
                });
            }
            Err(failure.into_error())
        }
    };
    Ok(family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(all(test, feature = "compat-json"))]
fn construct(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )
}

struct Construction {
    model: UsecaseDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_usecase(code: &str, meta: &PresentationMetadata) -> Result<Value> {
    let control = OperationControl::new();
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &control,
    )
    .expect("a private operation control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)?;
    render_model_to_compat_json(&construction.model, meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_usecase_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<UsecaseDiagramModel>> {
    Ok(construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .map(|value| value.model)
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_usecase_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_usecase_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.title.as_deref(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(family::CombinedSemanticParse::from_construction(
        construction,
        |value| {
            (
                render_model_to_compat_json(&value.model, meta),
                value.editor_facts,
            )
        },
        family::CombinedSemanticFailure::into_parts,
    ))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &UsecaseDiagramModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    let json_nodes: Vec<_> = model
        .json_nodes
        .iter()
        .map(|node| {
            let mut value = json!({
                "id": node.id, "value": node.value, "propertyOrder": node.property_order,
                "classes": node.classes, "styles": node.styles,
            });
            if let Some(encoding) = node.string_encoding {
                value["stringEncoding"] = json!(encoding);
            }
            value
        })
        .collect();
    Ok(
        json!({ "type": meta.diagram_type, "direction": model.direction,
        "nodes": model.nodes, "boundaries": model.boundaries, "relationships": model.relationships,
        "notes": model.notes, "jsonNodes": json_nodes, "classDefs": model.class_defs,
        "title": model.title, "accTitle": model.acc_title, "accDescr": model.acc_description }),
    )
}

fn construct_usecase_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    title: Option<&str>,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("usecase");
    let tokens = match lexer::lex(code, control)? {
        Ok(tokens) => tokens,
        Err(issue) => {
            return Ok(Err(failure(
                diagram_type,
                issue,
                #[cfg(feature = "editor-facts")]
                EditorSemanticFacts::new(),
                #[cfg(feature = "editor-facts")]
                editor_demand,
            )));
        }
    };
    let mut parser = parse::Parser::new(
        code,
        tokens,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    );
    let parsed = parser.parse()?;
    #[cfg(feature = "editor-facts")]
    parser.finalize_editor_facts()?;
    if let Err(issue) = parsed {
        return Ok(Err(failure(
            diagram_type,
            issue,
            #[cfg(feature = "editor-facts")]
            parser.facts,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        )));
    }
    let model = resolve::resolve(&parser.draft, title, control)?;
    match model {
        Ok(model) => Ok(Ok(Construction {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts: parser.facts,
        })),
        Err(issue) => Ok(Err(failure(
            diagram_type,
            issue,
            #[cfg(feature = "editor-facts")]
            parser.facts,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        ))),
    }
}

fn failure(
    diagram_type: &str,
    issue: ParseIssue,
    #[cfg(feature = "editor-facts")] mut facts: EditorSemanticFacts,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> family::CombinedSemanticFailure {
    #[cfg(feature = "editor-facts")]
    if editor_demand.needs_recovery() {
        facts.mark_recovered_from_parse_error(issue.message.clone(), Some(issue.span));
    }
    family::CombinedSemanticFailure::new(
        Error::diagram_parse_exact(diagram_type.to_string(), issue.message, issue.span),
        #[cfg(feature = "editor-facts")]
        editor_demand.needs_recovery().then_some(facts),
    )
}

#[cfg(all(test, feature = "compat-json"))]
mod tests;

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "usecase".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let mut metadata = metadata();
        metadata.title = Some("<script>secret</script>Authentication".to_string());
        let source = concat!(
            "usecase-beta\n",
            "actor User\n",
            "Login[Sign in]\n",
            "User --> Login\n",
            "json Data@{\"enabled\":true,\"items\":[2,1]}\n",
            "note for Login \"Try again\"\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("usecase");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_usecase_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Authentication"));
        assert_eq!(model.nodes[0].kind, UsecaseNodeKind::Actor);
        assert_eq!(model.nodes[1].label, "Sign in");
        assert_eq!(model.relationships[0].source, "User");
        assert_eq!(model.relationships[0].target, "Login");
        assert_eq!(model.notes[0].target, "Login");
        assert_eq!(model.json_nodes[0].value["enabled"], true);
        assert_eq!(model.json_nodes[0].value["items"][0], 2);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("usecase"),
            1,
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full_metadata = crate::PresentationMetadata {
                diagram_type: metadata.diagram_type.clone(),
                config: metadata.config.clone(),
                effective_config: crate::presentation::PresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ),
                title: metadata.title.clone(),
            };
            parse_usecase_model_for_render_controlled(
                source,
                &full_metadata,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());

            #[cfg(feature = "compat-json")]
            {
                parse_usecase(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "usecase-beta\nactor User\nA -->";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_usecase_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured parse diagnostic");
        };
        let start = source.len();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start)),);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_usecase_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
    }
}
