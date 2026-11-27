use std::fmt::Write as _;

use merman_core::MermaidEngine;
use merman_render::theme::{NativeTheme, ThemeCompiler};

use super::*;

#[test]
fn radar_native_series_wrap_independently_of_mermaid_color_slots() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                    "schema_version":2,"extends":"paper",
                    "roles":{"stroke.guide":"#567889","stroke.border":"#123456","text.muted":"#475869"},
                    "typography":{"sans":["Radar Body","sans-serif"],"base_size_px":19},
                    "form":{"chart":{"gridlines":true}},
                    "appearances":{"light":{"data":{"ink":["#15505D","#54216B","#713336","#433511"],"fill":["#CCEEDD","#EEDDEE","#EEDDDD","#EEEEDD"]}}}
                }"##,
            None,
        )
        .unwrap();
    let mut source = concat!(
        "---\nconfig:\n  themeVariables:\n    cScale0: '#aabbcc'\n---\n",
        "radar-beta\ntitle Coverage\naxis A[\"First\"], B[\"Second\"], C[\"Third\"]\n",
    )
    .to_owned();
    for index in 0..13 {
        writeln!(source, "curve series{index}[\"Series {index}\"]{{1,2,3}}").unwrap();
    }
    source.push_str("showLegend true\n");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let render = |session| {
        family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
    };
    let environment = RenderEnvironment::deterministic();
    let session = environment
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = render(session);
    let dom = roxmltree::Document::parse(rendered.svg()).unwrap();
    let css = dom
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(
        css.contains(".radarCurve-0{color:#15505D;fill:#CCEEDD;fill-opacity:0.28;stroke:#15505D")
    );
    assert!(
        css.contains(".radarCurve-12{color:#15505D;fill:#CCEEDD;fill-opacity:0.28;stroke:#15505D")
    );
    assert!(css.contains(".radarLegendBox-12{fill:#CCEEDD;"));
    assert!(css.contains(".radarAxisLine{stroke:#123456;"));
    assert!(css.contains(".radarAxisLabel{color:#475869;fill:#475869;}"));
    assert!(css.contains("\"Radar Body\", sans-serif"));
    assert!(
        dom.descendants()
            .any(|node| node.attribute("class") == Some("radarGraticule"))
    );

    let classic = render(environment.begin_session().unwrap());
    assert!(
        classic
            .svg()
            .contains(".radarCurve-0{color:#aabbcc;fill:#aabbcc;")
    );
}
