//! Narrow AST adapter for extracting Mermaid fixtures from upstream JavaScript and TypeScript.

use serde_json::{Map, Number, Value};
use std::collections::{HashMap, HashSet};
use std::fmt;
use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticStringExpression {
    /// Zero-based source-order position among static string expression candidates.
    pub(crate) source_ordinal: usize,
    pub(crate) value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CypressRenderHelper {
    ImgSnapshotTest,
    RenderGraph,
}

impl CypressRenderHelper {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ImgSnapshotTest => "imgSnapshotTest",
            Self::RenderGraph => "renderGraph",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CypressRenderCase {
    pub(crate) start_byte: usize,
    pub(crate) helper: CypressRenderHelper,
    pub(crate) test_name: Option<String>,
    pub(crate) diagram: String,
    pub(crate) options: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsupportedCypressReason {
    NotInlineTest,
    MissingGraph,
    DynamicGraph,
    MultipleGraphs,
    ApiRendering,
    DynamicApi,
    NonObjectOptions,
    DynamicOptions(&'static str),
}

impl fmt::Display for UnsupportedCypressReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInlineTest => {
                formatter.write_str("call is not directly inside an inline it/test callback")
            }
            Self::MissingGraph => formatter.write_str("graph argument 0 is missing"),
            Self::DynamicGraph => {
                formatter.write_str("graph argument 0 is not a static literal expression")
            }
            Self::MultipleGraphs => formatter.write_str(
                "graph argument 0 contains multiple diagrams and cannot map to one fixture",
            ),
            Self::ApiRendering => {
                formatter.write_str("api=true uses the Cypress XSS rendering path")
            }
            Self::DynamicApi => formatter.write_str("api argument 2 is dynamic or unsupported"),
            Self::NonObjectOptions => {
                formatter.write_str("options argument 1 is not an object literal")
            }
            Self::DynamicOptions(reason) => {
                write!(formatter, "options argument 1 is not static: {reason}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedCypressCall {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) helper: CypressRenderHelper,
    pub(crate) reason: UnsupportedCypressReason,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CypressRenderExtraction {
    pub(crate) cases: Vec<CypressRenderCase>,
    pub(crate) unsupported: Vec<UnsupportedCypressCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineTestContext<'tree> {
    name: Option<String>,
    skipped: bool,
    callback: Node<'tree>,
}

struct ParsedSource<'source> {
    source: &'source str,
    tree: Tree,
}

impl<'source> ParsedSource<'source> {
    fn parse(source: &'source str) -> Result<Self, &'static str> {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser
            .set_language(&language)
            .map_err(|_| "failed to initialize the TypeScript parser")?;
        let tree = parser
            .parse(source, None)
            .ok_or("TypeScript parser returned no syntax tree")?;
        if tree.root_node().has_error() {
            return Err("TypeScript parser returned an error tree");
        }
        Ok(Self { source, tree })
    }

    fn cypress_render_cases(&self) -> CypressRenderExtraction {
        let mut calls = all_named_nodes(self.tree.root_node())
            .into_iter()
            .filter(|node| node.kind() == "call_expression")
            .filter_map(|call| {
                let function = call.child_by_field_name("function")?;
                let helper = match normalized_callee(self.source, function)?.as_str() {
                    "imgSnapshotTest" => CypressRenderHelper::ImgSnapshotTest,
                    "renderGraph" => CypressRenderHelper::RenderGraph,
                    _ => return None,
                };
                Some((call, helper))
            })
            .collect::<Vec<_>>();
        calls.sort_by_key(|(call, _)| call.start_byte());

        let mut extraction = CypressRenderExtraction::default();
        for (call, helper) in calls {
            let reject = |reason| UnsupportedCypressCall {
                start_byte: call.start_byte(),
                end_byte: call.end_byte(),
                helper,
                reason,
            };
            let Some(test) = self.inline_test_context(call) else {
                extraction
                    .unsupported
                    .push(reject(UnsupportedCypressReason::NotInlineTest));
                continue;
            };
            if test.skipped {
                continue;
            }

            let argument_nodes = call_arguments(call);
            let Some(graph_node) = argument_nodes.first().copied() else {
                extraction
                    .unsupported
                    .push(reject(UnsupportedCypressReason::MissingGraph));
                continue;
            };
            if graph_node.kind() == "array" {
                extraction
                    .unsupported
                    .push(reject(UnsupportedCypressReason::MultipleGraphs));
                continue;
            }
            let locals = self.local_static_strings_before(test.callback, call);
            let Some(diagram) =
                evaluate_static_string_with_locals(self.source, graph_node, &locals)
            else {
                extraction
                    .unsupported
                    .push(reject(UnsupportedCypressReason::DynamicGraph));
                continue;
            };

            if let Some(api_node) = argument_nodes.get(2).copied() {
                match static_api_enabled(self.source, api_node) {
                    Some(false) => {}
                    Some(true) => {
                        extraction
                            .unsupported
                            .push(reject(UnsupportedCypressReason::ApiRendering));
                        continue;
                    }
                    None => {
                        extraction
                            .unsupported
                            .push(reject(UnsupportedCypressReason::DynamicApi));
                        continue;
                    }
                }
            }

            let options = match argument_nodes.get(1).copied() {
                None => Value::Object(Map::new()),
                Some(node) => match evaluate_static_json(self.source, node) {
                    Ok(Value::Object(options)) => Value::Object(options),
                    Ok(_) => {
                        extraction
                            .unsupported
                            .push(reject(UnsupportedCypressReason::NonObjectOptions));
                        continue;
                    }
                    Err(reason) => {
                        extraction
                            .unsupported
                            .push(reject(UnsupportedCypressReason::DynamicOptions(reason)));
                        continue;
                    }
                },
            };

            extraction.cases.push(CypressRenderCase {
                start_byte: call.start_byte(),
                helper,
                test_name: test.name,
                diagram,
                options,
            });
        }
        extraction
    }

    fn inline_test_context<'tree>(&self, call: Node<'tree>) -> Option<InlineTestContext<'tree>> {
        let mut ancestor = call.parent();
        while let Some(node) = ancestor {
            if is_function(node.kind()) {
                let arguments = node
                    .parent()
                    .filter(|parent| parent.kind() == "arguments")?;
                let test_call = arguments
                    .parent()
                    .filter(|parent| parent.kind() == "call_expression")?;
                let function = test_call.child_by_field_name("function")?;
                let skipped = match normalized_callee(self.source, function)?.as_str() {
                    "it" | "it.only" | "test" | "test.only" => false,
                    "it.skip" | "test.skip" => true,
                    _ => return None,
                };
                let name = call_arguments(test_call)
                    .first()
                    .and_then(|name| evaluate_static_string(self.source, *name));
                return Some(InlineTestContext {
                    name,
                    skipped,
                    callback: node,
                });
            }
            ancestor = node.parent();
        }
        None
    }

    fn local_static_strings_before(
        &self,
        callback: Node<'_>,
        call: Node<'_>,
    ) -> HashMap<String, String> {
        let Some(call_block) = nearest_statement_block(call) else {
            return HashMap::new();
        };
        let mut declarators = all_named_nodes(callback)
            .into_iter()
            .filter(|node| node.kind() == "variable_declarator")
            .filter(|declarator| declarator.end_byte() <= call.start_byte())
            .filter(|declarator| {
                declarator
                    .parent()
                    .filter(|declaration| declaration.kind() == "lexical_declaration")
                    .and_then(|declaration| declaration.child_by_field_name("kind"))
                    .is_some_and(|kind| kind.kind() == "const")
            })
            .filter(|declarator| {
                declarator
                    .parent()
                    .and_then(|declaration| declaration.parent())
                    .is_some_and(|scope| scope.id() == call_block.id())
            })
            .collect::<Vec<_>>();
        declarators.sort_by_key(Node::start_byte);

        let mut locals = HashMap::new();
        for declarator in declarators {
            let Some(name) = declarator
                .child_by_field_name("name")
                .filter(|name| name.kind() == "identifier")
                .and_then(|name| decode_identifier_node(self.source, name))
            else {
                continue;
            };
            let Some(value) = declarator
                .child_by_field_name("value")
                .and_then(|value| evaluate_static_string_with_locals(self.source, value, &locals))
            else {
                continue;
            };
            locals.insert(name, value);
        }
        locals
    }

    fn package_test_strings(&self) -> Vec<StaticStringExpression> {
        let mut seen = HashSet::new();
        let mut candidates = all_named_nodes(self.tree.root_node())
            .into_iter()
            .flat_map(|node| package_string_expression_candidates(self.source, node))
            .filter(|node| seen.insert(node.id()))
            .collect::<Vec<_>>();
        candidates.sort_by_key(Node::start_byte);

        candidates
            .into_iter()
            .enumerate()
            .filter(|(_, node)| !self.inside_skipped_test(*node))
            .filter(|(_, node)| !is_test_title(self.source, *node))
            .filter(|(_, node)| !self.binding_is_dynamic_fragment(*node))
            .filter_map(|(source_ordinal, node)| {
                evaluate_static_string(self.source, node).map(|value| StaticStringExpression {
                    source_ordinal,
                    value,
                })
            })
            .collect()
    }

    fn binding_is_dynamic_fragment(&self, candidate: Node<'_>) -> bool {
        let Some(declarator) = candidate
            .parent()
            .filter(|parent| parent.kind() == "variable_declarator")
        else {
            return false;
        };
        if declarator
            .child_by_field_name("value")
            .map(|value| value.id())
            != Some(candidate.id())
        {
            return false;
        }
        let Some(binding_name) = declarator
            .child_by_field_name("name")
            .filter(|name| name.kind() == "identifier")
            .and_then(|name| decode_identifier_node(self.source, name))
        else {
            return false;
        };

        all_named_nodes(self.tree.root_node())
            .into_iter()
            .filter(|node| node.kind() == "identifier")
            .filter(|node| {
                !(candidate.start_byte()..candidate.end_byte()).contains(&node.start_byte())
            })
            .filter(|node| {
                decode_identifier_node(self.source, *node).as_deref() == Some(binding_name.as_str())
            })
            .any(identifier_is_in_dynamic_string_composition)
    }

    fn inside_skipped_test(&self, node: Node<'_>) -> bool {
        let mut ancestor = Some(node);
        while let Some(current) = ancestor {
            if is_function(current.kind())
                && let Some(arguments) = current
                    .parent()
                    .filter(|parent| parent.kind() == "arguments")
                && let Some(call) = arguments
                    .parent()
                    .filter(|parent| parent.kind() == "call_expression")
                && call
                    .child_by_field_name("function")
                    .and_then(|function| normalized_callee(self.source, function))
                    .is_some_and(|callee| matches!(callee.as_str(), "it.skip" | "test.skip"))
            {
                return true;
            }
            ancestor = current.parent();
        }
        false
    }
}

pub(crate) fn extract_cypress_render_cases(
    source: &str,
) -> Result<CypressRenderExtraction, &'static str> {
    ParsedSource::parse(source).map(|source| source.cypress_render_cases())
}

pub(crate) fn extract_package_test_strings(
    source: &str,
) -> Result<Vec<StaticStringExpression>, &'static str> {
    ParsedSource::parse(source).map(|source| source.package_test_strings())
}

fn call_arguments(call: Node<'_>) -> Vec<Node<'_>> {
    let Some(arguments) = call.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .filter(|node| node.kind() != "comment")
        .collect()
}

fn normalized_callee(source: &str, node: Node<'_>) -> Option<String> {
    match node.kind() {
        "identifier" | "property_identifier" => decode_identifier_node(source, node),
        "member_expression" => {
            let object = normalized_callee(source, node.child_by_field_name("object")?)?;
            let property = normalized_callee(source, node.child_by_field_name("property")?)?;
            Some(format!("{object}.{property}"))
        }
        _ => None,
    }
}

fn decode_identifier_node(source: &str, node: Node<'_>) -> Option<String> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let mut decoded = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        if text.as_bytes().get(index) == Some(&b'\\') {
            let (character, next) = decode_identifier_escape(text, index)?;
            decoded.push(character);
            index = next;
        } else {
            let (character, next) = source_char(text, index)?;
            decoded.push(character);
            index = next;
        }
    }
    Some(decoded)
}

fn decode_identifier_escape(source: &str, slash: usize) -> Option<(char, usize)> {
    let bytes = source.as_bytes();
    if bytes.get(slash) != Some(&b'\\') || bytes.get(slash + 1) != Some(&b'u') {
        return None;
    }
    let digits_start = slash + 2;
    if bytes.get(digits_start) == Some(&b'{') {
        let mut value = 0u32;
        let mut digits = 0usize;
        let mut index = digits_start + 1;
        while let Some(&byte) = bytes.get(index) {
            if byte == b'}' {
                if digits == 0 {
                    return None;
                }
                return Some((char::from_u32(value)?, index + 1));
            }
            value = value
                .checked_mul(16)?
                .checked_add(u32::from(hex_value(byte)?))?;
            digits += 1;
            index += 1;
        }
        return None;
    }
    let (value, end) = parse_fixed_hex(source, digits_start, 4)?;
    Some((char::from_u32(value)?, end))
}

fn nearest_statement_block(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "statement_block" {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "arrow_function"
            | "function"
            | "function_expression"
            | "generator_function"
            | "generator_function_declaration"
    )
}

fn all_named_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut nodes = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_named() {
            nodes.push(node);
        }
        let mut cursor = node.walk();
        let mut children = node.named_children(&mut cursor).collect::<Vec<_>>();
        children.reverse();
        pending.extend(children);
    }
    nodes
}

