#![cfg(all(feature = "native-fonts", feature = "diagram-sequence"))]

use merman::svg::export::{NativeFontContext, NativeFontOptions};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};

fn font_context() -> NativeFontContext {
    let fonts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    NativeFontOptions::default()
        .with_system_fonts(false)
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .with_font_file(fonts.join("Cousine-Bold.ttf"))
        .load(&OperationControl::new())
        .expect("the checked-in Cousine regular and bold fonts load")
}

fn render_note(context: &NativeFontContext, placement: &str, label: &str, wrap: bool) -> String {
    // Spotless consumes weight 600. Config keeps this regression usable on 0.9,
    // which does not expose the current typed theme API.
    let source = format!(
        "---\nconfig:\n  fontFamily: Cousine\n  sequence:\n    noteFontWeight: 600\n    wrap: {wrap}\n---\nsequenceDiagram\nparticipant A as Client\nNote {placement}: {label}\n"
    );
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context.clone())
            .expect("native font environment"),
        ..Default::default()
    };
    match Renderer::new()
        .render(RenderRequest::svg(
            &source,
            OperationControl::new(),
            request,
        ))
        .unwrap_or_else(|error| panic!("Note {placement}: {label:?}, wrap={wrap}: {error}"))
    {
        RenderOutput::Svg(Some(output)) => output.into_parts().0,
        other => panic!("expected a Sequence SVG, got {other:?}"),
    }
}

fn has_class(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_whitespace().any(|class| class == name))
}

fn content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn assert_note_bounds(
    svg: &str,
    context: &NativeFontContext,
    placement: &str,
    label: &str,
) -> usize {
    let document = roxmltree::Document::parse(svg).expect("rendered SVG parses");
    let notes: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("rect") && has_class(*node, "note"))
        .collect();
    assert_eq!(notes.len(), 1, "the fixture must contain one note frame");
    let note = notes[0];
    let group = note.parent_element().expect("note group");
    let labels: Vec<_> = group
        .descendants()
        .filter(|node| node.has_tag_name("text") && has_class(*node, "noteText"))
        .collect();
    assert!(!labels.is_empty(), "the note must emit SVG text");
    let lines: Vec<_> = labels.iter().copied().map(content).collect();
    assert!(
        lines.iter().all(|line| !line.trim().is_empty()),
        "each fixture row must contain actual label text: {lines:?}"
    );
    assert_eq!(
        lines.join(" ").split_whitespace().collect::<Vec<_>>(),
        label
            .replace("<br/>", " ")
            .split_whitespace()
            .collect::<Vec<_>>(),
        "splitting and wrapping must preserve the complete note label"
    );

    // Only add IDs to the actual output. Preserve its CSS, text, transforms and
    // geometry so usvg independently shapes exactly what a consumer receives.
    assert!(
        note.attribute("id").is_none(),
        "note probe ID is unambiguous"
    );
    let mut annotations = vec![(
        note.range().start + "<rect".len(),
        " id=\"native-sequence-note-frame\"".to_owned(),
    )];
    for (index, text) in labels.iter().enumerate() {
        assert!(
            text.attribute("id").is_none(),
            "text probe ID is unambiguous"
        );
        annotations.push((
            text.range().start + "<text".len(),
            format!(" id=\"native-sequence-note-label-{index}\""),
        ));
    }
    annotations.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    let mut annotated = svg.to_owned();
    for (position, attribute) in annotations {
        annotated.insert_str(position, &attribute);
    }
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(&annotated, &options)
        .expect("the actual SVG shapes with the fixed font context");
    let bounds = tree
        .node_by_id("native-sequence-note-frame")
        .expect("the note frame survives SVG parsing")
        .abs_bounding_box();
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    for (index, line) in lines.iter().enumerate() {
        let id = format!("native-sequence-note-label-{index}");
        let Some(usvg::Node::Text(text)) = tree.node_by_id(&id) else {
            panic!("note row {index} must survive actual font shaping: {line:?}");
        };
        assert!(
            !text.flattened().children().is_empty(),
            "note row {index} must produce visible glyph outlines: {line:?}"
        );
        for span in text.chunks().iter().flat_map(|chunk| chunk.spans()) {
            assert_eq!(span.font().weight(), 600, "the consumed note font weight");
        }
        let glyphs: Vec<_> = text
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .collect();
        assert!(!glyphs.is_empty(), "note row {index} must shape glyphs");
        for glyph in glyphs {
            assert_eq!(
                tree.fontdb().face(glyph.font).unwrap().post_script_name,
                "Cousine-Bold",
                "weight 600 must select the fixed bold face"
            );
        }
        // usvg 0.47 creates the flattened root with an identity transform;
        // it does not inherit Text::abs_transform. Map its local outline bounds
        // into the same canvas coordinates as the text metrics and the frame.
        let outline_bounds = text
            .flattened()
            .bounding_box()
            .transform(text.abs_transform())
            .expect("the shaped glyph outlines have valid canvas bounds");
        for (kind, text_bounds) in [
            ("text", text.abs_bounding_box()),
            ("glyph outlines", outline_bounds),
        ] {
            let epsilon = 0.05;
            assert!(text_bounds.width() > 0.0 && text_bounds.height() > 0.0);
            assert!(
                text_bounds.x() + epsilon >= bounds.x()
                    && text_bounds.y() + epsilon >= bounds.y()
                    && text_bounds.right() <= bounds.right() + epsilon
                    && text_bounds.bottom() <= bounds.bottom() + epsilon,
                "Note {placement}, row {index} {line:?}: {kind} {text_bounds:?} must fit frame {bounds:?}"
            );
        }
    }
    lines.len()
}

fn assert_note_placements(label: &str, wrap: bool, expected_rows: Option<usize>) {
    let context = font_context();
    for placement in ["right of A", "left of A", "over A"] {
        let svg = render_note(&context, placement, label, wrap);
        let rows = assert_note_bounds(&svg, &context, placement, label);
        if let Some(expected) = expected_rows {
            assert_eq!(rows, expected, "Note {placement}: {label:?}");
        } else {
            assert!(rows > 1, "Note {placement} must actually wrap: {label:?}");
        }
    }
}

#[test]
fn native_sequence_single_line_notes_contain_the_drawn_label() {
    assert_note_placements("Verify token before returning data", false, Some(1));
}

#[test]
fn native_sequence_br_notes_contain_every_drawn_row() {
    assert_note_placements("Verify token<br/>Return data", false, Some(2));
}

#[test]
fn native_sequence_wrapped_notes_contain_every_drawn_row() {
    assert_note_placements(
        "Verify the session token before returning the requested data",
        true,
        None,
    );
}
