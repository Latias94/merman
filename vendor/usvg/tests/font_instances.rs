use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use usvg::fontdb::{Database, ID, Source};
use usvg::layout::PositionedGlyph;
use usvg::{
    FontFamily, FontInstance, FontOpticalSizing, FontStretch, FontStyle, FontVariation,
    ImageHrefPolicy, ImageKind, Node, Options, Text, Tree,
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
fn preserve_text_keeps_authored_axes_with_the_same_native_selector() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    options.set_font_instances(
        Some(Box::new(move |_, _| {
            Some((
                id,
                1.0,
                vec![
                    axis(b"wght", 850.0),
                    axis(b"wdth", 75.0),
                    axis(b"ital", 1.0),
                    axis(b"slnt", -12.0),
                    axis(b"opsz", 9.0),
                ],
            ))
        })),
        None,
    );
    let tree = render(
        &options,
        r#"font-size="40" style="font-kerning:none;font-optical-sizing:none;font-variation-settings:'wght' 900, 'CSTM' 1, 'opsz' 20""#,
        r#"AB<tspan style="font-weight:normal;font-style:normal;font-stretch:normal;font-optical-sizing:auto;font-variation-settings:normal">AX</tspan>X"#,
    );
    for use_single_quote in [false, true] {
        let svg = tree.to_string(&usvg::WriteOptions {
            preserve_text: true,
            use_single_quote,
            ..Default::default()
        });
        let document = roxmltree::Document::parse(&svg).unwrap();
        let runs: Vec<_> = document
            .descendants()
            .filter(|n| n.has_tag_name("tspan"))
            .collect();
        assert_eq!(runs.len(), 3);
        for run in runs {
            assert!(!run.has_attribute("font-variation-settings"));
            assert!(!run.has_attribute("font-optical-sizing"));
            let css = simplecss::DeclarationTokenizer::from(run.attribute("style").unwrap())
                .map(|d| (d.name, d.value))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(css.get("font-kerning"), Some(&"none"));
            if run.text() == Some("AX") {
                assert_eq!(css.len(), 1);
            } else {
                assert_eq!(css.get("font-optical-sizing"), Some(&"none"));
                assert_eq!(
                    css.get("font-variation-settings"),
                    Some(&"'wght' 900, 'CSTM' 1, 'opsz' 20")
                );
            }
        }
        let restored = Tree::from_str(&svg, &options).unwrap();
        let axes = |t: &Tree| {
            text(t)
                .layouted()
                .iter()
                .map(|s| s.variations.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(axes(&tree), axes(&restored), "{svg}");
        assert_eq!(
            tree.to_string(&Default::default()),
            restored.to_string(&Default::default()),
            "{svg}"
        );
    }
}

#[test]
fn preserved_variation_tags_survive_css_and_xml_escaping() {
    let mut options = Options::default();
    let id = variable_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let original = render(
        &options,
        r#"font-variation-settings="'ABC\' 1, 'wght' 800, &quot;A'B,&quot; 2, 'A&lt;&amp;&quot;' 3""#,
        "AX",
    );
    let expected = [
        axis(b"ABC\\", 1.0),
        axis(b"wght", 800.0),
        axis(b"A'B,", 2.0),
        axis(b"A<&\"", 3.0),
    ];
    assert_eq!(
        text(&original).chunks()[0].spans()[0].font().variations(),
        expected
    );
    for use_single_quote in [false, true] {
        let svg = original.to_string(&usvg::WriteOptions {
            preserve_text: true,
            use_single_quote,
            ..Default::default()
        });
        let restored = Tree::from_str(&svg, &options).unwrap();
        assert_eq!(
            text(&restored).chunks()[0].spans()[0].font().variations(),
            expected,
            "{svg}"
        );
        assert_eq!(
            original.to_string(&Default::default()),
            restored.to_string(&Default::default())
        );
    }
}

#[test]
fn italic_keeps_the_native_angle_of_a_slant_only_face() {
    let mut options = Options::default();
    let source = include_bytes!("fonts/slant-only.ttf").as_slice();
    let id = options
        .fontdb_mut()
        .load_font_source(Source::Binary(Arc::new(source)))[0];
    options.set_font_instances(
        Some(Box::new(move |_, _| {
            Some((id, 1.0, vec![axis(b"slnt", -12.0)]))
        })),
        None,
    );
    for (attrs, angle) in [
        ("font-style='italic'", -12.0),
        (
            r#"font-style='italic' font-variation-settings="'slnt' -6""#,
            -6.0,
        ),
        (
            r#"font-style='italic' font-variation-settings="'slnt' 0""#,
            0.0,
        ),
        ("font-style='normal'", 0.0),
    ] {
        let tree = render(&options, attrs, "AB");
        let control = render(
            &options,
            &format!(r#"font-variation-settings="'slnt' {angle}""#),
            "AB",
        );
        let axes = &text(&tree).layouted()[0].variations;
        assert_eq!(
            axes.iter()
                .find(|a| a.tag == *b"slnt")
                .map_or(0.0, |a| a.value),
            angle
        );
        assert_eq!(
            tree.to_string(&Default::default()),
            control.to_string(&Default::default())
        );
    }
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
fn arabic_decoration_boundary_paints_only_the_decorated_part_of_the_ligature() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    let variable = variable_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));

    let reference = render(&options, "font-size='20'", "لا");
    let expected = glyphs(&reference).next().unwrap();
    assert_eq!(expected.text, "لا");
    assert_ne!(expected.id.0, 0);
    for primary in [id, variable] {
        options.set_font_instances(
            Some(Box::new(move |_, _| Some(weight(primary, 1.0, 100.0)))),
            Some(Box::new(move |_, _, _, _, _, _| {
                Some((id, 1.0, Vec::new()))
            })),
        );
        for (content, left, right, owner) in [
            (
                "ل<tspan text-decoration='underline'>ا</tspan>",
                20.0,
                26.2,
                1,
            ),
            (
                "<tspan text-decoration='underline'>ل</tspan>ا",
                26.2,
                32.4,
                0,
            ),
            (
                "ل<tspan text-decoration='underline' letter-spacing='8'>ا</tspan>",
                20.0,
                26.2,
                1,
            ),
        ] {
            let tree = render(&options, "font-size='20'", content);
            let actual: Vec<_> = glyphs(&tree).collect();
            assert_eq!(actual.len(), 1, "{content}");
            assert_eq!(actual[0].font, id);
            assert_eq!(actual[0].id, expected.id);
            assert_eq!(actual[0].transform(), expected.transform());
            let spans = text(&tree).layouted();
            assert!(spans[1 - owner].underline.is_none());
            let underline = spans[owner].underline.as_ref().expect("visible underline");
            let bounds = underline.bounding_box().expect("underline bounds");
            assert!(underline.is_visible() && underline.fill().is_some());
            assert_near(bounds.left(), left);
            assert_near(bounds.right(), right);
        }
    }
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
    assert!(spans[0].underline.is_none());
    assert!(spans[2].underline.is_none());
    let underline = spans[1].underline.as_ref().expect("visible underline on i");
    let bounds = underline.bounding_box().expect("underline bounds");
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    assert!(underline.is_visible() && underline.fill().is_some());
    let first = spans[0].positioned_glyphs[0].transform().tx;
    let last = spans[2].positioned_glyphs[0].transform().tx;
    assert_near(bounds.left(), (first + last) / 2.0);
    assert_near(bounds.right(), last);

    let plain = render(options, "font-size='40'", "fiX");
    let plain: Vec<_> = glyphs(&plain).collect();
    assert_eq!(plain.len(), 2);
    assert_eq!(plain[0].text, "fi");

    let tracked = render(options, "font-size='40' letter-spacing='8'", "fiX");
    let tracked: Vec<_> = glyphs(&tracked).collect();
    assert_eq!(
        tracked.iter().map(|g| g.text.as_str()).collect::<Vec<_>>(),
        ["f", "i", "X"]
    );
    for (letter, index) in [("f", 1), ("i", 2)] {
        let pair = render(options, "font-size='40'", &format!("{letter}X"));
        let pair: Vec<_> = glyphs(&pair).collect();
        assert_eq!(pair.len(), 2);
        assert_near(
            tracked[index].transform().tx - tracked[index - 1].transform().tx,
            pair[1].transform().tx - pair[0].transform().tx + 8.0,
        );
    }

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
fn an_independently_anchored_chunk_starts_at_the_natural_advance() {
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
            assert_near(glyphs[3].transform().tx, 200.0 + 3.0 * natural_width);
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
        let scale = 120.0 / (93.0 + relative_gap);
        assert_near(
            glyphs[1].transform().tx,
            20.0 + (31.0 + relative_gap) * scale,
        );
        assert_near(
            glyphs[2].transform().tx,
            20.0 + (62.0 + relative_gap) * scale,
        );
        let rotated = glyphs[1].transform();
        let (sine, cosine) = 30.0_f32.to_radians().sin_cos();
        assert_near(rotated.sx, 0.05 * cosine * scale);
        assert_near(rotated.kx, -0.05 * sine * scale);
        assert_near(rotated.ky, 0.05 * sine);
        assert_near(rotated.sy, 0.05 * cosine);
    }
}

#[test]
fn rotated_spacing_and_glyphs_scales_ink_along_the_svg_inline_axis() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let plain = render(&options, "font-size='50' rotate='90'", "W");
    let scaled = render(
        &options,
        "font-size='50' rotate='90' textLength='100' lengthAdjust='spacingAndGlyphs'",
        "W",
    );
    let plain = text(&plain).stroke_bounding_box();
    let scaled = text(&scaled).stroke_bounding_box();
    assert_near(scaled.width(), plain.width() * 4.0);
    assert_near(scaled.height(), plain.height());
    assert_near(scaled.top(), plain.top());
}

