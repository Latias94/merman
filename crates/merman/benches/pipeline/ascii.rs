use super::identity::{benchmark_selected, emit_preflight, output_identity, verify_postflight};
use criterion::{BenchmarkId, Criterion};
use merman::ascii::{AsciiRenderOptions, AsciiResourcePolicy};
use merman::model::resources::{InputResourcePolicy, ResourceProfile};
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};
use std::hint::black_box;

const GROUP: &str = "ascii_end_to_end";

struct BenchmarkRenderer {
    renderer: Renderer,
    request: AsciiRequest,
}

impl BenchmarkRenderer {
    fn render(&self, name: &str, input: &str) -> String {
        let output = self
            .renderer
            .render(RenderRequest::ascii(
                input,
                OperationControl::new(),
                self.request.clone(),
            ))
            .unwrap_or_else(|error| panic!("{GROUP}/{name} render failed: {error}"));
        let RenderOutput::Ascii(Some(output)) = output else {
            panic!("{GROUP}/{name} returned no diagram");
        };
        output.into_text()
    }
}

fn fixtures() -> [(&'static str, &'static str); 10] {
    [
        (
            "flowchart_medium",
            include_str!("../fixtures/flowchart_medium.mmd"),
        ),
        (
            "sequence_medium",
            include_str!("../fixtures/sequence_medium.mmd"),
        ),
        ("class_medium", include_str!("../fixtures/class_medium.mmd")),
        ("er_medium", include_str!("../fixtures/er_medium.mmd")),
        (
            "xychart_medium",
            include_str!("../fixtures/xychart_medium.mmd"),
        ),
        (
            "flowchart_large",
            include_str!("../fixtures/flowchart_large.mmd"),
        ),
        (
            "sequence_mermaid_api_large",
            include_str!(
                "../../../../fixtures/sequence/upstream_docs_diagrams_mermaid_api_sequence.mmd"
            ),
        ),
        ("class_large", include_str!("../fixtures/class_large.mmd")),
        ("er_large", include_str!("../fixtures/er_large.mmd")),
        (
            "xychart_large",
            include_str!("../fixtures/xychart_large.mmd"),
        ),
    ]
}

pub(super) fn register(c: &mut Criterion) {
    let profile = ResourceProfile::UnboundedForTrustedInput;
    let renderer = BenchmarkRenderer {
        renderer: Renderer::new()
            .with_parse_options(merman::ParseOptions::strict())
            .with_resource_policy(InputResourcePolicy::for_profile(profile)),
        request: AsciiRequest {
            options: AsciiRenderOptions::ascii(),
            resources: AsciiResourcePolicy::for_profile(profile),
            viewport: Default::default(),
            native_theme: None,
        },
    };
    let mut group = c.benchmark_group(GROUP);

    for (name, input) in fixtures() {
        if !benchmark_selected(GROUP, name) {
            continue;
        }

        let identity =
            || output_identity("plain_ascii", renderer.render(name, input).as_bytes(), None);
        let preflight = identity();
        emit_preflight(GROUP, name, &preflight);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let output = renderer.render(name, black_box(data));
                black_box(output);
            });
        });
        verify_postflight(GROUP, name, &preflight, identity);
    }

    group.finish();
}
