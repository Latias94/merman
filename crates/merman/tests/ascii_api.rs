#![cfg(feature = "ascii")]

use merman::ascii::{AsciiRenderOptions, HeadlessAsciiRenderer, render_ascii_sync};

#[test]
fn render_ascii_sync_renders_flowchart_from_mermaid_text() {
    let engine = merman::Engine::new();
    let rendered = render_ascii_sync(
        &engine,
        "flowchart LR\nA --> B",
        merman::ParseOptions::strict(),
        &AsciiRenderOptions::ascii(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn headless_ascii_renderer_renders_sequence_with_unicode_defaults() {
    let renderer = HeadlessAsciiRenderer::new().with_strict_parsing();
    let rendered = renderer
        .render_ascii_sync("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello")
        .unwrap()
        .unwrap();

    assert!(rendered.contains("┌"));
    assert!(rendered.contains("Hello"));
    assert!(rendered.contains("►"));
}

#[test]
fn render_ascii_sync_returns_none_when_no_diagram_is_detected() {
    let engine = merman::Engine::new();
    let rendered = render_ascii_sync(
        &engine,
        "this is just prose",
        merman::ParseOptions::lenient(),
        &AsciiRenderOptions::default(),
    )
    .unwrap();

    assert!(rendered.is_none());
}
