use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::ascii::{AsciiRenderOptions, AsciiResourcePolicy};
use merman::resources::{InputResourcePolicy, ResourceProfile};
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, sync::OnceLock};

const GROUP: &str = "ascii_end_to_end";

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputIdentity {
    bytes: usize,
    sha256: String,
}

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
            include_str!("fixtures/flowchart_medium.mmd"),
        ),
        (
            "sequence_medium",
            include_str!("fixtures/sequence_medium.mmd"),
        ),
        ("class_medium", include_str!("fixtures/class_medium.mmd")),
        ("er_medium", include_str!("fixtures/er_medium.mmd")),
        (
            "xychart_medium",
            include_str!("fixtures/xychart_medium.mmd"),
        ),
        (
            "flowchart_large",
            include_str!("fixtures/flowchart_large.mmd"),
        ),
        (
            "sequence_mermaid_api_large",
            include_str!(
                "../../../fixtures/sequence/upstream_docs_diagrams_mermaid_api_sequence.mmd"
            ),
        ),
        ("class_large", include_str!("fixtures/class_large.mmd")),
        ("er_large", include_str!("fixtures/er_large.mmd")),
        ("xychart_large", include_str!("fixtures/xychart_large.mmd")),
    ]
}

fn exact_benchmark() -> Option<&'static str> {
    static EXACT: OnceLock<Option<String>> = OnceLock::new();
    EXACT
        .get_or_init(|| {
            let arguments = std::env::args().collect::<Vec<_>>();
            arguments
                .windows(2)
                .find(|pair| pair[0] == "--exact")
                .map(|pair| pair[1].clone())
        })
        .as_deref()
}

fn fixture_name_from_exact(exact: &str) -> Option<&str> {
    exact.strip_prefix(GROUP)?.strip_prefix('/')
}

fn benchmark_selected(name: &str) -> bool {
    exact_benchmark().is_none_or(|exact| fixture_name_from_exact(exact) == Some(name))
}

fn render_fixture(renderer: &BenchmarkRenderer, name: &str, input: &str) -> String {
    renderer.render(name, input)
}

fn output_identity(output: &str) -> OutputIdentity {
    assert!(!output.is_empty(), "benchmark produced empty output");
    OutputIdentity {
        bytes: output.len(),
        sha256: format!("{:x}", Sha256::digest(output.as_bytes())),
    }
}

fn emit_preflight(name: &str, identity: &OutputIdentity) {
    let receipt = json!({
        "schema_version": 1,
        "benchmark": format!("{GROUP}/{name}"),
        "output_kind": "plain_ascii",
        "output_bytes": identity.bytes,
        "output_sha256": identity.sha256,
        "svg_elements": null,
    });
    eprintln!(
        "[bench][preflight] {}",
        serde_json::to_string(&receipt).expect("preflight receipt must serialize")
    );
}

fn verify_postflight(
    renderer: &BenchmarkRenderer,
    name: &str,
    input: &str,
    preflight: &OutputIdentity,
) {
    if exact_benchmark().and_then(fixture_name_from_exact) != Some(name) {
        return;
    }
    let postflight = output_identity(&render_fixture(renderer, name, input));
    assert_eq!(
        &postflight, preflight,
        "{GROUP}/{name} output identity changed after timed iterations"
    );
    eprintln!("[bench][postflight] {GROUP}/{name}");
}

fn bench_ascii_end_to_end(c: &mut Criterion) {
    let profile = ResourceProfile::UnboundedForTrustedInput;
    let renderer = BenchmarkRenderer {
        renderer: Renderer::new()
            .with_parse_options(merman::ParseOptions::strict())
            .with_resource_policy(InputResourcePolicy::for_profile(profile)),
        request: AsciiRequest {
            options: AsciiRenderOptions::ascii(),
            resources: AsciiResourcePolicy::for_profile(profile),
            viewport: Default::default(),
        },
    };
    let mut group = c.benchmark_group(GROUP);

    for (name, input) in fixtures() {
        if !benchmark_selected(name) {
            continue;
        }

        let preflight = output_identity(&render_fixture(&renderer, name, input));
        emit_preflight(name, &preflight);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let output = renderer.render(name, black_box(data));
                black_box(output);
            });
        });
        verify_postflight(&renderer, name, input, &preflight);
    }

    group.finish();
}

criterion_group!(benches, bench_ascii_end_to_end);
criterion_main!(benches);
