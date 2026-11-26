//! Typed request fields shared by the binding transports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "svg")]
use serde_json::value::RawValue;

use super::BindingRuntimePolicy;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BindingOptions {
    #[allow(dead_code)]
    pub(crate) version: Option<u32>,
    pub(crate) runtime_policy: Option<BindingRuntimePolicy>,
    #[serde(flatten)]
    pub(crate) analysis: BindingAnalysisOptionsJson,
    pub(crate) parse: Option<ParseOptionsJson>,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[serde(skip)]
    pub(crate) theme: Option<merman::theme::ThemeSelection>,
    #[cfg(feature = "ascii")]
    pub(crate) ascii: Option<AsciiOptionsJson>,
    #[cfg(feature = "svg")]
    #[serde(
        default,
        deserialize_with = "crate::native_theme::deserialize_background"
    )]
    pub(crate) background: Option<merman::theme::Color>,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[serde(
        default,
        deserialize_with = "crate::native_theme::deserialize_export_matte"
    )]
    pub(crate) export_matte: Option<merman::theme::Color>,
    #[cfg(feature = "svg")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) environment: Option<RenderEnvironmentOptionsJson>,
    #[cfg(feature = "svg")]
    #[serde(skip)]
    pub(crate) text_measurement_selector_explicit: bool,
    #[cfg(feature = "svg")]
    pub(crate) svg: Option<SvgOptionsJson>,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) raster: Option<RasterOptionsJson>,
    #[cfg(feature = "jpeg")]
    pub(crate) jpeg: Option<JpegOptionsJson>,
    #[cfg(feature = "pdf")]
    pub(crate) pdf: Option<PdfOptionsJson>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BindingAnalysisOptionsJson {
    pub(crate) fixed_today: Option<String>,
    #[serde(default, deserialize_with = "deserialize_fixed_unix_millis")]
    pub(crate) fixed_unix_millis: Option<i64>,
    pub(crate) fixed_local_offset_minutes: Option<i32>,
    pub(crate) site_config: Option<Value>,
    pub(crate) resources: Option<ResourceOptionsJson>,
    #[cfg(feature = "analysis")]
    pub(crate) lint: Option<merman_analysis::LintOptionsJson>,
}

