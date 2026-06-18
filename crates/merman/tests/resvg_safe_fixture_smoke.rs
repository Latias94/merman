#![cfg(feature = "render")]

use merman::MermaidConfig;
use merman::render::HeadlessRenderer;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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

const BOUNDARY_FIXTURE_DIRS: &[&str] = &["error", "info", "zenuml"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fixture_paths_for_dirs(family_dirs: &[&str]) -> Vec<PathBuf> {
    let fixtures_root = workspace_root().join("fixtures");
    let mut out = Vec::new();

    for family in family_dirs {
        let dir = fixtures_root.join(family);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        out.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mmd")),
        );
    }

    out.sort();
    out
}

fn fixture_site_config_overrides() -> &'static Map<String, Value> {
    static OVERRIDES: OnceLock<Map<String, Value>> = OnceLock::new();
    OVERRIDES.get_or_init(|| {
        let value: Value = serde_json::from_str(include_str!(
            "../../../fixtures/_config/site_config_overrides.json"
        ))
        .expect("valid fixture site config override manifest");
        match value {
            Value::Object(map) => map,
            other => {
                panic!("fixture site config override manifest must be a JSON object, got {other:?}")
            }
        }
    })
}

fn fixture_site_config_for_relative_name(relative_name: &str) -> Option<merman::MermaidConfig> {
    let key = relative_name
        .strip_prefix("fixtures/")
        .unwrap_or(relative_name);
    fixture_site_config_overrides()
        .get(key)
        .cloned()
        .map(merman::MermaidConfig::from_value)
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

fn all_supported_fixture_paths() -> Vec<PathBuf> {
    let fixtures_root = workspace_root().join("fixtures");
    let mut out = Vec::new();
    let family_filter = audit_family_filter();
    let name_filter = audit_name_filter();

    for family in SUPPORTED_FIXTURE_DIRS {
        if let Some(filter) = &family_filter
            && !filter.contains(*family)
        {
            continue;
        }

        let dir = fixtures_root.join(family);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        out.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mmd"))
                .filter(|path| {
                    let Some(filter) = &name_filter else {
                        return true;
                    };
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default();
                    name.contains(filter)
                }),
        );
    }

    out.sort();
    out
}

fn boundary_fixture_paths() -> Vec<PathBuf> {
    fixture_paths_for_dirs(BOUNDARY_FIXTURE_DIRS)
}

fn audit_family_filter() -> Option<BTreeSet<&'static str>> {
    let raw = std::env::var("MERMAN_RESVG_SAFE_AUDIT_FAMILY").ok()?;
    let requested = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if requested.is_empty() {
        return None;
    }

    Some(
        SUPPORTED_FIXTURE_DIRS
            .iter()
            .copied()
            .filter(|family| requested.contains(*family))
            .collect(),
    )
}

fn audit_name_filter() -> Option<String> {
    std::env::var("MERMAN_RESVG_SAFE_AUDIT_FILTER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
    render_resvg_safe_with_options(name, source, false, None)
}

fn trusted_host_dark_flowchart_config() -> MermaidConfig {
    MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "mainBkg": "#111827",
            "primaryTextColor": "#f8fafc",
            "nodeBorder": "#38bdf8",
            "lineColor": "#f59e0b",
            "edgeLabelBackground": "#0f172a",
            "nodeTextColor": "#f8fafc"
        }
    }))
}

fn render_resvg_safe_for_fixture(
    name: &str,
    source: &str,
    relative_name: &str,
    lenient: bool,
) -> String {
    render_resvg_safe_with_options(
        name,
        source,
        lenient,
        fixture_site_config_for_relative_name(relative_name),
    )
}