fn assert_preserved_text_geometry(tree: &Tree, options: &Options) -> String {
    let svg = tree.to_string(&usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    });
    let reparsed = Tree::from_str(&svg, options).unwrap();
    let geometry = |tree: &Tree| {
        glyphs(tree)
            .map(|glyph| (glyph.id, glyph.text.clone(), glyph.transform()))
            .collect::<Vec<_>>()
    };
    assert_eq!(geometry(&reparsed), geometry(tree), "{svg}");
    svg
}

#[test]
fn adjusted_decoration_bridges_only_continuing_lines() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let tree = render(
        &options,
        "font-size='50' textLength='120' text-decoration='underline'",
        "A<tspan text-decoration='overline'>B</tspan>CD",
    );
    let spans = text(&tree).layouted();
    let underline = spans[0].underline.as_ref().unwrap().bounding_box().unwrap();
    assert_near(underline.left(), 20.0);
    assert_near(underline.right(), 140.0);
    assert!(spans[1..].iter().all(|span| span.underline.is_none()));
    let overline = spans[1].overline.as_ref().unwrap().bounding_box().unwrap();
    assert_near(overline.left(), 53.0);
    assert_near(overline.right(), 74.0);
}

#[test]
fn splitting_an_underlined_ligature_preserves_its_entire_line() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let attrs = "font-size='50' text-decoration='underline' textLength='120'";
    let reference = render(&options, attrs, "fiX");
    let split = render(&options, attrs, "f<tspan>i</tspan>X");
    let geometry = |tree: &Tree| {
        glyphs(tree)
            .map(|g| (g.id, g.transform()))
            .collect::<Vec<_>>()
    };
    assert_eq!(geometry(&reference), geometry(&split));
    let lines = |tree: &Tree| {
        text(tree)
            .layouted()
            .iter()
            .filter_map(|s| s.underline.as_ref())
            .map(|p| p.bounding_box().unwrap())
            .collect::<Vec<_>>()
    };
    let [full] = lines(&reference)[..] else {
        panic!("expected a single underline")
    };
    assert_eq!(lines(&split), [full]);
    let isolated = render(
        &options,
        "font-size='50' textLength='120'",
        "f<tspan text-decoration='underline'>i</tspan>X",
    );
    let isolated = lines(&isolated);
    assert_eq!(isolated.len(), 1);
    assert_near(isolated[0].left(), 34.5);
    assert_near(isolated[0].right(), 49.0);
}

