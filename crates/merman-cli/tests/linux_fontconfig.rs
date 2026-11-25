#![cfg(all(target_os = "linux", feature = "platform-system-font-query"))]

use merman::OperationControl;
use merman::svg::export::{
    NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../merman-export/tests/fonts")
        .join(name)
}

fn catalog(explicit: Option<&str>) -> NativeFontContext {
    let mut options = NativeFontOptions::default().with_system_fonts(true);
    if let Some(name) = explicit {
        options = options.with_font_file(fixture(name));
    }
    options.load_platform(&OperationControl::new()).unwrap()
}

fn selected(context: &NativeFontContext, family: &str, text: &str, language: &str) -> Vec<String> {
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let family = if family.is_empty() {
        String::new()
    } else {
        format!(r#" font-family="{family}""#)
    };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><text id="label" y="30"{family} xml:lang="{language}">{text}</text></svg>"#
    );
    let tree = usvg::Tree::from_str(&svg, &options).unwrap();
    context.check().unwrap();
    let Some(usvg::Node::Text(label)) = tree.node_by_id("label") else {
        return Vec::new();
    };
    label
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| {
            assert_ne!(glyph.id.0, 0);
            tree.fontdb()
                .face(glyph.font)
                .unwrap()
                .post_script_name
                .clone()
        })
        .collect()
}

// libtest can run other tests concurrently. Install the private Fontconfig environment
// before starting the child, without changing the test runner's process environment.
fn with_catalog(files: &[&str], rules: &str, test: fn(&Path)) {
    const ROOT: &str = "MERMAN_TEST_PRIVATE_FONTCONFIG";
    if let Some(root) = std::env::var_os(ROOT) {
        test(Path::new(&root));
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let fonts = root.path().join("fonts");
    fs::create_dir(&fonts).unwrap();
    for file in files {
        fs::copy(fixture(file), fonts.join(file)).unwrap();
    }
    let config = root.path().join("fonts.conf");
    fs::write(&config, format!(r#"<fontconfig><dir prefix="xdg">fonts</dir><cachedir prefix="xdg">cache</cachedir>{rules}</fontconfig>"#)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            std::thread::current().name().unwrap(),
            "--nocapture",
        ])
        .env(ROOT, root.path())
        .env("FONTCONFIG_FILE", config)
        .env("XDG_DATA_HOME", root.path())
        .env("XDG_DATA_DIRS", root.path())
        .env("XDG_CACHE_HOME", root.path())
        .env("FC_LANG", "en")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ordered_aliases_compete_with_an_explicit_face() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="strong"><family>Strong Body</family><prefer><family>Cousine</family></prefer></alias>
        <alias binding="same"><family>Same Body</family><prefer><family>Cousine</family></prefer></alias>
        <alias binding="weak"><family>Weak Body</family><prefer><family>Cousine</family></prefer></alias>
        <match target="pattern"><test name="family"><string>All Weak</string></test>
        <edit name="family" mode="assign_replace" binding="weak"><string>Cousine</string></edit></match>"#,
        |root| {
            let partial = catalog(Some("MermanProbeBase.ttf"));
            assert_eq!(
                selected(&partial, "All Weak", "A", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(
                selected(&partial, "Weak Body, MermanProbeBase, Noto Sans", "A", "en"),
                ["NotoSans-Regular"]
            );
            for also_installed in [true, false] {
                if !also_installed {
                    fs::remove_file(root.join("fonts/NotoSans-Latin-subset.ttf")).unwrap();
                }
                let ctx = catalog(Some("NotoSans-Latin-subset.ttf"));
                assert_eq!(
                    selected(&ctx, "Weak Body, Noto Sans", "A", "en"),
                    ["NotoSans-Regular"]
                );
                assert_eq!(
                    selected(&ctx, "Strong Body, Noto Sans", "A", "en"),
                    ["Cousine-Regular"]
                );
                assert_eq!(
                    selected(&ctx, "Same Body, Noto Sans", "A", "en"),
                    ["Cousine-Regular"]
                );
            }
            fs::remove_file(root.join("fonts/Cousine-Regular.ttf")).unwrap();
            assert_eq!(
                selected(
                    &catalog(Some("NotoSans-Latin-subset.ttf")),
                    "Noto Sans",
                    "A",
                    "en"
                ),
                ["NotoSans-Regular"]
            );
        },
    );
}

#[test]
fn cached_face_obeys_the_scan_charset_for_glyph_fallback() {
    with_catalog(
        &[
            "Cousine-Regular.ttf",
            "NotoSans-Latin-subset.ttf",
            "MermanProbeBase.ttf",
        ],
        r#"
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="charset" mode="assign"><charset><int>86</int></charset></edit></match>"#,
        |root| {
            let ctx = catalog(None);
            let stack = "MermanProbeBase, Cousine";
            assert_eq!(selected(&ctx, "Cousine", "V", "en"), ["Cousine-Regular"]);
            assert_eq!(selected(&ctx, stack, "A", "en"), ["NotoSans-Regular"]);
            assert_eq!(
                selected(&catalog(None), stack, "A", "en"),
                ["NotoSans-Regular"]
            );
            fs::remove_file(root.join("fonts/Cousine-Regular.ttf")).unwrap();
            assert_eq!(selected(&ctx, stack, "V", "en"), ["Cousine-Regular"]);
        },
    );
}

#[test]
fn distinct_clusters_keep_charset_dependent_family_rules() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="strong"><family>sans-serif</family><prefer><family>Review Body</family></prefer></alias>
        <match target="pattern">
        <test name="family" qual="any"><string>Review Body</string></test>
        <test name="charset" compare="contains"><charset><int>65</int></charset></test>
        <edit name="family" mode="prepend_first" binding="strong"><string>Cousine</string></edit></match>
        <match target="pattern">
        <test name="family" qual="any"><string>Review Body</string></test>
        <test name="charset" compare="contains"><charset><int>86</int></charset></test>
        <edit name="family" mode="prepend_first" binding="strong"><string>Noto Sans</string></edit></match>"#,
        |_| {
            let context = catalog(Some("MermanProbeBase.ttf"));
            for stack in [
                "MermanProbeBase, Review Body",
                "MermanProbeBase, sans-serif",
            ] {
                assert_eq!(
                    selected(&context, stack, "AVA", "en"),
                    ["Cousine-Regular", "NotoSans-Regular", "Cousine-Regular"]
                );
            }
        },
    );
}

