#![cfg(any(
    feature = "diagram-er",
    feature = "diagram-requirement",
    all(feature = "diagram-architecture", feature = "layout-cytoscape"),
    feature = "diagram-state",
    feature = "diagram-treemap",
    feature = "diagram-sankey",
))]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, ThemePortabilityRequirement, ThemePreset,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const ID: &str = "native-structural";
// Canvas, surface, surface-alt, border, line, accent. Ghibli distinguishes every role;
// Cyberpunk also exercises the dark preset's linked neon colors.
#[cfg(any(
    feature = "diagram-er",
    feature = "diagram-requirement",
    all(feature = "diagram-architecture", feature = "layout-cytoscape"),
    feature = "diagram-state",
))]
const PRESETS: [(ThemePreset, [&str; 6]); 2] = [
    (
        ThemePreset::Cyberpunk,
        [
            "#0B1020", "#151E35", "#202B46", "#53DCE4", "#53DCE4", "#F28BCB",
        ],
    ),
    (
        ThemePreset::Ghibli,
        [
            "#FAF5E8", "#EEF3E3", "#F3EFDF", "#78917B", "#637261", "#B79B65",
        ],
    ),
];

fn render(source: &str, theme: &DiagramTheme, config: Value) -> String {
    let engine = common::install_parse_compatibility(
        theme,
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config)),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
        .begin_session_with_theme(theme)
        .unwrap();
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some(ID.into()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
    .unwrap()
    .svg()
    .to_owned()
}

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn declaration<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').rev().find_map(|entry| {
        let (name, value) = entry.split_once(':')?;
        (name.trim() == property).then(|| value.trim().trim_end_matches("!important").trim())
    })
}

// These callers inspect inline paint on live terminals, with explicit stylesheet checks
// below where the family paints through CSS. This does not approximate a CSS cascade.
fn local_paint<'a>(node: Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")
        .and_then(|style| declaration(style, property))
        .or_else(|| node.attribute(property))
}

#[cfg(any(
    feature = "diagram-requirement",
    all(feature = "diagram-architecture", feature = "layout-cytoscape"),
))]
fn css(document: &Document<'_>, selector: &str, property: &str) -> Option<String> {
    let selector = format!("#{ID} {selector}");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| selectors.split(',').any(|value| value.trim() == selector))
        .filter_map(|(_, body)| declaration(body, property))
        .next_back()
        .map(str::to_owned)
}

#[cfg(any(feature = "diagram-requirement", feature = "diagram-state"))]
fn assert_split_paint(group: Node<'_, '_>, fill: &str, stroke: &str) {
    let paths = group
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2, "one fill and one contour for {group:?}");
    assert!(
        paths
            .iter()
            .all(|path| path.attribute("d").is_some_and(|d| !d.is_empty()))
    );
    assert_eq!(paths[0].attribute("stroke"), Some("none"));
    assert_eq!(paths[1].attribute("fill"), Some("none"));
    assert_eq!(
        local_paint(paths[0], "fill"),
        Some(fill),
        "fill of {group:?}"
    );
    assert_eq!(
        local_paint(paths[1], "stroke"),
        Some(stroke),
        "contour of {group:?}"
    );
}

#[cfg(feature = "diagram-er")]
mod er {
    use super::*;

    const SOURCE: &str = "erDiagram\nRECORD {\n string id\n string name\n string status\n}\n";

