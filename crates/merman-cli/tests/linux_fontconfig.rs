#![cfg(all(target_os = "linux", feature = "platform-system-font-query"))]

use merman::OperationControl;
use merman::svg::export::{NativeFontContext, NativeFontOptions};
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
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><text id="label" y="30" font-family="{family}" xml:lang="{language}">{text}</text></svg>"#
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
        <alias binding="weak"><family>Weak Body</family><prefer><family>Cousine</family></prefer></alias>"#,
        |root| {
            let partial = catalog(Some("MermanProbeBase.ttf"));
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
            fs::write(
                &conf,
                fs::read_to_string(&conf)
                    .unwrap()
                    .replace("Cousine", "Noto Sans"),
            )
            .unwrap();
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
    });
}