#[test]
fn new_operations_see_current_fonts_and_exports_keep_admitted_faces() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="strong"><family>Body</family><prefer><family>Cousine</family></prefer></alias>"#,
        |root| {
            let ctx = catalog(None);
            let unselected = catalog(None);
            assert_eq!(selected(&ctx, "Body", "A", "en"), ["Cousine-Regular"]);
            let export = ctx.for_export(&OperationControl::new()).unwrap();
            let conf = root.join("fonts.conf");
            let modified = conf.metadata().unwrap().modified().unwrap();
            fs::write(
                &conf,
                fs::read_to_string(&conf)
                    .unwrap()
                    .replace("Cousine", "Noto Sans"),
            )
            .unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&conf)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            assert_eq!(conf.metadata().unwrap().modified().unwrap(), modified);
            let next = ctx.for_operation(&OperationControl::new());
            assert_eq!(selected(&next, "Body", "A", "en"), ["NotoSans-Regular"]);
            fs::remove_file(root.join("fonts/Cousine-Regular.ttf")).unwrap();
            assert_eq!(selected(&export, "Body", "A", "en"), ["Cousine-Regular"]);
            assert_eq!(selected(&export, "Cousine", "V", "en"), ["Cousine-Regular"]);
            assert_eq!(
                selected(&unselected, "Cousine, Noto Sans", "A", "en"),
                ["NotoSans-Regular"]
            );
            assert_eq!(
                selected(
                    &ctx.for_operation(&OperationControl::new()),
                    "Cousine, Noto Sans",
                    "A",
                    "en"
                ),
                ["NotoSans-Regular"]
            );
            fs::rename(root.join("fonts"), root.join("detached-fonts")).unwrap();
            assert!(
                selected(
                    &ctx.for_operation(&OperationControl::new()),
                    "Cousine, Noto Sans",
                    "A",
                    "en"
                )
                .is_empty()
            );
        },
    );
}