#[test]
fn splitting_a_stroked_ligature_adds_no_decoration_edges() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let attrs = "font-size='50' fill='white' stroke='black' stroke-width='2' text-decoration='underline' textLength='120'";
    let reference = render(&options, attrs, "fiX");
    let expected = text(&reference).layouted()[0].underline.as_ref().unwrap();
    for content in [
        "f<tspan>i</tspan>X",
        "f<tspan fill='red'>i</tspan>X",
        "f<tspan text-decoration='underline' stroke='#000'>i</tspan>X",
    ] {
        let split = render(&options, attrs, content);
        let paths: Vec<_> = text(&split)
            .layouted()
            .iter()
            .filter_map(|span| span.underline.as_ref())
            .collect();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].data(), expected.data());
        assert_eq!(paths[0].stroke().unwrap().width().get(), 2.0);
    }

    for (style, first_width, first_color) in [
        ("stroke-width='4'", 4.0, usvg::Color::black()),
        ("stroke='red'", 2.0, usvg::Color::new_rgb(255, 0, 0)),
    ] {
        let changed = render(
            &options,
            attrs,
            &format!("<tspan text-decoration='underline' {style}>f</tspan>iX"),
        );
        let paths: Vec<_> = text(&changed)
            .layouted()
            .iter()
            .filter_map(|span| span.underline.as_ref())
            .collect();
        assert_eq!(paths.len(), 2);
        for (path, (left, right, stroke_width, color)) in paths.iter().zip([
            (20.0, 34.5, first_width, first_color),
            (34.5, 140.0, 2.0, usvg::Color::black()),
        ]) {
            assert_near(path.bounding_box().unwrap().left(), left);
            assert_near(path.bounding_box().unwrap().right(), right);
            assert_eq!(path.stroke().unwrap().width().get(), stroke_width);
            assert_eq!(path.stroke().unwrap().paint(), &usvg::Paint::Color(color));
            assert_eq!(path.data().verbs(), expected.data().verbs());
        }
    }
}

