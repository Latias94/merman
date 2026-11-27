//! The always-available Mermaid diagnostic diagram.

use merman_model::presentation::PresentationConfig;
use serde::{Deserialize, Serialize};

use crate::config::config_font_family_css;
use crate::svg_css::PreparedCommonCss;
use crate::text::{TextMeasurer, TextStyle};

mod svg;

pub use svg::render_error_diagram_svg;

pub const UPSTREAM_MERMAID_VERSION: &str = merman_model::baseline::PINNED_MERMAID_BASELINE_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDiagramLayout {
    pub viewbox_width: f64,
    pub viewbox_height: f64,
    pub max_width_px: f64,
}

const ERROR_BASELINE_VIEWBOX_WIDTH: f64 = 2412.0;
const ERROR_BASELINE_VIEWBOX_HEIGHT: f64 = 512.0;
#[cfg(test)]
const ERROR_BASELINE_MAX_WIDTH_PX: f64 = 512.0;
const ERROR_MESSAGE: &str = "Syntax error in text";
const ERROR_MESSAGE_FONT_SIZE_PX: f64 = 150.0;
const ERROR_MESSAGE_BASELINE_X_PX: f64 = 1440.0;
const ERROR_MESSAGE_BASELINE_Y_PX: f64 = 250.0;
const ERROR_VERSION_FONT_SIZE_PX: f64 = 100.0;
const ERROR_VERSION_BASELINE_X_PX: f64 = 1250.0;
const ERROR_VERSION_BASELINE_Y_PX: f64 = 400.0;

#[derive(Debug, Clone, Copy)]
struct ErrorTextSpec<'a> {
    text: &'a str,
    font_size_px: f64,
    baseline_x_px: f64,
    baseline_y_px: f64,
}

impl<'a> ErrorTextSpec<'a> {
    const fn new(text: &'a str, font_size_px: f64, baseline_x_px: f64, baseline_y_px: f64) -> Self {
        Self {
            text,
            font_size_px,
            baseline_x_px,
            baseline_y_px,
        }
    }
}

/// Prepared geometry for one inherited SVG text run.
///
/// The layout and writer share the same measured coordinates.
#[derive(Debug, Clone)]
struct ErrorTextRun {
    text: Box<str>,
    font_size_px: f64,
    x: f64,
    y: f64,
}

impl ErrorTextRun {
    fn text(&self) -> &str {
        &self.text
    }

    const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    const fn x(&self) -> f64 {
        self.x
    }

    const fn y(&self) -> f64 {
        self.y
    }
}

/// One measurement-owned viewport and its final inherited text runs.
#[derive(Debug)]
struct ErrorViewport {
    width_px: f64,
    height_px: f64,
    runs: Box<[ErrorTextRun]>,
}

impl ErrorViewport {
    fn prepare<'a>(
        baseline_width_px: f64,
        baseline_height_px: f64,
        specs: impl IntoIterator<Item = ErrorTextSpec<'a>>,
        font_family_css: &str,
        measurer: &dyn TextMeasurer,
    ) -> Self {
        debug_assert!(baseline_width_px.is_finite() && baseline_width_px > 0.0);
        debug_assert!(baseline_height_px.is_finite() && baseline_height_px > 0.0);

        struct MeasuredRun<'a> {
            spec: ErrorTextSpec<'a>,
            anchor_x_ratio: f64,
            anchor_y_ratio: f64,
        }

        let mut width_px = baseline_width_px;
        let mut height_px = baseline_height_px;
        let mut measured = Vec::new();
        for spec in specs {
            debug_assert!(spec.font_size_px.is_finite() && spec.font_size_px > 0.0);
            debug_assert!(spec.baseline_x_px > 0.0 && spec.baseline_x_px < baseline_width_px);
            debug_assert!(spec.baseline_y_px > 0.0 && spec.baseline_y_px < baseline_height_px);
            let anchor_x_ratio =
                (spec.baseline_x_px / baseline_width_px).clamp(f64::EPSILON, 1.0 - f64::EPSILON);
            let anchor_y_ratio =
                (spec.baseline_y_px / baseline_height_px).clamp(f64::EPSILON, 1.0 - f64::EPSILON);
            let style = TextStyle {
                font_family: Some(font_family_css.to_string()),
                font_size: spec.font_size_px,
                ..TextStyle::default()
            };
            let (left, right) = measurer.measure_svg_text_bbox_x(spec.text, &style);
            let bbox_height = measurer.measure_svg_raw_text_bbox_height_px(spec.text, &style);
            width_px = width_px
                .max(required_axis_extent(left.max(0.0), anchor_x_ratio))
                .max(required_axis_extent(right.max(0.0), 1.0 - anchor_x_ratio));
            height_px = height_px
                .max(required_axis_extent(bbox_height.max(0.0), anchor_y_ratio))
                .max(required_axis_extent(
                    bbox_height.max(0.0),
                    1.0 - anchor_y_ratio,
                ));
            measured.push(MeasuredRun {
                spec,
                anchor_x_ratio,
                anchor_y_ratio,
            });
        }

        let runs = measured
            .into_iter()
            .map(|measured| ErrorTextRun {
                text: measured.spec.text.into(),
                font_size_px: measured.spec.font_size_px,
                x: if width_px == baseline_width_px {
                    measured.spec.baseline_x_px
                } else {
                    measured.anchor_x_ratio * width_px
                },
                y: if height_px == baseline_height_px {
                    measured.spec.baseline_y_px
                } else {
                    measured.anchor_y_ratio * height_px
                },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            width_px,
            height_px,
            runs,
        }
    }

    const fn width_px(&self) -> f64 {
        self.width_px
    }

    const fn height_px(&self) -> f64 {
        self.height_px
    }

    fn runs(&self) -> &[ErrorTextRun] {
        &self.runs
    }

    fn run(&self, index: usize) -> &ErrorTextRun {
        &self.runs[index]
    }
}