#[test]
fn primary_family_uses_the_effective_text_language() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="strong"><family>sans-serif</family><prefer><family>Cousine</family></prefer></alias>
        <match target="pattern"><test name="lang" compare="contains"><string>ja</string></test>
        <edit name="family" mode="prepend_first" binding="strong"><string>Noto Sans</string></edit></match>"#,
        |_| {
            let ctx = catalog(None);
            for (language, expected) in [
                ("en", "Cousine-Regular"),
                ("ja", "NotoSans-Regular"),
                ("en", "Cousine-Regular"),
            ] {
                assert_eq!(selected(&ctx, "sans-serif", "A", language), [expected]);
            }
        },
    );
}

#[test]
fn system_ui_and_empty_defaults_keep_generic_family_precedence() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="weak"><family>system-ui</family><prefer><family>Cousine</family></prefer></alias>
        <alias binding="strong"><family>-apple-system</family><prefer><family>MermanVariableWeight</family></prefer></alias>
        <alias binding="strong"><family>sans-serif</family><prefer><family>Noto Sans</family></prefer></alias>
        <alias binding="strong"><family>serif</family><prefer><family>Cousine</family></prefer></alias>"#,
        |_| {
            let context = catalog(Some("MermanSymbolicTest.ttf"));
            assert_eq!(
                selected(&context, "system-ui", "A", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(selected(&context, "", "A", "en"), ["NotoSans-Regular"]);

            let context = catalog(Some("NotoSans-Latin-subset.ttf"));
            for family in ["system-ui", "BlinkMacSystemFont"] {
                assert_eq!(selected(&context, family, "A", "en"), ["Cousine-Regular"]);
            }
            assert_eq!(
                selected(&context, "sans-serif", "A", "en"),
                ["NotoSans-Regular"]
            );
            assert_eq!(
                selected(
                    &catalog(Some("MermanVariableWeight.ttf")),
                    "-apple-system",
                    "A",
                    "en"
                ),
                ["MermanVariableWeight-Thin"]
            );
        },
    );
}