#[test]
fn coalesced_decorations_keep_their_flattened_paths_and_paint_order() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let attrs = "font-size='50' fill='none' stroke='black' text-decoration='underline overline line-through'";
    let whole = render(&options, attrs, "<tspan stroke='none'>AQ</tspan>");
    let split = render(
        &options,
        attrs,
        "<tspan stroke='none'>A<tspan>Q</tspan></tspan>",
    );
    let [first, last] = text(&split).layouted() else {
        panic!("expected two positioned spans");
    };
    assert!(first.overline.is_some() && first.underline.is_some() && first.line_through.is_none());
    assert!(last.overline.is_none() && last.underline.is_none() && last.line_through.is_some());

    fn painted_paths(tree: &Tree) -> impl Iterator<Item = &usvg::Path> {
        text(tree)
            .flattened()
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Path(path) if path.is_visible() && path.stroke().is_some() => {
                    Some(path.as_ref())
                }
                _ => None,
            })
    }
    assert_eq!(painted_paths(&whole).count(), 3);
    assert!(
        painted_paths(&whole)
            .map(usvg::Path::data)
            .eq(painted_paths(&split).map(usvg::Path::data))
    );
}

#[test]
fn decoration_paint_tracks_positive_layout_members() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (position, spacing, missing_is_painted) in [
        ("dx='0 32'", -25, false),
        ("x='20 57'", -25, false),
        ("", -25, false),
        ("dx='0 32'", -20, true),
    ] {
        let tree = render(
            &options,
            "font-size='50' text-decoration='underline'",
            &format!("<tspan fill='none' letter-spacing='{spacing}' {position}>ZQ</tspan>"),
        );
        let glyph_paint: Vec<_> = text(&tree)
            .layouted()
            .iter()
            .flat_map(|span| {
                span.positioned_glyphs.iter().map(|glyph| {
                    (
                        glyph.text.as_str(),
                        glyph.id.0 == 0,
                        span.glyph_has_visible_paint(glyph),
                    )
                })
            })
            .collect();
        assert_eq!(
            glyph_paint,
            [("Z", false, true), ("Q", true, missing_is_painted)],
            "{position} {spacing}"
        );
    }
}

#[test]
fn vertical_overline_is_half_a_font_height_from_the_strike() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let tree = render(
        &options,
        "font-size='50' writing-mode='vertical-rl' text-decoration='overline line-through underline'",
        "W",
    );
    let span = &text(&tree).layouted()[0];
    let left =
        |line: &Option<Box<usvg::Path>>| line.as_ref().unwrap().bounding_box().unwrap().left();
    // This face has an ascender of 800 and a descender of -200 at 1000 units/em.
    assert_near(left(&span.overline) - left(&span.line_through), 25.0);
    assert_near(left(&span.line_through) - left(&span.underline), 25.0);
}

