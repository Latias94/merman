//! Text measurement helpers used to derive overrides and validate rendering.

use crate::XtaskError;
use crate::util::*;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub(crate) fn measure_text(args: Vec<String>) -> Result<(), XtaskError> {
    use merman_render::text::TextMeasurer as _;

    let mut text: Option<String> = None;
    let mut font_family: Option<String> = None;
    let mut font_size: f64 = 16.0;
    let mut font_weight: Option<String> = None;
    let mut wrap_mode: String = "svg".to_string();
    let mut max_width: Option<f64> = None;
    let mut measurer: String = "vendored".to_string();
    let mut svg_bbox_x: bool = false;
    let mut svg_computed_length: bool = false;
    let mut svg_simple_bbox_width: bool = false;
    let mut markdown: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--text" => {
                i += 1;
                text = args.get(i).map(|s| s.to_string());
            }
            "--font-family" => {
                i += 1;
                font_family = args.get(i).map(|s| s.to_string());
            }
            "--font-size" => {
                i += 1;
                font_size = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--font-weight" => {
                i += 1;
                font_weight = args.get(i).map(|s| s.to_string());
            }
            "--wrap-mode" => {
                i += 1;
                wrap_mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "svg".to_string());
            }
            "--max-width" => {
                i += 1;
                max_width = args.get(i).and_then(|s| s.parse::<f64>().ok());
            }
            "--measurer" => {
                i += 1;
                measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "vendored".to_string());
            }
            "--svg-bbox-x" => svg_bbox_x = true,
            "--svg-computed-length" => svg_computed_length = true,
            "--svg-simple-bbox-width" => svg_simple_bbox_width = true,
            "--markdown" => markdown = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(text) = text else {
        return Err(XtaskError::Usage);
    };

    let wrap_mode = match wrap_mode.as_str() {
        "html" | "htmllike" => merman_render::text::WrapMode::HtmlLike,
        "svg-single-run" | "svg-singlerun" | "svglikesinglerun" => {
            merman_render::text::WrapMode::SvgLikeSingleRun
        }
        _ => merman_render::text::WrapMode::SvgLike,
    };

    let style = merman_render::text::TextStyle {
        font_family,
        font_size,
        font_weight,
    };

    let metrics = if matches!(
        measurer.as_str(),
        "deterministic" | "deterministic-text" | "deterministic-text-measurer"
    ) {
        let m = merman_render::text::DeterministicTextMeasurer::default();
        if markdown {
            merman_render::text::measure_markdown_with_flowchart_bold_deltas(
                &m, &text, &style, max_width, wrap_mode,
            )
        } else {
            m.measure_wrapped(&text, &style, max_width, wrap_mode)
        }
    } else {
        let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
        if markdown {
            merman_render::text::measure_markdown_with_flowchart_bold_deltas(
                &m, &text, &style, max_width, wrap_mode,
            )
        } else {
            m.measure_wrapped(&text, &style, max_width, wrap_mode)
        }
    };

    println!("text: {:?}", text);
    println!("font_family: {:?}", style.font_family);
    println!("font_size: {}", style.font_size);
    println!("wrap_mode: {:?}", wrap_mode);
    println!("max_width: {:?}", max_width);
    println!("markdown: {}", markdown);
    println!("width: {}", metrics.width);
    println!("height: {}", metrics.height);
    println!("line_count: {}", metrics.line_count);
    if svg_bbox_x {
        let (left, right) = if matches!(
            measurer.as_str(),
            "deterministic" | "deterministic-text" | "deterministic-text-measurer"
        ) {
            let m = merman_render::text::DeterministicTextMeasurer::default();
            m.measure_svg_text_bbox_x(&text, &style)
        } else {
            let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
            m.measure_svg_text_bbox_x(&text, &style)
        };
        println!("svg_bbox_x_left: {}", left);
        println!("svg_bbox_x_right: {}", right);
        println!("svg_bbox_x_width: {}", left + right);
    }
    if svg_computed_length {
        let w = if matches!(
            measurer.as_str(),
            "deterministic" | "deterministic-text" | "deterministic-text-measurer"
        ) {
            let m = merman_render::text::DeterministicTextMeasurer::default();
            m.measure_svg_text_computed_length_px(&text, &style)
        } else {
            let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
            m.measure_svg_text_computed_length_px(&text, &style)
        };
        println!("svg_computed_length: {}", w);
    }
    if svg_simple_bbox_width {
        let w = if matches!(
            measurer.as_str(),
            "deterministic" | "deterministic-text" | "deterministic-text-measurer"
        ) {
            let m = merman_render::text::DeterministicTextMeasurer::default();
            m.measure_svg_simple_text_bbox_width_px(&text, &style)
        } else {
            let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
            m.measure_svg_simple_text_bbox_width_px(&text, &style)
        };
        println!("svg_simple_bbox_width: {}", w);
    }

    Ok(())
}