#[test]
fn system_ui_weak_caller_alias_preserves_native_language_order() {
    with_catalog(
        &["NotoSans-Latin-subset.ttf", "MermanVariableWeight.ttf"],
        r#"
        <alias binding="weak"><family>system-ui</family><prefer><family>Cousine</family><family>Noto Sans</family><family>Merman Variable Weight</family></prefer></alias>
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="lang" mode="assign"><langset><string>en</string></langset></edit></match>
        <match target="scan"><test name="family"><string>Noto Sans</string></test>
        <edit name="lang" mode="assign"><langset><string>en</string></langset></edit></match>
        <match target="scan"><test name="family"><string>Merman Variable Weight</string></test>
        <edit name="lang" mode="assign"><langset><string>ja</string></langset></edit></match>"#,
        |_| {
            assert_eq!(
                selected(&catalog(None), "system-ui", "A", "en"),
                ["NotoSans-Regular"]
            );
            let context = catalog(Some("Cousine-Regular.ttf"));
            assert_eq!(
                selected(&context, "system-ui", "A", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(
                selected(&context, "system-ui", "A", "ja"),
                ["MermanVariableWeight-Thin"]
            );
        },
    );
}

#[test]
fn scan_charsets_require_a_complete_normalized_cluster() {
    with_catalog(
        &[
            "MermanProbeBase.ttf",
            "Cousine-Regular.ttf",
            "Cousine-Bold.ttf",
        ],
        r#"
        <match target="scan"><test name="postscriptname"><string>Cousine-Regular</string></test>
        <edit name="charset" mode="assign"><charset><int>101</int></charset></edit></match>
        <match target="scan"><test name="postscriptname"><string>Cousine-Bold</string></test>
        <edit name="charset" mode="assign"><charset><int>233</int></charset></edit></match>"#,
        |_| {
            let context = catalog(None);
            assert_eq!(
                selected(&context, "Cousine", "e", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(
                selected(&context, "MermanProbeBase, Cousine", "e&#x301;", "en"),
                ["Cousine-Bold"]
            );
        },
    );
}

#[test]
fn a_scan_charset_mirrors_only_in_rtl_layout() {
    with_catalog(
        &["Cousine-Regular.ttf", "Cousine-Bold.ttf"],
        r#"
        <match target="scan"><test name="postscriptname"><string>Cousine-Regular</string></test>
        <edit name="charset" mode="assign"><charset><int>40</int></charset></edit></match>"#,
        |_| {
            let context = catalog(Some("MermanProbeBase.ttf"));
            let stack = "MermanProbeBase, Cousine";
            assert_eq!(
                selected(&context, stack, "f(i", "en"),
                ["MermanProbeBase", "Cousine-Regular", "MermanProbeBase"]
            );
            assert_eq!(
                selected(&context, stack, "f)i", "en"),
                ["MermanProbeBase", "Cousine-Bold", "MermanProbeBase"]
            );
        },
    );
}

#[test]
fn query_substitutions_see_variation_selectors_and_joiners() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        r#"
        <alias binding="strong"><family>Body</family><prefer><family>Cousine</family></prefer></alias>
        <match target="pattern">
        <test name="family" qual="any"><string>Body</string></test>
        <test name="charset" compare="contains"><charset><int>65</int><int>65039</int><int>8205</int></charset></test>
        <edit name="family" mode="prepend_first" binding="strong"><string>Noto Sans</string></edit></match>"#,
        |_| {
            let context = catalog(Some("MermanProbeBase.ttf"));
            let stack = "MermanProbeBase, Body";
            assert_eq!(
                selected(&context, stack, "A\u{fe0f}", "en")[0],
                "Cousine-Regular"
            );
            assert_eq!(
                selected(&context, stack, "A\u{fe0f}\u{200d}", "en")[0],
                "NotoSans-Regular"
            );
        },
    );
}

#[test]
fn a_variation_only_candidate_needs_a_real_non_default_mapping() {
    with_catalog(
        &["Cousine-Regular.ttf"],
        r#"
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="charset" mode="assign"><charset><int>86</int></charset></edit></match>"#,
        |root| {
            let fixture = fs::read(fixture("MermanProbeFull.ttf")).unwrap();
            let variant = replace_table(fixture, b"cmap", &coverage_cmap(&['f'], Some('A')));
            fs::write(root.join("fonts/Variant.ttf"), variant).unwrap();
            let context = catalog(Some("MermanProbeBase.ttf"));
            // Admit Cousine first to cover both the file probe and the snapshot path.
            assert_eq!(
                selected(&context, "Cousine", "V", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(
                selected(&context, "MermanProbeBase, Cousine", "A\u{fe0f}", "en"),
                ["MermanProbeFull"]
            );
        },
    );
}

#[test]
fn rtl_variation_beats_nominal_mirroring_with_bounded_cmap_reads() {
    with_catalog(
        &[],
        r#"
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="lang" mode="assign"><langset><string>en</string></langset></edit></match>
        <match target="scan"><test name="family"><string>MermanProbeFull</string></test>
        <edit name="lang" mode="assign"><langset><string>ja</string></langset></edit></match>"#,
        |root| {
            let write_font = |name: &str, path: &str, cmap: &[u8]| {
                let bytes = fs::read(fixture(name)).unwrap();
                fs::write(root.join(path), replace_table(bytes, b"cmap", cmap)).unwrap();
            };
            // Same selector, different base. The complete cmap exceeds the
            // discovery budget, but its candidate mapping can be skipped.
            let mut large = variant_cmap(&['∈'], Some(('A', '\u{fe0f}', false)), 10);
            large.resize(64 * 1024, 0);
            write_font("Cousine-Regular.ttf", "fonts/Nominal.ttf", large.as_slice());
            // The RTL shaper mirrors ∋ to ∈ in this face, so its ∋ + VS16
            // mapping is irrelevant. The other face keeps ∋ and uses its UVS.
            write_font(
                "Cousine-Bold.ttf",
                "fonts/OtherBase.ttf",
                &coverage_cmap(&['∈'], Some('∋')),
            );
            write_font(
                "MermanProbeFull.ttf",
                "fonts/Variant.ttf",
                &coverage_cmap(&['f'], Some('∋')),
            );
            write_font(
                "MermanProbeBase.ttf",
                "Base.ttf",
                &coverage_cmap(&['א'], None),
            );
            let options = NativeFontOptions::default()
                .with_system_fonts(true)
                .with_font_file(root.join("Base.ttf"));
            let context = options
                .clone()
                .load_platform(&OperationControl::new())
                .unwrap();
            assert_eq!(
                selected(&context, "MermanProbeBase", "א∋א", "en"),
                ["MermanProbeBase", "Cousine-Regular", "MermanProbeBase"]
            );
            let options = options.with_limits(NativeFontLimits {
                max_system_scan_bytes: 8192,
                ..NativeFontLimits::default()
            });
            for corrupt in [false, true] {
                let context = options
                    .clone()
                    .load_platform(&OperationControl::new())
                    .unwrap();
                if corrupt {
                    // The catalog saw a valid nominal-only font. A stale cmap
                    // length must not stop a speculative search for UVS.
                    let path = root.join("fonts/Nominal.ttf");
                    let mut font = fs::read(&path).unwrap();
                    let cmap = table_record(&font, b"cmap");
                    let length = (font.len() as u32).to_be_bytes();
                    font[cmap + 12..cmap + 16].copy_from_slice(&length);
                    fs::write(path, font).unwrap();
                }
                assert_eq!(
                    selected(&context, "MermanProbeBase", "א∋\u{fe0f}א", "en"),
                    ["MermanProbeBase", "MermanProbeFull", "MermanProbeBase"]
                );
            }
        },
    );
}

#[test]
fn an_unrelated_ideographic_uvs_does_not_read_the_full_cmap() {
    with_catalog(
        &[],
        r#"
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="lang" mode="assign"><langset><string>en</string></langset></edit></match>
        <match target="scan"><test name="family"><string>MermanProbeFull</string></test>
        <edit name="lang" mode="assign"><langset><string>ja</string></langset></edit></match>"#,
        |root| {
            for (name, base) in [("Cousine-Regular.ttf", '文'), ("MermanProbeFull.ttf", '中')] {
                let font = fs::read(fixture(name)).unwrap();
                let mut cmap = variant_cmap(&['中'], Some((base, '\u{e0100}', false)), 10);
                if base == '文' {
                    cmap.resize(64 * 1024, 0);
                }
                fs::write(
                    root.join("fonts").join(name),
                    replace_table(font, b"cmap", &cmap),
                )
                .unwrap();
            }
            let context = NativeFontOptions::default()
                .with_system_fonts(true)
                .with_font_file(fixture("MermanProbeBase.ttf"))
                .with_limits(NativeFontLimits {
                    max_system_scan_bytes: 8192,
                    ..NativeFontLimits::default()
                })
                .load_platform(&OperationControl::new())
                .unwrap();
            assert_eq!(
                selected(&context, "MermanProbeBase", "中\u{e0100}", "en"),
                ["MermanProbeFull"]
            );
        },
    );
}

#[test]
fn variation_covers_canonically_equivalent_scan_charsets() {
    with_catalog(
        &[],
        r#"
        <match target="scan"><test name="family"><string>MermanProbeFull</string></test>
        <edit name="charset" mode="assign"><charset><int>233</int></charset></edit></match>"#,
        |root| {
            let fixture = fs::read(fixture("MermanProbeFull.ttf")).unwrap();
            let cmap = coverage_cmap(&['e', 'é', '\u{301}'], Some('❤'));
            fs::write(
                root.join("fonts/Variant.ttf"),
                replace_table(fixture, b"cmap", &cmap),
            )
            .unwrap();
            let context = catalog(Some("MermanProbeBase.ttf"));
            for text in ["e\u{301}\u{200d}❤\u{fe0f}", "é\u{200d}❤\u{fe0f}"] {
                let selected = selected(&context, "MermanProbeBase", text, "en");
                assert!(!selected.is_empty(), "{text:?}");
                assert!(
                    selected.iter().all(|name| name == "MermanProbeFull"),
                    "{text:?}: {selected:?}"
                );
            }
        },
    );
}

#[test]
fn default_and_non_default_variations_follow_native_rank() {
    with_catalog(
        &[],
        r#"
        <match target="scan"><test name="family"><string>MermanProbeFull</string></test>
        <edit name="lang" mode="assign"><langset><string>en</string></langset></edit></match>
        <match target="scan"><test name="family"><string>MermanProbePartial</string></test>
        <edit name="lang" mode="assign"><langset><string>ja</string></langset></edit></match>"#,
        |root| {
            for (file, default) in [
                ("MermanProbeFull.ttf", true),
                ("MermanProbePartial.ttf", false),
            ] {
                let font = fs::read(fixture(file)).unwrap();
                let cmap = variant_cmap(&['❤'], Some(('❤', '\u{fe0f}', default)), 10);
                fs::write(
                    root.join("fonts").join(file),
                    replace_table(font, b"cmap", &cmap),
                )
                .unwrap();
            }
            let context = catalog(Some("MermanProbeBase.ttf"));
            for (lang, expected) in [("en", "MermanProbeFull"), ("ja", "MermanProbePartial")] {
                assert_eq!(
                    selected(&context, "MermanProbeBase", "❤\u{fe0f}", lang),
                    [expected]
                );
            }
        },
    );
}

#[test]
fn mongolian_selector_is_context_for_shaping_and_not_scan_coverage() {
    with_catalog(&[], "", |root| {
        let font = fs::read(fixture("MermanProbeFull.ttf")).unwrap();
        let cmap = coverage_cmap(&['\u{1820}'], None);
        fs::write(
            root.join("fonts/Mongolian.ttf"),
            replace_table(font, b"cmap", &cmap),
        )
        .unwrap();
        let context = catalog(Some("MermanProbeBase.ttf"));
        assert_eq!(
            selected(&context, "MermanProbeBase", "\u{1820}\u{180f}", "mn"),
            ["MermanProbeFull"]
        );
    });
}

#[test]
fn legacy_arabic_symbol_font_can_supply_ordinary_arabic_text() {
    with_catalog(&[], "", |root| {
        let font = fs::read(fixture("MermanProbeFull.ttf")).unwrap();
        let mut cmap = coverage_cmap(&['\u{f141}'], None);
        cmap[6..8].fill(0); // Microsoft Symbol encoding.
        let record = table_record(&font, b"OS/2");
        let offset = table_offset(&font, record);
        let mut os2 = font[offset..offset + 78].to_vec();
        os2[..2].fill(0);
        os2[62] = 0xb2;
        let font = replace_table(font, b"OS/2", &os2);
        fs::write(
            root.join("fonts/Legacy.ttf"),
            replace_table(font, b"cmap", &cmap),
        )
        .unwrap();
        let context = catalog(Some("MermanProbeBase.ttf"));
        assert_eq!(
            selected(&context, "MermanProbeBase", "ا", "ar"),
            ["MermanProbeFull"]
        );
    });
}

#[test]
fn a_stale_leaf_under_a_linked_directory_can_fall_through() {
    with_catalog(
        &["Cousine-Regular.ttf", "NotoSans-Latin-subset.ttf"],
        "",
        |root| {
            let fonts = root.join("fonts");
            let physical = root.join("physical-fonts");
            fs::rename(&fonts, &physical).unwrap();
            std::os::unix::fs::symlink(&physical, &fonts).unwrap();
            let context = catalog(None);
            fs::remove_file(physical.join("Cousine-Regular.ttf")).unwrap();
            assert_eq!(
                selected(&context, "Cousine, Noto Sans", "A", "en"),
                ["NotoSans-Regular"]
            );
        },
    );
}

#[test]
fn cluster_fallback_uses_the_requested_cjk_orthography() {
    with_catalog(
        &["MermanProbeFull.ttf", "NotoSansSC-subset.ttf"],
        r#"
        <match target="scan"><test name="family"><string>MermanProbeFull</string></test>
        <edit name="lang" mode="assign"><langset><string>ja</string></langset></edit></match>
        <match target="scan"><test name="postscriptname"><string>NotoSansSC-Thin</string></test>
        <edit name="lang" mode="assign"><langset><string>zh-cn</string></langset></edit></match>"#,
        |_| {
            let context = catalog(Some("MermanProbeBase.ttf"));
            for (language, expected) in [("ja", "MermanProbeFull"), ("zh-Hans", "NotoSansSC-Thin")]
            {
                assert_eq!(
                    selected(&context, "MermanProbeBase", "中", language),
                    [expected]
                );
            }
        },
    );
}

#[test]
fn concurrent_catalog_initialization_and_readers() {
    with_catalog(&["Cousine-Regular.ttf"], "", |_| {
        let start = std::sync::Barrier::new(4);
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    start.wait();
                    let context = catalog(None);
                    assert_eq!(
                        selected(&context, "Cousine", "A", "en"),
                        ["Cousine-Regular"]
                    );
                    let next = context.for_operation(&OperationControl::new());
                    assert_eq!(
                        selected(&next, "sans-serif", "A", "en"),
                        ["Cousine-Regular"]
                    );
                });
            }
        });
    });
}

