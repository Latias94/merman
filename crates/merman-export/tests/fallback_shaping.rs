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
        assert!(tree.node_by_id("t").is_some());
    }
}
