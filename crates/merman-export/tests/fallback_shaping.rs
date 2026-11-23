#![cfg(feature = "native-fonts")]

use std::sync::Arc;
use usvg::fontdb::{Database, ID, Source};

const BASE: &[u8] = include_bytes!("fonts/MermanProbeBase.ttf");
const FULL: &[u8] = include_bytes!("fonts/MermanProbeFull.ttf");

fn load(db: &mut Database, data: &[u8]) -> ID {
    db.load_font_source(Source::Binary(Arc::new(data.to_vec())))[0]
}

fn options(primary: &[u8]) -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    let primary = load(options.fontdb_mut(), primary);
    options.font_resolver.select_font = Box::new(move |_, _| Some(primary));
    options
}

fn render(options: &usvg::Options, content: &str) -> usvg::Tree {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="100"><text id="t" x="10" y="60" font-size="20">{content}</text></svg>"#
    );
    usvg::Tree::from_str(&source, options).unwrap()
}

fn text(tree: &usvg::Tree) -> &usvg::Text {
    let Some(usvg::Node::Text(text)) = tree.node_by_id("t") else {
        panic!("expected rendered text")
    };
    text
}

fn glyphs(tree: &usvg::Tree) -> Vec<(ID, u16, &str)> {
    text(tree)
        .layouted()
        .iter()
        .flat_map(|s| &s.positioned_glyphs)
        .map(|g| (g.font, g.id.0, g.text.as_str()))
        .collect()
}

fn transforms(tree: &usvg::Tree) -> Vec<usvg::Transform> {
    text(tree)
        .layouted()
        .iter()
        .flat_map(|s| &s.positioned_glyphs)
        .map(|glyph| glyph.transform())
        .collect()
}

fn fixed_fallback(options: &mut usvg::Options, bytes: &[u8], scale: f32) -> ID {
    let face = load(options.fontdb_mut(), bytes);
    options.set_text_fallback(
        None,
        Some(Box::new(move |_, _, _, _, _, _| Some((face, scale)))),
    );
    face
}

