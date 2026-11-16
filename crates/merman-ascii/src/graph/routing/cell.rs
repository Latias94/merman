use super::super::model::GraphEdgeStroke;
use super::super::surface::GraphSurface;
use crate::canvas::CanvasColor;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use rustc_hash::FxHashMap as HashMap;

type Canvas<'surface> = dyn GraphSurface + 'surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RouteCellState {
    directions: u8,
    unicode: bool,
    normal_directions: u8,
    dotted_directions: u8,
    heavy_directions: u8,
    crossing: bool,
    color: CanvasColor,
}

pub(crate) type RouteCells = HashMap<(usize, usize), RouteCellState>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RouteCellPaint {
    pub(super) stroke: GraphEdgeStroke,
    pub(super) directions: u8,
    pub(super) unicode: bool,
    pub(super) diagram_type: &'static str,
    pub(super) color: CanvasColor,
}

pub(super) fn set_route_cell_with_paint(
    canvas: &mut Canvas<'_>,
    route_cells: &mut RouteCells,
    x: usize,
    y: usize,
    ch: char,
    paint: RouteCellPaint,
) -> Result<()> {
    let Some(existing) = canvas.get(x, y) else {
        return Ok(());
    };
    if is_marker(existing) || (existing != ' ' && route_char_directions(existing) == 0) {
        return Ok(());
    }
    let incoming_directions = if paint.directions == 0 {
        route_char_directions(ch)
    } else {
        paint.directions
    };
    let existing_route = route_cells.get(&(x, y)).copied();
    let merged_directions = match existing_route {
        Some(existing_route) => {
            let incompatible = [
                (GraphEdgeStroke::Normal, existing_route.normal_directions),
                (GraphEdgeStroke::Dotted, existing_route.dotted_directions),
                (GraphEdgeStroke::Thick, existing_route.heavy_directions),
            ]
            .into_iter()
            .any(|(stroke, directions)| {
                directions != 0
                    && stroke != paint.stroke
                    && !is_transverse_crossing(directions, incoming_directions)
            });
            if existing_route.unicode != paint.unicode || incompatible {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: paint.diagram_type,
                    feature: "mixed-stroke route junctions",
                });
            }
            existing_route.directions | incoming_directions
        }
        None => incoming_directions,
    };
    let normal_directions = existing_route.map_or(0, |state| state.normal_directions)
        | if paint.stroke == GraphEdgeStroke::Normal {
            incoming_directions
        } else {
            0
        };
    let dotted_directions = existing_route.map_or(0, |state| state.dotted_directions)
        | if paint.stroke == GraphEdgeStroke::Dotted {
            incoming_directions
        } else {
            0
        };
    let heavy_directions = existing_route.map_or(0, |state| state.heavy_directions)
        | if paint.stroke == GraphEdgeStroke::Thick {
            incoming_directions
        } else {
            0
        };
    // Keep both axes in the state, but show only the horizontal overpass. A later parallel
    // contributor must not turn the underpass into a junction; an actual branch or turn does.
    let crossing = existing_route.is_some_and(|state| {
        is_transverse_crossing(state.directions, incoming_directions)
            || (state.crossing && matches!(incoming_directions, DIR_HORIZONTAL | DIR_VERTICAL))
    });
    let was_routed = existing_route.is_some();
    let merged = if crossing {
        let horizontal_stroke = [
            (GraphEdgeStroke::Normal, normal_directions),
            (GraphEdgeStroke::Dotted, dotted_directions),
            (GraphEdgeStroke::Thick, heavy_directions),
        ]
        .into_iter()
        .find_map(|(stroke, directions)| {
            (directions & DIR_HORIZONTAL == DIR_HORIZONTAL).then_some(stroke)
        })
        .unwrap_or(paint.stroke);
        stroke_route_char(horizontal_stroke, DIR_HORIZONTAL, paint.unicode)
    } else if was_routed {
        stroke_route_char(paint.stroke, merged_directions, paint.unicode)
    } else {
        ch
    };
    let role = if is_marker(merged) {
        AsciiColorRole::EdgeArrow
    } else if was_routed && !crossing && merged_directions.count_ones() > 2 {
        AsciiColorRole::Junction
    } else {
        AsciiColorRole::EdgeLine
    };
    let color = if crossing {
        if incoming_directions == DIR_HORIZONTAL {
            paint.color
        } else {
            existing_route.map_or(paint.color, |state| state.color)
        }
    } else {
        match (paint.color, role) {
            (CanvasColor::Role(_), AsciiColorRole::Junction | AsciiColorRole::EdgeArrow) => {
                CanvasColor::Role(role)
            }
            (color, _) => color,
        }
    };
    canvas.set_canvas_color(x, y, merged, color)?;
    route_cells.insert(
        (x, y),
        RouteCellState {
            directions: merged_directions,
            unicode: paint.unicode,
            normal_directions,
            dotted_directions,
            heavy_directions,
            crossing,
            color,
        },
    );
    Ok(())
}