    fn assert_rows(document: &Document<'_>, odd: &str, even: &str) {
        for (class, count, expected) in [("row-rect-odd", 2, odd), ("row-rect-even", 1, even)] {
            let rows = document
                .descendants()
                .filter(|node| has_class(*node, class))
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), count, "live {class} rows");
            for row in rows {
                let fill = row
                    .children()
                    .find(|node| {
                        node.has_tag_name("path") && node.attribute("stroke") == Some("none")
                    })
                    .expect("live row fill path");
                assert!(fill.attribute("d").is_some_and(|d| !d.is_empty()));
                assert_eq!(local_paint(fill, "fill"), Some(expected), "{class}");
            }
        }
    }

    #[test]
    fn native_rows_use_surface_and_surface_alt_in_html_and_svg_modes() {
        for (preset, [_, surface, alternate, _, _, _]) in PRESETS {
            let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            for html in [false, true] {
                let svg = render(
                    SOURCE,
                    &theme,
                    json!({"layout":"dagre", "look":"classic", "htmlLabels":html}),
                );
                let document = Document::parse(&svg).unwrap();
                assert_eq!(
                    document
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html
                );
                assert!(
                    document
                        .descendants()
                        .any(|node| node.has_tag_name(if html { "span" } else { "text" }))
                );
                assert_rows(&document, surface, alternate);
            }
        }
    }

    #[test]
    fn native_row_overrides_keep_config_owners_local() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Ghibli)
            .unwrap();
        for html in [false, true] {
            for (source, variables, odd, even) in [
                (SOURCE, json!({"rowOdd":"#123456"}), "#123456", "#F3EFDF"),
                (SOURCE, json!({"rowEven":"#123456"}), "#EEF3E3", "#123456"),
            ] {
                let svg = render(
                    source,
                    &theme,
                    json!({"layout":"dagre", "look":"classic", "htmlLabels":html, "themeVariables":variables}),
                );
                assert_rows(&Document::parse(&svg).unwrap(), odd, even);
            }
        }
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn native_requirement_borders_and_referenced_relation_markers_keep_separate_roles() {
    const SOURCE: &str = "requirementDiagram\nrequirement req {\nid: R-1\ntext: Required behavior\n}\nelement impl {\ntype: simulation\n}\nreq - satisfies -> impl\n";
    for (preset, [_, surface, _, border, line, _]) in PRESETS {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        for (variables, body_stroke, relation) in [
            (json!({}), border, line),
            (json!({"nodeBorder":"#123456"}), "#123456", line),
            (json!({"relationColor":"#654321"}), border, "#654321"),
        ] {
            let svg = render(
                SOURCE,
                &theme,
                json!({"layout":"dagre", "look":"classic", "themeVariables":variables}),
            );
            let document = Document::parse(&svg).unwrap();
            let bodies = document
                .descendants()
                .filter(|node| node.has_tag_name("g") && has_class(*node, "outer-path"))
                .collect::<Vec<_>>();
            assert_eq!(bodies.len(), 2);
            for body in bodies {
                assert_split_paint(body, surface, body_stroke);
            }

            let edge = document
                .descendants()
                .find(|node| node.has_tag_name("path") && has_class(*node, "relationshipLine"))
                .expect("live requirement relation");
            assert!(edge.attribute("d").is_some_and(|d| !d.is_empty()));
            let edge_css = css(&document, ".relationshipLine", "stroke").unwrap();
            assert_eq!(local_paint(edge, "stroke").unwrap_or(&edge_css), relation);
            let marker_id = edge
                .attribute("marker-end")
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
                .expect("the arrow marker actually referenced by the relation");
            let arrow = marker
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert!(arrow.attribute("d").is_some_and(|d| !d.is_empty()));
            let marker_css = css(&document, "marker", "stroke").unwrap();
            assert_eq!(
                local_paint(arrow, "stroke")
                    .or_else(|| local_paint(marker, "stroke"))
                    .unwrap_or(&marker_css),
                relation
            );
        }
    }
}

