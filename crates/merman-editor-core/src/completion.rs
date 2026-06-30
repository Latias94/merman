use crate::context::CompletionContext;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_core::{diagram_header_facts_for_profile, selected_baseline_registry_profile};
use serde::{Deserialize, Serialize};

const COMMON_TEMPLATE_DETAIL: &str = "diagram template";

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(context) = CompletionContext::from_snapshot(snapshot, position) else {
        return CompletionList {
            is_incomplete: false,
            items: Vec::new(),
        };
    };

    let mut items = Vec::new();

    if context.offer_diagram_headers() {
        items.extend(diagram_header_items(context.prefix_range()));
        items.extend(template_items(context.prefix_range()));
    } else if context.offer_template_items() {
        items.extend(template_items(context.prefix_range()));
    }

    if context.offer_operator_items() {
        items.extend(operator_items(context.operator_range()));
    }

    if context.offer_frontmatter_items() {
        items.extend(frontmatter_items(context.frontmatter_text_edit_range()));
    }

    if context.offer_direction_items() {
        items.extend(direction_items(context.prefix_range()));
    }

    if context.offer_directive_items() {
        items.extend(directive_items(&context));
    }

    if context.offer_shape_items() {
        items.extend(shape_items(&context));
    }

    if context.offer_class_name_items() {
        items.extend(class_name_items(
            context.fence(),
            context.document_uri(),
            context.class_name_text_edit_range(),
        ));
    }

    if context.offer_style_snippet_items() {
        items.extend(style_snippet_items(context.style_text_edit_range()));
    }

    if context.offer_interaction_snippet_items() {
        items.extend(interaction_snippet_items(
            context.interaction_text_edit_range(),
        ));
    }

    if context.offer_node_items() {
        items.extend(node_items(
            context.fence(),
            context.document_uri(),
            context.node_text_edit_range(),
        ));
    }

    CompletionList {
        is_incomplete: false,
        items,
    }
}

fn diagram_header_items(range: Option<Range>) -> Vec<CompletionItem> {
    diagram_header_facts_for_profile(selected_baseline_registry_profile())
        .iter()
        .map(|fact| {
            keyword_completion(
                fact.label,
                fact.detail,
                range,
                None,
                CompletionDataKind::DiagramHeader,
            )
        })
        .collect()
}

fn operator_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion(
            "-->",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "---",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "-.->",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "==>",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        snippet_completion(
            "-->|label|",
            "labeled edge operator",
            range,
            "-->|${1:label}|",
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "<|--",
            "inheritance operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "*--",
            "composition operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "o--",
            "aggregation operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
    ]
}

fn directive_items(context: &CompletionContext<'_>) -> Vec<CompletionItem> {
    let range = context.prefix_range();
    let has_directives = context
        .fence()
        .text_index
        .directive_prefixes()
        .any(|prefix| is_directive_helper_prefix(prefix.as_str()));
    let directive_label = if has_directives {
        "directive helper"
    } else {
        "comment"
    };
    vec![
        snippet_completion(
            ":::className",
            directive_label,
            range,
            ":::${1:className}",
            CompletionDataKind::Directive,
        ),
        snippet_completion(
            "::icon(name)",
            "node icon directive",
            range,
            "::icon(${1:logos:github-icon})",
            CompletionDataKind::Directive,
        ),
        snippet_completion(
            "%% comment",
            "comment",
            range,
            "%% ${1:comment}",
            CompletionDataKind::Directive,
        ),
    ]
}

fn is_directive_helper_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "classDef"
            | "class"
            | "style"
            | "cssClass"
            | "linkStyle"
            | "click"
            | "link"
            | "callback"
            | ":::"
    )
}

fn direction_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion(
            "direction TB",
            "top to bottom",
            range,
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction BT",
            "bottom to top",
            range,
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction LR",
            "left to right",
            range,
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction RL",
            "right to left",
            range,
            None,
            CompletionDataKind::Direction,
        ),
    ]
}