pub(super) fn set_edge_cell_with_paint(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    ch: char,
    color: CanvasColor,
) -> crate::Result<()> {
    if canvas.get(x, y).is_none() {
        return Ok(());
    }
    canvas.set_canvas_color(x, y, ch, color)
}

fn is_marker(ch: char) -> bool {
    matches!(
        ch,
        '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼' | 'o' | 'x' | '○' | '×'
    )
}

pub(super) const DIR_UP: u8 = 1;
pub(super) const DIR_RIGHT: u8 = 2;
pub(super) const DIR_DOWN: u8 = 4;
pub(super) const DIR_LEFT: u8 = 8;
const DIR_HORIZONTAL: u8 = DIR_LEFT | DIR_RIGHT;
const DIR_VERTICAL: u8 = DIR_UP | DIR_DOWN;
const DIR_ALL: u8 = DIR_UP | DIR_RIGHT | DIR_DOWN | DIR_LEFT;

pub(super) fn is_transverse_crossing(first: u8, second: u8) -> bool {
    matches!(
        (first, second),
        (DIR_HORIZONTAL, DIR_VERTICAL) | (DIR_VERTICAL, DIR_HORIZONTAL)
    )
}

pub(super) fn route_char_directions(ch: char) -> u8 {
    match ch {
        '-' | '=' | '.' | '─' | '┄' | '━' => DIR_HORIZONTAL,
        '|' | '#' | ':' | '│' | '┆' | '┃' => DIR_VERTICAL,
        '┌' | '╭' | '┏' => DIR_RIGHT | DIR_DOWN,
        '┐' | '╮' | '┓' => DIR_LEFT | DIR_DOWN,
        '└' | '╰' | '┗' => DIR_UP | DIR_RIGHT,
        '┘' | '╯' | '┛' => DIR_UP | DIR_LEFT,
        '├' | '┝' | '┣' => DIR_UP | DIR_RIGHT | DIR_DOWN,
        '┤' | '┥' | '┫' => DIR_UP | DIR_DOWN | DIR_LEFT,
        '┬' | '┰' | '┳' => DIR_RIGHT | DIR_DOWN | DIR_LEFT,
        '┴' | '┸' | '┻' => DIR_UP | DIR_RIGHT | DIR_LEFT,
        '+' | '·' | '┼' | '╋' | '┿' | '╂' => DIR_ALL,
        _ => 0,
    }
}

pub(super) fn edge_line_stroke_char(stroke: GraphEdgeStroke, ch: char, unicode: bool) -> char {
    if !unicode || stroke != GraphEdgeStroke::Thick {
        return ch;
    }

    // EdgeLine sits on an existing thin node or group border. Preserve the border weight and
    // strengthen only route-owned branches; a RouteCell junction remains fully stroke-owned.
    match ch {
        '├' => '┝',
        '┤' => '┥',
        '┬' => '┰',
        '┴' => '┸',
        _ => ch,
    }
}

