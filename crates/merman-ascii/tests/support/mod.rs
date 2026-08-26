use merman_ascii::{
    AsciiOutput, AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy, AsciiViewportPolicy,
    TerminalWidthProfile,
};
use merman_core::diagram::{ParsedDiagramRender, RenderSemanticModel};
use merman_core::{Engine, OperationControl, ParseOptions, runtime::OperationContext};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

/// Return the local-semantic fixture contents used by characterization tests.
#[allow(dead_code)]
pub(crate) fn local_semantic_input(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

/// Measure the rendered text in terminal display cells and logical rows.
#[allow(dead_code)]
pub(crate) fn terminal_extent(rendered: &str) -> (usize, usize) {
    terminal_extent_with_profile(rendered, TerminalWidthProfile::Unicode)
}

/// Measure rendered text with the selected terminal display-width convention.
#[allow(dead_code)]
pub(crate) fn terminal_extent_with_profile(
    rendered: &str,
    profile: TerminalWidthProfile,
) -> (usize, usize) {
    (
        rendered
            .lines()
            .map(|line| terminal_line_width(line, profile))
            .max()
            .unwrap_or_default(),
        rendered.lines().count(),
    )
}

/// Assert that every emitted row occupies the same number of terminal cells.
#[allow(dead_code)]
pub(crate) fn assert_rectangular_terminal_grid(rendered: &str) {
    assert_rectangular_terminal_grid_with_profile(rendered, TerminalWidthProfile::Unicode);
}

/// Assert rectangularity with the selected terminal display-width convention.
#[allow(dead_code)]
pub(crate) fn assert_rectangular_terminal_grid_with_profile(
    rendered: &str,
    profile: TerminalWidthProfile,
) {
    let mut lines = rendered.lines();
    let Some(first) = lines.next() else {
        return;
    };
    let width = terminal_line_width(first, profile);
    for line in lines {
        assert_eq!(
            terminal_line_width(line, profile),
            width,
            "rendered lines should stay terminal-cell aligned:\n{rendered}"
        );
    }
}

#[allow(dead_code)]
fn terminal_line_width(line: &str, profile: TerminalWidthProfile) -> usize {
    match profile {
        TerminalWidthProfile::Unicode => UnicodeWidthStr::width(line),
        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
        _ => UnicodeWidthStr::width(line),
    }
}

pub(crate) fn parse_model(source: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected")
        .into_parts()
        .1
}

#[allow(dead_code)]
pub(crate) fn render_model(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    render_model_with_resources(model, options, AsciiResourcePolicy::default())
}

pub(crate) fn render_model_with_resources(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    render_controlled_model(
        model,
        options,
        &OperationControl::new(),
        &context,
        resources,
    )
}

#[allow(dead_code)]
pub(crate) fn render_model_report(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    AsciiRenderer::new(*options)?.render_model_report(
        model,
        viewport,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

#[allow(dead_code)]
pub(crate) fn render_parsed(
    parsed: &ParsedDiagramRender,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    AsciiRenderer::new(*options)?.render_parsed(
        parsed,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

#[allow(dead_code)]
pub(crate) fn render_source(source: &str) -> String {
    render_model(&parse_model(source), &AsciiRenderOptions::ascii()).expect("diagram should render")
}

pub(crate) fn render_controlled_model(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    control: &OperationControl,
    context: &OperationContext,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    AsciiRenderer::new(*options)?.render_model(model, control, context, resources)
}
