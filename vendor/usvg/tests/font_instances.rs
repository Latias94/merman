use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use usvg::fontdb::{Database, ID, Source};
use usvg::{FontInstance, FontOpticalSizing, FontVariation, ImageKind, Node, Options, Text, Tree};

const COLLECTION: &[u8] = include_bytes!("fonts/instance-transport.ttc");

fn variable_face(db: &mut Database) -> ID {
    let ids = db.load_font_source(Source::Binary(Arc::new(COLLECTION)));
    let id = ids[1];
    assert_eq!(db.face(id).unwrap().index, 1);
    id
}

fn render(options: &Options, attrs: &str, content: &str) -> Tree {
    Tree::from_str(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="150">
                <text id="label" x="20" y="60" {attrs}>{content}</text>
            </svg>"#
        ),
        options,
    )
    .unwrap()
}

fn text(tree: &Tree) -> &Text {
    let Some(Node::Text(text)) = tree.node_by_id("label") else {
        panic!("expected the rendered text node");
    };
    text
}

fn axis(tag: &[u8; 4], value: f32) -> FontVariation {
    FontVariation::new(*tag, value)
}

fn weight(id: ID, scale: f32, value: f32) -> FontInstance {
    (id, scale, vec![axis(b"wght", value)])
}

fn scalar_options(db: &Arc<Database>, id: ID) -> Options<'static> {
    let mut options = Options {
        fontdb: db.clone(),
        ..Options::default()
    };
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    options
}

#[test]
fn effective_axes_size_and_metrics_match_explicit_coordinates() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    let direct = scalar_options(&options.fontdb, id);
    let native = vec![
        axis(b"wght", 850.0),
        axis(b"wght", f32::NAN),
        axis(b"wdth", 75.0),
        axis(b"ital", 1.0),
        axis(b"slnt", -12.0),
        axis(b"opsz", 9.0),
        axis(b"CSTM", 4.0),
        axis(b"fake", 300.0),
    ];
    options.set_font_instances(
        Some(Box::new(move |_, _| Some((id, 1.5, native.clone())))),
        None,
    );

    let native_tree = render(&options, "font-size='20'", "AX");
    let span = &text(&native_tree).layouted()[0];
    assert_eq!(span.font_size.get(), 30.0);
    assert_eq!(span.font_optical_sizing, FontOpticalSizing::None);
    assert_eq!(
        span.variations,
        [
            axis(b"CSTM", 1.0),
            axis(b"ital", 1.0),
            axis(b"opsz", 30.0),
            axis(b"slnt", -12.0),
            axis(b"wdth", 75.0),
            axis(b"wght", 850.0),
        ]
    );

    // The fixture's MVAR ascender varies linearly from 800 to 900 as weight runs
    // from 100 to 900. The ascender is rounded to font units before layout.
    let expected_height = (894.0 + 200.0) * 30.0 / 1000.0;
    let actual_height = text(&native_tree).bounding_box().height();
    assert!(
        (actual_height - expected_height).abs() < 0.001,
        "{actual_height} vs {expected_height}"
    );

    let decoration =
        "alignment-baseline='text-before-edge' text-decoration='underline overline line-through'";
    let native_tree = render(&options, &format!("font-size='20' {decoration}"), "AX");
    let control = render(
        &direct,
        &format!(
            r#"font-size='30' {decoration} font-variation-settings="'wght' 850, 'wdth' 75, 'ital' 1, 'slnt' -12, 'opsz' 30, 'CSTM' 1""#
        ),
        "AX",
    );
    assert_eq!(
        text(&native_tree).bounding_box(),
        text(&control).bounding_box()
    );
    assert_eq!(
        text(&native_tree).stroke_bounding_box(),
        text(&control).stroke_bounding_box()
    );
    assert_eq!(
        native_tree.to_string(&usvg::WriteOptions::default()),
        control.to_string(&usvg::WriteOptions::default())
    );

    let normal = render(
        &options,
        "font-size='20' style='font-weight: normal; font-style: normal; font-stretch: normal; font-optical-sizing: none'",
        "AX",
    );
    assert_eq!(
        text(&normal).layouted()[0].variations,
        [axis(b"CSTM", 1.0), axis(b"opsz", 9.0), axis(b"wght", 400.0)]
    );
    let explicit = render(
        &options,
        r#"font-size='20' font-weight='normal' font-variation-settings="'wght' 650, 'opsz' 22, 'wght' 700""#,
        "AX",
    );
    let explicit = &text(&explicit).layouted()[0].variations;
    assert!(explicit.contains(&axis(b"wght", 700.0)));
    assert!(explicit.contains(&axis(b"opsz", 22.0)));

    let default_opsz = render(
        &options,
        r#"font-size='20' font-variation-settings="'opsz' 12""#,
        "AX",
    );
    let span = &text(&default_opsz).layouted()[0];
    assert_eq!(span.font_optical_sizing, FontOpticalSizing::None);
    assert!(!span.variations.iter().any(|axis| axis.tag == *b"opsz"));

    let default = render(&direct, "font-optical-sizing='none'", "AX");
    assert_eq!(
        text(&default).layouted()[0].variations,
        [axis(b"wght", 400.0)]
    );
}