#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
#[test]
fn native_architecture_paints_the_standalone_arrow_and_respects_its_local_owner() {
    const SOURCE: &str = "architecture-beta\nservice api(server)[API]\nservice db(database)[Database]\napi:R --> L:db\n";
    for (preset, [_, _, _, _, line, _]) in PRESETS {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        for (variables, edge_stroke, arrow_fill) in [
            (json!({}), line, line),
            (json!({"archEdgeArrowColor":"#123456"}), line, "#123456"),
            (json!({"archEdgeColor":"#654321"}), "#654321", line),
        ] {
            let svg = render(
                SOURCE,
                &theme,
                json!({"architecture":{"numIter":1,"randomize":false}, "themeVariables":variables}),
            );
            let document = Document::parse(&svg).unwrap();
            let edge = document
                .descendants()
                .find(|node| node.has_tag_name("path") && has_class(*node, "edge"))
                .expect("live architecture edge");
            let arrows = edge
                .parent()
                .unwrap()
                .children()
                .filter(|node| node.has_tag_name("polygon") && has_class(*node, "arrow"))
                .collect::<Vec<_>>();
            assert_eq!(arrows.len(), 1, "standalone arrow beside its edge");
            assert!(
                arrows[0]
                    .attribute("points")
                    .is_some_and(|points| !points.is_empty())
            );
            let edge_css = css(&document, ".edge", "stroke").unwrap();
            let arrow_css = css(&document, ".arrow", "fill").unwrap();
            assert_eq!(
                local_paint(edge, "stroke").unwrap_or(&edge_css),
                edge_stroke
            );
            assert_eq!(
                local_paint(arrows[0], "fill").unwrap_or(&arrow_css),
                arrow_fill
            );
        }
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn native_state_start_end_choice_and_fork_paint_each_actual_surface() {
    const SOURCE: &str = "stateDiagram-v2\n[*] --> Ready\nstate Decide <<choice>>\nstate Fork <<fork>>\nReady --> Decide\nDecide --> Fork\nFork --> [*]\n";
    for (preset, [canvas, _, _, _, line, accent]) in PRESETS {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        for look in ["classic", "neo"] {
            let svg = render(SOURCE, &theme, json!({"layout":"dagre", "look":look}));
            let document = Document::parse(&svg).unwrap();
            let start = document
                .descendants()
                .find(|node| node.has_tag_name("circle") && has_class(*node, "state-start"))
                .expect("live start-state circle");
            for property in ["fill", "stroke"] {
                assert_eq!(
                    start
                        .attribute("style")
                        .and_then(|style| declaration(style, property)),
                    Some(line),
                    "{preset:?}/{look} start {property}"
                );
            }
            let outer = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && has_class(*node, "outer-path")
                        && node.children().any(|child| child.has_tag_name("g"))
                })
                .expect("live end-state outer surface");
            assert_split_paint(outer, canvas, line);
            let inner = outer
                .children()
                .find(|node| node.has_tag_name("g"))
                .unwrap();
            assert_split_paint(inner, line, line);
            for name in ["Decide", "Fork"] {
                let fragment = format!("-state-{name}-");
                let node = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && has_class(*node, "node")
                            && node
                                .attribute("id")
                                .is_some_and(|id| id.contains(&fragment))
                    })
                    .unwrap_or_else(|| panic!("live {name} node"));
                assert_split_paint(
                    node.children()
                        .find(|child| child.has_tag_name("g"))
                        .unwrap(),
                    accent,
                    line,
                );
            }
        }
    }
}

#[cfg(feature = "diagram-treemap")]
mod treemap {
    use super::*;

    fn rgb(css: &str) -> [f64; 3] {
        let channels = merman_markup::theme_color::ThemeColor::parse(css)
            .unwrap()
            .rgba_channels();
        assert_eq!(channels.alpha, 1.0);
        [channels.red, channels.green, channels.blue]
    }

    fn over(node: Node<'_, '_>, background: [f64; 3]) -> [f64; 3] {
        let fill = rgb(local_paint(node, "fill").unwrap());
        let alpha = local_paint(node, "fill-opacity")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        std::array::from_fn(|i| fill[i] * alpha + background[i] * (1.0 - alpha))
    }

