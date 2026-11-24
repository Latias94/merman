use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use usvg::fontdb::{Database, ID, Source};
use usvg::layout::PositionedGlyph;
use usvg::{
    FontFamily, FontInstance, FontOpticalSizing, FontStretch, FontStyle, FontVariation, ImageKind,
    Node, Options, Text, Tree,
};

const COLLECTION: &[u8] = include_bytes!("fonts/instance-transport.ttc");

fn static_face(db: &mut Database) -> ID {
    db.load_font_source(Source::Binary(Arc::new(COLLECTION)))[0]
}

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

fn render_on_path(options: &Options, d: &str, offset: f32, attrs: &str, content: &str) -> Tree {
    Tree::from_str(
        &format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="420" height="180">
                <defs><path id="line" d="{d}"/></defs>
                <text id="label" {attrs}><textPath href="#line" startOffset="{offset}">{content}</textPath></text>
            </svg>"##,
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

fn glyphs(tree: &Tree) -> impl Iterator<Item = &PositionedGlyph> {
    text(tree)
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
}

fn assert_near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} vs {expected}");
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
fn normal_weight_preserves_lam_alef_when_the_primary_instances_match() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    let face = options.fontdb.face(id).unwrap().clone();
    let alias = options.fontdb_mut().push_face_info(face);
    options.set_font_instances(
        Some(Box::new(move |font, _| {
            let face = if font.families() == [FontFamily::Named("Alias".into())] {
                alias
            } else {
                id
            };
            Some(weight(face, 1.0, 400.0))
        })),
        None,
    );

    let reference = render(&options, "font-size='20'", "لا");
    let expected: Vec<_> = glyphs(&reference).collect();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].text, "لا");
    assert_ne!(expected[0].id.0, 0);
    for content in [
        "ل<tspan font-weight='normal'>ا</tspan>",
        "ل<tspan font-family='Alias' font-weight='normal'>ا</tspan>",
        "ل<tspan fill='red'>ا</tspan>",
    ] {
        let tree = render(&options, "font-size='20'", content);
        let actual: Vec<_> = glyphs(&tree).collect();
        assert_eq!(actual.len(), 1, "{content}");
        assert_eq!(actual[0].text, "لا");
        assert_eq!(actual[0].id, expected[0].id);
        assert_eq!(actual[0].transform(), expected[0].transform());
        assert_eq!(
            text(&tree).stroke_bounding_box(),
            text(&reference).stroke_bounding_box()
        );
    }

    options.set_font_instances(
        Some(Box::new(move |_, _| Some(weight(id, 1.0, 850.0)))),
        None,
    );
    let tree = render(
        &options,
        "font-size='20'",
        "ل<tspan font-weight='normal'>ا</tspan>",
    );
    let spans = text(&tree).layouted();
    assert_eq!(spans.len(), 2);
    for (span, (letter, coordinate)) in spans.iter().zip([("ل", 850.0), ("ا", 400.0)]) {
        assert!(span.variations.contains(&axis(b"wght", coordinate)));
        assert_eq!(span.positioned_glyphs[0].text, letter);
    }
}

#[test]
fn arabic_decoration_boundary_keeps_joining_forms_and_inner_underline() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));

    let tree = render(
        &options,
        "font-size='20'",
        "ل<tspan text-decoration='underline'>ا</tspan>",
    );
    let [lam, alef] = text(&tree).layouted() else {
        panic!("expected one glyph span on each side of the decoration boundary");
    };
    for (span, letter) in [(lam, "ل"), (alef, "ا")] {
        let [glyph] = span.positioned_glyphs.as_slice() else {
            panic!("expected one glyph for {letter}");
        };
        assert_eq!(glyph.text, letter);
        let isolated = render(&options, "font-size='20'", letter);
        assert_ne!(glyph.id, glyphs(&isolated).next().unwrap().id);
    }

    assert!(lam.underline.is_none());
    let underline = alef.underline.as_ref().expect("visible underline on alef");
    let bounds = underline.bounding_box().expect("underline bounds");
    assert!(underline.is_visible() && underline.fill().is_some());
    assert_near(bounds.left(), alef.positioned_glyphs[0].transform().tx);
    assert_near(bounds.right(), lam.positioned_glyphs[0].transform().tx);
}

