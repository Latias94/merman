use futures::executor::block_on;
use merman_core::{Engine, MAX_DIAGRAM_NESTING_DEPTH, ParseOptions};

fn parse_err(input: &str) -> String {
    let engine = Engine::new();
    block_on(engine.parse_diagram(input, ParseOptions::strict()))
        .expect_err("deeply nested diagram should return a parse error")
        .to_string()
}

fn assert_nesting_error(name: &str, input: String) {
    let err = parse_err(&input);
    assert!(
        err.contains("nesting depth exceeds maximum"),
        "{name} error should mention nesting depth, got: {err}"
    );
}

fn flowchart(depth: usize) -> String {
    let mut lines = vec!["flowchart TD".to_string()];
    for i in 0..depth {
        lines.push(format!("subgraph n{i}"));
    }
    lines.push("leaf[leaf]".to_string());
    for _ in 0..depth {
        lines.push("end".to_string());
    }
    lines.join("\n")
}

fn state(depth: usize) -> String {
    let mut lines = vec!["stateDiagram-v2".to_string()];
    for i in 0..depth {
        lines.push(format!("state s{i} {{"));
    }
    lines.push("leaf".to_string());
    for _ in 0..depth {
        lines.push("}".to_string());
    }
    lines.join("\n")
}

fn block(depth: usize) -> String {
    let mut lines = vec!["block-beta".to_string()];
    for i in 0..depth {
        lines.push(format!("{}block:b{i}", "  ".repeat(i)));
    }
    lines.push(format!("{}leaf", "  ".repeat(depth)));
    for i in (0..depth).rev() {
        lines.push(format!("{}end", "  ".repeat(i)));
    }
    lines.join("\n")
}

fn mindmap(depth: usize) -> String {
    let mut lines = vec!["mindmap".to_string(), "root".to_string()];
    for i in 0..depth {
        lines.push(format!("{}n{i}", "  ".repeat(i + 1)));
    }
    lines.join("\n")
}

fn treemap(depth: usize) -> String {
    let mut lines = vec!["treemap-beta".to_string()];
    for i in 0..depth {
        lines.push(format!("{}\"n{i}\"", "  ".repeat(i)));
    }
    lines.push(format!("{}\"leaf\": 1", "  ".repeat(depth)));
    lines.join("\n")
}

fn c4(depth: usize) -> String {
    let mut lines = vec!["C4Context".to_string()];
    for i in 0..depth {
        lines.push(format!("{}Boundary(b{i}, \"B{i}\") {{", "  ".repeat(i)));
    }
    lines.push(format!("{}System(s, \"S\")", "  ".repeat(depth)));
    for i in (0..depth).rev() {
        lines.push(format!("{}}}", "  ".repeat(i)));
    }
    lines.join("\n")
}

#[test]
fn deeply_nested_flowchart_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("flowchart", flowchart(depth));
}

#[test]
fn deeply_nested_state_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("state", state(depth));
}

#[test]
fn deeply_nested_block_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("block", block(depth));
}

#[test]
fn deeply_nested_mindmap_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("mindmap", mindmap(depth));
}

#[test]
fn deeply_nested_treemap_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("treemap", treemap(depth));
}

#[test]
fn deeply_nested_c4_returns_parse_error() {
    let depth = MAX_DIAGRAM_NESTING_DEPTH + 2;
    assert_nesting_error("c4", c4(depth));
}