fn package_string_expression_candidates<'tree>(
    source: &str,
    node: Node<'tree>,
) -> Vec<Node<'tree>> {
    match node.kind() {
        // A declaration owns its complete initializer. Do not mine nested array/object strings:
        // they can be fragments of a runtime-composed diagram.
        "variable_declarator" => node.child_by_field_name("value").into_iter().collect(),
        // A direct Mermaid input argument is another complete expression boundary. Restrict this
        // to parser/rendering consumers so assertion strings such as `toBe("treeView-beta")`
        // cannot become header-only fixtures.
        "call_expression" if is_package_diagram_consumer(source, node) => call_arguments(node),
        _ => Vec::new(),
    }
}

fn is_package_diagram_consumer(source: &str, node: Node<'_>) -> bool {
    normalized_callee(
        source,
        match node.child_by_field_name("function") {
            Some(function) => function,
            None => return false,
        },
    )
    .and_then(|callee| callee.rsplit('.').next().map(str::to_string))
    .is_some_and(|name| {
        matches!(
            name.as_str(),
            "parse" | "render" | "getDiagramFromText" | "detectType"
        )
    })
}

fn is_static_array_join_expression(source: &str, node: Node<'_>) -> bool {
    let Some(function) = node.child_by_field_name("function") else {
        return false;
    };
    if function.kind() != "member_expression" {
        return false;
    }
    let Some(property) = function
        .child_by_field_name("property")
        .and_then(|property| decode_identifier_node(source, property))
    else {
        return false;
    };
    property == "join"
        && function
            .child_by_field_name("object")
            .is_some_and(|object| object.kind() == "array")
}