#[test]
fn fallback_keeps_each_clusters_authored_request_and_joining_context() {
    let mut options = Options::default();
    let ids = options
        .fontdb_mut()
        .load_font_source(Source::Binary(Arc::new(COLLECTION)));
    let (fallback, primary) = (ids[0], ids[1]);
    let face = options.fontdb.face(primary).unwrap().clone();
    let alias = options.fontdb_mut().push_face_info(face);

    let attrs = r#"font-family='Primary' font-size='20' xml:lang='ar'
        font-variation-settings="'wght' 100, 'wdth' 100, 'ital' 0, 'slnt' 0""#;
    let content = "ل<tspan font-family='Alternate' font-weight='700' font-style='italic' \
        font-stretch='condensed' xml:lang='AR'>ا</tspan>";

    for alternate_scale in [1.0, 1.5] {
        options.set_font_instances(
            Some(Box::new(move |font, _| {
                Some(if font.weight() == 700 {
                    weight(alias, 1.0, 200.0)
                } else {
                    weight(primary, 1.0, 100.0)
                })
            })),
            Some(Box::new(move |font, cluster, language, rtl, tried, _| {
                assert!(rtl);
                let (family, weight_value, style, stretch, language_tag, selection, scale) =
                    match cluster {
                        "ل" => (
                            "Primary",
                            400,
                            FontStyle::Normal,
                            FontStretch::Normal,
                            "ar",
                            weight(primary, 1.0, 100.0),
                            1.0,
                        ),
                        "ا" => (
                            "Alternate",
                            700,
                            FontStyle::Italic,
                            FontStretch::Condensed,
                            "AR",
                            weight(alias, 1.0, 200.0),
                            alternate_scale,
                        ),
                        _ => panic!("unexpected missing cluster {cluster:?}"),
                    };
                assert_eq!(font.families(), [FontFamily::Named(family.into())]);
                assert_eq!(font.weight(), weight_value);
                assert_eq!(font.style(), style);
                assert_eq!(font.stretch(), stretch);
                assert_eq!(language, Some(language_tag));
                assert_eq!(tried, [selection]);
                Some((fallback, scale, Vec::new()))
            })),
        );

        let tree = render(&options, attrs, content);
        let glyphs: Vec<_> = glyphs(&tree)
            .map(|g| {
                assert_eq!(g.font, fallback);
                assert_ne!(g.id.0, 0);
                (g.text.as_str(), g.font_size())
            })
            .collect();
        if alternate_scale == 1.0 {
            assert_eq!(glyphs, [("لا", 20.0)]);
        } else {
            assert_eq!(glyphs, [("ل", 20.0), ("ا", 30.0)]);
        }
    }
}

fn assert_ligature_decoration_and_offsets(options: &Options) {
    let decorated = render(
        options,
        "font-family='Calibri' font-size='40'",
        "f<tspan font-family='Calibri, serif' text-decoration='underline'>i</tspan>X",
    );
    let spans = text(&decorated).layouted();
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[1].positioned_glyphs[0].text, "i");
    assert!(spans[0].underline.is_none());
    assert!(spans[2].underline.is_none());
    let underline = spans[1].underline.as_ref().expect("visible underline on i");
    let bounds = underline.bounding_box().expect("underline bounds");
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    assert!(underline.is_visible() && underline.fill().is_some());
    assert_near(bounds.left(), spans[1].positioned_glyphs[0].transform().tx);
    assert_near(bounds.right(), spans[2].positioned_glyphs[0].transform().tx);

    let plain = render(options, "font-size='40'", "fiX");
    let plain: Vec<_> = glyphs(&plain).collect();
    assert_eq!(plain.len(), 2);
    assert_eq!(plain[0].text, "fi");

    let shifted = render(
        options,
        "font-size='40'",
        "f<tspan dx='30' dy='10'>i</tspan>X",
    );
    let shifted: Vec<_> = glyphs(&shifted).collect();
    assert_eq!(shifted.len(), 2);
    assert_eq!(shifted[0].id, plain[0].id);
    assert_eq!(shifted[0].transform(), plain[0].transform());
    assert_near(shifted[1].transform().tx, plain[1].transform().tx + 30.0);
    assert_near(shifted[1].transform().ty, plain[1].transform().ty + 10.0);

    let continued = render(
        options,
        "font-size='40'",
        "f<tspan dx='30' dy='10'>i</tspan><tspan y='110'>X</tspan>",
    );
    let last = glyphs(&continued).last().unwrap().transform();
    assert_near(last.tx, plain[1].transform().tx + 30.0);
    assert_near(last.ty, 110.0);
}

#[test]
fn cross_span_text_keeps_its_decoration_and_ligature_offsets() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    assert_ligature_decoration_and_offsets(&options);
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows Calibri"]
fn calibri_ligature_keeps_its_cross_span_geometry() {
    let font =
        std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts/calibri.ttf");
    let mut options = Options::default();
    let ids = options
        .fontdb_mut()
        .load_font_source(Source::Binary(Arc::new(std::fs::read(font).unwrap())));
    let id = ids[0];
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    assert_ligature_decoration_and_offsets(&options);
}