#[test]
fn named_fontconfig_instance_and_render_rule_reach_svg_geometry() {
    with_catalog(
        &["Cousine-Regular.ttf"],
        r#"
        <match target="pattern"><test name="family"><string>Merman Variable Weight</string></test>
        <edit name="weight" mode="assign"><int>200</int></edit></match>
        <match target="font"><test target="pattern" name="lang" compare="contains"><string>fr</string></test>
        <edit name="fontvariations" mode="assign"><string>wght=550,bad,wght=500,wdth=NaN</string></edit></match>"#,
        |root| {
            add_named_weight_fixture(&root.join("fonts/Named.ttf"));
            let native = catalog(None);
            let explicit = NativeFontOptions::default()
                .with_font_file(fixture("MermanVariableWeight.ttf"))
                .load(&OperationControl::new())
                .unwrap();
            let named = variable_geometry(&native, "en", "");
            let overridden = variable_geometry(&native, "fr", "");
            assert_eq!(selected(&native, "Cousine", "A", "fr"), ["Cousine-Regular"]);
            assert_ne!(named, overridden);
            assert_eq!(named, variable_geometry(&explicit, "en", "'wght' 700"));
            assert_eq!(overridden, variable_geometry(&explicit, "en", "'wght' 500"));
        },
    );
}

