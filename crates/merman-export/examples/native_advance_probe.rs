//! Benchmark-only single-face shaping experiment; not a production measurement backend.
//!
//! Run with `cargo run -p merman-export --release --features native-fonts
//! --example native_advance_probe -- 100`. Font acquisition is outside the measured loop.
//! Native caching is disabled. Each direct iteration reparses its explicit face and shapes
//! one run, without usvg's bidi segmentation, fallback, or automatic optical sizing.

#[cfg(feature = "native-fonts")]
mod probe {
    use merman_core::OperationControl;
    use merman_export::{NativeFontContext, NativeFontLimits, NativeFontOptions};
    use merman_render::environment::{
        HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        TextMeasurementOperation, TextMeasurementPhase,
    };
    use merman_render::text::{TextStyle, WrapMode};
    use std::error::Error;
    use std::hint::black_box;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    struct Case {
        name: &'static str,
        file: &'static str,
        family: &'static str,
        text: String,
        contract: &'static str,
    }

    fn font(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts")
            .join(name)
    }

    fn direct(bytes: &[u8], text: &str, font_size: f64) -> Result<(f64, bool)> {
        let face = rustybuzz::Face::from_slice(bytes, 0).ok_or("invalid prototype font")?;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let advance = shaped
            .glyph_positions()
            .iter()
            .map(|position| f64::from(position.x_advance))
            .sum::<f64>()
            * font_size
            / f64::from(face.units_per_em());
        let missing = shaped.glyph_infos().iter().any(|glyph| glyph.glyph_id == 0);
        Ok((advance.abs(), missing))
    }

    fn native(context: &NativeFontContext, text: &str, style: &TextStyle) -> Result<f64> {
        let request = HostTextMeasurementRequest {
            operation: TextMeasurementOperation::CanvasMeasureTextWidth,
            phase: TextMeasurementPhase::ComputedLength,
            text,
            style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        match context.measure(request)? {
            Some(HostTextMeasurement::Length(width)) => Ok(width),
            _ => Err("native backend did not return advance".into()),
        }
    }

    fn timed(iterations: usize, mut measure: impl FnMut() -> Result<f64>) -> Result<f64> {
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(measure()?);
        }
        Ok(start.elapsed().as_secs_f64() * 1_000_000.0 / iterations as f64)
    }

    pub fn run() -> Result<()> {
        let iterations = std::env::args()
            .nth(1)
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(100);
        if !(1..=1_000).contains(&iterations) {
            return Err("iterations must be in 1..=1000".into());
        }
        let cases = [
            (
                "kerning",
                "NotoSans-Latin-subset.ttf",
                "Noto Sans",
                "AVA",
                "single-face",
            ),
            (
                "ligature",
                "NotoSans-Latin-subset.ttf",
                "Noto Sans",
                "fi",
                "single-face",
            ),
            (
                "spaces",
                "NotoSans-Latin-subset.ttf",
                "Noto Sans",
                "  AV  ",
                "preserve-only",
            ),
            (
                "nbsp",
                "NotoSans-Latin-subset.ttf",
                "Noto Sans",
                "AV\u{a0}",
                "preserve-only",
            ),
            (
                "rtl",
                "Cousine-Regular.ttf",
                "Cousine",
                "אבגד",
                "single-direction-only",
            ),
            (
                "mixed-bidi",
                "Cousine-Regular.ttf",
                "Cousine",
                "AV אבגד 12",
                "unsupported-bidi-runs",
            ),
            (
                "cjk",
                "NotoSansSC-subset.ttf",
                "Noto Sans SC",
                "中中",
                "single-face",
            ),
            (
                "fallback",
                "Cousine-Regular.ttf",
                "Cousine",
                "A中V",
                "unsupported-fallback",
            ),
        ]
        .map(|(name, file, family, text, contract)| Case {
            name,
            file,
            family,
            text: text.to_owned(),
            contract,
        });
        let mut cases = cases.into_iter().collect::<Vec<_>>();
        cases.push(Case {
            name: "long-single-face",
            file: "NotoSans-Latin-subset.ttf",
            family: "Noto Sans",
            text: "AV fi ".repeat(100),
            contract: "single-face",
        });
        let limits = NativeFontLimits {
            max_cache_entries: 0,
            max_cache_bytes: 0,
            max_measurements: 4 * iterations * cases.len() + cases.len(),
            ..Default::default()
        };
        let context = NativeFontOptions::default()
            .with_font_file(font("NotoSans-Latin-subset.ttf"))
            .with_font_file(font("Cousine-Regular.ttf"))
            .with_font_file(font("NotoSansSC-subset.ttf"))
            .with_limits(limits)
            .load(&OperationControl::new())?;
        println!(
            "case,round,order,iterations,native_us,direct_us,native_px,direct_px,delta_px,missing_glyph,contract"
        );
        for case in cases {
            let bytes = std::fs::read(font(case.file))?;
            let style = TextStyle {
                font_family: Some(case.family.to_owned()),
                font_size: 31.0,
                font_weight: Some("400".to_owned()),
                font_style: Some("normal".to_owned()),
            };
            let expected = native(&context, &case.text, &style)?;
            let (actual, missing) = direct(&bytes, &case.text, style.font_size)?;
            for round in 0..4 {
                let direct_measure = || direct(&bytes, &case.text, style.font_size).map(|v| v.0);
                let native_measure = || native(&context, &case.text, &style);
                let (native_us, direct_us) = if round % 2 == 0 {
                    let native_us = timed(iterations, native_measure)?;
                    (native_us, timed(iterations, direct_measure)?)
                } else {
                    let direct_us = timed(iterations, direct_measure)?;
                    (timed(iterations, native_measure)?, direct_us)
                };
                println!(
                    "{},{},{},{},{:.3},{:.3},{:.6},{:.6},{:.6},{},{}",
                    case.name,
                    round,
                    if round % 2 == 0 {
                        "native-first"
                    } else {
                        "direct-first"
                    },
                    iterations,
                    native_us,
                    direct_us,
                    expected,
                    actual,
                    actual - expected,
                    missing,
                    case.contract,
                );
            }
        }
        eprintln!(
            "Not admitted: no shared usvg shaping-run API, bidi-run reconstruction, fallback selection, XML whitespace folding, variable-axis/automatic opsz coverage, resource accounting, or cancellation. Equal total widths do not prove run or glyph parity. All fixtures are static; variable fonts remain untested."
        );
        Ok(())
    }
}

#[cfg(feature = "native-fonts")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}

#[cfg(not(feature = "native-fonts"))]
fn main() {
    eprintln!("Enable the native-fonts feature to run this benchmark-only prototype.");
    std::process::exit(2);
}
