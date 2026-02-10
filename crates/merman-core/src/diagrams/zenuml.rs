use crate::{Error, ParseMetadata, Result};
use serde_json::Value;

/// Parses a ZenUML diagram into a Mermaid-like semantic model.
///
/// Upstream Mermaid integrates ZenUML via the `mermaid-zenuml` external diagram package, which
/// uses `@zenuml/core` in the browser. `merman` is headless and pure Rust, so for now we implement
/// a conservative compatibility mode: a small ZenUML subset is translated into Mermaid
/// `sequenceDiagram` syntax and then parsed by the existing sequence parser.
///
/// This is intended to support basic `Actor->Actor: message` diagrams for headless integrations.
pub fn parse_zenuml(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut out = String::new();
    out.push_str("sequenceDiagram\n");

    let mut saw_header = false;

    for raw in code.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if !saw_header && line.to_ascii_lowercase().starts_with("zenuml") {
            saw_header = true;
            continue;
        }

        // Pass through common metadata directives as-is when possible.
        if line.to_ascii_lowercase().starts_with("title ") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if line.to_ascii_lowercase().starts_with("acctitle ") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if line.to_ascii_lowercase().starts_with("accdescr ") {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let Some(seq_line) = translate_message_line(line) else {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: format!("unsupported zenuml statement: {line}"),
            });
        };
        out.push_str(&seq_line);
        out.push('\n');
    }

    crate::diagrams::sequence::parse_sequence(&out, meta)
}

fn translate_message_line(line: &str) -> Option<String> {
    // Minimal supported syntax:
    //   Alice->Bob: Hello
    //   Bob-->Alice: Reply
    //
    // Map to Mermaid sequence syntax:
    //   Alice->>Bob: Hello
    //   Bob-->>Alice: Reply
    let (lhs, label) = if let Some((a, b)) = line.split_once(':') {
        (a.trim(), Some(b.trim()))
    } else {
        (line.trim(), None)
    };

    let (from, arrow, to) = if let Some((a, b)) = lhs.split_once("-->") {
        (a.trim(), "-->", b.trim())
    } else if let Some((a, b)) = lhs.split_once("->") {
        (a.trim(), "->", b.trim())
    } else {
        return None;
    };

    if from.is_empty() || to.is_empty() {
        return None;
    }

    let seq_arrow = match arrow {
        "-->" => "-->>",
        "->" => "->>",
        _ => return None,
    };

    let mut out = String::new();
    out.push_str(from);
    out.push_str(seq_arrow);
    out.push_str(to);
    if let Some(lbl) = label {
        if !lbl.is_empty() {
            out.push_str(": ");
            out.push_str(lbl);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use crate::{Engine, ParseOptions};

    #[test]
    fn zenuml_basic_translates_to_sequence_model() {
        let engine = Engine::new();
        let input = "zenuml\n  Alice->Bob: Hello\n  Bob-->Alice: Reply\n";
        let parsed =
            futures::executor::block_on(engine.parse_diagram(input, ParseOptions::lenient()))
                .unwrap()
                .unwrap();
        assert_eq!(parsed.meta.diagram_type, "zenuml");
        assert!(parsed.model.get("messages").is_some());
    }
}