#[test]
fn devanagari_keeps_its_script_before_and_after_a_latin_run() {
    let mut options = Options::default();
    let ids = options
        .fontdb_mut()
        .load_font_source(Source::Binary(Arc::new(COLLECTION)));
    let (direct, partial) = (ids[0], ids[1]);
    options.font_resolver.select_font = Box::new(move |_, _| Some(direct));
    let standalone = render(&options, "font-size='20'", "कि");
    let ids_and_offsets = |tree: &Tree| {
        glyphs(tree)
            .filter(|glyph| glyph.text != "X")
            .map(|glyph| (glyph.id, glyph.transform().tx))
            .collect::<Vec<_>>()
    };
    let (_, expected_prebase) = ids_and_offsets(&standalone)[0];
    assert_near(expected_prebase, 20.0);
    assert_eq!(glyphs(&standalone).count(), 2);
    assert_near(glyphs(&standalone).last().unwrap().transform().tx, 24.4);
    let expected_ids: Vec<_> = glyphs(&standalone).map(|g| g.id).collect();

    for primary in [direct, partial] {
        options.set_font_instances(
            Some(Box::new(move |_, _| Some(weight(primary, 1.0, 100.0)))),
            Some(Box::new(move |_, _, _, _, _, _| {
                Some((direct, 1.0, Vec::new()))
            })),
        );
        for content in ["Xकि", "किX", "Xक<tspan fill='red'>ि</tspan>"] {
            let tree = render(&options, "font-size='20'", content);
            let actual = ids_and_offsets(&tree);
            assert_eq!(
                actual.iter().map(|&(id, _)| id).collect::<Vec<_>>(),
                expected_ids,
                "{content}"
            );
            let origin = if content.starts_with('X') { 30.0 } else { 20.0 };
            assert_near(actual[0].1, origin);
            assert_near(actual[1].1, origin + 4.4);
        }
    }
}

#[test]
fn rtl_text_path_places_each_cluster_at_its_own_distance() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let positions = |path, dx, start_offset| {
        let tree = render_on_path(
            &options,
            path,
            start_offset,
            &format!("font-size='42' dx='{dx}'"),
            "אבג",
        );
        glyphs(&tree)
            .map(|g| {
                assert_ne!(g.id.0, 0);
                assert_eq!(g.transform().ty, 96.0);
                (g.text.chars().next().unwrap(), g.transform().tx)
            })
            .collect::<Vec<_>>()
    };
    let segmented = "M10 96 H88 H250";
    for path in ["M10 96 H250", segmented, "M10 96 H88 H88 H250"] {
        assert_eq!(
            positions(path, "0 100 0", 0.0),
            [('ג', 68.0), ('ב', 89.0), ('א', 10.0)]
        );
        assert_eq!(
            positions(path, "0 -100 100", 0.0),
            [('ג', 89.0), ('ב', 10.0), ('א', 131.0)]
        );
    }
    assert_eq!(
        positions(segmented, "0 250 -250", 0.0),
        [('ג', 10.0), ('א', 52.0)]
    );
    assert_eq!(
        positions(segmented, "0 21 21", 25.5),
        [('ג', 35.5), ('ב', 35.5), ('א', 35.5)]
    );
    assert_eq!(
        positions("M300 10 H300 M10 96 H88 H250", "0 0 0", -10.5),
        [('ג', -0.5), ('ב', 20.5), ('א', 41.5)]
    );
}

#[test]
fn rtl_path_keeps_glyphs_when_spacing_makes_the_advance_negative() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for d in ["M10 96 H250", "M10 96 H88 H250"] {
        for (style, content, offset, expected) in [
            (
                "letter-spacing='-35'",
                "אבג",
                90.0,
                &[("ג", 128.0), ("ב", 114.0), ("א", 100.0)][..],
            ),
            (
                "word-spacing='-40'",
                "אב ג",
                75.0,
                &[("ג", 93.5), (" ", 114.5), ("ב", 85.0), ("א", 106.0)][..],
            ),
        ] {
            let tree = render_on_path(
                &options,
                d,
                offset,
                &format!("font-size='42' {style}"),
                content,
            );
            assert_eq!(
                glyphs(&tree)
                    .map(|g| (g.text.as_str(), g.transform().tx))
                    .collect::<Vec<_>>(),
                expected,
            );
        }
    }
}

#[test]
fn text_path_uses_the_limiting_direction_at_degenerate_curve_endpoints() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (d, offset, angle) in [
        ("M80 20 Q80 20 80 120", -10.5, 90.0_f32),
        ("M80 20 C80 20 80 20 80 120", -10.5, 90.0),
        ("M80 20 C80 120 80 120 80 120", 89.5, 90.0),
        ("M80 120 Q80 20 80 20", 89.5, -90.0),
    ] {
        let tree = render_on_path(&options, d, offset, "font-size='50'", "A");
        let transform = glyphs(&tree).next().unwrap().transform();
        assert_near(transform.ky.atan2(transform.sx).to_degrees(), angle);
    }
}

