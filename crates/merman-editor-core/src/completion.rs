use crate::context::CompletionQuery;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_analysis::FenceTextIndexSource;
use merman_core::{EditorExpectedSyntaxKind, diagram_header_facts};
use serde::{Deserialize, Serialize};

const COMMON_TEMPLATE_DETAIL: &str = "diagram template";
pub const COMPLETION_TRIGGER_CHARACTERS: &[char] =
    &[' ', '\n', '-', '>', '%', '[', '(', '{', '/', '\\', '@', ':'];
const TEMPLATE_PREFIXES: &[&str] = &[
    "flow", "seq", "icon", "acc", "class", "state", "er", "gantt", "pie", "journey", "mind",
];

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(query) = CompletionQuery::from_snapshot(snapshot, position) else {
        return CompletionList {
            is_incomplete: false,
            fact_source: None,
            items: Vec::new(),
        };
    };

    let mut items = Vec::new();
    let expected_syntax = query.expected_syntax.map(|expected| expected.kind);
    let parser_backed = query.source.is_parser_backed();
    let offer_headers =
        expected_syntax.is_none() && offer_diagram_headers(query.source_start, query.prefix);
    let offer_templates = offer_template_items(&query);
    let offer_operator = expected_syntax == Some(EditorExpectedSyntaxKind::Operator);
    let offer_direction = expected_syntax == Some(EditorExpectedSyntaxKind::DirectionValue);
    let offer_shape = matches!(
        expected_syntax,
        Some(EditorExpectedSyntaxKind::ShapeValue | EditorExpectedSyntaxKind::ShapeTrigger)
    );
    let offer_directives =
        parser_backed && expected_syntax == Some(EditorExpectedSyntaxKind::Directive);
    let offer_frontmatter = expected_syntax == Some(EditorExpectedSyntaxKind::Frontmatter)
        || (query.source_start && query.source.is_unavailable());
    let offer_class_names =
        parser_backed && expected_syntax == Some(EditorExpectedSyntaxKind::ClassName);
    let offer_styles =
        parser_backed && expected_syntax == Some(EditorExpectedSyntaxKind::StyleValue);
    let offer_interactions =
        parser_backed && expected_syntax == Some(EditorExpectedSyntaxKind::InteractionAction);
    let offer_nodes = matches!(
        expected_syntax,
        Some(EditorExpectedSyntaxKind::NodeIdentifier | EditorExpectedSyntaxKind::IdList)
    );

    if offer_headers {
        items.extend(diagram_header_items(query.prefix_range()));
        items.extend(template_items(query.prefix_range()));
    } else if offer_templates {
        items.extend(template_items(query.prefix_range()));
    }

    if offer_operator {
        items.extend(operator_items(&query, query.operator_range()));
    }

    if offer_frontmatter {
        items.extend(frontmatter_items(query.prefix_range()));
    }

    if offer_direction {
        items.extend(direction_items(&query));
    }

    if offer_directives {
        items.extend(directive_items(&query));
    }

    if offer_shape {
        items.extend(shape_items(&query));
    }

    if offer_class_names {
        items.extend(class_name_items(
            query.fence,
            query.document_uri(),
            query.class_name_range(),
        ));
    }

    if offer_styles {
        items.extend(style_snippet_items(query.style_range()));
    }

    if offer_interactions {
        items.extend(interaction_snippet_items(query.interaction_range()));
    }

    if offer_nodes {
        items.extend(node_items(
            query.fence,
            query.document_uri(),
            query.expected_node_range(),
        ));
    }

    CompletionList {
        is_incomplete: false,
        fact_source: Some(query.source),
        items,
    }
}

fn offer_diagram_headers(source_start: bool, prefix: &str) -> bool {
    if !source_start {
        return false;
    }
    let prefix = prefix.trim_end();

    prefix.is_empty()
        || (!prefix.chars().any(char::is_whitespace)
            && diagram_header_facts()
                .iter()
                .any(|fact| fact.label.starts_with(prefix)))
}

fn offer_template_items(query: &CompletionQuery<'_>) -> bool {
    if !query.source_start || query.in_directive {
        return false;
    }
    let prefix = query.prefix.trim_end();
    !prefix.is_empty()
        && !prefix.chars().any(char::is_whitespace)
        && TEMPLATE_PREFIXES
            .iter()
            .any(|template_prefix| template_prefix.starts_with(prefix))
}