#[test]
fn adjacent_fallback_clusters_share_ligatures_and_preserve_primary_glyphs() {
    let mut options = options(include_bytes!("fonts/NotoSansSC-subset.ttf"));
    let primary = options.fontdb.faces().next().unwrap().id;
    let full = fixed_fallback(&mut options, FULL, 1.5);
    let tree = render(&options, r#"中f<tspan fill="red">i✓</tspan>中"#);
    let spans = text(&tree).layouted();
    assert_eq!(
        glyphs(&tree),
        [
            (primary, 3, "中"),
            (full, 5, "fi"),
            (full, 3, "✓"),
            (primary, 3, "中")
        ]
    );
    assert_eq!(text(&tree).bounding_box().width(), 70.0);
    let positions: Vec<_> = spans
        .iter()
        .flat_map(|s| &s.positioned_glyphs)
        .map(|g| (g.transform().tx, g.font_size()))
        .collect();
    assert_eq!(
        positions,
        [(10.0, 20.0), (30.0, 30.0), (45.0, 30.0), (60.0, 20.0)]
    );
    assert_eq!(
        spans[1].fill.as_ref().unwrap().paint(),
        &usvg::Paint::Color(usvg::Color::black())
    );
    assert_eq!(
        spans[2].fill.as_ref().unwrap().paint(),
        &usvg::Paint::Color(usvg::Color::new_rgb(255, 0, 0))
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the macOS Geeza Pro font"]
fn rtl_fallback_matches_direct_shaping_across_a_paint_boundary() {
    let mut options = options(BASE);
    let data = std::fs::read("/System/Library/Fonts/GeezaPro.ttc").unwrap();
    let geeza = load(options.fontdb_mut(), &data);
    options.set_text_fallback(
        None,
        Some(Box::new(move |_, _, _, rtl, _, _| {
            assert!(rtl);
            Some((geeza, 1.0))
        })),
    );
    let markup = r#"<tspan fill="red">ل</tspan>ام"#;
    let fallback = render(&options, markup);
    options.font_resolver.select_font = Box::new(move |_, _| Some(geeza));
    let direct = render(&options, markup);
    assert_eq!(
        glyphs(&direct).iter().map(|g| g.2).collect::<Vec<_>>(),
        ["لا", "م"]
    );
    assert_eq!(glyphs(&fallback), glyphs(&direct));
    assert_eq!(text(&fallback).bounding_box(), text(&direct).bounding_box());
    for (actual, expected) in text(&fallback)
        .layouted()
        .iter()
        .zip(text(&direct).layouted())
    {
        assert_eq!(
            actual.positioned_glyphs.len(),
            expected.positioned_glyphs.len()
        );
        assert_eq!(
            actual.positioned_glyphs[0].transform(),
            expected.positioned_glyphs[0].transform()
        );
    }
}

#[test]
fn fallback_kerning_matches_the_same_face_used_directly() {
    let mut options = options(BASE);
    let latin = fixed_fallback(
        &mut options,
        include_bytes!("fonts/NotoSans-Latin-subset.ttf"),
        1.0,
    );
    let fallback = render(&options, "AV");
    options.font_resolver.select_font = Box::new(move |_, _| Some(latin));
    let direct = render(&options, "AV");
    assert_eq!(glyphs(&fallback), glyphs(&direct));
    assert_eq!(text(&fallback).bounding_box(), text(&direct).bounding_box());
    assert_eq!(transforms(&fallback), transforms(&direct));
}

#[test]
fn rtl_combining_marks_keep_the_base_characters_paint_and_offsets() {
    let mut options = options(BASE);
    let cousine = fixed_fallback(
        &mut options,
        include_bytes!("fonts/Cousine-Regular.ttf"),
        1.0,
    );
    let markup = r#"א<tspan fill="red">ב</tspan>&#x5B8;ל"#;
    let fallback = render(&options, markup);
    options.font_resolver.select_font = Box::new(move |_, _| Some(cousine));
    let direct = render(&options, markup);
    assert_eq!(text(&fallback).layouted()[1].positioned_glyphs.len(), 2);
    assert_eq!(glyphs(&fallback), glyphs(&direct));
    assert_eq!(transforms(&fallback), transforms(&direct));
    assert_eq!(text(&fallback).bounding_box(), text(&direct).bounding_box());
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the macOS Kohinoor Devanagari font"]
fn one_fallback_face_can_use_different_shaping_scripts_in_the_same_direction() {
    let mut options = options(include_bytes!("fonts/NotoSansSC-subset.ttf"));
    let data = std::fs::read("/System/Library/Fonts/Kohinoor.ttc").unwrap();
    let kohinoor = fixed_fallback(&mut options, &data, 1.0);
    let fallback = render(&options, "fi·कि");
    options.font_resolver.select_font = Box::new(move |_, _| Some(kohinoor));
    let direct = render(&options, r#"fi·<tspan xml:lang="hi">कि</tspan>"#);
    assert_eq!(glyphs(&fallback), glyphs(&direct));
    assert_eq!(transforms(&fallback), transforms(&direct));
}

#[test]
fn a_change_of_fallback_face_or_scale_breaks_the_ligature() {
    for change_face in [true, false] {
        let mut options = options(include_bytes!("fonts/NotoSansSC-subset.ttf"));
        let full = load(options.fontdb_mut(), FULL);
        let (other, scale) = if change_face {
            (load(options.fontdb_mut(), BASE), 1.0)
        } else {
            (full, 1.5)
        };
        options.set_text_fallback(
            None,
            Some(Box::new(move |_, source, _, _, _, _| {
                Some(if source == "f" {
                    (full, 1.0)
                } else {
                    (other, scale)
                })
            })),
        );
        let tree = render(&options, "fi");
        assert_eq!(glyphs(&tree), [(full, 1, "f"), (other, 2, "i")]);
        assert_eq!(text(&tree).bounding_box().width(), 10.0 + 10.0 * scale);
    }
}

#[test]
fn a_lazily_discovered_scalar_face_can_repair_an_earlier_cluster() {
    for content in ["✓f中", "中f✓"] {
        let mut options = options(BASE);
        let primary = options.fontdb.faces().next().unwrap().id;
        options.font_resolver.select_fallback =
            Box::new(|character, _, db| (character == '中').then(|| load(Arc::make_mut(db), FULL)));
        let tree = render(&options, content);
        let full = tree.fontdb().faces().find(|f| f.id != primary).unwrap().id;
        let glyphs = glyphs(&tree);
        assert_eq!(glyphs.len(), 3);
        for (face, id, source) in glyphs {
            assert_ne!(id, 0, "{content} is missing {source}");
            assert_eq!(face, if source == "f" { primary } else { full });
        }
    }
}

#[test]
fn repeated_clusters_reuse_the_selected_face_across_primary_glyphs() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut options = options(BASE);
    let full = load(options.fontdb_mut(), FULL);
    let calls = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&calls);
    options.set_text_fallback(
        None,
        Some(Box::new(move |_, cluster, _, _, _, _| {
            assert_eq!(cluster, "中");
            recorded.fetch_add(1, Ordering::Relaxed);
            Some((full, 1.0))
        })),
    );
    let tree = render(&options, &"\u{e000}中".repeat(4));
    assert_eq!(glyphs(&tree).len(), 8);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn equivalent_inherited_and_default_languages_keep_the_ligature() {
    let mut options = options(BASE);
    options.set_text_fallback(Some("en-US".into()), None);
    let reference = render(&options, "fi");
    assert_eq!(glyphs(&reference).len(), 1);
    for markup in [
        r#"f<tspan xml:lang="EN-us">i</tspan>"#,
        r#"<tspan xml:lang="en-US">f<tspan xml:lang="en-us">i</tspan></tspan>"#,
    ] {
        let tree = render(&options, markup);
        assert_eq!(glyphs(&tree), glyphs(&reference));
        assert_eq!(text(&tree).bounding_box(), text(&reference).bounding_box());
    }
    for tag in ["tr", ""] {
        let tree = render(&options, &format!(r#"f<tspan xml:lang="{tag}">i</tspan>"#));
        assert_eq!(glyphs(&tree).len(), 2);
    }
}

#[test]
fn inherited_and_overridden_languages_select_their_own_fallbacks() {
    let mut options = options(BASE);
    let inherited = load(options.fontdb_mut(), FULL);
    let overridden = load(
        options.fontdb_mut(),
        include_bytes!("fonts/NotoSansSC-subset.ttf"),
    );
    options.set_text_fallback(
        Some("ja".into()),
        Some(Box::new(move |_, cluster, language, _, _, _| {
            assert_eq!(cluster, "中");
            let face = match language {
                Some("zh-Hans") => inherited,
                Some("zh-Hant") => overridden,
                other => panic!("unexpected fallback language {other:?}"),
            };
            Some((face, 1.0))
        })),
    );
    let tree = render(
        &options,
        r#"<tspan xml:lang="zh-Hans">中<tspan xml:lang="zh-Hant">中</tspan>中</tspan>"#,
    );
    assert_eq!(
        glyphs(&tree)
            .into_iter()
            .map(|(id, ..)| id)
            .collect::<Vec<_>>(),
        [inherited, overridden, inherited]
    );
}

#[test]
fn fallback_receives_well_formed_language_tags() {
    for (tag, expected) in [
        ("ja-x", None),
        ("en-u", None),
        ("en-u-x-a", None),
        ("en-u-foo-u-bar", None),
        ("sl-rozaj-ROZAJ", None),
        ("", None),
        ("zh-Hant-TW", Some("zh-Hant-TW")),
        ("en-u-nu-latn-x-a", Some("en-u-nu-latn-x-a")),
        ("x-a", Some("x-a")),
        ("i-klingon", Some("i-klingon")),
        ("sgn-BE-FR", Some("sgn-BE-FR")),
    ] {
        let mut options = options(BASE);
        let full = load(options.fontdb_mut(), FULL);
        options.set_text_fallback(
            Some("fr".into()),
            Some(Box::new(move |_, _, lang, _, _, _| {
                assert_eq!(lang, expected, "xml:lang={tag:?}");
                Some((full, 1.0))
            })),
        );
        let tree = render(&options, &format!(r#"<tspan xml:lang="{tag}">中</tspan>"#));
        assert_eq!(glyphs(&tree), [(full, 4, "中")]);
    }
}