#[test]
fn fallback_retries_the_same_face_and_keeps_distinct_instances_apart() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    let calls = Arc::new(AtomicUsize::new(0));
    let fallback_calls = calls.clone();
    options.set_font_instances(
        Some(Box::new(move |font, _| {
            assert_eq!(font.selection_language(), Some("sr-latn"));
            Some(weight(id, 1.0, 100.0))
        })),
        Some(Box::new(move |_, cluster, lang, rtl, tried, _| {
            assert_eq!(lang, Some("sr-Latn"));
            assert!(!rtl);
            assert_eq!(tried[0], weight(id, 1.0, 100.0));
            fallback_calls.fetch_add(1, Ordering::Relaxed);
            match tried.len() {
                1 => Some(weight(id, 0.0, 900.0)),
                2 => Some(weight(id, 1.5, 120.0)),
                3 => {
                    assert_eq!(tried[1..], [weight(id, 0.0, 900.0), weight(id, 1.5, 120.0)]);
                    Some(weight(id, 1.5, if cluster == "A" { 900.0 } else { 650.0 }))
                }
                _ => panic!("unexpected additional attempts for {cluster}"),
            }
        })),
    );
    options.set_text_fallback(
        Some("fr".into()),
        Some(Box::new(|_, _, _, _, _, _| {
            panic!("the installed instance resolver owns fallback selection");
        })),
    );

    let tree = render(&options, "xml:lang='sr-Latn' font-size='20'", "XAVA");
    let text = text(&tree);
    assert_eq!(calls.load(Ordering::Relaxed), 6);
    let spans = text.layouted();
    assert_eq!(spans.len(), 4);
    for (span, (letter, size, weight)) in spans.iter().zip([
        ("X", 20.0, None),
        ("A", 30.0, Some(900.0)),
        ("V", 30.0, Some(650.0)),
        ("A", 30.0, Some(900.0)),
    ]) {
        assert_eq!(span.font_size.get(), size);
        assert_eq!(span.positioned_glyphs.len(), 1);
        let glyph = &span.positioned_glyphs[0];
        assert_eq!(glyph.text, letter);
        assert_eq!(glyph.font, id);
        assert_eq!(glyph.font_size(), size);
        assert_ne!(glyph.id.0, 0);
        let mut expected = vec![axis(b"opsz", size)];
        if let Some(weight) = weight {
            expected.push(axis(b"wght", weight));
        }
        assert_eq!(span.variations, expected);
    }
}

#[test]
fn equivalent_fallback_instances_join_across_physical_face_aliases() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    let face = options.fontdb.face(id).unwrap().clone();
    let alias = options.fontdb_mut().push_face_info(face);
    assert_ne!(id, alias);
    options.set_font_instances(
        Some(Box::new(move |_, _| Some(weight(id, 1.0, 100.0)))),
        Some(Box::new(move |_, cluster, _, _, _, _| {
            Some(if cluster == "A" {
                weight(id, 1.0, 900.0)
            } else {
                weight(alias, 1.0, 990.0)
            })
        })),
    );

    let tree = render(&options, "font-size='20'", "AV");
    let spans = text(&tree).layouted();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].positioned_glyphs.len(), 1);
    assert_eq!(spans[0].positioned_glyphs[0].text, "AV");
    assert_eq!(
        spans[0].variations,
        [axis(b"opsz", 20.0), axis(b"wght", 900.0)]
    );
}

#[test]
fn variable_colr_layers_use_the_selected_outline() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    options.set_font_instances(
        Some(Box::new(move |_, _| Some(weight(id, 1.0, 850.0)))),
        None,
    );

    let default = render(
        &options,
        "font-optical-sizing='none' font-weight='100'",
        "Z",
    );
    let varied = render(&options, "font-optical-sizing='none'", "Z");
    let control = render(&options, "font-optical-sizing='none'", "A");
    assert!(matches!(
        text(&varied).flattened().children(),
        [Node::Group(_)]
    ));
    assert!(
        text(&varied).stroke_bounding_box().width() > text(&default).stroke_bounding_box().width()
    );
    assert_eq!(
        text(&varied).stroke_bounding_box(),
        text(&control).stroke_bounding_box()
    );
}

#[test]
fn nested_svg_inherits_the_hooks_and_keeps_its_admitted_collection() {
    let mut options = Options {
        font_family: "Custom default family".into(),
        ..Options::default()
    };
    options.set_text_fallback(Some("sv".into()), None);
    options.set_font_instances(
        Some(Box::new(|font, db| {
            assert_eq!(
                font.families(),
                &[usvg::FontFamily::Named("Custom default family".into())]
            );
            assert_eq!(font.selection_language(), Some("sv"));
            let id = variable_face(Arc::make_mut(db));
            Some(weight(id, 1.0, 100.0))
        })),
        Some(Box::new(|_, cluster, language, rtl, tried, _| {
            assert_eq!(cluster, "A");
            assert_eq!(language, Some("sv"));
            assert!(!rtl);
            assert_eq!(tried, [weight(tried[0].0, 1.0, 100.0)]);
            Some(weight(tried[0].0, 2.0, 900.0))
        })),
    );
    let nested = r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="60">
        <text id="label" x="5" y="30" font-size="15">A</text></svg>"#;
    let nested = base64::engine::general_purpose::STANDARD.encode(nested);
    let tree = Tree::from_str(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="60">
                <image id="image" width="60" height="60" href="data:image/svg+xml;base64,{nested}"/>
            </svg>"#
        ),
        &options,
    )
    .unwrap();
    let Some(Node::Group(group)) = tree.node_by_id("image") else {
        panic!("expected a group for the nested SVG image");
    };
    let [Node::Image(image)] = group.children() else {
        panic!("expected a nested SVG image in the group");
    };
    let ImageKind::SVG(nested) = image.kind() else {
        panic!("expected an SVG image");
    };
    let span = &text(nested).layouted()[0];
    assert_eq!(span.font_size.get(), 30.0);
    assert_eq!(span.variations, [axis(b"opsz", 30.0), axis(b"wght", 900.0)]);
    assert_eq!(
        nested
            .fontdb()
            .face(span.positioned_glyphs[0].font)
            .unwrap()
            .index,
        1
    );
    assert!(options.fontdb.is_empty());
}