fn render_resvg_safe_with_options(
    name: &str,
    source: &str,
    lenient: bool,
    site_config: Option<merman::MermaidConfig>,
) -> String {
    let mut renderer = if lenient {
        HeadlessRenderer::new().with_lenient_parsing()
    } else {
        HeadlessRenderer::new().with_strict_parsing()
    };

    if let Some(site_config) = site_config {
        renderer = renderer.with_site_config(site_config);
    }

    renderer
        .with_vendored_text_measurer()
        .with_diagram_id(name)
        .render_svg_resvg_safe_sync(source)
        .unwrap_or_else(|err| panic!("{name}: headless resvg-safe render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn is_docs_placeholder_fixture(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "..." || trimmed == "... More Fields ..."
    })
}

fn is_known_unrenderable_fixture(relative_name: &str, source: &str) -> bool {
    if relative_name.contains("parser_only_spec") {
        return true;
    }

    if is_docs_placeholder_fixture(source) {
        return true;
    }

    matches!(
        relative_name,
        // Mermaid 11.15 parser tests classify this trailing-comma Radar example as invalid.
        "fixtures/radar/upstream_docs_radar_examples_005.mmd"
            // These Treemap classDef bare-token fixtures have `diagramType: "error"` goldens.
            // Strict public rendering should reject them; the all-supported renderability audit
            // skips them so it can keep testing contentful Treemap fixtures.
            | "fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db.mmd"
            | "fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db_parser_only_.mmd"
    )
}

fn assert_resvg_safe_output(name: &str, source: &str, svg: &str) {
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

    assert_rasterizes_when_enabled(name, source, svg);
}

fn assert_expected_labels_and_colors(
    name: &str,
    svg: &str,
    expected_labels: &[&str],
    expected_colors: &[&str],
) {
    for label in expected_labels {
        assert!(
            svg.contains(label),
            "{name}: expected visible label {label:?}"
        );
    }
    for color in expected_colors {
        assert!(
            svg.contains(color),
            "{name}: expected visible theme color {color:?}"
        );
    }
}

#[cfg(feature = "raster")]
fn assert_rasterizes_when_enabled(name: &str, source: &str, svg: &str) {
    let png =
        merman::render::raster::svg_to_png(svg, &merman::render::raster::RasterOptions::default())
            .unwrap_or_else(|err| {
                panic!("{name}: resvg-safe output should rasterize to PNG: {err}")
            });

    assert!(
        png.starts_with(b"\x89PNG\r\n\x1a\n") && png.len() > 8,
        "{name}: expected non-empty PNG bytes from resvg-safe output"
    );
    if source_has_visible_diagram_content(source) {
        assert_png_has_visible_non_background_ink(name, &png);
    }
}

#[cfg(feature = "raster")]
fn assert_png_has_visible_non_background_ink(name: &str, png_bytes: &[u8]) {
    let decoder = png::Decoder::new(png_bytes);
    let mut reader = decoder
        .read_info()
        .unwrap_or_else(|err| panic!("{name}: expected decodable PNG output: {err}"));
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .unwrap_or_else(|err| panic!("{name}: expected readable PNG frame: {err}"));

    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "{name}: expected RGBA PNG output"
    );
    assert_eq!(
        info.bit_depth,
        png::BitDepth::Eight,
        "{name}: expected 8-bit PNG output"
    );

    let pixels = &buf[..info.buffer_size()];
    let Some(background) = pixels.chunks_exact(4).next() else {
        panic!("{name}: expected at least one PNG pixel");
    };

    let differing_pixels = pixels
        .chunks_exact(4)
        .filter(|px| rgba_pixel_visibly_differs_from_background(px, background))
        .take(16)
        .count();
    assert!(
        differing_pixels >= 8,
        "{name}: rasterized PNG appears blank or all background-colored"
    );
}

#[cfg(feature = "raster")]
fn rgba_pixel_visibly_differs_from_background(pixel: &[u8], background: &[u8]) -> bool {
    let channel_delta = |i: usize| pixel[i].abs_diff(background[i]) as u16;
    let alpha_delta = channel_delta(3);
    let rgb_delta = channel_delta(0) + channel_delta(1) + channel_delta(2);
    alpha_delta > 3 || (pixel[3] > 0 && rgb_delta > 8)
}