fn add_named_weight_fixture(path: &Path) {
    let font = fs::read(fixture("MermanVariableWeight.ttf")).unwrap();
    let record = table_record(&font, b"fvar");
    let start = table_offset(&font, record);
    let length = u32::from_be_bytes(font[record + 12..record + 16].try_into().unwrap()) as usize;
    let mut table = font[start..start + length].to_vec();
    assert_eq!(table[8..16], [0, 1, 0, 20, 0, 0, 0, 8]);
    table[12..14].copy_from_slice(&1u16.to_be_bytes());
    // Use the existing name ID 256 and one 16.16 design coordinate for weight 700.
    table.extend_from_slice(&[1, 0, 0, 0]);
    table.extend_from_slice(&(700u32 << 16).to_be_bytes());
    fs::write(path, replace_table(font, b"fvar", &table)).unwrap();
}

fn table_record(font: &[u8], tag: &[u8; 4]) -> usize {
    let table_count = usize::from(u16::from_be_bytes(font[4..6].try_into().unwrap()));
    (0..table_count)
        .map(|index| 12 + 16 * index)
        .find(|&start| &font[start..start + 4] == tag)
        .unwrap()
}

fn table_offset(font: &[u8], record: usize) -> usize {
    u32::from_be_bytes(font[record + 8..record + 12].try_into().unwrap()) as usize
}