#[test]
fn rotated_decoration_scales_inline_and_ends_before_an_upright_glyph() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let bounds = |tree: &Tree| {
        text(tree).layouted()[0]
            .underline
            .as_ref()
            .unwrap()
            .bounding_box()
            .unwrap()
    };
    let attrs = "font-size='50' rotate='90' text-decoration='underline'";
    let plain = bounds(&render(&options, attrs, "W"));
    let scaled = bounds(&render(
        &options,
        &format!("{attrs} textLength='100' lengthAdjust='spacingAndGlyphs'"),
        "W",
    ));
    assert_near(scaled.width(), 4.0 * plain.width());
    assert_near(scaled.height(), plain.height());
    assert_near(scaled.left() - 20.0, 4.0 * (plain.left() - 20.0));
    let mixed = bounds(&render(
        &options,
        "font-size='50' rotate='90 0' text-decoration='underline'",
        "WX",
    ));
    assert_near(mixed.left(), plain.left());
    assert_near(mixed.bottom(), plain.bottom());
    assert_near(mixed.right(), 70.0);
}

#[test]
fn text_length_measures_tracked_cells_and_disjoint_visual_ranges() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for mode in ["spacing", "spacingAndGlyphs"] {
        let tracked = render(
            &options,
            "font-size='50' letter-spacing='8'",
            &format!("<tspan textLength='21' lengthAdjust='{mode}'>A</tspan>BX"),
        );
        let x: Vec<_> = glyphs(&tracked).map(|g| g.transform().tx).collect();
        // A single cell has no internal spacing opportunity. Scaling, however,
        // fits its complete 29-unit tracked advance into the requested 21 units.
        let expected = if mode == "spacing" {
            [20.0, 49.0, 78.0]
        } else {
            [20.0, 41.0, 70.0]
        };
        assert_eq!(x, expected);
        let control = render(&options, "font-size='50'", "A<tspan>Bא</tspan>בגX");
        let bidi = render(
            &options,
            "font-size='50'",
            &format!("A<tspan textLength='96' lengthAdjust='{mode}'>Bא</tspan>בגX"),
        );
        assert_eq!(
            control.to_string(&Default::default()),
            bidi.to_string(&Default::default())
        );
    }
    let bidi = render(
        &options,
        "font-size='50'",
        "A<tspan textLength='100' lengthAdjust='spacingAndGlyphs'>Bא</tspan>בגX",
    );
    assert_near(glyphs(&bidi).last().unwrap().transform().tx, 141.0);
}

#[test]
fn one_text_length_is_shared_across_y_positioned_chunks() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (mode, step) in [("spacing", 33.0), ("spacingAndGlyphs", 30.0)] {
        for (trailing, trailing_x, trailing_y) in [
            ("X", 160.0, 140.0),
            ("<tspan y='110'>X</tspan>", 124.0, 110.0),
        ] {
            let tree = render(
                &options,
                "font-size='50'",
                &format!(
                    "<tspan x='40' y='70' textLength='120' lengthAdjust='{mode}'>AB<tspan y='140'>CD</tspan></tspan>{trailing}"
                ),
            );
            let actual: Vec<_> = glyphs(&tree).collect();
            assert_eq!(actual.len(), 5);
            for (i, glyph) in actual[..4].iter().enumerate() {
                assert_ne!(glyph.id.0, 0);
                assert_near(glyph.transform().tx, 40.0 + i as f32 * step);
                assert_near(glyph.transform().ty, if i < 2 { 70.0 } else { 140.0 });
            }
            assert_eq!(actual[4].text, "X");
            assert_near(actual[4].transform().tx, trailing_x);
            assert_near(actual[4].transform().ty, trailing_y);
            assert_preserved_text_geometry(&tree, &options);
        }
    }

    let nested = render(
        &options,
        "font-size='50'",
        "<tspan x='40' textLength='120' lengthAdjust='spacingAndGlyphs'><tspan textLength='30' lengthAdjust='spacingAndGlyphs'>AB</tspan><tspan y='110'>CD</tspan></tspan><tspan y='150'>X</tspan>",
    );
    let starts: Vec<_> = glyphs(&nested).map(|glyph| glyph.transform().tx).collect();
    assert_eq!(starts.len(), 5);
    for (actual, expected) in starts.into_iter().zip([40.0, 55.0, 100.0, 130.0, 124.0]) {
        assert_near(actual, expected);
    }
    assert_preserved_text_geometry(&nested, &options);
}