#[cfg(not(feature = "raster"))]
fn assert_rasterizes_when_enabled(_name: &str, _source: &str, _svg: &str) {
    // Raster validation runs when this test is executed with `--features raster`.
}

fn source_has_visible_diagram_content(source: &str) -> bool {
    let mut in_frontmatter = false;
    let mut in_accessibility_block = false;
    let mut diagram_kind = SourceDiagramKind::Other;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        if in_accessibility_block {
            if trimmed.contains('}') {
                in_accessibility_block = false;
            }
            continue;
        }
        if trimmed == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter {
            continue;
        }
        if is_title_metadata(trimmed) {
            continue;
        }
        if skip_accessibility_metadata(trimmed, &mut in_accessibility_block) {
            continue;
        }
        if is_non_visual_directive_metadata(trimmed) {
            continue;
        }

        if let Some((kind, rest)) = strip_mermaid_header(trimmed) {
            diagram_kind = kind;
            let rest = rest.trim().trim_matches(';').trim();
            if is_title_metadata(rest) {
                continue;
            }
            if skip_accessibility_metadata(rest, &mut in_accessibility_block) {
                continue;
            }
            if is_non_visual_directive_metadata(rest) {
                continue;
            }
            if !rest.is_empty() {
                return true;
            }
            continue;
        }

        if diagram_kind == SourceDiagramKind::Journey && trimmed.starts_with("section ") {
            continue;
        }
        if diagram_kind == SourceDiagramKind::Radar && is_radar_option_line(trimmed) {
            continue;
        }
        if diagram_kind == SourceDiagramKind::State && is_state_non_visual_line(trimmed) {
            continue;
        }
        if diagram_kind == SourceDiagramKind::Treemap && !is_treemap_value_line(trimmed) {
            continue;
        }

        return true;
    }

    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceDiagramKind {
    Journey,
    Other,
    Radar,
    State,
    Treemap,
}

fn is_title_metadata(line: &str) -> bool {
    line.strip_prefix("title")
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
        || line.starts_with("title:")
}

fn skip_accessibility_metadata(line: &str, in_accessibility_block: &mut bool) -> bool {
    let Some(rest) = line
        .strip_prefix("accTitle")
        .or_else(|| line.strip_prefix("accDescr"))
    else {
        return false;
    };

    let rest = rest.trim_start();
    if rest.starts_with(':') {
        return true;
    }
    if rest.starts_with('{') {
        *in_accessibility_block = !rest.contains('}');
        return true;
    }
    false
}

fn is_non_visual_directive_metadata(line: &str) -> bool {
    is_class_def_metadata(line) || is_click_metadata(line) || is_link_style_metadata(line)
}

fn is_class_def_metadata(line: &str) -> bool {
    line.strip_prefix("classDef")
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
}

fn is_click_metadata(line: &str) -> bool {
    line.strip_prefix("click")
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
}

fn is_link_style_metadata(line: &str) -> bool {
    line.strip_prefix("linkStyle")
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
}