pub(super) fn stroke_route_char(stroke: GraphEdgeStroke, directions: u8, unicode: bool) -> char {
    if !unicode {
        return match (stroke, directions) {
            (GraphEdgeStroke::Normal, DIR_HORIZONTAL) => '-',
            (GraphEdgeStroke::Normal, DIR_VERTICAL) => '|',
            (GraphEdgeStroke::Normal, _) => '+',
            (GraphEdgeStroke::Dotted, DIR_HORIZONTAL) => '.',
            (GraphEdgeStroke::Dotted, DIR_VERTICAL) => ':',
            (GraphEdgeStroke::Dotted, _) => ':',
            (GraphEdgeStroke::Thick, DIR_HORIZONTAL) => '=',
            (GraphEdgeStroke::Thick, DIR_VERTICAL) => '#',
            (GraphEdgeStroke::Thick, _) => '#',
            (GraphEdgeStroke::Invisible, _) => ' ',
        };
    }

    match stroke {
        GraphEdgeStroke::Normal => unicode_normal_route_char(directions),
        GraphEdgeStroke::Dotted => match directions {
            DIR_HORIZONTAL => '┄',
            DIR_VERTICAL => '┆',
            _ => '·',
        },
        GraphEdgeStroke::Thick => unicode_thick_route_char(directions),
        GraphEdgeStroke::Invisible => ' ',
    }
}

fn unicode_normal_route_char(dirs: u8) -> char {
    // Round only wire-owned turns. Node contours, frame contacts and branching
    // junctions keep their own glyphs so the reader can distinguish paths from boxes.
    match dirs {
        DIR_HORIZONTAL => '─',
        DIR_VERTICAL => '│',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN) => '╭',
        dirs if dirs == (DIR_DOWN | DIR_LEFT) => '╮',
        dirs if dirs == (DIR_UP | DIR_RIGHT) => '╰',
        dirs if dirs == (DIR_UP | DIR_LEFT) => '╯',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_DOWN) => '├',
        dirs if dirs == (DIR_UP | DIR_DOWN | DIR_LEFT) => '┤',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN | DIR_LEFT) => '┬',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_LEFT) => '┴',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_DOWN | DIR_LEFT) => '┼',
        _ => '┼',
    }
}

