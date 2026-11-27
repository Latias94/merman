use super::*;

#[test]
fn split_ascii_indent_counts_only_spaces_and_tabs() {
    let (indent, rest) = split_ascii_indent(" \t\u{00A0}root");
    assert_eq!(indent, 2);
    assert_eq!(rest, "\u{00A0}root");
}

#[test]
fn box_drawing_eof_mapping_preserves_token_ends_and_eof_insertions() {
    for newline in ["\n", "\r\n"] {
        for class_suffix in ["", " :::ready"] {
            for ending in ["", newline] {
                let text = format!("treeView-beta{newline}├── file.txt{class_suffix}   {ending}");
                let (source, issues) = preprocess_tree_view_body(
                    &text,
                    "treeView-beta".len() + newline.len(),
                    &OperationControl::new(),
                )
                .unwrap();
                assert!(issues.is_empty());
                let eof = source.text.len();
                assert_eq!(
                    source.source_span(SourceSpan::new(eof, eof)),
                    SourceSpan::new(text.len(), text.len())
                );

                let crlf_extra = newline.len() - 1;
                let mut expected = vec![(
                    "file.txt",
                    SourceSpan::new(24 + crlf_extra, 32 + crlf_extra),
                )];
                if !class_suffix.is_empty() {
                    expected.push(("ready", SourceSpan::new(36 + crlf_extra, 41 + crlf_extra)));
                }
                for (payload, span) in &expected {
                    let start = source.text.find(*payload).unwrap();
                    assert_eq!(
                        source.source_span(SourceSpan::new(start, start + payload.len())),
                        *span
                    );
                    assert_eq!(&text[span.start..span.end], *payload);
                }
            }
        }
    }
}

#[test]
fn adjacent_quoted_nodes_scan_each_physical_line_once() {
    const HEADER: &str = "treeView-beta\n";
    for count in [8, 64, 1_024] {
        for ending in ["", "\n"] {
            let source = format!("{HEADER}{}{ending}", "\"x\"".repeat(count));
            TREE_VIEW_BODY_LINE_SCAN_BYTES.with(|bytes| bytes.set(0));
            let parsed = parse_tree_view_source_controlled(&source, &OperationControl::new())
                .unwrap()
                .unwrap();
            let scanned_bytes = TREE_VIEW_BODY_LINE_SCAN_BYTES.with(|bytes| bytes.get());
            assert_eq!(scanned_bytes, source.len() - HEADER.len());
            assert_eq!(parsed.nodes.len(), count);
            let model = parsed.into_model(&MermaidConfig::default()).unwrap();
            assert_eq!(model.root.children.len(), count);
            assert!(
                model.root.children.iter().all(|node| {
                    node.name == "x" && node.level == 0 && node.children.is_empty()
                })
            );
        }
    }
}