#[test]
fn preserve_text_keeps_one_path_and_length_declaration_across_chunks() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let tree = render_on_path(
        &options,
        "M10 96 H250",
        4.0,
        "x='0 70 140' font-size='50' textLength='120' lengthAdjust='spacingAndGlyphs'",
        "A<tspan fill='red'>B</tspan>C",
    );
    let actual: Vec<_> = glyphs(&tree).map(|g| g.transform()).collect();
    assert_eq!(actual.len(), 3);
    for (index, transform) in actual.iter().enumerate() {
        assert_near(transform.tx, 14.0 + 70.0 * index as f32);
        assert_near(transform.ty, 96.0);
    }
    let svg = assert_preserved_text_geometry(&tree, &options);
    assert_eq!(svg.matches("<textPath").count(), 1);
    assert_eq!(svg.matches("textLength=").count(), 1);
}

#[test]
fn nested_text_lengths_keep_the_child_extent_and_outer_continuation() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (outer_mode, expected_b, expected_d, outer_scale) in [
        ("spacing", 85.0, 139.0, 1.0),
        ("spacingAndGlyphs", 75.0, 125.0, 120.0 / 72.0),
    ] {
        for (inner_mode, inner_step, inner_scale) in [
            ("spacing", 9.0, 1.0),
            ("spacingAndGlyphs", 15.0, 30.0 / 42.0),
        ] {
            let tree = render(
                &options,
                "font-size='50'",
                &format!(
                    "<tspan x='40' textLength='120' lengthAdjust='{outer_mode}'>A<tspan textLength='30' lengthAdjust='{inner_mode}' text-decoration='underline'>B<tspan fill='red'>C</tspan></tspan>D</tspan><tspan y='140'>X</tspan>"
                ),
            );
            let actual: Vec<_> = glyphs(&tree).map(|g| g.transform()).collect();
            assert_eq!(actual.len(), 5);
            for (transform, x) in
                actual
                    .iter()
                    .zip([40.0, expected_b, expected_b + inner_step, expected_d, 124.0])
            {
                assert_near(transform.tx, x);
            }
            assert_near(actual[0].sx, 0.05 * outer_scale);
            assert_near(actual[3].sx, 0.05 * outer_scale);
            assert_near(actual[1].sx, 0.05 * inner_scale);
            assert_near(actual[2].sx, actual[1].sx);
            assert_near(actual[2].tx + 420.0 * actual[2].sx - actual[1].tx, 30.0);
            let inner_spans = &text(&tree).layouted()[1..3];
            let underline = inner_spans[0]
                .underline
                .as_ref()
                .unwrap()
                .bounding_box()
                .unwrap();
            assert_near(underline.left(), expected_b);
            assert_near(underline.right(), expected_b + 30.0);
            assert!(inner_spans[1].underline.is_none());

            let svg = assert_preserved_text_geometry(&tree, &options);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let inner = document
                .descendants()
                .find(|node| node.attribute("textLength") == Some("30"))
                .unwrap();
            assert_eq!(inner.attribute("lengthAdjust"), Some(inner_mode));
        }
    }

    let tree = render(
        &options,
        "font-size='50'",
        "<tspan x='40' textLength='120' lengthAdjust='spacingAndGlyphs'>A<tspan textLength='30' lengthAdjust='spacingAndGlyphs'>BC</tspan></tspan>X",
    );
    let actual: Vec<_> = glyphs(&tree).map(|g| g.transform()).collect();
    assert_eq!(actual.len(), 4);
    assert_near(actual[2].tx + 420.0 * actual[2].sx - actual[1].tx, 30.0);
    assert_near(actual[3].tx, 160.0);
}

#[test]
fn end_anchor_uses_the_extent_of_the_only_resolved_child() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for mode in ["spacing", "spacingAndGlyphs"] {
        let tree = render(
            &options,
            "font-size='50'",
            &format!(
                "<tspan x='220' text-anchor='end' textLength='120' lengthAdjust='spacingAndGlyphs'><tspan textLength='30' lengthAdjust='{mode}'>BC</tspan></tspan>"
            ),
        );
        let gs: Vec<_> = glyphs(&tree).map(|g| g.transform()).collect();
        assert_eq!(gs.len(), 2);
        assert_near(gs[0].tx, 190.0);
        assert_near(gs[1].tx + 420.0 * gs[1].sx, 220.0);
        let bounds = text(&tree).bounding_box();
        assert_near(bounds.left(), 190.0);
        assert_near(bounds.width(), 30.0);
        let ink = text(&tree).stroke_bounding_box();
        assert_near(ink.left(), 190.0);
        assert_near(ink.right(), 220.0 - 40.0 * gs[1].sx);
        assert_preserved_text_geometry(&tree, &options);
        let continued = render(
            &options,
            "font-size='50'",
            &format!(
                "<tspan x='220' text-anchor='end' textLength='120' lengthAdjust='spacingAndGlyphs'><tspan textLength='30' lengthAdjust='{mode}'>BC</tspan></tspan><tspan y='90'>X</tspan>"
            ),
        );
        let continued_glyphs: Vec<_> = glyphs(&continued).map(|g| g.transform()).collect();
        assert_eq!(&continued_glyphs[..2], &gs);
        assert_near(continued_glyphs[2].tx, 262.0);
        assert_near(continued_glyphs[2].ty, 90.0);
        assert_preserved_text_geometry(&continued, &options);
    }
}