fn strip_mermaid_header(line: &str) -> Option<(SourceDiagramKind, &str)> {
    strip_flowchart_header(line, "flowchart")
        .or_else(|| strip_flowchart_header(line, "graph"))
        .or_else(|| strip_plain_header(line, "architecture-beta"))
        .or_else(|| strip_plain_header(line, "block"))
        .or_else(|| strip_plain_header(line, "C4Component"))
        .or_else(|| strip_plain_header(line, "C4Container"))
        .or_else(|| strip_plain_header(line, "C4Context"))
        .or_else(|| strip_plain_header(line, "classDiagram"))
        .or_else(|| strip_plain_header(line, "erDiagram"))
        .or_else(|| strip_plain_header(line, "gitGraph"))
        .or_else(|| strip_plain_header_kind(line, "journey", SourceDiagramKind::Journey))
        .or_else(|| strip_plain_header(line, "kanban"))
        .or_else(|| strip_plain_header(line, "mindmap"))
        .or_else(|| strip_plain_header(line, "packet"))
        .or_else(|| strip_plain_header(line, "packet-beta"))
        .or_else(|| strip_plain_header(line, "pie"))
        .or_else(|| strip_plain_header(line, "quadrantChart"))
        .or_else(|| strip_plain_header_kind(line, "radar", SourceDiagramKind::Radar))
        .or_else(|| strip_plain_header_kind(line, "radar-beta", SourceDiagramKind::Radar))
        .or_else(|| strip_plain_header(line, "requirementDiagram"))
        .or_else(|| strip_plain_header(line, "sankey"))
        .or_else(|| strip_plain_header(line, "sequenceDiagram"))
        .or_else(|| strip_plain_header_kind(line, "stateDiagram", SourceDiagramKind::State))
        .or_else(|| strip_plain_header_kind(line, "stateDiagram-v2", SourceDiagramKind::State))
        .or_else(|| strip_plain_header(line, "timeline"))
        .or_else(|| strip_plain_header_kind(line, "treemap", SourceDiagramKind::Treemap))
        .or_else(|| strip_plain_header_kind(line, "treemap-beta", SourceDiagramKind::Treemap))
        .or_else(|| strip_plain_header(line, "xychart"))
        .or_else(|| strip_plain_header(line, "xychart-beta"))
}

fn strip_plain_header<'a>(line: &'a str, header: &str) -> Option<(SourceDiagramKind, &'a str)> {
    strip_plain_header_kind(line, header, SourceDiagramKind::Other)
}

fn strip_plain_header_kind<'a>(
    line: &'a str,
    header: &str,
    kind: SourceDiagramKind,
) -> Option<(SourceDiagramKind, &'a str)> {
    let rest = line.strip_prefix(header)?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace() && ch != ';')
    {
        return None;
    }
    Some((kind, rest))
}

fn strip_flowchart_header<'a>(line: &'a str, header: &str) -> Option<(SourceDiagramKind, &'a str)> {
    let (_, rest) = strip_plain_header(line, header)?;
    Some((
        SourceDiagramKind::Other,
        strip_flowchart_direction(rest.trim_start()),
    ))
}

fn strip_flowchart_direction(rest: &str) -> &str {
    for direction in ["TB", "TD", "BT", "RL", "LR"] {
        if rest.eq_ignore_ascii_case(direction) {
            return "";
        }
        if rest
            .get(..direction.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(direction))
            && rest[direction.len()..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || ch == ';')
        {
            return rest[direction.len()..].trim_start();
        }
    }
    rest
}

fn is_radar_option_line(line: &str) -> bool {
    ["ticks", "showLegend", "graticule", "min", "max"]
        .iter()
        .any(|keyword| {
            line.strip_prefix(keyword)
                .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
        })
}

fn is_state_non_visual_line(line: &str) -> bool {
    is_state_bare_keyword_line(line) || is_state_floating_note_alias_line(line)
}

fn is_state_bare_keyword_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("state") else {
        return false;
    };
    if !rest.chars().next().is_some_and(char::is_whitespace) {
        return false;
    }
    let rest = rest.trim().trim_end_matches(';').trim();
    !rest.is_empty()
        && rest.split_whitespace().count() == 1
        && !rest.contains('{')
        && !rest.contains(':')
        && !rest.contains("<<")
}

fn is_state_floating_note_alias_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("note") else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with('"') && rest.contains(" as ")
}

fn is_treemap_value_line(line: &str) -> bool {
    if is_class_def_metadata(line) || line.starts_with("class ") {
        return false;
    }

    for (idx, _) in line.match_indices(':') {
        let after = line[idx + 1..].trim_start();
        if after.starts_with(':') {
            continue;
        }
        if after
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '-' || ch == '.')
        {
            return true;
        }
    }

    false
}

