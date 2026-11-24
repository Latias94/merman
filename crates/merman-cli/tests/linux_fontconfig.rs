#![cfg(all(target_os = "linux", feature = "platform-system-font-query"))]

use merman::OperationControl;
use merman::svg::export::{NativeFontContext, NativeFontError, NativeFontOptions};
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
        <alias binding="strong"><family>sans-serif</family><prefer><family>Noto Sans</family></prefer></alias>
        <alias binding="strong"><family>serif</family><prefer><family>Cousine</family></prefer></alias>"#,
        |_| {
            let context = catalog(Some("MermanSymbolicTest.ttf"));
            assert_eq!(
                selected(&context, "system-ui", "A", "en"),
                ["Cousine-Regular"]
            );
            assert_eq!(selected(&context, "", "A", "en"), ["NotoSans-Regular"]);
        },
    );
}

#[test]
fn normalized_clusters_can_pass_a_declared_charset() {
    with_catalog(
        &["MermanProbeBase.ttf", "Cousine-Regular.ttf"],
        r#"
        <match target="scan"><test name="family"><string>Cousine</string></test>
        <edit name="charset" mode="assign"><charset><int>233</int></charset></edit></match>"#,
        |_| {
            let context = catalog(None);
            assert_eq!(
                selected(&context, "MermanProbeBase, Cousine", "e&#x301;", "en"),
                ["Cousine-Regular"]
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
            assert_ne!(named, overridden);
            assert_eq!(named, variable_geometry(&explicit, "en", "'wght' 700"));
            assert_eq!(overridden, variable_geometry(&explicit, "en", "'wght' 500"));
        },
    );
}

fn add_named_weight_fixture(path: &Path) {
    let mut font = fs::read(fixture("MermanVariableWeight.ttf")).unwrap();
    let table_count = usize::from(u16::from_be_bytes(font[4..6].try_into().unwrap()));
    let record = |tag: &[u8; 4]| {
        (0..table_count)
            .map(|index| 12 + 16 * index)
            .find(|&start| &font[start..start + 4] == tag)
            .unwrap()
    };
    let offset = |start: usize| u32::from_be_bytes(font[start + 8..start + 12].try_into().unwrap());
    let fvar = record(b"fvar");
    let head = offset(record(b"head")) as usize;
    let start = offset(fvar) as usize;
    let length = u32::from_be_bytes(font[fvar + 12..fvar + 16].try_into().unwrap()) as usize;
    let mut table = font[start..start + length].to_vec();
    assert_eq!(table[8..16], [0, 1, 0, 20, 0, 0, 0, 8]);
    table[12..14].copy_from_slice(&1u16.to_be_bytes());
    // Use the existing name ID 256 and one 16.16 design coordinate for weight 700.
    table.extend_from_slice(&[1, 0, 0, 0]);
    table.extend_from_slice(&(700u32 << 16).to_be_bytes());
    let checksum = |bytes: &[u8]| {
        assert_eq!(bytes.len() % 4, 0);
        bytes.chunks_exact(4).fold(0u32, |sum, word| {
            sum.wrapping_add(u32::from_be_bytes(word.try_into().unwrap()))
        })
    };
    let end = font.len() as u32;
    font[fvar + 4..fvar + 8].copy_from_slice(&checksum(&table).to_be_bytes());
    font[fvar + 8..fvar + 12].copy_from_slice(&end.to_be_bytes());
    font[fvar + 12..fvar + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
    font.extend_from_slice(&table);
    font[head + 8..head + 12].fill(0);
    let adjustment = 0xb1b0_afba_u32.wrapping_sub(checksum(&font));
    font[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    fs::write(path, font).unwrap();
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
        label.flattened().bounding_box().width(),
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