    fn assert_label(group: Node<'_, '_>, class: &str, ink: &str, background: [f64; 3]) {
        let label = group
            .children()
            .find(|node| node.has_tag_name("text") && has_class(*node, class))
            .expect("live category label");
        assert!(label.text().is_some_and(|text| !text.is_empty()));
        assert_ne!(
            label
                .attribute("style")
                .and_then(|style| declaration(style, "display")),
            Some("none")
        );
        let foreground = rgb(local_paint(label, "fill").unwrap());
        assert_eq!(foreground, rgb(ink), "category-text reaches {class}");
        let luminance = |rgb: [f64; 3]| {
            let [r, g, b] = rgb.map(|channel| {
                let channel = channel / 255.0;
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            });
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        let (fg, bg) = (luminance(foreground), luminance(background));
        let ratio = (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
        assert!(
            ratio >= 4.5,
            "{class}: {ratio:.2}:1 on the composited category surface"
        );
    }

    #[test]
    fn native_categories_keep_distinct_section_and_leaf_fills_with_readable_labels() {
        const SOURCE: &str =
            "treemap\n\"Section A\"\n  \"Leaf A\": 42\n\"Section B\"\n  \"Leaf B\": 24\n";
        for (preset, fills, ink) in [
            (ThemePreset::Cyberpunk, ["#67e8f9", "#f9a8d4"], "#000000"),
            (ThemePreset::Ghibli, ["#9bc5a1", "#efbc88"], "#314A3E"),
        ] {
            let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            let svg = render(SOURCE, &theme, json!({}));
            let document = Document::parse(&svg).unwrap();
            let canvas = document
                .descendants()
                .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
                .unwrap();
            let canvas = rgb(local_paint(canvas, "fill").unwrap());
            for (suffix, fill) in [("A", fills[0]), ("B", fills[1])] {
                let section_name = format!("Section {suffix}");
                let leaf_name = format!("Leaf {suffix}");
                let group = |class: &str, text: &str| {
                    document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text")
                                && has_class(*node, class)
                                && node.text() == Some(text)
                        })
                        .unwrap()
                        .parent()
                        .unwrap()
                };
                let section = group("treemapSectionLabel", &section_name);
                let section_rect = section
                    .children()
                    .find(|node| node.has_tag_name("rect") && has_class(*node, "treemapSection"))
                    .unwrap();
                assert_eq!(local_paint(section_rect, "fill"), Some(fill));
                let section_background = over(section_rect, canvas);
                assert_label(section, "treemapSectionLabel", ink, section_background);
                let leaf = group("treemapLabel", &leaf_name);
                let leaf_rect = leaf
                    .children()
                    .find(|node| node.has_tag_name("rect") && has_class(*node, "treemapLeaf"))
                    .unwrap();
                assert_eq!(local_paint(leaf_rect, "fill"), Some(fill));
                assert_label(
                    leaf,
                    "treemapLabel",
                    ink,
                    over(leaf_rect, section_background),
                );
            }
        }
    }
}

#[cfg(feature = "diagram-sankey")]
#[test]
fn ghibli_sankey_marks_use_all_six_chart_inks_with_canvas_contrast() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Ghibli)
        .unwrap();
    let svg = render(
        "sankey\nA,B,10\nB,C,8\nC,D,6\nD,E,4\nE,F,2\n",
        &theme,
        json!({}),
    );
    let document = Document::parse(&svg).unwrap();
    let canvas = document
        .descendants()
        .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
        .unwrap();
    let luminance = |css: &str| {
        let color = merman_markup::theme_color::ThemeColor::parse(css)
            .unwrap()
            .rgba_channels();
        assert_eq!(color.alpha, 1.0);
        let [r, g, b] = [color.red, color.green, color.blue].map(|channel| {
            let channel = channel / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let background = luminance(local_paint(canvas, "fill").unwrap());
    let marks = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && has_class(*node, "node"))
        .flat_map(|node| node.children().filter(|child| child.has_tag_name("rect")))
        .collect::<Vec<_>>();
    assert_eq!(marks.len(), 6);
    for (mark, expected) in marks.into_iter().zip([
        "#556F50", "#3E7183", "#97603F", "#825C7A", "#876E24", "#4E6C69",
    ]) {
        assert!(
            ["width", "height"]
                .into_iter()
                .all(|name| { mark.attribute(name).unwrap().parse::<f64>().unwrap() > 0.0 })
        );
        let fill = local_paint(mark, "fill").unwrap();
        assert_eq!(fill, expected);
        let ink = luminance(fill);
        let ratio = (ink.max(background) + 0.05) / (ink.min(background) + 0.05);
        assert!(
            ratio >= 3.0,
            "Sankey mark {fill}: {ratio:.2}:1 against its canvas"
        );
    }
}
