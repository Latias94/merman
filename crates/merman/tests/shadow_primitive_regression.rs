//! Native pixel oracles for the shared shadow writer's identity and opaque-tint lowering.

#![cfg(all(
    feature = "svg",
    feature = "png",
    feature = "native-fonts",
    feature = "diagram-flowchart",
    feature = "diagram-sequence"
))]

use std::fmt::Write as _;
use std::io::Cursor;

use merman::svg::export::{
    NativeFontContext, NativeFontLimits, RasterOptions, SvgConversionLimits,
};
use merman::svg::{
    DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectColorSpace,
    EffectComposition, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue, ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};

#[derive(Clone, Copy)]
struct Shadow {
    input: EffectInput,
    offset: [f32; 2],
    sigma: f32,
    color: &'static str,
}

fn document(
    source: &str,
    target: ThemeTarget,
    composition: EffectComposition,
    color_space: EffectColorSpace,
    shadows: &[Shadow],
) -> merman::RenderedDocument {
    let graph = EffectGraph::new(
        "shadow-regression",
        shadows.iter().map(|shadow| EffectPrimitive::DropShadow {
            input: shadow.input,
            offset_x: shadow.offset[0],
            offset_y: shadow.offset[1],
            blur_radius: shadow.sigma,
            spread: 0.0,
            color: ThemeColorValue::parse(shadow.color).unwrap(),
        }),
    )
    .unwrap()
    .with_color_space(color_space)
    .with_composition(composition)
    .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(
                DiagramEffectSet::default()
                    .with_graph(graph)
                    .unwrap()
                    .with_binding(EffectBinding::new(target, "shadow-regression").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    let engine = merman::MermaidEngine::new().with_site_config(
        merman::SourcePresentationConfig::from_value(serde_json::json!({
            "theme": "default", "look": "classic", "htmlLabels": false, "fontFamily": "Cousine"
        })),
    );
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .with_presentation_engine(engine)
        .render(
            RenderRequest::document(
                source,
                OperationControl::new(),
                SvgRequest {
                    environment: SvgEnvironment::deterministic()
                        .with_native_font_context(font_context())
                        .unwrap(),
                    ..Default::default()
                },
            )
            .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("expected a rendered shadow fixture")
    };
    document
}

fn is_shadow_filter(node: roxmltree::Node<'_, '_>) -> bool {
    node.has_tag_name("filter")
        && node.attribute("id").is_some_and(|id| {
            id.ends_with("-theme-effect-shadow-regression") || id.ends_with("-theme-effect-label")
        })
}

fn filter_body_range(node: roxmltree::Node<'_, '_>, svg: &str) -> std::ops::Range<usize> {
    let range = node.range();
    let start = range.start + svg[range.clone()].find('>').unwrap() + 1;
    start..range.end - "</filter>".len()
}

// Freeze the pre-optimization expansion as an independent native rendering oracle.
fn legacy_primitives(
    shadows: &[Shadow],
    composition: EffectComposition,
    color_space: EffectColorSpace,
    include_source: bool,
) -> String {
    let mut out = String::new();
    if composition == EffectComposition::Cascade
        && include_source
        && color_space == EffectColorSpace::LinearRgb
    {
        for shadow in shadows {
            out.push_str("<feDropShadow");
            if shadow.input == EffectInput::SourceGraphic {
                out.push_str(r#" in="SourceGraphic""#);
            }
            write!(
                out,
                r#" dx="{}" dy="{}" stdDeviation="{}" flood-color="{}"/>"#,
                shadow.offset[0], shadow.offset[1], shadow.sigma, shadow.color
            )
            .unwrap();
        }
        return out;
    }
    let mut first_live = 0;
    for (index, shadow) in shadows.iter().enumerate() {
        let input = if composition == EffectComposition::Independent {
            "SourceAlpha".to_owned()
        } else if shadow.input == EffectInput::SourceGraphic {
            first_live = index;
            "SourceGraphic".to_owned()
        } else {
            format!("merman-shadow-{}-result", index - 1)
        };
        write!(
            out,
            r#"<feGaussianBlur in="{input}" stdDeviation="{}" result="merman-shadow-{index}-blur"/>"#,
            shadow.sigma
        )
        .unwrap();
        let mask = if shadow.offset == [0.0, 0.0] {
            "blur"
        } else {
            write!(
                out,
                r#"<feOffset in="merman-shadow-{index}-blur" dx="{}" dy="{}" result="merman-shadow-{index}-offset"/>"#,
                shadow.offset[0], shadow.offset[1]
            )
            .unwrap();
            "offset"
        };
        write!(
            out,
            r#"<feFlood flood-color="{}" result="merman-shadow-{index}-flood"/><feComposite in="merman-shadow-{index}-flood" in2="merman-shadow-{index}-{mask}" operator="in" result="merman-shadow-{index}-shadow"/>"#,
            shadow.color
        )
        .unwrap();
        if composition == EffectComposition::Cascade
            && (include_source || index + 1 < shadows.len())
        {
            write!(
                out,
                r#"<feMerge result="merman-shadow-{index}-result"><feMergeNode in="merman-shadow-{index}-shadow"/><feMergeNode in="{input}"/></feMerge>"#,
            )
            .unwrap();
        }
    }
    if composition == EffectComposition::Independent || !include_source {
        out.push_str(r#"<feMerge result="merman-shadow-stack-result">"#);
        if composition == EffectComposition::Independent {
            for index in 0..shadows.len() {
                write!(out, r#"<feMergeNode in="merman-shadow-{index}-shadow"/>"#).unwrap();
            }
            if include_source {
                out.push_str(r#"<feMergeNode in="SourceGraphic"/>"#);
            }
        } else {
            for index in (first_live..shadows.len()).rev() {
                write!(out, r#"<feMergeNode in="merman-shadow-{index}-shadow"/>"#).unwrap();
            }
        }
        out.push_str("</feMerge>");
    }
    out
}

fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

fn font_context() -> NativeFontContext {
    static FONTS: std::sync::OnceLock<NativeFontContext> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            NativeFontContext::from_font_bytes(
                [include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf").to_vec()],
                NativeFontLimits::default(),
                &OperationControl::new(),
            )
            .unwrap()
        })
        .clone()
}

fn raster_options() -> RasterOptions {
    RasterOptions::default().with_font_context(font_context())
}

fn raster(svg: &str, options: &RasterOptions) -> (u32, u32, Vec<u8>) {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let svg = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
    decode(&merman::svg::export::svg_to_png(&svg, options).unwrap())
}

// Differential probes admit the old expansion and unchanged larger cascades. The public
// Paper Cut export below is checked separately against the unchanged default budget of 10.
fn legacy_options() -> RasterOptions {
    raster_options().with_conversion_limits(SvgConversionLimits {
        max_filter_primitives_per_filter: Some(16),
        ..Default::default()
    })
}

fn assert_same_pixels(actual: (u32, u32, Vec<u8>), expected: (u32, u32, Vec<u8>), context: &str) {
    assert_eq!((actual.0, actual.1), (expected.0, expected.1), "{context}");
    assert_eq!(actual.2.len(), expected.2.len(), "{context}");
    let first_difference = actual
        .2
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.2.as_chunks::<4>().0)
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected);
    assert!(
        first_difference.is_none(),
        "{context}: first differing RGBA pixel {first_difference:?}"
    );
}

#[test]
fn paper_stack_fits_default_png_budget_without_changing_pixels() {
    assert_eq!(
        SvgConversionLimits::default().max_filter_primitives_per_filter,
        Some(10)
    );
    let shadows = [
        Shadow {
            input: EffectInput::SourceGraphic,
            offset: [3.0, 8.0],
            sigma: 2.2,
            color: "rgba(36,62,74,0.17)",
        },
        Shadow {
            input: EffectInput::SourceGraphic,
            offset: [5.0, 6.0],
            sigma: 0.0,
            color: "#7eb4aa",
        },
        Shadow {
            input: EffectInput::SourceGraphic,
            offset: [2.0, 3.0],
            sigma: 0.0,
            color: "#eb9c88",
        },
    ];
    for (source, target) in [
        ("flowchart LR\nA[Paper] --> B[Cut]", ThemeTarget::Node),
        ("sequenceDiagram\nA->>B: Paper", ThemeTarget::Actor),
    ] {
        let document = document(
            source,
            target,
            EffectComposition::Independent,
            EffectColorSpace::Srgb,
            &shadows,
        );
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        let filters: Vec<_> = xml
            .descendants()
            .filter(|node| is_shadow_filter(*node))
            .collect();
        assert!(
            filters.len() >= 2,
            "{target:?}: expected painted body shadows"
        );
        for filter in &filters {
            assert_eq!(
                filter
                    .children()
                    .filter(roxmltree::Node::is_element)
                    .count(),
                9
            );
            assert_eq!(
                filter
                    .children()
                    .filter(|n| n.has_tag_name("feGaussianBlur"))
                    .count(),
                1
            );
            assert_eq!(
                filter
                    .children()
                    .filter(|n| n.has_tag_name("feColorMatrix"))
                    .count(),
                2
            );
            assert!(!filter.children().any(|n| n.has_tag_name("feDropShadow")));
        }
        let original = document
            .export_png(&RasterOptions::default(), OperationControl::new())
            .expect("the shared writer must fit the unchanged native primitive budget");
        let too_small = RasterOptions::default().with_conversion_limits(SvgConversionLimits {
            max_filter_primitives_per_filter: Some(8),
            ..Default::default()
        });
        let error = document
            .export_png(&too_small, OperationControl::new())
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("max_svg_conversion_filter_primitives_per_filter"),
            "{error}",
        );
        let legacy = legacy_primitives(
            &shadows,
            EffectComposition::Independent,
            EffectColorSpace::Srgb,
            false,
        );
        let legacy_svg = format!("<filter>{legacy}</filter>");
        let legacy_xml = roxmltree::Document::parse(&legacy_svg).unwrap();
        // Each old stage has four primitives, plus the final stack merge.
        assert_eq!(
            legacy_xml
                .root_element()
                .children()
                .filter(roxmltree::Node::is_element)
                .count(),
            13
        );
        let mut reference = document.svg().to_owned();
        for filter in filters.iter().rev() {
            reference.replace_range(filter_body_range(*filter, document.svg()), &legacy);
        }
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let sealed = merman_render::svg::finalize_resvg_svg(&reference, &session).unwrap();
        let error = merman::svg::export::svg_to_png(&sealed, &raster_options()).unwrap_err();
        assert!(
            matches!(
                error,
                merman::svg::export::ExportError::SvgConversionLimit {
                    limit_name: "max_filter_primitives_per_filter",
                    actual: 13,
                    max: 10,
                }
            ),
            "{error}",
        );
        assert_same_pixels(
            decode(original.bytes()),
            raster(&reference, &legacy_options()),
            &format!("{target:?}"),
        );
    }
}

fn probe(body: &str, color_space: EffectColorSpace, paint: &str) -> String {
    let color_space = match color_space {
        EffectColorSpace::Srgb => "sRGB",
        EffectColorSpace::LinearRgb => "linearRGB",
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="80" viewBox="0 0 96 80"><defs><filter id="probe" filterUnits="userSpaceOnUse" x="0" y="0" width="96" height="80" color-interpolation-filters="{color_space}">{body}</filter></defs><g filter="url(#probe)">{paint}</g></svg>"#
    )
}

#[test]
fn optimized_shadows_preserve_cascade_resets_masks_and_translucent_source_pixels() {
    use EffectInput::{Previous, SourceGraphic};
    let paint = r##"<rect x="24.25" y="24.5" width="22.5" height="18.25" fill="#3a91c8" fill-opacity="0.37" stroke="#a547c1" stroke-opacity="0.49" stroke-width="2.5"/><circle cx="45" cy="42" r="10.25" fill="#d05e71" opacity="0.63"/>"##;
    for color_space in [EffectColorSpace::Srgb, EffectColorSpace::LinearRgb] {
        for (composition, inputs) in [
            (EffectComposition::Independent, [SourceGraphic; 3]),
            (
                EffectComposition::Cascade,
                [SourceGraphic, Previous, Previous],
            ),
            (
                EffectComposition::Cascade,
                [SourceGraphic, Previous, SourceGraphic],
            ),
            (
                EffectComposition::Cascade,
                [SourceGraphic, SourceGraphic, Previous],
            ),
        ] {
            let shadows = [
                Shadow {
                    input: inputs[0],
                    offset: [-0.0, 0.0],
                    sigma: -0.0,
                    color: "rgb(126,180,170)",
                },
                Shadow {
                    input: inputs[1],
                    offset: [3.5, -2.0],
                    sigma: 1.5,
                    color: "rgba(36,62,74,0.37)",
                },
                Shadow {
                    input: inputs[2],
                    offset: [-4.0, 3.25],
                    sigma: 0.0,
                    color: "#eb9c88",
                },
            ];
            for include_source in [false, true] {
                let target = if include_source {
                    ThemeTarget::NodeLabel
                } else {
                    ThemeTarget::Node
                };
                let document = document(
                    "flowchart LR\nA[Alpha]",
                    target,
                    composition,
                    color_space,
                    &shadows,
                );
                let xml = roxmltree::Document::parse(document.svg()).unwrap();
                let filter = xml
                    .descendants()
                    .find(|node| is_shadow_filter(*node))
                    .unwrap();
                let body = &document.svg()[filter_body_range(filter, document.svg())];
                let legacy = legacy_primitives(&shadows, composition, color_space, include_source);
                let actual = raster(&probe(body, color_space, paint), &legacy_options());
                let expected = raster(&probe(&legacy, color_space, paint), &legacy_options());
                assert!(actual.2.iter().skip(3).step_by(4).any(|alpha| *alpha > 0));
                assert_same_pixels(
                    actual,
                    expected,
                    &format!("{composition:?}/{inputs:?}/{color_space:?}/source={include_source}"),
                );
            }
        }
    }
}

#[test]
fn tint_folding_requires_exact_opaque_srgb_channels() {
    let paint = r##"<rect x="24.25" y="24.5" width="22.5" height="18.25" fill="#3a91c8" fill-opacity="0.37"/>"##;
    for (color, eligible) in [
        ("#7eb4aa", true),
        ("rgb(126,180,170)", true),
        ("rgba(126,180,170,1)", true),
        ("hsl(0,100%,50%)", true),
        ("rgb(126.5,180,170)", false),
        ("hsl(12,70%,60%)", false),
        ("rgba(126,180,170,0.999)", false),
        ("#7eb4aafe", false),
        ("transparent", false),
    ] {
        let shadows = [Shadow {
            input: EffectInput::SourceGraphic,
            offset: [-0.0, 0.0],
            sigma: 0.0,
            color,
        }];
        for color_space in [EffectColorSpace::Srgb, EffectColorSpace::LinearRgb] {
            let document = document(
                "flowchart LR\nA[Alpha]",
                ThemeTarget::Node,
                EffectComposition::Independent,
                color_space,
                &shadows,
            );
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            let filter = xml
                .descendants()
                .find(|node| is_shadow_filter(*node))
                .unwrap();
            let folded = eligible && color_space == EffectColorSpace::Srgb;
            assert_eq!(
                filter
                    .children()
                    .any(|node| node.has_tag_name("feColorMatrix")),
                folded,
                "{color}/{color_space:?}"
            );
            assert!(
                !filter.children().any(
                    |node| node.has_tag_name("feGaussianBlur") || node.has_tag_name("feOffset")
                )
            );
            let body = &document.svg()[filter_body_range(filter, document.svg())];
            let legacy =
                legacy_primitives(&shadows, EffectComposition::Independent, color_space, false);
            assert_same_pixels(
                raster(&probe(body, color_space, paint), &raster_options()),
                raster(&probe(&legacy, color_space, paint), &raster_options()),
                &format!("{color}/{color_space:?}"),
            );
        }
    }
}

#[test]
fn source_is_painted_once_and_depth_depends_only_on_source_alpha() {
    let shadows = [Shadow {
        input: EffectInput::SourceGraphic,
        offset: [24.0, 0.0],
        sigma: 0.0,
        color: "rgba(96,64,144,0.5)",
    }];
    for color_space in [EffectColorSpace::Srgb, EffectColorSpace::LinearRgb] {
        for include_source in [false, true] {
            let target = if include_source {
                ThemeTarget::NodeLabel
            } else {
                ThemeTarget::Node
            };
            let document = document(
                "flowchart LR\nA[Alpha]",
                target,
                EffectComposition::Independent,
                color_space,
                &shadows,
            );
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            let filter = xml
                .descendants()
                .find(|node| is_shadow_filter(*node))
                .unwrap();
            let body = &document.svg()[filter_body_range(filter, document.svg())];
            let paint =
                r##"<rect x="8" y="8" width="16" height="16" fill="#304080" fill-opacity="0.5"/>"##;
            let (width, _, pixels) = raster(&probe(body, color_space, paint), &raster_options());
            let alpha = |x: usize| pixels[(12 * width as usize + x) * 4 + 3];
            assert_eq!(
                alpha(12),
                if include_source { 128 } else { 0 },
                "source paint ownership: {color_space:?}"
            );
            assert_eq!(
                alpha(36),
                64,
                "source alpha times shadow color opacity: {color_space:?}"
            );
            if !include_source {
                let recolored = paint.replace("#304080", "#e08020");
                assert_same_pixels(
                    (width, 80, pixels),
                    raster(&probe(body, color_space, &recolored), &raster_options()),
                    &format!("depth must ignore the source's RGB: {color_space:?}"),
                );
            }
        }
    }
}