fn coverage_cmap(nominal: &[char], vs16_base: Option<char>) -> Vec<u8> {
    variant_cmap(nominal, vs16_base.map(|base| (base, '\u{fe0f}', false)), 10)
}

fn variant_cmap(nominal: &[char], variant: Option<(char, char, bool)>, encoding: u8) -> Vec<u8> {
    let nominal_offset = if variant.is_some() { 20u32 } else { 12 };
    let nominal_length = 16 + 12 * nominal.len() as u32;
    let mut cmap = vec![0, 0, 0, if variant.is_some() { 2 } else { 1 }];
    if variant.is_some() {
        cmap.extend_from_slice(&[0, 0, 0, 5]);
        cmap.extend_from_slice(&(nominal_offset + nominal_length).to_be_bytes());
    }
    cmap.extend_from_slice(&[0, 3, 0, encoding]);
    cmap.extend_from_slice(&nominal_offset.to_be_bytes());
    cmap.extend_from_slice(&[0, 12, 0, 0]);
    cmap.extend_from_slice(&nominal_length.to_be_bytes());
    cmap.extend_from_slice(&[0; 4]);
    cmap.extend_from_slice(&(nominal.len() as u32).to_be_bytes());
    for &ch in nominal {
        let code = u32::from(ch).to_be_bytes();
        cmap.extend_from_slice(&code);
        cmap.extend_from_slice(&code);
        cmap.extend_from_slice(&1u32.to_be_bytes());
    }
    if let Some((base, selector, default)) = variant {
        cmap.extend_from_slice(&[0, 14, 0, 0, 0, if default { 29 } else { 30 }, 0, 0, 0, 1]);
        cmap.extend_from_slice(&u32::from(selector).to_be_bytes()[1..]);
        cmap.extend_from_slice(if default {
            &[0, 0, 0, 21, 0, 0, 0, 0]
        } else {
            &[0, 0, 0, 0, 0, 0, 0, 21]
        });
        cmap.extend_from_slice(&1u32.to_be_bytes());
        cmap.extend_from_slice(&u32::from(base).to_be_bytes()[1..]);
        cmap.extend_from_slice(if default { &[0][..] } else { &[0, 1] });
    }
    cmap
}