fn is_test_title(source: &str, node: Node<'_>) -> bool {
    let Some(arguments) = node.parent().filter(|parent| parent.kind() == "arguments") else {
        return false;
    };
    let Some(call) = arguments
        .parent()
        .filter(|parent| parent.kind() == "call_expression")
    else {
        return false;
    };
    if call_arguments(call).first().map(Node::id) != Some(node.id()) {
        return false;
    }
    call.child_by_field_name("function")
        .and_then(|function| normalized_callee(source, function))
        .is_some_and(|callee| {
            matches!(
                callee.as_str(),
                "it" | "it.only" | "it.skip" | "test" | "test.only" | "test.skip"
            )
        })
}

fn identifier_is_in_dynamic_string_composition(identifier: Node<'_>) -> bool {
    let mut ancestor = identifier.parent();
    while let Some(node) = ancestor {
        if matches!(
            node.kind(),
            "array"
                | "augmented_assignment_expression"
                | "binary_expression"
                | "template_substitution"
        ) {
            return true;
        }
        if matches!(
            node.kind(),
            "statement_block" | "program" | "call_expression" | "variable_declarator"
        ) {
            return false;
        }
        ancestor = node.parent();
    }
    false
}

fn evaluate_static_string(source: &str, node: Node<'_>) -> Option<String> {
    evaluate_static_string_with_locals(source, node, &HashMap::new())
}

