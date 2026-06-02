#![cfg(feature = "render")]

use merman::render::HeadlessRenderer;
use std::path::{Path, PathBuf};

const SUPPORTED_FIXTURE_DIRS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "flowchart",
    "gantt",
    "gitgraph",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treemap",
    "xychart",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fixture_sample_paths() -> Vec<PathBuf> {
    let fixtures_root = workspace_root().join("fixtures");
    let mut out = Vec::new();

    for family in SUPPORTED_FIXTURE_DIRS {
        let dir = fixtures_root.join(family);
        let basic = dir.join("basic.mmd");
        if basic.exists() {
            out.push(basic);
        }

        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut candidates = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mmd"))
            .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("basic.mmd"))
            .collect::<Vec<_>>();
        candidates.sort();

        let mut picked_representatives = 0usize;
        for path in candidates {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.starts_with("zed_pr_57644_") {
                out.push(path);
                continue;
            }
            if picked_representatives < 3 && is_representative_fixture_name(name) {
                out.push(path);
                picked_representatives += 1;
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

fn is_representative_fixture_name(name: &str) -> bool {
    name.starts_with("stress_")
        || name.starts_with("kanban_stress_")
        || name.starts_with("upstream_docs_")
        || name.starts_with("upstream_cypress_")
        || name.starts_with("upstream_pkgtests_")
        || name.starts_with("upstream_examples_")
        || name.starts_with("upstream_")
}

fn render_resvg_safe(name: &str, source: &str) -> String {
    HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id(name)
        .render_svg_resvg_safe_sync(source)
        .unwrap_or_else(|err| panic!("{name}: headless resvg-safe render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn assert_resvg_safe_output(name: &str, svg: &str) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
    roxmltree::Document::parse(svg)
        .unwrap_or_else(|err| panic!("{name}: resvg-safe output should be XML-parseable: {err}"));

    assert!(
        !svg.contains("<foreignObject") && !svg.contains("</foreignObject>"),
        "{name}: resvg-safe output should not rely on foreignObject"
    );
    assert!(
        !svg.contains("@keyframes") && !svg.contains(":root"),
        "{name}: resvg-safe output should strip unsupported CSS constructs"
    );

    for bad in [
        "NaN",
        "Infinity",
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
        "transform:undefined",
    ] {
        assert!(
            !svg.contains(bad),
            "{name}: output should not leak invalid visual value {bad:?}"
        );
    }

    let mut cursor = 0;
    while let Some(rel_start) = svg[cursor..].find("<style") {
        let tag_start = cursor + rel_start;
        let Some(rel_tag_end) = svg[tag_start..].find('>') else {
            panic!("{name}: malformed style start tag");
        };
        let content_start = tag_start + rel_tag_end + 1;
        let Some(rel_close) = svg[content_start..].find("</style>") else {
            panic!("{name}: malformed style element");
        };
        let content_end = content_start + rel_close;
        assert!(
            !svg[content_start..content_end].trim().is_empty(),
            "{name}: resvg-safe output should not contain empty style elements"
        );
        cursor = content_end + "</style>".len();
    }

    assert_rasterizes_when_enabled(name, svg);
}

#[cfg(feature = "raster")]
fn assert_rasterizes_when_enabled(name: &str, svg: &str) {
    let png =
        merman::render::raster::svg_to_png(svg, &merman::render::raster::RasterOptions::default())
            .unwrap_or_else(|err| {
                panic!("{name}: resvg-safe output should rasterize to PNG: {err}")
            });

    assert!(
        png.starts_with(b"\x89PNG\r\n\x1a\n") && png.len() > 8,
        "{name}: expected non-empty PNG bytes from resvg-safe output"
    );
}

#[cfg(not(feature = "raster"))]
fn assert_rasterizes_when_enabled(_name: &str, _svg: &str) {
    // Raster validation runs when this test is executed with `--features raster`.
}

#[test]
fn host_reported_diagrams_render_headless_resvg_safe() {
    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "host-kanban-attrs",
            r#"kanban
    backlog[Backlog]
      api[Define FFI API]@{ assigned: "Core", priority: "High" }
      docs[Write README]@{ assigned: "Docs", priority: "Low" }
    progress[In Progress]
      flutter[Flutter packaging]@{ assigned: "Mobile", priority: "High" }
    done[Done]
      ci[CI matrix]@{ assigned: "Infra", priority: "Very Low" }
"#,
            &[
                "Backlog",
                "Define FFI API",
                "Core",
                "In Progress",
                "Flutter packaging",
                "CI matrix",
            ],
            &[],
        ),
        (
            "host-gitgraph-merge",
            r#"gitGraph
    commit
    commit
    branch develop
    checkout develop
    commit
    commit
    checkout main
    merge develop
    commit
    branch feature
    checkout feature
    commit
    checkout main
    merge feature
"#,
            &["main", "develop", "feature"],
            &[],
        ),
        (
            "host-dark-theme-flowchart",
            r##"%%{init: {"themeVariables": {"mainBkg": "#111827", "primaryTextColor": "#f8fafc", "nodeBorder": "#38bdf8", "lineColor": "#f59e0b", "edgeLabelBackground": "#0f172a", "nodeTextColor": "#f8fafc"}}}%%
flowchart TD
  A[Dark Node] -->|Readable Edge| B[Other]
"##,
            &["Dark Node", "Readable Edge", "Other"],
            &["#111827", "#f8fafc", "#38bdf8", "#f59e0b"],
        ),
    ];

    for (name, source, expected_labels, expected_colors) in cases {
        let svg = render_resvg_safe(name, source);
        assert_resvg_safe_output(name, &svg);

        for label in *expected_labels {
            assert!(
                svg.contains(label),
                "{name}: expected visible label {label:?}"
            );
        }
        for color in *expected_colors {
            assert!(
                svg.contains(color),
                "{name}: expected visible theme color {color:?}"
            );
        }
    }
}

#[test]
fn representative_fixtures_render_headless_resvg_safe() {
    let fixtures = fixture_sample_paths();
    assert!(
        fixtures.len() >= SUPPORTED_FIXTURE_DIRS.len(),
        "expected at least one representative fixture for each supported family"
    );

    for path in fixtures {
        let relative_name = path
            .strip_prefix(workspace_root())
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{relative_name}: read {}: {err}", path.display()));
        let diagram_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("fixture");
        let svg = render_resvg_safe(diagram_id, &source);
        assert_resvg_safe_output(&relative_name, &svg);
    }
}