fn unicode_thick_route_char(dirs: u8) -> char {
    match dirs {
        DIR_HORIZONTAL => '━',
        DIR_VERTICAL => '┃',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN) => '┏',
        dirs if dirs == (DIR_DOWN | DIR_LEFT) => '┓',
        dirs if dirs == (DIR_UP | DIR_RIGHT) => '┗',
        dirs if dirs == (DIR_UP | DIR_LEFT) => '┛',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_DOWN) => '┣',
        dirs if dirs == (DIR_UP | DIR_DOWN | DIR_LEFT) => '┫',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN | DIR_LEFT) => '┳',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_LEFT) => '┻',
        DIR_ALL => '╋',
        _ => '╋',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas as RawCanvas;
    use crate::color::AsciiRgb;
    use crate::options::TerminalWidthProfile;

    const PAINT_ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    fn paint_test_cell(
        canvas: &mut RawCanvas,
        route_cells: &mut RouteCells,
        stroke: GraphEdgeStroke,
        directions: u8,
        unicode: bool,
        color: CanvasColor,
    ) -> Result<()> {
        set_route_cell_with_paint(
            canvas,
            route_cells,
            0,
            0,
            stroke_route_char(stroke, directions, unicode),
            RouteCellPaint {
                stroke,
                directions,
                unicode,
                diagram_type: "flowchart",
                color,
            },
        )
    }

    #[test]
    fn stroke_route_char_preserves_unicode_corner_and_line_identity() {
        assert_eq!(
            stroke_route_char(GraphEdgeStroke::Normal, DIR_RIGHT | DIR_DOWN, true),
            '╭'
        );
        assert_eq!(
            stroke_route_char(GraphEdgeStroke::Dotted, DIR_HORIZONTAL, true),
            '┄'
        );
        assert_eq!(
            stroke_route_char(GraphEdgeStroke::Thick, DIR_VERTICAL, true),
            '┃'
        );
    }

    #[test]
    fn edge_line_stroke_char_preserves_light_borders_and_strengthens_only_route_branches() {
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Thick, '├', true),
            '┝'
        );
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Thick, '┤', true),
            '┥'
        );
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Thick, '┬', true),
            '┰'
        );
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Thick, '┴', true),
            '┸'
        );
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Dotted, '├', true),
            '├'
        );
        assert_eq!(
            edge_line_stroke_char(GraphEdgeStroke::Thick, '|', false),
            '|'
        );
    }

    #[test]
    fn mixed_stroke_overlap_is_rejected_without_overwriting_the_existing_route() {
        let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
        let mut route_cells = RouteCells::default();
        set_route_cell_with_paint(
            &mut canvas,
            &mut route_cells,
            0,
            0,
            '─',
            RouteCellPaint {
                stroke: GraphEdgeStroke::Normal,
                directions: DIR_HORIZONTAL,
                unicode: true,
                diagram_type: "flowchart",
                color: CanvasColor::Role(AsciiColorRole::EdgeLine),
            },
        )
        .expect("the first route owner should paint");

        let error = set_route_cell_with_paint(
            &mut canvas,
            &mut route_cells,
            0,
            0,
            '┄',
            RouteCellPaint {
                stroke: GraphEdgeStroke::Dotted,
                directions: DIR_HORIZONTAL,
                unicode: true,
                diagram_type: "flowchart",
                color: CanvasColor::Role(AsciiColorRole::EdgeLine),
            },
        )
        .expect_err("mixed route strokes must not be silently merged");

        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "mixed-stroke route junctions"
            }
        ));
        assert_eq!(canvas.get(0, 0), Some('─'));
    }

    #[test]
    fn transverse_routes_preserve_horizontal_stroke_and_color_in_any_paint_order() {
        let horizontal_color = CanvasColor::Direct(AsciiRgb::new(220, 30, 40));
        let vertical_color = CanvasColor::Direct(AsciiRgb::new(20, 60, 210));
        for (horizontal, ascii, unicode_glyph) in [
            (GraphEdgeStroke::Normal, '-', '─'),
            (GraphEdgeStroke::Dotted, '.', '┄'),
            (GraphEdgeStroke::Thick, '=', '━'),
        ] {
            for vertical in [
                GraphEdgeStroke::Normal,
                GraphEdgeStroke::Dotted,
                GraphEdgeStroke::Thick,
            ] {
                for unicode in [false, true] {
                    for reverse in [false, true] {
                        let mut canvas =
                            RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
                        let mut route_cells = RouteCells::default();
                        let mut strokes = [
                            (horizontal, DIR_HORIZONTAL, horizontal_color),
                            (vertical, DIR_VERTICAL, vertical_color),
                        ];
                        if reverse {
                            strokes.reverse();
                        }
                        for (stroke, directions, color) in strokes {
                            paint_test_cell(
                                &mut canvas,
                                &mut route_cells,
                                stroke,
                                directions,
                                unicode,
                                color,
                            )
                            .expect("pure transverse strokes should cross without joining");
                        }
                        assert_eq!(
                            canvas.get(0, 0),
                            Some(if unicode { unicode_glyph } else { ascii })
                        );
                        assert_eq!(canvas.get_color(0, 0), Some(horizontal_color));
                        let state = route_cells[&(0, 0)];
                        assert!(state.crossing);
                        assert_eq!(state.directions, DIR_ALL);
                        for (stroke, directions, _) in strokes {
                            let retained = match stroke {
                                GraphEdgeStroke::Normal => state.normal_directions,
                                GraphEdgeStroke::Dotted => state.dotted_directions,
                                GraphEdgeStroke::Thick => state.heavy_directions,
                                GraphEdgeStroke::Invisible => unreachable!(),
                            };
                            assert_eq!(retained & directions, directions);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn crossing_survives_parallel_contributors_in_every_paint_permutation() {
        let horizontal_color = CanvasColor::Role(AsciiColorRole::EdgeLine);
        let vertical_color = CanvasColor::Direct(AsciiRgb::new(20, 60, 210));
        for (horizontal, vertical, repeated_axis) in [
            (
                GraphEdgeStroke::Normal,
                GraphEdgeStroke::Normal,
                DIR_HORIZONTAL,
            ),
            (
                GraphEdgeStroke::Normal,
                GraphEdgeStroke::Normal,
                DIR_VERTICAL,
            ),
            (
                GraphEdgeStroke::Dotted,
                GraphEdgeStroke::Thick,
                DIR_HORIZONTAL,
            ),
            (
                GraphEdgeStroke::Thick,
                GraphEdgeStroke::Normal,
                DIR_VERTICAL,
            ),
        ] {
            let repeated_stroke = if repeated_axis == DIR_HORIZONTAL {
                horizontal
            } else {
                vertical
            };
            let contributors = [
                (horizontal, DIR_HORIZONTAL),
                (vertical, DIR_VERTICAL),
                (repeated_stroke, repeated_axis),
            ];
            for unicode in [false, true] {
                let mut expected_state = None;
                for order in PAINT_ORDERS {
                    let mut canvas =
                        RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
                    let mut route_cells = RouteCells::default();
                    for index in order {
                        let (stroke, directions) = contributors[index];
                        paint_test_cell(
                            &mut canvas,
                            &mut route_cells,
                            stroke,
                            directions,
                            unicode,
                            if directions == DIR_HORIZONTAL {
                                horizontal_color
                            } else {
                                vertical_color
                            },
                        )
                        .expect("parallel contributors should preserve an existing crossing");
                    }
                    assert_eq!(
                        canvas.get(0, 0),
                        Some(stroke_route_char(horizontal, DIR_HORIZONTAL, unicode))
                    );
                    assert_eq!(canvas.get_color(0, 0), Some(horizontal_color));
                    let state = route_cells[&(0, 0)];
                    assert!(state.crossing);
                    assert_eq!(state.directions, DIR_ALL);
                    assert_eq!(*expected_state.get_or_insert(state), state);
                }
            }
        }
    }

    #[test]
    fn horizontal_overpass_leaves_vertical_neighbors_and_terminals_intact() {
        let mut canvas = RawCanvas::with_width_profile(5, 5, TerminalWidthProfile::Unicode);
        let mut route_cells = RouteCells::default();
        for (directions, stroke) in [
            (DIR_VERTICAL, GraphEdgeStroke::Dotted),
            (DIR_HORIZONTAL, GraphEdgeStroke::Thick),
        ] {
            for offset in 1..=3 {
                let (x, y) = if directions == DIR_HORIZONTAL {
                    (offset, 2)
                } else {
                    (2, offset)
                };
                set_route_cell_with_paint(
                    &mut canvas,
                    &mut route_cells,
                    x,
                    y,
                    stroke_route_char(stroke, directions, true),
                    RouteCellPaint {
                        stroke,
                        directions,
                        unicode: true,
                        diagram_type: "flowchart",
                        color: CanvasColor::Role(AsciiColorRole::EdgeLine),
                    },
                )
                .expect("the two routes should cross");
            }
        }
        for (x, y, marker) in [(2, 0, '▲'), (2, 4, '▼'), (0, 2, '◄'), (4, 2, '►')] {
            set_edge_cell_with_paint(
                &mut canvas,
                x,
                y,
                marker,
                CanvasColor::Role(AsciiColorRole::EdgeArrow),
            )
            .expect("terminals should paint independently of the crossing");
        }
        let rows: Vec<String> = (0..5)
            .map(|y| (0..5).map(|x| canvas.get(x, y).unwrap()).collect())
            .collect();
        assert_eq!(rows, ["  ▲  ", "  ┆  ", "◄━━━►", "  ┆  ", "  ▼  "]);
    }

    #[test]
    fn shared_branch_turns_and_real_junctions_remain_joined_in_every_paint_permutation() {
        for (directions, expected) in [
            (
                [DIR_RIGHT | DIR_DOWN, DIR_UP | DIR_RIGHT, DIR_UP | DIR_RIGHT],
                '├',
            ),
            ([DIR_HORIZONTAL, DIR_VERTICAL, DIR_RIGHT | DIR_DOWN], '┼'),
            (
                [DIR_UP | DIR_RIGHT | DIR_DOWN, DIR_HORIZONTAL, DIR_VERTICAL],
                '┼',
            ),
            ([DIR_ALL, DIR_HORIZONTAL, DIR_VERTICAL], '┼'),
        ] {
            for unicode in [false, true] {
                for order in PAINT_ORDERS {
                    let mut canvas =
                        RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
                    let mut route_cells = RouteCells::default();
                    for index in order {
                        paint_test_cell(
                            &mut canvas,
                            &mut route_cells,
                            GraphEdgeStroke::Normal,
                            directions[index],
                            unicode,
                            CanvasColor::Role(AsciiColorRole::EdgeLine),
                        )
                        .expect("shared turns and real junctions should merge");
                    }
                    assert_eq!(canvas.get(0, 0), Some(if unicode { expected } else { '+' }));
                    assert_eq!(
                        canvas.get_color(0, 0),
                        Some(CanvasColor::Role(AsciiColorRole::Junction))
                    );
                    assert!(!route_cells[&(0, 0)].crossing);
                }
            }
        }
    }

    #[test]
    fn incompatible_contributors_leave_crossing_glyph_color_and_state_unchanged() {
        let horizontal_color = CanvasColor::Direct(AsciiRgb::new(220, 30, 40));
        for (stroke, directions, unicode) in [
            (GraphEdgeStroke::Normal, DIR_RIGHT | DIR_DOWN, true),
            (GraphEdgeStroke::Dotted, DIR_HORIZONTAL, true),
            (GraphEdgeStroke::Thick, DIR_VERTICAL, true),
            (GraphEdgeStroke::Normal, DIR_HORIZONTAL, false),
        ] {
            let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
            let mut route_cells = RouteCells::default();
            for (stroke, directions) in [
                (GraphEdgeStroke::Normal, DIR_HORIZONTAL),
                (GraphEdgeStroke::Dotted, DIR_VERTICAL),
            ] {
                paint_test_cell(
                    &mut canvas,
                    &mut route_cells,
                    stroke,
                    directions,
                    true,
                    horizontal_color,
                )
                .expect("the original crossing should paint");
            }
            let original_state = route_cells[&(0, 0)];
            let error = paint_test_cell(
                &mut canvas,
                &mut route_cells,
                stroke,
                directions,
                unicode,
                CanvasColor::Role(AsciiColorRole::EdgeLine),
            )
            .expect_err("incompatible strokes and encodings must remain rejected");
            assert!(matches!(error, AsciiError::UnsupportedFeature { .. }));
            assert_eq!(canvas.get(0, 0), Some('─'));
            assert_eq!(canvas.get_color(0, 0), Some(horizontal_color));
            assert_eq!(route_cells[&(0, 0)], original_state);
        }
    }

    #[test]
    fn route_contributors_do_not_overwrite_or_recolor_markers_and_labels() {
        for (glyph, role) in [
            ('►', AsciiColorRole::EdgeArrow),
            ('x', AsciiColorRole::EdgeArrow),
            ('L', AsciiColorRole::EdgeLabel),
        ] {
            for previously_routed in [false, true] {
                let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
                let mut route_cells = RouteCells::default();
                if previously_routed {
                    paint_test_cell(
                        &mut canvas,
                        &mut route_cells,
                        GraphEdgeStroke::Normal,
                        DIR_VERTICAL,
                        true,
                        CanvasColor::Role(AsciiColorRole::EdgeLine),
                    )
                    .expect("the original route should paint");
                }
                canvas.set_role(0, 0, glyph, role);
                let original_state = route_cells.get(&(0, 0)).copied();
                paint_test_cell(
                    &mut canvas,
                    &mut route_cells,
                    GraphEdgeStroke::Normal,
                    DIR_HORIZONTAL,
                    true,
                    CanvasColor::Role(AsciiColorRole::EdgeLine),
                )
                .expect("existing markers and labels should be left untouched");
                assert_eq!(canvas.get(0, 0), Some(glyph));
                assert_eq!(canvas.get_color(0, 0), Some(CanvasColor::Role(role)));
                assert_eq!(route_cells.get(&(0, 0)).copied(), original_state);
            }
        }
    }
}