fn shape_items(context: &CompletionContext<'_>) -> Vec<CompletionItem> {
    vec![
        shape_completion("circle", "circle shape", context),
        shape_completion("rounded", "rounded shape", context),
        shape_completion("diamond", "diamond shape", context),
        shape_completion("hexagon", "hexagon shape", context),
        shape_completion("stadium", "stadium shape", context),
        shape_completion("subroutine", "subroutine shape", context),
        shape_completion("cylinder", "cylinder shape", context),
        shape_completion("trapezoid", "trapezoid shape", context),
        shape_completion("inv_trapezoid", "inverse trapezoid shape", context),
        shape_completion("doublecircle", "double circle shape", context),
    ]
}

fn node_items(
    fence: &FenceSnapshot,
    document_uri: &str,
    range: Option<Range>,
) -> Vec<CompletionItem> {
    fence
        .text_index
        .node_ids()
        .into_iter()
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: CompletionItemKind::Variable,
            detail: Some("node identifier".to_string()),
            data: Some(CompletionResolveData {
                kind: CompletionDataKind::NodeIdentifier,
                label: id.clone(),
            }),
            insert_text: Some(id.clone()),
            insert_text_format: CompletionInsertTextFormat::PlainText,
            text_edit: range.map(|range| CompletionTextEdit {
                range,
                new_text: id.clone(),
            }),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(document_uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
        })
        .collect()
}

fn class_name_items(
    fence: &FenceSnapshot,
    document_uri: &str,
    range: Option<Range>,
) -> Vec<CompletionItem> {
    fence
        .text_index
        .class_names()
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Class,
            detail: Some("class name".to_string()),
            data: Some(CompletionResolveData {
                kind: CompletionDataKind::ClassName,
                label: name.clone(),
            }),
            insert_text: Some(name.clone()),
            insert_text_format: CompletionInsertTextFormat::PlainText,
            text_edit: range.map(|range| CompletionTextEdit {
                range,
                new_text: name.clone(),
            }),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(document_uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
        })
        .collect()
}

fn style_snippet_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        snippet_completion(
            "fill/stroke style",
            "style properties",
            range,
            "fill:${1:#eef},stroke:${2:#447},stroke-width:${3:1px}",
            CompletionDataKind::Style,
        ),
        snippet_completion(
            "text style",
            "style properties",
            range,
            "color:${1:#222},font-size:${2:14px},font-weight:${3|normal,bold|}",
            CompletionDataKind::Style,
        ),
        snippet_completion(
            "dashed stroke style",
            "style properties",
            range,
            "stroke-dasharray:${1:5 5},stroke:${2:#447},stroke-width:${3:2px}",
            CompletionDataKind::Style,
        ),
    ]
}

fn interaction_snippet_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        snippet_completion(
            "href link action",
            "interaction action",
            range,
            "href \"${1:https://example.com}\" \"${2:Tooltip}\" ${3|_blank,_self|}",
            CompletionDataKind::Interaction,
        ),
        snippet_completion(
            "callback action",
            "interaction action",
            range,
            "call ${1:callback}(${2:arg})",
            CompletionDataKind::Interaction,
        ),
    ]
}

fn frontmatter_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        snippet_completion(
            "config:",
            "frontmatter config",
            range,
            "config:\n  ${1:theme}: ${2:default}",
            CompletionDataKind::Frontmatter,
        ),
        snippet_completion(
            "theme:",
            "frontmatter config",
            range,
            "theme: ${1|default,dark,forest,neutral,base|}",
            CompletionDataKind::Frontmatter,
        ),
        snippet_completion(
            "themeCSS: |",
            "frontmatter config",
            range,
            "themeCSS: |\n  ${1:.node rect { filter: drop-shadow(1px 1px 1px #999); }}",
            CompletionDataKind::Frontmatter,
        ),
        snippet_completion(
            "themeVariables:",
            "frontmatter config",
            range,
            "themeVariables:\n  ${1:primaryColor}: ${2:#f4f4f4}",
            CompletionDataKind::Frontmatter,
        ),
    ]
}

fn keyword_completion(
    label: &str,
    detail: &str,
    range: Option<Range>,
    replacement: Option<&str>,
    data_kind: CompletionDataKind,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: CompletionItemKind::Keyword,
        detail: Some(detail.to_string()),
        data: Some(CompletionResolveData {
            kind: data_kind,
            label: label.to_string(),
        }),
        insert_text: Some(label.to_string()),
        insert_text_format: CompletionInsertTextFormat::PlainText,
        text_edit: range.map(|range| CompletionTextEdit {
            range,
            new_text: replacement.unwrap_or(label).to_string(),
        }),
        label_details: None,
    }
}