fn diagram_header_items(range: Option<Range>) -> Vec<CompletionItem> {
    diagram_header_facts()
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

fn operator_items(query: &CompletionQuery<'_>, range: Option<Range>) -> Vec<CompletionItem> {
    query
        .fence
        .text_index()
        .completion_vocabulary()
        .operators()
        .iter()
        .map(|candidate| {
            if let Some(snippet) = candidate.snippet_text() {
                snippet_completion(
                    candidate.label(),
                    candidate.detail(),
                    range,
                    snippet,
                    CompletionDataKind::Operator,
                )
            } else {
                keyword_completion(
                    candidate.label(),
                    candidate.detail(),
                    range,
                    None,
                    CompletionDataKind::Operator,
                )
            }
        })
        .collect()
}

fn directive_items(query: &CompletionQuery<'_>) -> Vec<CompletionItem> {
    let range = query.prefix_range();
    vec![
        snippet_completion(
            ":::className",
            "directive helper",
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

fn direction_items(query: &CompletionQuery<'_>) -> Vec<CompletionItem> {
    let values = query
        .fence
        .text_index()
        .completion_vocabulary()
        .directions();
    if let Some(range) = query.direction_value_range() {
        return values
            .iter()
            .map(|candidate| {
                keyword_completion(
                    candidate.label(),
                    candidate.detail(),
                    Some(range),
                    None,
                    CompletionDataKind::Direction,
                )
            })
            .collect();
    }

    values
        .iter()
        .map(|candidate| {
            keyword_completion(
                &format!("direction {}", candidate.label()),
                candidate.detail(),
                query.prefix_range(),
                None,
                CompletionDataKind::Direction,
            )
        })
        .collect()
}

fn shape_items(query: &CompletionQuery<'_>) -> Vec<CompletionItem> {
    merman_core::diagrams::flowchart::flowchart_public_shape_names()
        .map(|shape| shape_completion(shape, &format!("{shape} shape"), query))
        .collect()
}

fn node_items(
    fence: &FenceSnapshot,
    document_uri: &str,
    range: Option<Range>,
) -> Vec<CompletionItem> {
    fence
        .text_index()
        .node_ids()
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
                detail: Some(format!("fence {}", fence.index() + 1)),
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
        .text_index()
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
                detail: Some(format!("fence {}", fence.index() + 1)),
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

fn shape_completion(value: &str, detail: &str, query: &CompletionQuery<'_>) -> CompletionItem {
    let label = format!("@{{ shape: {value} }}");
    if let Some(edit) = query.shape_value_edit(value) {
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
            query.shape_trigger_range(),
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
            "Sets the diagram direction with `{}` in the current Mermaid family and syntax context.",
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
    pub fact_source: Option<FenceTextIndexSource>,
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
    use crate::types::{DocumentKind, Position};
    use crate::workspace::DocumentWorkspace;

    #[test]
    fn markdown_outside_mermaid_fence_returns_no_completion() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/readme.md",
                1,
                "# Notes\n\nplain prose\n".to_string(),
                DocumentKind::Markdown,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(2, 3));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn source_start_offers_headers_and_templates() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                "flow".to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

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
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                "flowchart TD\nunsupported".to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(1, "unsupported".len()));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn parser_payload_context_returns_no_completion() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                "sequenceDiagram\nAlice->Bob: Hello".to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(1, 18));

        assert!(completion.items.is_empty());
    }

    #[test]
    fn parser_expected_node_slot_reuses_known_entity_ids() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                "flowchart TD\nA--> ".to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

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

    #[test]
    fn parser_expected_flowchart_direction_edits_only_direction_value() {
        let text = "flowchart TD\nsubgraph group\ndirection L\nend\n";
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                text.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(2, "direction L".len()));

        let labels = completion
            .items
            .iter()
            .filter(|item| {
                item.data
                    .as_ref()
                    .is_some_and(|data| data.kind == CompletionDataKind::Direction)
            })
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["TB", "TD", "BT", "LR", "RL"]);

        let item = completion
            .items
            .iter()
            .find(|item| item.label == "LR")
            .expect("LR direction completion");
        let edit = item.text_edit.as_ref().expect("direction text edit");
        assert_eq!(edit.new_text, "LR");
        assert_eq!(edit.range.start.line, 2);
        assert_eq!(edit.range.start.character, "direction ".len());
        assert_eq!(edit.range.end.line, 2);
        assert_eq!(edit.range.end.character, "direction L".len());
    }

    #[test]
    fn parser_expected_block_arrow_direction_uses_block_values() {
        let text = "block\n  blockArrow<[\"&nbsp;\"]>(r";
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                "file:///tmp/example.mmd",
                1,
                text.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(
            &snapshot,
            Position::new(1, "  blockArrow<[\"&nbsp;\"]>(r".len()),
        );
        assert_eq!(
            completion.fact_source,
            Some(merman_analysis::FenceTextIndexSource::ParserRecovered)
        );

        let labels = completion
            .items
            .iter()
            .filter(|item| {
                item.data
                    .as_ref()
                    .is_some_and(|data| data.kind == CompletionDataKind::Direction)
            })
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["right", "left", "up", "down", "x", "y"]);
        assert!(!labels.contains(&"LR"));
        assert!(!labels.contains(&"direction LR"));

        let item = completion
            .items
            .iter()
            .find(|item| item.label == "right")
            .expect("right direction completion");
        let edit = item.text_edit.as_ref().expect("direction text edit");
        assert_eq!(edit.new_text, "right");
        assert_eq!(edit.range.start.line, 1);
        assert_eq!(
            edit.range.start.character,
            "  blockArrow<[\"&nbsp;\"]>(".len()
        );
        assert_eq!(edit.range.end.line, 1);
        assert_eq!(
            edit.range.end.character,
            "  blockArrow<[\"&nbsp;\"]>(r".len()
        );
    }
}