fn deserialize_fixed_unix_millis<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Number>::deserialize(deserializer)?;
    value
        .map(|value| {
            value
                .as_i64()
                .or_else(|| {
                    let value = value.as_f64()?;
                    (value.is_finite()
                        && value.fract() == 0.0
                        && (-8_640_000_000_000_000.0..=8_640_000_000_000_000.0).contains(&value))
                    .then_some(value as i64)
                })
                .ok_or_else(|| {
                    serde::de::Error::custom(
                        "fixed_unix_millis must be an integer between -8640000000000000 and 8640000000000000",
                    )
                })
        })
        .transpose()
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParseOptionsJson {
    pub(crate) suppress_errors: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceOptionsJson {
    pub(crate) profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) limits: BTreeMap<String, usize>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsciiOptionsJson {
    pub(crate) charset: Option<String>,
    #[serde(default, alias = "widthProfile")]
    pub(crate) width_profile: Option<String>,
    #[serde(default, alias = "defaultDirection")]
    pub(crate) default_direction: Option<String>,
    #[serde(default, alias = "colorMode")]
    pub(crate) color_mode: Option<String>,
    pub(crate) theme: Option<AsciiThemeOptionsJson>,
    #[serde(default, alias = "boxBorderPadding")]
    pub(crate) box_border_padding: Option<usize>,
    #[serde(default, alias = "nodePaddingX")]
    pub(crate) node_padding_x: Option<usize>,
    #[serde(default, alias = "nodePaddingY")]
    pub(crate) node_padding_y: Option<usize>,
    #[serde(default, alias = "graphPaddingX")]
    pub(crate) graph_padding_x: Option<usize>,
    #[serde(default, alias = "graphPaddingY")]
    pub(crate) graph_padding_y: Option<usize>,
    #[serde(default, alias = "flowchartNodeLabelWrapWidth")]
    pub(crate) flowchart_node_label_wrap_width: Option<usize>,
    #[serde(default, alias = "sequenceParticipantSpacing")]
    pub(crate) sequence_participant_spacing: Option<usize>,
    #[serde(default, alias = "sequenceMessageSpacing")]
    pub(crate) sequence_message_spacing: Option<usize>,
    #[serde(default, alias = "sequenceSelfMessageWidth")]
    pub(crate) sequence_self_message_width: Option<usize>,
    #[serde(default, alias = "sequenceMirrorActors")]
    pub(crate) sequence_mirror_actors: Option<bool>,
    #[serde(default, alias = "xychartVerticalPlotHeight")]
    pub(crate) xychart_vertical_plot_height: Option<usize>,
    #[serde(default, alias = "xychartCategoryBandWidth")]
    pub(crate) xychart_category_band_width: Option<usize>,
    #[serde(default, alias = "xychartHorizontalPlotWidth")]
    pub(crate) xychart_horizontal_plot_width: Option<usize>,
    #[serde(default, alias = "relationSummaryDiagnostics")]
    pub(crate) relation_summary_diagnostics: Option<bool>,
    #[serde(default, alias = "maxWidth")]
    pub(crate) max_width: Option<usize>,
    pub(crate) overflow: Option<String>,
    #[serde(default, alias = "trimTrailingSpaces")]
    pub(crate) trim_trailing_spaces: Option<bool>,
    #[serde(default, alias = "layoutProfile")]
    pub(crate) layout_profile: Option<String>,
    #[serde(default, alias = "graphPresentation")]
    pub(crate) graph_presentation: Option<String>,
    #[serde(default, alias = "graphRoutingSpace")]
    pub(crate) graph_routing_space: Option<String>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsciiThemeOptionsJson {
    #[serde(default, alias = "fg")]
    pub(crate) foreground: Option<String>,
    #[serde(default, alias = "bg")]
    pub(crate) background: Option<String>,
    pub(crate) line: Option<String>,
    pub(crate) accent: Option<String>,
    pub(crate) muted: Option<String>,
    pub(crate) surface: Option<String>,
    pub(crate) border: Option<String>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutOptionsJson {
    pub(crate) adaptation: Option<String>,
    pub(crate) container_width: Option<f64>,
    pub(crate) container_height: Option<f64>,
    pub(crate) screen_available_width: Option<f64>,
    pub(crate) fold_wide_ranks: Option<bool>,
    pub(crate) capture_shoal_frame: Option<bool>,
    pub(crate) previous_shoal_frame: Option<Box<RawValue>>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenderEnvironmentOptionsJson {
    pub(crate) text_measurement: Option<String>,
    pub(crate) math_renderer: Option<String>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SvgOptionsJson {
    pub(crate) diagram_id: Option<String>,
    #[serde(default, alias = "viewBoxPadding")]
    pub(crate) viewbox_padding: Option<f64>,
    pub(crate) pipeline: Option<String>,
    pub(crate) root_background_color: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RasterOptionsJson {
    pub(crate) scale: Option<f64>,
    pub(crate) matte: Option<String>,
    pub(crate) fit_to: Option<RasterFitOptionsJson>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RasterFitOptionsJson {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

#[cfg(feature = "jpeg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JpegOptionsJson {
    pub(crate) quality: Option<u8>,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PdfOptionsJson {
    pub(crate) page_paint: Option<String>,
    #[serde(default, alias = "filterScale")]
    pub(crate) filter_scale: Option<f64>,
    pub(crate) page_policy: Option<PdfPageOptionsJson>,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum PdfPageOptionsJson {
    FitSvg,
    Fixed { width_pt: f64, height_pt: f64 },
    FitCssWidth { max_width_px: f64 },
}