fn snippet_completion(
    label: &str,
    detail: &str,
    range: Option<Range>,
    snippet: &str,
    data_kind: CompletionDataKind,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: CompletionItemKind::Snippet,
        detail: Some(detail.to_string()),
        data: Some(CompletionResolveData {
            kind: data_kind,
            label: label.to_string(),
        }),
        insert_text: Some(snippet.to_string()),
        insert_text_format: CompletionInsertTextFormat::Snippet,
        text_edit: range.map(|range| CompletionTextEdit {
            range,
            new_text: snippet.to_string(),
        }),
        label_details: None,
    }
}

fn template_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        snippet_completion(
            "flowchart template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "flowchart ${1|TD,TB,BT,LR,RL|}\n  ${2:A}[${3:Start}] --> ${4:B}[${5:Next}]",
            CompletionDataKind::Template,
        ),
        snippet_completion(
            "sequence template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "sequenceDiagram\n  participant ${1:A} as ${2:Alice}\n  participant ${3:B} as ${4:Bob}\n  ${1:A}->>${3:B}: ${5:Message}",
            CompletionDataKind::Template,
        ),
        snippet_completion(
            "icon node template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "${1:A}@{ icon: \"${2:logos:github-icon}\", form: \"${3|square,rounded,circle|}\", label: \"${4:Label}\" }",
            CompletionDataKind::Template,
        ),
        snippet_completion(
            "accessibility template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "accTitle: ${1:Diagram title}\naccDescr: ${2:Diagram description}",
            CompletionDataKind::Template,
        ),
        snippet_completion(
            "frontmatter config template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "---\nconfig:\n  theme: ${1|default,dark,forest,neutral,base|}\n---\n${2:flowchart TD}\n  ${3:A} --> ${4:B}",
            CompletionDataKind::Template,
        ),
        snippet_completion(
            "themeCSS frontmatter template",
            COMMON_TEMPLATE_DETAIL,
            range,
            "---\nconfig:\n  themeCSS: |\n    ${1:.node rect { filter: drop-shadow(1px 1px 1px #999); }}\n---\n${2:flowchart TD}\n  ${3:A} --> ${4:B}",
            CompletionDataKind::Template,
        ),
    ]
}

fn shape_completion(value: &str, detail: &str, context: &CompletionContext<'_>) -> CompletionItem {
    let label = format!("@{{ shape: {value} }}");
    if let Some(edit) = context.shape_value_edit(value) {
        keyword_completion(
            &label,
            detail,
            Some(edit.range),
            Some(&edit.replacement),
            CompletionDataKind::Shape,
        )
    } else {
        keyword_completion(
            &label,
            detail,
            context.shape_trigger_range(),
            Some(&label),
            CompletionDataKind::Shape,
        )
    }
}