#[test]
fn default_svg_and_resvg_safe_svg_keep_separate_contracts() {
    let source = r#"flowchart TD
  A[Start] --> B{Is it working?}
  B -->|Yes| C[Ship it]
  B -->|No| D[Debug]
"#;
    let renderer = HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("export-contract");

    let parity_svg = renderer
        .render_svg_sync(source)
        .expect("parity render should succeed")
        .expect("diagram should be detected");
    assert!(
        parity_svg.contains("<foreignObject"),
        "parity SVG should preserve Mermaid HTML label DOM: {parity_svg}"
    );
    for label in ["Start", "Ship it"] {
        assert!(
            parity_svg.contains(label),
            "parity SVG should keep visible label {label:?}: {parity_svg}"
        );
    }

    let export_svg = renderer
        .render_svg_resvg_safe_sync(source)
        .expect("resvg-safe render should succeed")
        .expect("diagram should be detected");
    assert_resvg_safe_output("export-contract", source, &export_svg);
    assert!(
        export_svg.contains(r#"data-merman-foreignobject="fallback""#),
        "resvg-safe SVG should keep generated text fallbacks: {export_svg}"
    );
    for label in ["Start", "Is it working?", "Yes", "Ship it", "No", "Debug"] {
        assert!(
            export_svg.contains(label),
            "resvg-safe SVG should keep visible label {label:?}: {export_svg}"
        );
    }
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
    ];

    for (name, source, expected_labels, expected_colors) in cases {
        let svg = render_resvg_safe(name, source);
        assert_resvg_safe_output(name, source, &svg);
        assert_expected_labels_and_colors(name, &svg, expected_labels, expected_colors);
    }

    let name = "host-dark-theme-flowchart";
    let source = r##"flowchart TD
  A[Dark Node] -->|Readable Edge| B[Other]
"##;
    let svg = render_resvg_safe_with_options(
        name,
        source,
        false,
        Some(trusted_host_dark_flowchart_config()),
    );
    assert_resvg_safe_output(name, source, &svg);
    assert_expected_labels_and_colors(
        name,
        &svg,
        &["Dark Node", "Readable Edge", "Other"],
        &["#111827", "#f8fafc", "#f59e0b", "#38bdf8"],
    );
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
        let svg = render_resvg_safe_for_fixture(diagram_id, &source, &relative_name, false);
        assert_resvg_safe_output(&relative_name, &source, &svg);
    }
}

#[test]
fn boundary_fixtures_render_headless_resvg_safe() {
    let fixtures = boundary_fixture_paths();
    assert!(
        fixtures.len() >= 30,
        "expected boundary fixtures for error/info/zenuml renderability"
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
        let lenient_error_entrypoint = relative_name.starts_with("fixtures/error/");
        let svg = if lenient_error_entrypoint {
            render_resvg_safe_for_fixture(diagram_id, &source, &relative_name, true)
        } else {
            render_resvg_safe_for_fixture(diagram_id, &source, &relative_name, false)
        };

        // The `error` corpus includes suppressed parse-error fixtures whose source may be
        // parser-only, but the host-visible error diagram must still rasterize with ink.
        let ink_source = if lenient_error_entrypoint {
            "error\n"
        } else {
            source.as_str()
        };
        assert_resvg_safe_output(&relative_name, ink_source, &svg);
    }
}

