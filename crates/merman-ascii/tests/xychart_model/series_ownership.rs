use super::*;
use merman_ascii::{AsciiCharset, TerminalWidthProfile};

fn colored_run(mode: AsciiColorMode, color: AsciiRgb, text: &str) -> String {
    match mode {
        AsciiColorMode::Html => format!(
            "<span style=\"color:#{:02x}{:02x}{:02x}\">{text}</span>",
            color.r, color.g, color.b
        ),
        AsciiColorMode::TrueColor => format!(
            "\u{1b}[38;2;{};{};{}m{text}\u{1b}[0m",
            color.r, color.g, color.b
        ),
        _ => panic!("ownership assertions require HTML or truecolor output"),
    }
}

#[test]
fn xychart_shared_line_cells_preserve_exclusive_point_colors() {
    let axis_color = AsciiRgb::from_hex24(0x202020);
    let series_colors = [
        AsciiRgb::from_hex24(0xa11020),
        AsciiRgb::from_hex24(0x3050c0),
    ];
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::ChartAxis, axis_color)
        .with_role(AsciiColorRole::ChartSeries(0), series_colors[0])
        .with_role(AsciiColorRole::ChartSeries(1), series_colors[1]);

    for orientation in ["vertical", "horizontal"] {
        for dense_owner in 0..2 {
            let dense = XyChartPlotModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: Vec::new(),
                data: vec![
                    ("A".to_string(), Some(2.0)),
                    ("C".to_string(), Some(2.0)),
                    ("E".to_string(), Some(2.0)),
                ],
                point_labels: Vec::new(),
            };
            let sparse = XyChartPlotModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: Vec::new(),
                data: vec![("A".to_string(), Some(2.0)), ("E".to_string(), Some(2.0))],
                point_labels: Vec::new(),
            };
            let plots = if dense_owner == 0 {
                vec![dense, sparse]
            } else {
                vec![sparse, dense]
            };
            let model = typed_xychart_model(
                orientation,
                XyChartAxisModel::Band {
                    title: String::new(),
                    categories: ["A", "B", "C", "D", "E"].map(str::to_string).to_vec(),
                },
                0.0,
                3.0,
                plots,
            );

            for base in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                    let unicode = base.charset == AsciiCharset::Unicode
                        && profile == TerminalWidthProfile::Unicode;
                    let markers = if unicode { ['●', '○'] } else { ['*', 'o'] };
                    let marker = markers[dense_owner].to_string();
                    for mode in [AsciiColorMode::Html, AsciiColorMode::TrueColor] {
                        let options = base
                            .with_terminal_width_profile(profile)
                            .with_xychart_vertical_plot_height(3)
                            .with_xychart_category_band_width(1)
                            .with_xychart_horizontal_plot_width(3)
                            .with_color_mode(mode)
                            .with_color_theme(theme);
                        let rendered = render_typed_xychart(&model, &options)
                            .expect("overlapping lines should preserve cell ownership");
                        let point = colored_run(mode, series_colors[dense_owner], &marker);

                        // A/E are shared samples; C belongs only to the dense series even
                        // though the sparse series passes through it. B/D have shared edges.
                        let expected_rows = if orientation == "vertical" {
                            let (left, right) = if unicode {
                                ("X───", "───X")
                            } else {
                                ("X---", "---X")
                            };
                            vec![
                                String::new(),
                                format!(
                                    "{}{}{}",
                                    colored_run(mode, axis_color, left),
                                    point,
                                    colored_run(mode, axis_color, right)
                                ),
                                String::new(),
                            ]
                        } else {
                            let shared_point = colored_run(mode, axis_color, "X");
                            let shared_edge =
                                colored_run(mode, axis_color, if unicode { "│" } else { "|" });
                            vec![
                                format!(" {shared_point}"),
                                format!(" {shared_edge}"),
                                format!(" {point}"),
                                format!(" {shared_edge}"),
                                format!(" {shared_point}"),
                            ]
                        };
                        let actual_rows = rendered
                            .lines()
                            .skip(1)
                            .take(expected_rows.len())
                            .map(str::to_string)
                            .collect::<Vec<_>>();

                        assert_eq!(
                            actual_rows, expected_rows,
                            "{orientation}, owner={dense_owner}, charset={:?}, \
                             profile={profile:?}, mode={mode:?}:\n{rendered}",
                            base.charset
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn xychart_seven_line_markers_cycle_by_kind_with_interleaved_bars() {
    let series = [
        (XyChartPlotType::Bar, "BarA", 8.0),
        (XyChartPlotType::Line, "线·1", 1.0),
        (XyChartPlotType::Line, "线·2", 2.0),
        (XyChartPlotType::Bar, "BarB", 8.0),
        (XyChartPlotType::Line, "线·3", 3.0),
        (XyChartPlotType::Line, "线·4", 4.0),
        (XyChartPlotType::Bar, "BarC", 8.0),
        (XyChartPlotType::Line, "线·5", 5.0),
        (XyChartPlotType::Line, "线·6", 6.0),
        (XyChartPlotType::Line, "线·7", 7.0),
    ];

    for orientation in ["vertical", "horizontal"] {
        let model = typed_xychart_model(
            orientation,
            XyChartAxisModel::Band {
                title: String::new(),
                categories: vec!["类".to_string()],
            },
            0.0,
            8.0,
            series
                .iter()
                .map(|&(plot_type, title, value)| XyChartPlotModel {
                    plot_type,
                    title: Some(title.to_string()),
                    values: vec![value],
                    data: Vec::new(),
                    point_labels: Vec::new(),
                })
                .collect(),
        );

        for base in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let unicode = base.charset == AsciiCharset::Unicode
                    && profile == TerminalWidthProfile::Unicode;
                let markers = if unicode {
                    ['●', '○', '□', '▲', '◆', '▼', '●']
                } else {
                    ['*', 'o', 's', '^', 'd', 'v', '*']
                };
                let bar = if unicode { '█' } else { '#' };
                let series_markers = [
                    bar, markers[0], markers[1], bar, markers[2], markers[3], bar, markers[4],
                    markers[5], markers[6],
                ];
                let options = base
                    .with_terminal_width_profile(profile)
                    .with_xychart_vertical_plot_height(8)
                    .with_xychart_category_band_width(3)
                    .with_xychart_horizontal_plot_width(8);
                let rendered = render_typed_xychart(&model, &options)
                    .expect("seven line series and interleaved bars should render");
                let expected_legend = series
                    .iter()
                    .zip(series_markers)
                    .map(|((_, title, _), marker)| format!("{marker} {title}"))
                    .collect::<Vec<_>>()
                    .join("  ")
                    + "  X shared samples";
                assert_eq!(rendered.lines().next(), Some(expected_legend.as_str()));

                // Vertical bars fill three lanes before line points overwrite the middle
                // lane. Horizontal bars occupy three rows, followed by the line anchor row.
                let expected_plot = if orientation == "vertical" {
                    vec![
                        bar.to_string().repeat(3),
                        format!("{bar}{}{bar}", markers[6]),
                        format!("{bar}{}{bar}", markers[5]),
                        format!("{bar}{}{bar}", markers[4]),
                        format!("{bar}{}{bar}", markers[3]),
                        format!("{bar}{}{bar}", markers[2]),
                        format!("{bar}{}{bar}", markers[1]),
                        format!("{bar}{}{bar}", markers[0]),
                    ]
                } else {
                    vec![
                        bar.to_string().repeat(8),
                        bar.to_string().repeat(8),
                        bar.to_string().repeat(8),
                        markers.iter().collect::<String>(),
                    ]
                };
                let actual_plot = rendered
                    .lines()
                    .skip(1)
                    .take(expected_plot.len())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual_plot, expected_plot,
                    "{orientation}, charset={:?}, profile={profile:?}:\n{rendered}",
                    base.charset
                );

                let expected_headers = series
                    .iter()
                    .zip(series_markers)
                    .enumerate()
                    .map(|(index, ((plot_type, title, _), marker))| {
                        let kind = match plot_type {
                            XyChartPlotType::Bar => "bar",
                            XyChartPlotType::Line => "line",
                        };
                        format!(
                            "values: {marker} series={index} type={kind} \
                             title(bytes={})=\"{title}\" samples=[",
                            title.len()
                        )
                    })
                    .collect::<Vec<_>>();
                let actual_headers = rendered
                    .lines()
                    .filter(|line| line.starts_with("values: ") && line.ends_with(" samples=["))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                assert_eq!(actual_headers, expected_headers, "{rendered}");
            }
        }
    }
}