fn evaluate_static_string_with_locals(
    source: &str,
    node: Node<'_>,
    locals: &HashMap<String, String>,
) -> Option<String> {
    match node.kind() {
        "string" => decode_quoted_string(source, node),
        "template_string" => decode_template_string(source, node, locals),
        "identifier" => decode_identifier_node(source, node)
            .and_then(|identifier| locals.get(&identifier).cloned()),
        "binary_expression" => {
            let operator = node.child_by_field_name("operator")?;
            if operator.utf8_text(source.as_bytes()).ok()? != "+" {
                return None;
            }
            let mut value = evaluate_static_string_with_locals(
                source,
                node.child_by_field_name("left")?,
                locals,
            )?;
            value.push_str(&evaluate_static_string_with_locals(
                source,
                node.child_by_field_name("right")?,
                locals,
            )?);
            Some(value)
        }
        "call_expression" if is_static_array_join_expression(source, node) => {
            evaluate_static_array_join(source, node, locals)
        }
        "parenthesized_expression"
        | "as_expression"
        | "satisfies_expression"
        | "non_null_expression" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .and_then(|child| evaluate_static_string_with_locals(source, child, locals))
        }
        "type_assertion" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .last()
                .and_then(|child| evaluate_static_string_with_locals(source, child, locals))
        }
        _ => None,
    }
}

fn evaluate_static_array_join(
    source: &str,
    node: Node<'_>,
    locals: &HashMap<String, String>,
) -> Option<String> {
    let function = node.child_by_field_name("function")?;
    let array = function.child_by_field_name("object")?;
    let mut cursor = array.walk();
    let values = array
        .named_children(&mut cursor)
        .filter(|element| element.kind() != "comment")
        .map(|element| evaluate_static_string_with_locals(source, element, locals))
        .collect::<Option<Vec<_>>>()?;
    let separator = match call_arguments(node).as_slice() {
        [] => ",".to_string(),
        [separator] => evaluate_static_string_with_locals(source, *separator, locals)?,
        _ => return None,
    };
    Some(values.join(&separator))
}

fn static_api_enabled(source: &str, node: Node<'_>) -> Option<bool> {
    if node.utf8_text(source.as_bytes()).ok()?.trim() == "undefined" {
        return Some(false);
    }
    match node.kind() {
        "true" => Some(true),
        "false" => Some(false),
        "parenthesized_expression" | "as_expression" | "satisfies_expression" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .and_then(|child| static_api_enabled(source, child))
        }
        _ => None,
    }
}