fn required_axis_extent(measured_extent: f64, available_ratio: f64) -> f64 {
    if measured_extent == 0.0 {
        return 0.0;
    }
    measured_extent / available_ratio
}

#[derive(Debug)]
pub struct ErrorTypographyThemePlan {
    css_binding: PreparedCommonCss,
    terminal_geometry: ErrorViewport,
}

impl ErrorTypographyThemePlan {
    fn resolve_with_message(
        effective_config: &PresentationConfig,
        measurer: &dyn TextMeasurer,
        message: Option<&str>,
    ) -> Self {
        let details = wrap_error_message(message.unwrap_or_default());
        let baseline_height = if details.is_empty() {
            ERROR_BASELINE_VIEWBOX_HEIGHT
        } else {
            500.0 + details.len() as f64 * 56.0
        };
        let family = config_font_family_css(effective_config);
        let css_binding = PreparedCommonCss::new(effective_config.as_value(), Some(&family));
        let version = format!("mermaid version {UPSTREAM_MERMAID_VERSION}");
        let terminal_geometry = ErrorViewport::prepare(
            ERROR_BASELINE_VIEWBOX_WIDTH,
            baseline_height,
            [
                ErrorTextSpec::new(
                    ERROR_MESSAGE,
                    ERROR_MESSAGE_FONT_SIZE_PX,
                    ERROR_MESSAGE_BASELINE_X_PX,
                    ERROR_MESSAGE_BASELINE_Y_PX,
                ),
                ErrorTextSpec::new(
                    &version,
                    ERROR_VERSION_FONT_SIZE_PX,
                    ERROR_VERSION_BASELINE_X_PX,
                    ERROR_VERSION_BASELINE_Y_PX,
                ),
            ]
            .into_iter()
            .chain(details.iter().enumerate().map(|(index, line)| {
                ErrorTextSpec::new(line, 42.0, 1440.0, 510.0 + index as f64 * 56.0)
            })),
            &family,
            measurer,
        );
        Self {
            css_binding,
            terminal_geometry,
        }
    }

    fn css_binding(&self) -> &PreparedCommonCss {
        &self.css_binding
    }

    const fn viewport_width_px(&self) -> f64 {
        self.terminal_geometry.width_px()
    }

    const fn viewport_height_px(&self) -> f64 {
        self.terminal_geometry.height_px()
    }

    fn message_geometry(&self) -> &ErrorTextRun {
        self.terminal_geometry.run(0)
    }

    fn version_geometry(&self) -> &ErrorTextRun {
        self.terminal_geometry.run(1)
    }

    fn detail_geometry(&self) -> &[ErrorTextRun] {
        &self.terminal_geometry.runs()[2..]
    }
}