#[test]
fn text_length_matches_the_natural_tracked_chunk_at_each_anchor() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    for (anchor, first_x) in [("start", 20.0), ("middle", -9.0), ("end", -38.0)] {
        let attrs = format!("font-size='50' letter-spacing='8' text-anchor='{anchor}'");
        let natural = render(&options, &attrs, "AB");
        let adjusted = render(
            &options,
            &format!("{attrs} textLength='58' lengthAdjust='spacingAndGlyphs'"),
            "AB",
        );
        for tree in [&natural, &adjusted] {
            let transforms: Vec<_> = glyphs(tree).map(|glyph| glyph.transform()).collect();
            assert_eq!(transforms.len(), 2);
            assert_near(transforms[0].tx, first_x);
            assert_near(transforms[1].tx, first_x + 29.0);
            assert_near(transforms[1].sx, 0.05);
        }
        assert_eq!(
            adjusted.to_string(&Default::default()),
            natural.to_string(&Default::default())
        );
    }
}

#[test]
fn text_length_includes_css_spacing_at_the_trailing_edge() {
    let mut options = Options::default();
    let id = static_face(options.fontdb_mut());
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let attrs = "font-size='50' xml:space='preserve'";
    let natural = render(&options, attrs, "<tspan word-spacing='20'>A </tspan>X");
    for (mode, b, scale) in [
        ("spacing", 71.0, 1.0),
        ("spacingAndGlyphs", 60.0, 80.0 / 58.0),
    ] {
        let tracked = render(
            &options,
            attrs,
            &format!("<tspan letter-spacing='8' textLength='80' lengthAdjust='{mode}'>AB</tspan>X"),
        );
        let gs: Vec<_> = glyphs(&tracked).map(|g| g.transform()).collect();
        assert_eq!(gs.len(), 3);
        assert_near(gs[1].tx, b);
        assert_near(gs[0].sx, 0.05 * scale);
        assert_near(gs[2].tx, 100.0);

        let spaced = render(
            &options,
            attrs,
            &format!(
                "<tspan word-spacing='20' textLength='53.5' lengthAdjust='{mode}'>A </tspan>X"
            ),
        );
        assert_near(glyphs(&spaced).last().unwrap().transform().tx, 73.5);
        assert_eq!(
            spaced.to_string(&Default::default()),
            natural.to_string(&Default::default())
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
    let nested = nested_image(&tree);
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

fn nested_image(tree: &Tree) -> &Tree {
    let Some(Node::Group(group)) = tree.node_by_id("image") else {
        panic!("expected a group for the nested SVG image");
    };
    let [Node::Image(image)] = group.children() else {
        panic!("expected a nested SVG image in the group");
    };
    let ImageKind::SVG(nested) = image.kind() else {
        panic!("expected an SVG image");
    };
    nested
}

fn svg_document(content: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="60">{content}</svg>"#)
}

fn svg_image(content: &str) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(svg_document(content));
    format!(r#"<image id="image" href="data:image/svg+xml;base64,{data}"/>"#)
}

#[test]
fn lazy_fonts_shape_svg_images_in_document_order() {
    let label = r#"<text id="label" y="30">A</text>"#;
    let image = svg_image(label);
    for (before, after) in [("", ""), (label, ""), ("", label)] {
        let source = svg_document(&format!("{before}{image}{after}"));
        let mut calls = 0;
        let tree = Tree::try_from_str_with_text_fonts(
            &source,
            &Options::default(),
            ImageHrefPolicy::DataOnly,
            |options| {
                calls += 1;
                options.font_family = "Merman Instance Test".into();
                static_face(options.fontdb_mut());
                true
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(calls, 1);
        let assert_letter = |tree: &Tree| {
            let glyphs: Vec<_> = glyphs(tree).collect();
            assert_eq!(glyphs.len(), 1);
            assert_eq!(glyphs[0].text, "A");
            assert_ne!(glyphs[0].id.0, 0);
            assert!(tree.fontdb().face(glyphs[0].font).is_some());
        };
        assert_letter(nested_image(&tree));
        if !before.is_empty() || !after.is_empty() {
            assert_letter(&tree);
        }
    }
}

#[test]
fn lazy_fonts_skip_empty_image_text_and_propagate_nested_decline() {
    let blank = svg_image(r#"<rect width="5" height="5"/><text>  </text>"#);
    let tree = Tree::try_from_str_with_text_fonts(
        &svg_document(&blank),
        &Options::default(),
        ImageHrefPolicy::DataOnly,
        |_| panic!("trimmed text does not need fonts"),
    )
    .unwrap()
    .unwrap();
    assert!(nested_image(&tree).root().has_children());

    let image = svg_image("<text>A</text>");
    let mut calls = 0;
    let tree = Tree::try_from_str_with_text_fonts(
        &svg_document(&image),
        &Options::default(),
        ImageHrefPolicy::DataOnly,
        |_| {
            calls += 1;
            false
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert!(tree.is_none());
}

#[test]
fn lazy_fonts_file_images_allow_only_embedded_descendants() {
    let directory = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let name = format!("usvg-nested-{}", std::process::id());
    let leaf = directory.join(format!("{name}-leaf.svg"));
    let child = directory.join(format!("{name}.svg"));
    let external = leaf
        .to_str()
        .unwrap()
        .replace('&', "&amp;")
        .replace('"', "&quot;");
    let label = r#"<text id="label" y="30">A</text>"#;
    let content = format!(
        r#"<image id="external" href="{external}"/>{}"#,
        svg_image(label)
    );
    std::fs::write(&leaf, svg_document(label)).unwrap();
    std::fs::write(&child, svg_document(&content)).unwrap();
    let source = svg_document(&format!(r#"<image id="image" href="{name}.svg"/>"#));
    let options = Options {
        resources_dir: Some(directory),
        ..Options::default()
    };
    let tree = Tree::try_from_str_with_text_fonts(
        &source,
        &options,
        ImageHrefPolicy::DataAndFiles,
        |fonts| {
            fonts.font_family = "Merman Instance Test".into();
            static_face(fonts.fontdb_mut());
            true
        },
    );
    let data_only =
        Tree::try_from_str_with_text_fonts(&source, &options, ImageHrefPolicy::DataOnly, |_| {
            panic!("a blocked file cannot request fonts")
        });
    std::fs::remove_file(child).unwrap();
    std::fs::remove_file(leaf).unwrap();
    let tree = tree.unwrap().unwrap();
    let nested = nested_image(&tree);
    assert!(nested.node_by_id("external").is_none());
    assert_eq!(glyphs(nested_image(nested)).next().unwrap().text, "A");
    assert!(data_only.unwrap().unwrap().node_by_id("image").is_none());
}

#[test]
fn lazy_fonts_configured_image_resolvers_own_their_result() {
    let calls = AtomicUsize::new(0);
    let mut options = Options::default();
    options.image_href_resolver.resolve_data = Box::new(|mime, _, _| {
        assert_eq!(mime, "image/svg+xml");
        calls.fetch_add(1, Ordering::Relaxed);
        None
    });
    options.image_href_resolver.resolve_string = Box::new(|href, _| {
        assert_eq!(href, "custom:image");
        calls.fetch_add(1, Ordering::Relaxed);
        Some(ImageKind::SVG(
            Tree::from_str(
                &svg_document(r#"<path d="M0 0h5v5z"/>"#),
                &Options::default(),
            )
            .unwrap(),
        ))
    });
    let source = svg_document(&format!(
        r#"{}<image id="custom" href="custom:image"/>"#,
        svg_image("<text>A</text>")
    ));
    let tree =
        Tree::try_from_str_with_text_fonts(&source, &options, ImageHrefPolicy::Configured, |_| {
            panic!("custom image resolvers supplied no text to convert")
        })
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(tree.node_by_id("image").is_none());
    assert!(tree.node_by_id("custom").is_some());
}
