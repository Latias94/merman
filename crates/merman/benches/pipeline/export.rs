use criterion::{Criterion, SamplingMode, Throughput};
use merman::svg::SvgPipeline;
use merman::svg::export::NativeFontOptions;
use merman::theme::{BuiltinPreset, ThemeCompiler, ThemeSelection};
use merman::{
    MermaidEngine, OperationControl, ParseOptions, PdfRequest, PngRequest, RenderOutput,
    RenderRequest, RenderTarget, Renderer, SourcePresentationConfig, SvgEnvironment, SvgRequest,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    hint::black_box,
    io::Cursor,
    path::{Path, PathBuf},
};

const SCENES: [(&str, &str); 3] = [
    (
        "flowchart",
        include_str!("../../../../fixtures/themes/public-cyberpunk/flowchart.mmd"),
    ),
    (
        "sequence",
        include_str!("../../../../fixtures/themes/public-cyberpunk/sequence.mmd"),
    ),
    (
        "xychart",
        include_str!("../../../../fixtures/themes/public-cyberpunk/xychart.mmd"),
    ),
];

fn output_bytes(output: &RenderOutput) -> &[u8] {
    match output {
        RenderOutput::Svg(Some(output)) => output.svg().as_bytes(),
        RenderOutput::Png(Some(output)) => output.bytes(),
        RenderOutput::Pdf(Some(output)) => output.bytes(),
        _ => panic!("benchmark must produce the requested graphical output"),
    }
}

fn check_output(format: &str, bytes: &[u8]) {
    match format {
        "svg" => {
            let svg = std::str::from_utf8(bytes).unwrap();
            let document = roxmltree::Document::parse(svg).unwrap();
            assert_eq!(document.root_element().tag_name().name(), "svg");
        }
        "png" => {
            let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert!(info.width > 0 && info.height > 0);
        }
        "pdf" => {
            // Framing and byte replay are not PDF visual qualification.
            assert!(bytes.starts_with(b"%PDF-"));
            assert!(bytes.trim_ascii_end().ends_with(b"%%EOF"));
        }
        _ => unreachable!(),
    }
}

pub(super) fn register(criterion: &mut Criterion) {
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    let font_context = NativeFontOptions::default()
        .with_font_file(fonts.join("NotoSans-Latin-subset.ttf"))
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .load(&OperationControl::new())
        .expect("checked-in export benchmark fonts must load");
    let environment = SvgEnvironment::deterministic()
        .with_native_font_context(font_context)
        .expect("export benchmark must use the same fonts for measurement and painting");
    let renderer = Renderer::new()
        .with_parse_options(ParseOptions::strict())
        .with_presentation_engine(MermaidEngine::new().with_site_config(
            SourcePresentationConfig::from_value(json!({
                "htmlLabels": false,
            })),
        ));
    let cyberpunk = ThemeSelection::compiled(
        ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Cyberpunk, None)
            .unwrap(),
    );
    let artifact_directory = std::env::var_os("MERMAN_BENCH_ARTIFACT_DIR").map(PathBuf::from);
    if let Some(directory) = &artifact_directory {
        std::fs::create_dir_all(directory).unwrap();
    }

    let mut group = criterion.benchmark_group("native_export");
    // Expensive PDF cases need a fixed number of samples without linear iteration growth.
    group.sampling_mode(SamplingMode::Flat);
    group.throughput(Throughput::Elements(1));
    for (family, source) in SCENES {
        for (theme_id, theme) in [("default", None), ("cyberpunk", Some(cyberpunk.clone()))] {
            let svg = SvgRequest {
                environment: environment.clone(),
                pipeline: Some(SvgPipeline::resvg_safe()),
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some(format!("native-export-{family}-{theme_id}")),
                    ..Default::default()
                },
                ..Default::default()
            };
            for (format, target) in [
                ("svg", RenderTarget::Svg(svg.clone())),
                (
                    "png",
                    RenderTarget::Png(PngRequest {
                        svg: svg.clone(),
                        options: Default::default(),
                    }),
                ),
                (
                    "pdf",
                    RenderTarget::Pdf(PdfRequest {
                        svg: svg.clone(),
                        options: Default::default(),
                    }),
                ),
            ] {
                let name = format!("{family}/{theme_id}/{format}");
                let mut request = RenderRequest::new(source, target, OperationControl::new());
                if let Some(theme) = &theme {
                    request = request.with_native_theme(theme.clone());
                }
                let before = renderer
                    .render(request.clone())
                    .unwrap_or_else(|error| panic!("{name} preflight failed: {error}"));
                let bytes = output_bytes(&before);
                check_output(format, bytes);
                eprintln!(
                    "[bench][preflight] {}",
                    json!({
                        "benchmark": format!("native_export/{name}"),
                        "source_sha256": data_encoding::HEXLOWER.encode(&Sha256::digest(source.as_bytes())),
                        "output_bytes": bytes.len(),
                        "output_sha256": data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)),
                    })
                );
                if let Some(directory) = &artifact_directory {
                    std::fs::write(
                        directory.join(format!("{family}-{theme_id}.{format}")),
                        bytes,
                    )
                    .unwrap();
                }
                group.bench_function(&name, |b| {
                    b.iter(|| black_box(renderer.render(black_box(request.clone())).unwrap()));
                });
                let after = renderer.render(request.clone()).unwrap();
                assert_eq!(
                    bytes,
                    output_bytes(&after),
                    "{name} output changed after timing"
                );
                eprintln!("[bench][postflight] native_export/{name}");
            }
        }
    }
    group.finish();
}