#[test]
fn adjusted_text_length_carries_the_end_position_into_the_next_chunk() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (letters, visual, natural_width) in [("ABC", "ABC", 21.0), ("אבג", "גבא", 25.0)] {
        for mode in ["spacing", "spacingAndGlyphs"] {
            let tree = render(
                &options,
                "font-size='50'",
                &format!(
                    "<tspan x='200' text-anchor='end' textLength='120' lengthAdjust='{mode}'>\
                    {letters}</tspan><tspan text-anchor='start' y='110'>X</tspan>"
                ),
            );
            let glyphs: Vec<_> = glyphs(&tree).collect();
            assert_eq!(glyphs.len(), 4);
            let (step, right_side_bearing) = if mode == "spacing" {
                ((120.0 - natural_width) / 2.0, 2.0)
            } else {
                (40.0, 2.0 * 40.0 / natural_width)
            };
            for (index, (glyph, letter)) in glyphs.iter().zip(visual.chars()).enumerate() {
                assert_eq!(glyph.text, letter.to_string());
                assert_near(glyph.transform().tx, 80.0 + index as f32 * step);
            }
            let [Node::Path(adjusted), Node::Path(_)] = text(&tree).flattened().children() else {
                panic!("expected the outlines of two text chunks");
            };
            assert_near(
                adjusted.bounding_box().unwrap().right(),
                200.0 - right_side_bearing,
            );
            assert_eq!(glyphs[3].text, "X");
            assert_near(glyphs[3].transform().tx, 200.0);
            assert_near(glyphs[3].transform().ty, 110.0);
        }
    }
}

#[test]
fn spacing_and_glyphs_scales_existing_spacing_and_rotates_the_scaled_glyph() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for relative_gap in [0.0, 10.0] {
        let tree = render(
            &options,
            &format!(
                "font-size='50' letter-spacing='10' dx='0 {relative_gap}' textLength='120' lengthAdjust='spacingAndGlyphs'"
            ),
            "A<tspan rotate='30' fill='red'>B</tspan>C",
        );
        let glyphs: Vec<_> = glyphs(&tree).collect();
        let scale = 120.0 / (83.0 + relative_gap);
        assert_near(
            glyphs[1].transform().tx,
            20.0 + (31.0 + relative_gap) * scale,
        );
        assert_near(
            glyphs[2].transform().tx,
            20.0 + (62.0 + relative_gap) * scale,
        );
        let rotated = glyphs[1].transform();
        assert_near(rotated.ky.atan2(rotated.sx).to_degrees(), 30.0);
        assert_near(rotated.sx.hypot(rotated.ky), 0.05 * scale);
        assert_near(rotated.kx.hypot(rotated.sy), 0.05);
    }
}

#[test]
fn nested_text_lengths_resolve_once_and_in_child_first_order() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (mode, expected) in [
        ("spacing", [20.0, 63.0, 78.0, 115.0, 140.0]),
        (
            "spacingAndGlyphs",
            [
                20.0,
                20.0 + 21.0 * (120.0 / 76.0),
                20.0 + 36.0 * (120.0 / 76.0),
                20.0 + 51.0 * (120.0 / 76.0),
                140.0,
            ],
        ),
    ] {
        let tree = render(
            &options,
            "font-size='50'",
            &format!(
                "<tspan textLength='120' lengthAdjust='{mode}'>A<tspan textLength='30' lengthAdjust='spacingAndGlyphs'>B<tspan fill='red'>C</tspan></tspan>X</tspan><tspan y='110'>X</tspan>"
            ),
        );
        let actual: Vec<_> = glyphs(&tree).map(|g| g.transform()).collect();
        assert_eq!(actual.len(), expected.len());
        for (transform, expected_x) in actual.iter().zip(expected) {
            assert_near(transform.tx, expected_x);
        }
        let parent_scale = if mode == "spacing" { 1.0 } else { 120.0 / 76.0 };
        assert_near(actual[1].sx, 0.05 * (30.0 / 42.0) * parent_scale);
        assert_near(actual[2].sx, actual[1].sx);

        let svg = tree.to_string(&usvg::WriteOptions {
            preserve_text: true,
            ..usvg::WriteOptions::default()
        });
        let reparsed = Tree::from_str(&svg, &options).unwrap();
        assert_eq!(
            glyphs(&reparsed).map(|g| g.transform()).collect::<Vec<_>>(),
            actual
        );
    }
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