pub fn completion_documentation(data: &CompletionResolveData) -> String {
    match data.kind {
        CompletionDataKind::DiagramHeader => format!(
            "Starts a Mermaid `{}` diagram. Use it as the first statement in a plain Mermaid file or fenced Mermaid block.",
            data.label
        ),
        CompletionDataKind::Operator => format!(
            "Inserts the Mermaid `{}` relationship operator between diagram identifiers.",
            data.label
        ),
        CompletionDataKind::Direction => format!(
            "Sets flow direction with `{}`. Direction statements are valid inside flowchart subgraphs and supported flowchart contexts.",
            data.label
        ),
        CompletionDataKind::Directive => format!(
            "Inserts `{}` as a Mermaid directive or comment helper for the current fence.",
            data.label
        ),
        CompletionDataKind::Shape => format!(
            "Inserts Mermaid flowchart shape object syntax for `{}`.",
            data.label
        ),
        CompletionDataKind::ClassName => format!(
            "Reuses the `{}` class name already defined in the current Mermaid fence.",
            data.label
        ),
        CompletionDataKind::NodeIdentifier => format!(
            "Reuses the `{}` identifier already present in the current Mermaid fence.",
            data.label
        ),
        CompletionDataKind::Style => {
            format!("Inserts Mermaid style properties for `{}`.", data.label)
        }
        CompletionDataKind::Interaction => format!(
            "Inserts a Mermaid click/link/callback action for `{}`.",
            data.label
        ),
        CompletionDataKind::Frontmatter => format!(
            "Inserts Mermaid frontmatter configuration for `{}`.",
            data.label
        ),
        CompletionDataKind::Template => format!(
            "Inserts the `{}` Mermaid authoring template with editable placeholders.",
            data.label
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub data: Option<CompletionResolveData>,
    pub insert_text: Option<String>,
    pub insert_text_format: CompletionInsertTextFormat,
    pub text_edit: Option<CompletionTextEdit>,
    pub label_details: Option<CompletionItemLabelDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionItemKind {
    Keyword,
    Variable,
    Class,
    Snippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionInsertTextFormat {
    PlainText,
    Snippet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionTextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItemLabelDetails {
    pub description: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionDataKind {
    DiagramHeader,
    Operator,
    Direction,
    Directive,
    Shape,
    ClassName,
    NodeIdentifier,
    Style,
    Interaction,
    Frontmatter,
    Template,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResolveData {
    pub kind: CompletionDataKind,
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::{CompletionDataKind, completion_for_snapshot};
    use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
    use crate::types::{DocumentKind, DocumentUri, Position};
    use crate::workspace::DocumentWorkspace;
    use merman_analysis::{FenceTextIndex, SourceMap};
    use merman_core::{
        EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
        EditorSemanticSymbol, SourceSpan,
    };

    #[test]
    fn markdown_outside_mermaid_fence_returns_no_completion() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            "file:///tmp/readme.md",
            1,
            "# Notes\n\nplain prose\n".to_string(),
            DocumentKind::Markdown,
        );

        let completion = completion_for_snapshot(&snapshot, Position::new(2, 3));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn source_start_offers_headers_and_templates() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            "file:///tmp/example.mmd",
            1,
            "flow".to_string(),
            DocumentKind::Diagram,
        );

        let completion = completion_for_snapshot(&snapshot, Position::new(0, 4));

        assert!(
            completion
                .items
                .iter()
                .any(|item| item.data.as_ref().is_some_and(|data| {
                    data.kind == CompletionDataKind::DiagramHeader
                        && data.label.starts_with("flowchart")
                }))
        );
        assert!(
            completion
                .items
                .iter()
                .any(|item| item.data.as_ref().is_some_and(|data| {
                    data.kind == CompletionDataKind::Template && data.label == "flowchart template"
                }))
        );
    }

    #[test]
    fn unsupported_diagram_body_context_returns_no_completion() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nunsupported".to_string(),
            DocumentKind::Diagram,
        );

        let completion = completion_for_snapshot(&snapshot, Position::new(1, "unsupported".len()));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn parser_payload_context_returns_no_completion() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(28, 33),
        ));
        let snapshot = snapshot_with_facts(
            "sequenceDiagram\nAlice->Bob: Hello",
            Some("sequence"),
            facts,
        );

        let completion = completion_for_snapshot(&snapshot, Position::new(1, 18));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn parser_expected_node_slot_reuses_known_entity_ids() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "A",
            Some("flowchart node".to_string()),
            EditorSemanticKind::Module,
            SourceSpan::new(13, 14),
            SourceSpan::new(13, 14),
        ));
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            SourceSpan::new(18, 18),
        ));
        let snapshot = snapshot_with_facts("flowchart TD\nA--> ", Some("flowchart-v2"), facts);

        let completion = completion_for_snapshot(&snapshot, Position::new(1, 5));

        assert_eq!(
            completion
                .items
                .iter()
                .filter(|item| item
                    .data
                    .as_ref()
                    .is_some_and(|data| { data.kind == CompletionDataKind::NodeIdentifier }))
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["A"]
        );
    }

    fn snapshot_with_facts(
        text: &str,
        diagram_type: Option<&str>,
        facts: EditorSemanticFacts,
    ) -> DocumentSnapshot {
        DocumentSnapshot {
            uri: DocumentUri::from("file:///tmp/example.mmd"),
            version: 1,
            kind: DocumentKind::Diagram,
            text: text.to_string(),
            source_map: SourceMap::new(text.to_string()),
            fences: vec![FenceSnapshot {
                index: 0,
                start: 0,
                body_start: 0,
                end: text.len(),
                text: text.to_string(),
                diagram_type: diagram_type.map(str::to_string),
                text_index: FenceTextIndex::from_core_facts(facts),
            }],
        }
    }
}