fn evaluate_static_json(source: &str, node: Node<'_>) -> Result<Value, &'static str> {
    match node.kind() {
        "object" => {
            let mut out = Map::new();
            let mut cursor = node.walk();
            for property in node.named_children(&mut cursor) {
                match property.kind() {
                    "comment" => {}
                    "pair" => {
                        let key = static_property_key(source, property)
                            .ok_or("computed or unsupported property key")?;
                        let value = property
                            .child_by_field_name("value")
                            .ok_or("property has no value")?;
                        out.insert(key, evaluate_static_json(source, value)?);
                    }
                    "shorthand_property_identifier" => {
                        return Err("shorthand property is dynamic");
                    }
                    "spread_element" => return Err("object spread is dynamic"),
                    _ => return Err("unsupported object member"),
                }
            }
            Ok(Value::Object(out))
        }
        "array" => {
            let mut values = Vec::new();
            let mut cursor = node.walk();
            for element in node.named_children(&mut cursor) {
                if element.kind() != "comment" {
                    values.push(evaluate_static_json(source, element)?);
                }
            }
            Ok(Value::Array(values))
        }
        "string" | "template_string" | "binary_expression" => evaluate_static_string(source, node)
            .map(Value::String)
            .ok_or("string expression is dynamic"),
        "parenthesized_expression"
        | "as_expression"
        | "satisfies_expression"
        | "non_null_expression" => {
            let mut cursor = node.walk();
            let expression = node
                .named_children(&mut cursor)
                .next()
                .ok_or("wrapped expression has no value")?;
            evaluate_static_json(source, expression)
        }
        "type_assertion" => {
            let mut cursor = node.walk();
            let expression = node
                .named_children(&mut cursor)
                .last()
                .ok_or("type assertion has no value")?;
            evaluate_static_json(source, expression)
        }
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        "null" => Ok(Value::Null),
        "number" | "unary_expression" => evaluate_static_number(source, node)
            .map(Value::Number)
            .ok_or("unsupported number literal"),
        _ => Err("unsupported expression kind"),
    }
}

fn evaluate_static_number(source: &str, node: Node<'_>) -> Option<Number> {
    if node.kind() == "unary_expression" {
        let operator = node
            .child_by_field_name("operator")?
            .utf8_text(source.as_bytes())
            .ok()?;
        if !matches!(operator, "+" | "-") {
            return None;
        }
        let argument = node.child_by_field_name("argument")?;
        let value = evaluate_static_number(source, argument)?.as_f64()?;
        return normalized_javascript_number(if operator == "-" { -value } else { value });
    }
    if node.kind() != "number" {
        return None;
    }

    let literal = node.utf8_text(source.as_bytes()).ok()?.replace('_', "");
    let value = if let Some(digits) = literal
        .strip_prefix("0x")
        .or_else(|| literal.strip_prefix("0X"))
    {
        u64::from_str_radix(digits, 16).ok()? as f64
    } else if let Some(digits) = literal
        .strip_prefix("0b")
        .or_else(|| literal.strip_prefix("0B"))
    {
        u64::from_str_radix(digits, 2).ok()? as f64
    } else if let Some(digits) = literal
        .strip_prefix("0o")
        .or_else(|| literal.strip_prefix("0O"))
    {
        u64::from_str_radix(digits, 8).ok()? as f64
    } else {
        literal.parse::<f64>().ok()?
    };
    normalized_javascript_number(value)
}

fn normalized_javascript_number(value: f64) -> Option<Number> {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    if value == 0.0 {
        return Some(Number::from(0));
    }
    if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER {
        return if value.is_sign_negative() {
            Some(Number::from(value as i64))
        } else {
            Some(Number::from(value as u64))
        };
    }
    Number::from_f64(value)
}

fn static_property_key(source: &str, pair: Node<'_>) -> Option<String> {
    let key = pair.child_by_field_name("key")?;
    match key.kind() {
        "property_identifier" => decode_identifier_node(source, key),
        "string" => decode_quoted_string(source, key),
        "number" => key.utf8_text(source.as_bytes()).ok().map(str::to_string),
        _ => None,
    }
}

fn decode_quoted_string(source: &str, node: Node<'_>) -> Option<String> {
    let bytes = source.as_bytes();
    let quote = *bytes.get(node.start_byte())?;
    if !matches!(quote, b'\'' | b'"') || bytes.get(node.end_byte() - 1) != Some(&quote) {
        return None;
    }
    decode_string_contents(source, node.start_byte() + 1, node.end_byte() - 1, false)
}

fn decode_template_string(
    source: &str,
    node: Node<'_>,
    locals: &HashMap<String, String>,
) -> Option<String> {
    let bytes = source.as_bytes();
    if bytes.get(node.start_byte()) != Some(&b'`') || bytes.get(node.end_byte() - 1) != Some(&b'`')
    {
        return None;
    }

    let mut out = String::new();
    let mut source_cursor = node.start_byte() + 1;
    let mut child_cursor = node.walk();
    for child in node.named_children(&mut child_cursor) {
        if child.kind() != "template_substitution" {
            continue;
        }
        out.push_str(&decode_string_contents(
            source,
            source_cursor,
            child.start_byte(),
            true,
        )?);
        let expression = child.named_child(0)?;
        if expression.kind() != "identifier" {
            return None;
        }
        let identifier = decode_identifier_node(source, expression)?;
        out.push_str(locals.get(&identifier)?);
        source_cursor = child.end_byte();
    }
    out.push_str(&decode_string_contents(
        source,
        source_cursor,
        node.end_byte() - 1,
        true,
    )?);
    Some(out)
}

fn decode_string_contents(
    source: &str,
    start: usize,
    end: usize,
    normalize_raw_newlines: bool,
) -> Option<String> {
    let bytes = source.as_bytes();
    let mut out = String::new();
    let mut index = start;
    while index < end {
        if bytes.get(index) == Some(&b'\\') {
            let next = decode_escape(source, index, &mut out)?;
            if next > end {
                return None;
            }
            index = next;
        } else if normalize_raw_newlines && bytes.get(index) == Some(&b'\r') {
            out.push('\n');
            index += if bytes.get(index + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
        } else {
            let (character, next) = source_char(source, index)?;
            if next > end {
                return None;
            }
            out.push(character);
            index = next;
        }
    }
    Some(out)
}

fn decode_escape(source: &str, slash: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(slash) != Some(&b'\\') {
        return None;
    }
    let escaped = slash + 1;
    match *bytes.get(escaped)? {
        b'b' => push_escape(out, '\u{0008}', escaped + 1),
        b'f' => push_escape(out, '\u{000c}', escaped + 1),
        b'n' => push_escape(out, '\n', escaped + 1),
        b'r' => push_escape(out, '\r', escaped + 1),
        b't' => push_escape(out, '\t', escaped + 1),
        b'v' => push_escape(out, '\u{000b}', escaped + 1),
        b'0' if !bytes.get(escaped + 1).is_some_and(u8::is_ascii_digit) => {
            push_escape(out, '\0', escaped + 1)
        }
        b'0'..=b'9' => None,
        b'\n' => Some(escaped + 1),
        b'\r' => Some(if bytes.get(escaped + 1) == Some(&b'\n') {
            escaped + 2
        } else {
            escaped + 1
        }),
        0xe2 if source.get(escaped..)?.starts_with('\u{2028}')
            || source.get(escaped..)?.starts_with('\u{2029}') =>
        {
            Some(escaped + '\u{2028}'.len_utf8())
        }
        b'x' => {
            let (value, end) = parse_fixed_hex(source, escaped + 1, 2)?;
            push_escape(out, char::from_u32(value)?, end)
        }
        b'u' => decode_unicode_escape(source, slash, out),
        _ => {
            let (character, next) = source_char(source, escaped)?;
            push_escape(out, character, next)
        }
    }
}

fn push_escape(out: &mut String, character: char, end: usize) -> Option<usize> {
    out.push(character);
    Some(end)
}

fn decode_unicode_escape(source: &str, slash: usize, out: &mut String) -> Option<usize> {
    let bytes = source.as_bytes();
    let digits_start = slash + 2;
    if bytes.get(digits_start) == Some(&b'{') {
        let mut value = 0u32;
        let mut digits = 0usize;
        let mut index = digits_start + 1;
        while let Some(&byte) = bytes.get(index) {
            if byte == b'}' {
                if digits == 0 {
                    return None;
                }
                out.push(char::from_u32(value)?);
                return Some(index + 1);
            }
            value = value
                .checked_mul(16)?
                .checked_add(u32::from(hex_value(byte)?))?;
            digits += 1;
            index += 1;
        }
        return None;
    }

    let (first, first_end) = parse_fixed_hex(source, digits_start, 4)?;
    if (0xd800..=0xdbff).contains(&first) {
        if bytes.get(first_end) != Some(&b'\\') || bytes.get(first_end + 1) != Some(&b'u') {
            return None;
        }
        let (second, second_end) = parse_fixed_hex(source, first_end + 2, 4)?;
        if !(0xdc00..=0xdfff).contains(&second) {
            return None;
        }
        let scalar = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
        out.push(char::from_u32(scalar)?);
        Some(second_end)
    } else if (0xdc00..=0xdfff).contains(&first) {
        None
    } else {
        out.push(char::from_u32(first)?);
        Some(first_end)
    }
}

fn parse_fixed_hex(source: &str, start: usize, digits: usize) -> Option<(u32, usize)> {
    let end = start.checked_add(digits)?;
    let mut value = 0u32;
    for &byte in source.as_bytes().get(start..end)? {
        value = (value << 4) | u32::from(hex_value(byte)?);
    }
    Some((value, end))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn source_char(source: &str, index: usize) -> Option<(char, usize)> {
    let character = source.get(index..)?.chars().next()?;
    Some((character, index + character.len_utf8()))
}

#[cfg(test)]
mod tests {
    use super::{
        CypressRenderHelper, UnsupportedCypressReason, extract_cypress_render_cases,
        extract_package_test_strings,
    };

    #[test]
    fn cypress_extraction_preserves_unicode_and_static_options() {
        let source = r#"
it('renders unicode', () => {
  imgSnapshotTest(`flowchart LR
  开始 --> 完成🚀: \u007D \uD83D\uDE0E`, ({
    theme: 'dark',
    flowchart: { htmlLabels: false, titleTopMargin: 0 },
    negativeMargin: -4,
  } as const));
});
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert!(
            extraction.unsupported.is_empty(),
            "{:#?}",
            extraction.unsupported
        );
        assert_eq!(extraction.cases.len(), 1);
        assert_eq!(
            extraction.cases[0].diagram,
            "flowchart LR\n  开始 --> 完成🚀: } 😎"
        );
        assert_eq!(
            extraction.cases[0].test_name.as_deref(),
            Some("renders unicode")
        );
        assert_eq!(
            extraction.cases[0].options,
            serde_json::json!({
                "theme": "dark",
                "flowchart": { "htmlLabels": false, "titleTopMargin": 0 },
                "negativeMargin": -4,
            })
        );
    }

    #[test]
    fn cypress_extraction_uses_argument_zero_and_rejects_dynamic_values() {
        let source = r#"
it('dynamic graph', () => {
  imgSnapshotTest(makeGraph(), {}, false, 'flowchart LR\nWRONG-->B');
});
it('dynamic options', () => {
  imgSnapshotTest('flowchart LR\nA-->B', { theme });
});
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert!(extraction.cases.is_empty());
        assert_eq!(
            extraction
                .unsupported
                .iter()
                .map(|diagnostic| diagnostic.reason)
                .collect::<Vec<_>>(),
            [
                UnsupportedCypressReason::DynamicGraph,
                UnsupportedCypressReason::DynamicOptions("shorthand property is dynamic"),
            ]
        );
    }

    #[test]
    fn cypress_extraction_normalizes_escaped_helper_and_skip_identifiers() {
        let source = r#"
it.\u0073kip('skipped', () => {
  imgSnapshotTest('flowchart LR\nSKIP-->B');
});
it('active', () => {
  render\u0047raph('flowchart LR\nA-->B', {});
});
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert!(extraction.unsupported.is_empty());
        assert_eq!(extraction.cases.len(), 1);
        assert_eq!(extraction.cases[0].helper, CypressRenderHelper::RenderGraph);
        assert_eq!(extraction.cases[0].diagram, "flowchart LR\nA-->B");
    }

    #[test]
    fn cypress_extraction_rejects_multi_diagram_api_and_indirect_callbacks() {
        let source = r#"
it('multiple', () => {
  renderGraph(['flowchart LR\nA-->B', 'flowchart LR\nB-->C']);
});
it('api', () => {
  imgSnapshotTest('flowchart LR\nC-->D', {}, true);
});
function callback() {
  imgSnapshotTest('flowchart LR\nD-->E');
}
it('indirect', callback);
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert!(extraction.cases.is_empty());
        assert_eq!(
            extraction
                .unsupported
                .iter()
                .map(|diagnostic| diagnostic.reason)
                .collect::<Vec<_>>(),
            [
                UnsupportedCypressReason::MultipleGraphs,
                UnsupportedCypressReason::ApiRendering,
                UnsupportedCypressReason::NotInlineTest,
            ]
        );
    }

    #[test]
    fn cypress_extraction_treats_explicit_undefined_api_as_the_default_path() {
        let source = r#"
it('validation callback', () => {
  imgSnapshotTest('railroad-beta\ndigit = terminal("0") ;', {}, undefined, validate);
});
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert!(
            extraction.unsupported.is_empty(),
            "{:#?}",
            extraction.unsupported
        );
        assert_eq!(extraction.cases.len(), 1);
        assert_eq!(
            extraction.cases[0].diagram,
            "railroad-beta\ndigit = terminal(\"0\") ;"
        );
    }

    #[test]
    fn cypress_extraction_resolves_only_preceding_same_block_const_strings() {
        let source = r#"
const outer = 'WRONG';
it('same block', () => {
  const title = 'Test Title';
  renderGraph(`---
title: ${title}
---
flowchart LR
A-->B`, { layout: 'elk' });
});
it('outer scope', () => {
  renderGraph(`flowchart LR\n${outer}-->B`, {});
});
it('nested block', () => {
  if (ready) {
    const nested = 'WRONG';
  }
  renderGraph(`flowchart LR\n${nested}-->B`, {});
});
"#;

        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");

        assert_eq!(extraction.cases.len(), 1);
        assert!(extraction.cases[0].diagram.contains("title: Test Title"));
        assert_eq!(
            extraction
                .unsupported
                .iter()
                .map(|diagnostic| diagnostic.reason)
                .collect::<Vec<_>>(),
            [
                UnsupportedCypressReason::DynamicGraph,
                UnsupportedCypressReason::DynamicGraph,
            ]
        );
    }

    #[test]
    fn package_extraction_keeps_only_static_runtime_strings() {
        let source = r#"
import value from 'not-a-fixture';
type Diagram = 'stateDiagram-v2\nWRONG';
const graph = 'gitGraph TB:\n' + 'commit id: "完成🚀"\n';
const dynamic = `flowchart LR\nA["${label}"] --> B`;
const matcher = /"sequenceDiagram"/;
const tagged = html`classDiagram\nclass Wrong`;
it('flowchart LR\nTITLE-->ONLY', () => parser.parse(graph));
it.skip('skipped', () => {
  const skipped = 'stateDiagram-v2\n[*] --> WRONG';
});
"#;

        let strings = extract_package_test_strings(source).expect("valid TypeScript");

        assert_eq!(
            strings
                .into_iter()
                .map(|expression| expression.value)
                .collect::<Vec<_>>(),
            ["gitGraph TB:\ncommit id: \"完成🚀\"\n"]
        );
    }

    #[test]
    fn package_extraction_drops_fragments_used_by_dynamic_composition() {
        let source = r#"
const header = 'gitGraph:\n';
const graph = header + 'commit\n';
parser.parse(graph);
let accumulated = seed;
accumulated += 'stateDiagram-v2\n' + '\n';
"#;

        assert!(
            extract_package_test_strings(source)
                .expect("valid TypeScript")
                .is_empty()
        );
    }

    #[test]
    fn package_extraction_materializes_static_array_join_without_leaking_elements() {
        let source = r#"
const tree = [
  'treeView-beta', // header
  'root/', // root node
  '└── README.md', // leaf
].join('\n');
const wardley = [
  'wardley-beta',
  'title Example',
  'component Alpha [0.2, 0.1]',
].join('\n');
const dynamic = ['gitGraph:', branchName].join('\n');
parser.parse(['stateDiagram-v2', '[*] --> Done'].join('\n'));
expect(lines[0]).toBe('treeView-beta');
"#;

        assert_eq!(
            extract_package_test_strings(source)
                .expect("TypeScript source should parse")
                .into_iter()
                .map(|expression| expression.value)
                .collect::<Vec<_>>(),
            [
                "treeView-beta\nroot/\n└── README.md",
                "wardley-beta\ntitle Example\ncomponent Alpha [0.2, 0.1]",
                "stateDiagram-v2\n[*] --> Done",
            ]
        );
    }

    #[test]
    fn package_extraction_handles_pinned_tree_and_wardley_array_join_sources() {
        let source_root = crate::cmd::mermaid_repo_root().join("packages/mermaid/src/diagrams");
        let tree_path = source_root.join("treeView/boxDrawingPreprocessor.spec.ts");
        let wardley_path = source_root.join("wardley/wardleyParser.spec.ts");
        if !tree_path.is_file() || !wardley_path.is_file() {
            return;
        }

        let tree_source = std::fs::read_to_string(&tree_path).expect("read pinned treeView source");
        let tree_values = extract_package_test_strings(&tree_source)
            .expect("pinned treeView source should parse")
            .into_iter()
            .map(|expression| expression.value)
            .collect::<Vec<_>>();
        assert!(tree_values.contains(
            &"treeView-beta\nroot/\n├── src/\n│   └── index.js\n└── README.md".to_string()
        ));
        assert!(
            !tree_values.iter().any(|value| value == "treeView-beta"),
            "assertion text must not become a header-only fixture: {tree_values:#?}"
        );

        let wardley_source =
            std::fs::read_to_string(&wardley_path).expect("read pinned Wardley source");
        let wardley_values = extract_package_test_strings(&wardley_source)
            .expect("pinned Wardley source should parse")
            .into_iter()
            .map(|expression| expression.value)
            .collect::<Vec<_>>();
        assert!(wardley_values.contains(
            &"wardley-beta\ntitle Example\ncomponent Alpha [0.2, 0.1]\ncomponent Beta [0.4, 0.3]\nAlpha -> Beta".to_string()
        ));
    }

    #[test]
    fn string_decoder_handles_line_continuations_and_rejects_surrogates() {
        let source = r#"
it('continuation', () => {
  renderGraph("flowchart LR\
A-->B", {});
});
"#;
        let extraction = extract_cypress_render_cases(source).expect("valid TypeScript");
        assert_eq!(extraction.cases[0].diagram, "flowchart LRA-->B");

        let surrogate = r#"it('bad', () => renderGraph("flowchart LR\nA[\uD83D]", {}));"#;
        let extraction = extract_cypress_render_cases(surrogate).expect("valid TypeScript");
        assert_eq!(
            extraction.unsupported[0].reason,
            UnsupportedCypressReason::DynamicGraph
        );
    }

    #[test]
    fn invalid_typescript_is_rejected() {
        assert!(extract_cypress_render_cases("const broken = ;").is_err());
        assert!(extract_package_test_strings("const broken = ;").is_err());
    }
}
