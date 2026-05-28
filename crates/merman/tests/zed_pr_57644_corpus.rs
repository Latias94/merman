#![cfg(feature = "render")]

use merman::render::HeadlessRenderer;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn zed_pr_57644_fixtures() -> Vec<PathBuf> {
    let fixtures_root = workspace_root().join("fixtures");
    let mut out = Vec::new();
    let mut stack = vec![fixtures_root];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("zed_pr_57644_") && name.ends_with(".mmd"))
            {
                out.push(path);
            }
        }
    }

    out.sort();
    out
}

#[test]
fn zed_pr_57644_corpus_renders_headless_resvg_safe() {
    let fixtures = zed_pr_57644_fixtures();
    assert!(!fixtures.is_empty(), "expected Zed PR #57644 fixtures");

    for fixture in fixtures {
        let name = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("<invalid fixture name>");
        let source = std::fs::read_to_string(&fixture)
            .unwrap_or_else(|err| panic!("{name}: read {}: {err}", fixture.display()));
        let svg = HeadlessRenderer::new()
            .with_vendored_text_measurer()
            .with_diagram_id(name)
            .render_svg_resvg_safe_sync(&source)
            .unwrap_or_else(|err| panic!("{name}: headless render failed: {err}"))
            .unwrap_or_else(|| panic!("{name}: no diagram detected"));

        assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
        assert!(
            !svg.contains("<foreignObject"),
            "{name}: resvg-safe output should not rely on foreignObject"
        );
        assert!(
            !svg.contains("@keyframes") && !svg.contains(":root"),
            "{name}: resvg-safe output should strip unsupported CSS constructs"
        );
        for bad in [
            "NaN",
            r#"fill="undefined""#,
            r#"stroke="undefined""#,
            r#"width="undefined""#,
            r#"height="undefined""#,
            r#"transform="undefined""#,
            r#"d="undefined""#,
            "fill:undefined",
            "stroke:undefined",
            "width:undefined",
            "height:undefined",
        ] {
            assert!(
                !svg.contains(bad),
                "{name}: output should not leak invalid visual value {bad:?}"
            );
        }
    }
}