/// Wraps the upstream error text at 75 Unicode code points and at most four lines.
fn wrap_error_message(message: &str) -> Vec<String> {
    const MAX_LINE_LENGTH: usize = 75;
    const MAX_LINES: usize = 4;
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_length = 0;
    for token in
        message.split(|ch: char| (ch.is_whitespace() && ch != '\u{0085}') || ch == '\u{feff}')
    {
        let mut chars = token.chars().peekable();
        while chars.peek().is_some() {
            let word: String = chars.by_ref().take(MAX_LINE_LENGTH).collect();
            let word_length = word.chars().count();
            let separator_length = usize::from(!current.is_empty());
            if current_length + separator_length + word_length > MAX_LINE_LENGTH {
                lines.push(std::mem::take(&mut current));
                if lines.len() == MAX_LINES {
                    let last = &mut lines[MAX_LINES - 1];
                    *last = last.chars().take(MAX_LINE_LENGTH - 3).collect();
                    last.push_str("...");
                    return lines;
                }
                current_length = 0;
            }
            if !current.is_empty() {
                current.push(' ');
                current_length += 1;
            }
            current.push_str(&word);
            current_length += word_length;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Prepares the shared text geometry for the diagnostic layout and SVG.
pub fn prepare_error_diagram(
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    message: Option<&str>,
) -> (ErrorDiagramLayout, ErrorTypographyThemePlan) {
    let typography_theme =
        ErrorTypographyThemePlan::resolve_with_message(config, measurer, message);
    let viewbox_width = typography_theme.viewport_width_px();
    let layout = ErrorDiagramLayout {
        viewbox_width,
        viewbox_height: typography_theme.viewport_height_px(),
        max_width_px: typography_theme.viewport_height_px()
            * (viewbox_width / ERROR_BASELINE_VIEWBOX_WIDTH),
    };
    (layout, typography_theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn error_layout_preserves_upstream_canvas_and_anchors_for_the_default_font() {
        let measurer = DeterministicTextMeasurer::default();
        let config = PresentationConfig::default();
        let (layout, typography_theme) = prepare_error_diagram(&config, &measurer, None);

        assert_eq!(layout.viewbox_width, ERROR_BASELINE_VIEWBOX_WIDTH);
        assert_eq!(layout.viewbox_height, ERROR_BASELINE_VIEWBOX_HEIGHT);
        assert_eq!(layout.max_width_px, ERROR_BASELINE_MAX_WIDTH_PX);
        assert_eq!(
            typography_theme.message_geometry().x(),
            ERROR_MESSAGE_BASELINE_X_PX
        );
        assert_eq!(
            typography_theme.message_geometry().y(),
            ERROR_MESSAGE_BASELINE_Y_PX
        );
        assert_eq!(
            typography_theme.version_geometry().x(),
            ERROR_VERSION_BASELINE_X_PX
        );
        assert_eq!(
            typography_theme.version_geometry().y(),
            ERROR_VERSION_BASELINE_Y_PX
        );
    }
}

#[cfg(test)]
mod wrapping_tests {
    use super::wrap_error_message;

    #[test]
    fn wraps_error_text_using_upstream_word_and_code_point_boundaries() {
        assert!(wrap_error_message("\t \n\u{feff}").is_empty());
        assert_eq!(wrap_error_message("A short error"), ["A short error"]);
        assert_eq!(
            wrap_error_message(&format!(
                "a{}{} c{}",
                " ".repeat(40),
                "b".repeat(40),
                "d".repeat(39)
            )),
            [
                format!("a {}", "b".repeat(40)),
                format!("c{}", "d".repeat(39))
            ],
        );
        assert_eq!(
            wrap_error_message(&"x".repeat(160)),
            ["x".repeat(75), "x".repeat(75), "x".repeat(10)]
        );
        assert_eq!(
            wrap_error_message(&"\u{1f600}".repeat(80)),
            ["\u{1f600}".repeat(75), "\u{1f600}".repeat(5)]
        );
        assert_eq!(wrap_error_message("a\u{0085}b"), ["a\u{0085}b"]);
    }

    #[test]
    fn error_text_is_capped_only_when_more_than_four_lines_are_needed() {
        assert_eq!(
            wrap_error_message(&"x".repeat(300)),
            vec!["x".repeat(75); 4]
        );
        let lines = wrap_error_message(&"x".repeat(301));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3], format!("{}...", "x".repeat(72)));
        let lines = wrap_error_message(&vec!["\u{1f600}".repeat(70); 6].join(" "));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3], format!("{}...", "\u{1f600}".repeat(70)));
    }
}

#[cfg(test)]
mod detail_tests {
    use super::*;

    #[test]
    fn detail_text_expands_the_viewport() {
        let config = PresentationConfig::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let plan = ErrorTypographyThemePlan::resolve_with_message(
            &config,
            &measurer,
            Some("A short error"),
        );
        assert!(plan.viewport_height_px() > 556.0);
        assert_eq!(plan.detail_geometry().len(), 1);
        let detail = &plan.detail_geometry()[0];
        assert_eq!(detail.text(), "A short error");
        assert_eq!(detail.x(), 1440.0);
        assert!(detail.y() > 510.0);
        assert!(detail.y() < plan.viewport_height_px());
        assert_eq!(detail.font_size_px(), 42.0);
    }
}
