use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::ascii::{AsciiRenderOptions, HeadlessAsciiRenderer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, sync::OnceLock};

const GROUP: &str = "ascii_end_to_end";

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputIdentity {
    bytes: usize,
    sha256: String,
}

fn fixtures() -> [(&'static str, &'static str); 5] {
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

fn render_fixture(renderer: &HeadlessAsciiRenderer, name: &str, input: &str) -> String {
    renderer
        .render_ascii_sync(input)
        .unwrap_or_else(|error| panic!("{GROUP}/{name} render failed: {error}"))
        .unwrap_or_else(|| panic!("{GROUP}/{name} returned no diagram"))
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
    renderer: &HeadlessAsciiRenderer,
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
    let renderer = HeadlessAsciiRenderer::new()
        .with_strict_parsing()
        .with_ascii_options(AsciiRenderOptions::ascii());
    let mut group = c.benchmark_group(GROUP);

    for (name, input) in fixtures() {
        if !benchmark_selected(name) {
            continue;
        }

        let preflight = output_identity(&render_fixture(&renderer, name, input));
        emit_preflight(name, &preflight);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let output = renderer
                    .render_ascii_sync(black_box(data))
                    .expect("ASCII end-to-end benchmark render")
                    .expect("ASCII end-to-end benchmark diagram");
                black_box(output);
            });
        });
        verify_postflight(&renderer, name, input, &preflight);
    }

    group.finish();
}

criterion_group!(benches, bench_ascii_end_to_end);
criterion_main!(benches);