#[test]
#[ignore = "manual HPD-080 audit over all supported fixtures; default smoke stays representative"]
fn all_supported_fixtures_render_headless_resvg_safe_audit() {
    let fixtures = all_supported_fixture_paths();
    let filtered_audit =
        audit_family_filter().is_some() || audit_name_filter().as_deref().is_some();
    assert!(
        fixtures.len() > 100 || (filtered_audit && !fixtures.is_empty()),
        "expected a broad supported fixture corpus, or a non-empty filtered audit"
    );

    let mut rendered = 0usize;
    let mut skipped_unrenderable = 0usize;

    for path in fixtures {
        let relative_name = path
            .strip_prefix(workspace_root())
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{relative_name}: read {}: {err}", path.display()));
        if is_known_unrenderable_fixture(&relative_name, &source) {
            skipped_unrenderable += 1;
            continue;
        }
        let diagram_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("fixture");
        let svg = render_resvg_safe_for_fixture(diagram_id, &source, &relative_name, false);
        assert_resvg_safe_output(&relative_name, &source, &svg);
        rendered += 1;
    }

    assert!(
        rendered > 100 || (filtered_audit && rendered > 0),
        "expected the manual audit to render a broad corpus; rendered={rendered}, skipped_unrenderable={skipped_unrenderable}"
    );
}

#[test]
fn known_error_golden_fixtures_are_skipped_by_manual_audit() {
    assert!(is_known_unrenderable_fixture(
        "fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db.mmd",
        "treemap\nclassDef c fill:#ff0000, stroke:rgb(1\\,2\\,3), color;\n"
    ));
    assert!(is_known_unrenderable_fixture(
        "fixtures/treemap/upstream_treemap_classdef_and_css_compiled_styles_db_parser_only_.mmd",
        "treemap\nclassDef c fill:#ff0000, stroke:rgb(1\\,2\\,3), color;\n"
    ));
}

#[test]
fn source_content_gate_distinguishes_accessibility_only_from_visible_content() {
    assert!(!source_has_visible_diagram_content(
        "architecture-beta\naccTitle: Accessibility Title\naccDescr: Accessibility Description\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "architecture-beta\naccDescr {\n    Accessibility Description\n}\n"
    ));
    assert!(!source_has_visible_diagram_content("packet\n"));
    assert!(!source_has_visible_diagram_content("packet-beta\n"));
    assert!(!source_has_visible_diagram_content(
        "graph LR\n      click X callback \"X\";\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "stateDiagram-v2\n classDef exampleClass background:#bbb;\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "stateDiagram-v2\n state foo\n note \"This is a floating note\" as N1\n"
    ));
    assert!(source_has_visible_diagram_content(
        "stateDiagram-v2\n state \"Long state description\" as S1\n"
    ));
    assert!(source_has_visible_diagram_content(
        "stateDiagram-v2\n state fork_state <<fork>>\n"
    ));
    assert!(source_has_visible_diagram_content(
        "stateDiagram-v2\n foo: bar\n note \"This is a floating note\" as N1\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "pie accDescr {\n    Accessibility Description\n}\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "architecture-beta title Simple Architecture Diagram\n"
    ));
    assert!(source_has_visible_diagram_content("graph TD;a-X-node;\n"));
    assert!(source_has_visible_diagram_content(
        "flowchart LR\n  A[Alpha] --> B[Beta]\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "journey\naccTitle: The title\nsection Order from website\n"
    ));
    assert!(source_has_visible_diagram_content(
        "journey\nsection Order from website\n  Add to cart: 5: Me\n"
    ));
    assert!(!source_has_visible_diagram_content(
        "radar-beta\n  ticks 10\n  showLegend false\n  graticule polygon\n  min 1\n  max 10\n"
    ));
    assert!(source_has_visible_diagram_content(
        "radar-beta\n  axis A,B,C\n  curve mycurve{1,2,3}\n"
    ));
    assert!(!source_has_visible_diagram_content("treemap\n\"Root\"\n"));
    assert!(!source_has_visible_diagram_content(
        "treemap\nclassDef myClass fill:red;\n"
    ));
    assert!(source_has_visible_diagram_content(
        "treemap\n\"Root\"\n  \"Leaf\": 100:::leafClass\n"
    ));
}