fn replace_table(mut font: Vec<u8>, tag: &[u8; 4], table: &[u8]) -> Vec<u8> {
    let record = table_record(&font, tag);
    let head = table_offset(&font, table_record(&font, b"head"));
    let end = font.len() as u32;
    font.extend_from_slice(table);
    font.resize(font.len().next_multiple_of(4), 0);
    let checksum = |bytes: &[u8]| {
        let (words, remainder) = bytes.as_chunks::<4>();
        assert!(remainder.is_empty());
        words.iter().fold(0u32, |sum, &word| {
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    };
    let table_sum = checksum(&font[end as usize..]);
    font[record + 4..record + 8].copy_from_slice(&table_sum.to_be_bytes());
    font[record + 8..record + 12].copy_from_slice(&end.to_be_bytes());
    font[record + 12..record + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
    font[head + 8..head + 12].fill(0);
    let adjustment = 0xb1b0_afba_u32.wrapping_sub(checksum(&font));
    font[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    font
}

fn variable_geometry(context: &NativeFontContext, language: &str, settings: &str) -> (f32, f32) {
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><text id="label" y="30" font-family="Merman Variable Weight" font-variation-settings="{settings}" xml:lang="{language}">AV</text></svg>"#
    );
    let tree = usvg::Tree::from_str(&svg, &options).unwrap();
    context.check().unwrap();
    let Some(usvg::Node::Text(label)) = tree.node_by_id("label") else {
        panic!("variable fixture did not shape");
    };
    let glyphs: Vec<_> = label
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .collect();
    let [first, last] = glyphs.as_slice() else {
        panic!("variable fixture did not shape both glyphs");
    };
    for glyph in &glyphs {
        let face = tree.fontdb().face(glyph.font).unwrap();
        assert_eq!(face.index, 0);
        assert_eq!(face.post_script_name, "MermanVariableWeight-Thin");
    }
    (
        last.transform().tx - first.transform().tx,
        label.flattened().bounding_box().unwrap().width(),
    )
}

#[test]
fn bitmap_font_leaves_the_next_authored_face_available() {
    with_catalog(&["NotoSans-Latin-subset.ttf"], "", |root| {
        let bdf = r#"STARTFONT 2.1
FONT -misc-review-medium-r-normal--8-80-75-75-c-80-iso10646-1
SIZE 8 75 75
FONTBOUNDINGBOX 8 8 0 -1
STARTPROPERTIES 6
FAMILY_NAME "Review Bitmap"
FONT_ASCENT 7
FONT_DESCENT 1
CHARSET_REGISTRY "ISO10646"
CHARSET_ENCODING "1"
SPACING "C"
ENDPROPERTIES
CHARS 1
STARTCHAR A
ENCODING 65
SWIDTH 1000 0
DWIDTH 8 0
BBX 8 8 0 -1
BITMAP
18
24
42
42
7E
42
42
00
ENDCHAR
ENDFONT
"#;
        fs::write(root.join("fonts/bitmap.bdf"), bdf).unwrap();
        assert_eq!(
            selected(&catalog(None), "Review Bitmap, Noto Sans", "A", "en"),
            ["NotoSans-Regular"]
        );
        let fonts = root.join("fonts");
        let modified = fonts.metadata().unwrap().modified().unwrap();
        fs::remove_file(fonts.join("NotoSans-Latin-subset.ttf")).unwrap();
        fs::File::open(&fonts)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let empty = || {
            NativeFontOptions::default()
                .with_system_fonts(true)
                .load_platform(&OperationControl::new())
        };
        assert!(matches!(empty(), Err(NativeFontError::EmptyFontSet)));
        fs::remove_file(root.join("fonts/bitmap.bdf")).unwrap();
        assert!(matches!(empty(), Err(NativeFontError::EmptyFontSet)));
    });
}
