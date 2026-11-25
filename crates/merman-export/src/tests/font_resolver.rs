use super::*;
use crate::font_environment::shared_system_fontdb;
use crate::font_selection::{FontSelectionPolicy, face_family_name};
use std::sync::{Arc, Mutex};

#[test]
fn browser_font_resolver_matches_named_families_case_insensitively() {
    let (fontdb, expected_id) = controlled_fontdb();

    assert_eq!(
        selected_font_id(Arc::clone(&fontdb), "'Maße Test Sans'"),
        expected_id,
        "the installed family spelling must select its named face"
    );
    assert_eq!(
        selected_font_id(Arc::clone(&fontdb), "'maße test sans'"),
        expected_id,
        "CSS family matching must ignore ASCII case"
    );
    assert_eq!(
        selected_font_id(Arc::clone(&fontdb), "'MASSE TEST SANS'"),
        expected_id,
        "CSS family matching must use Unicode default case folding"
    );
    assert_eq!(
        selected_font_id(
            Arc::clone(&fontdb),
            "'__merman_missing_font__', 'masse test sans'"
        ),
        expected_id,
        "case-insensitive matching must preserve CSS family stack order"
    );
}

#[test]
fn browser_font_resolver_rejects_a_poisoned_generic_but_preserves_valid_serif() {
    for fixture in [
        include_bytes!("../../tests/fonts/MermanSymbolicTest.ttf").as_slice(),
        include_bytes!("../../tests/fonts/MermanPictorialTest.ttf").as_slice(),
    ] {
        let mut database = usvg::fontdb::Database::new();
        database.load_font_data(fixture.to_vec());
        let symbol = face_family_name(database.faces().next().unwrap()).unwrap();
        database.load_font_data(
            include_bytes!(
                "../../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf"
            )
            .to_vec(),
        );
        let serif = database
            .faces()
            .find(|face| face.post_script_name == "DejaVuSerif")
            .unwrap()
            .id;
        database.set_serif_family(symbol.clone());
        database.set_sans_serif_family(symbol.clone());
        database.set_monospace_family(symbol.clone());
        FontSelectionPolicy::from_database(&database).configure_generics(&mut database);
        for family in ["serif", "sans-serif", "monospace", "__missing__"] {
            assert_eq!(
                selected_font_id(Arc::new(database.clone()), family),
                serif,
                "poisoned {family}"
            );
        }
        database.set_serif_family("DejaVu Serif");
        FontSelectionPolicy::from_database(&database).configure_generics(&mut database);
        assert_eq!(selected_font_id(Arc::new(database), "serif"), serif);
    }
}

fn controlled_fontdb() -> (Arc<usvg::fontdb::Database>, usvg::fontdb::ID) {
    let source_face = shared_system_fontdb()
        .faces()
        .next()
        .cloned()
        .expect("native export tests require one valid system font face");

    let mut fontdb = usvg::fontdb::Database::new();
    let mut fallback = source_face.clone();
    fallback.families = vec![(
        "Merman Fallback Serif".to_string(),
        usvg::fontdb::Language::English_UnitedStates,
    )];
    fallback.weight = usvg::fontdb::Weight::NORMAL;
    fallback.stretch = usvg::fontdb::Stretch::Normal;
    fallback.style = usvg::fontdb::Style::Normal;

    let mut target = source_face;
    target.families = vec![(
        "Maße Test Sans".to_string(),
        usvg::fontdb::Language::English_UnitedStates,
    )];
    target.weight = usvg::fontdb::Weight::NORMAL;
    target.stretch = usvg::fontdb::Stretch::Normal;
    target.style = usvg::fontdb::Style::Normal;

    fontdb.set_serif_family("Merman Fallback Serif");
    fontdb.set_sans_serif_family("Merman Fallback Serif");
    fontdb.set_monospace_family("Merman Fallback Serif");
    let _fallback_id = fontdb.push_face_info(fallback);
    let target_id = fontdb.push_face_info(target);
    (Arc::new(fontdb), target_id)
}

fn selected_font_id(fontdb: Arc<usvg::fontdb::Database>, font_family: &str) -> usvg::fontdb::ID {
    let selected = Arc::new(Mutex::new(Vec::new()));
    let selected_for_resolver = Arc::clone(&selected);
    let resolver =
        browser_like_font_resolver(Arc::new(FontSelectionPolicy::from_database(&fontdb)));
    let select_font = resolver.select_font;
    let options = usvg::Options {
        fontdb,
        font_resolver: usvg::FontResolver {
            select_font: Box::new(move |font, fontdb| {
                let id = select_font(font, fontdb);
                if let Some(id) = id {
                    selected_for_resolver
                        .lock()
                        .expect("selected-font recorder lock")
                        .push(id);
                }
                id
            }),
            select_fallback: resolver.select_fallback,
        },
        ..Default::default()
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60"><text x="10" y="40" font-size="24" style="font-family: {font_family};">A</text></svg>"#
    );
    usvg::Tree::from_str(&svg, &options).expect("parse test SVG with native font resolver");

    selected
        .lock()
        .expect("selected-font recorder lock")
        .first()
        .copied()
        .expect("text parsing must select a primary font")
}
