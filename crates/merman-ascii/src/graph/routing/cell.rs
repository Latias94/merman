use super::super::charset::GraphCharset;
use super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeStroke};
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
    let was_routed = existing_route.is_some();
    let merged = if is_marker(existing) {
        existing
    } else if was_routed {
        if usize::from(normal_directions != 0)
            + usize::from(dotted_directions != 0)
            + usize::from(heavy_directions != 0)
            > 1
        {
            mixed_crossing_char(heavy_directions, paint.unicode)
        } else {
            stroke_route_char(paint.stroke, merged_directions, paint.unicode)
        }
    } else {
        ch
    };
    let role = if is_marker(merged) {
        AsciiColorRole::EdgeArrow
    } else if was_routed && merged != existing {
        AsciiColorRole::Junction
    } else {
        AsciiColorRole::EdgeLine
    };
    let color = match (paint.color, was_routed && merged != existing) {
        (CanvasColor::Role(_), true) => CanvasColor::Role(role),
        (color, _) => color,
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

fn mixed_crossing_char(heavy_directions: u8, unicode: bool) -> char {
    if !unicode {
        return '+';
    }
    match heavy_directions {
        DIR_HORIZONTAL => '┿',
        DIR_VERTICAL => '╂',
        DIR_ALL => '╋',
        _ => '┼',
    }
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
    match dirs {
        DIR_HORIZONTAL => '─',
        DIR_VERTICAL => '│',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN) => '┌',
        dirs if dirs == (DIR_DOWN | DIR_LEFT) => '┐',
        dirs if dirs == (DIR_UP | DIR_RIGHT) => '└',
        dirs if dirs == (DIR_UP | DIR_LEFT) => '┘',
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

pub(super) fn edge_line_char(
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    direction: GraphDirection,
) -> char {
    match (edge.stroke, direction.canonical()) {
        (GraphEdgeStroke::Normal, GraphDirection::LeftRight) => charset.horizontal,
        (GraphEdgeStroke::Normal, GraphDirection::TopDown) => charset.vertical,
        (GraphEdgeStroke::Dotted, GraphDirection::LeftRight) => charset.dotted_horizontal,
        (GraphEdgeStroke::Dotted, GraphDirection::TopDown) => charset.dotted_vertical,
        (GraphEdgeStroke::Thick, GraphDirection::LeftRight) => charset.thick_horizontal,
        (GraphEdgeStroke::Thick, GraphDirection::TopDown) => charset.thick_vertical,
        (GraphEdgeStroke::Invisible, _) => ' ',
        (
            GraphEdgeStroke::Normal | GraphEdgeStroke::Dotted | GraphEdgeStroke::Thick,
            GraphDirection::RightLeft | GraphDirection::BottomTop,
        ) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas as RawCanvas;
    use crate::options::TerminalWidthProfile;

    #[test]
    fn stroke_route_char_preserves_unicode_corner_and_line_identity() {
        assert_eq!(
            stroke_route_char(GraphEdgeStroke::Normal, DIR_RIGHT | DIR_DOWN, true),
            '┌'
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
    fn transverse_mixed_strokes_preserve_each_axis_in_any_paint_order() {
        for (horizontal, vertical, expected) in [
            (GraphEdgeStroke::Normal, GraphEdgeStroke::Dotted, '┼'),
            (GraphEdgeStroke::Dotted, GraphEdgeStroke::Thick, '╂'),
            (GraphEdgeStroke::Thick, GraphEdgeStroke::Normal, '┿'),
        ] {
            for reverse in [false, true] {
                let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
                let mut route_cells = RouteCells::default();
                let mut strokes = [(horizontal, DIR_HORIZONTAL), (vertical, DIR_VERTICAL)];
                if reverse {
                    strokes.reverse()
                }
                for (stroke, directions) in [strokes[0], strokes[1], strokes[0]] {
                    set_route_cell_with_paint(
                        &mut canvas,
                        &mut route_cells,
                        0,
                        0,
                        stroke_route_char(stroke, directions, true),
                        RouteCellPaint {
                            stroke,
                            directions,
                            unicode: true,
                            diagram_type: "flowchart",
                            color: CanvasColor::Role(AsciiColorRole::EdgeLine),
                        },
                    )
                    .expect("transverse crossings must also support another same-stroke owner");
                }
                assert_eq!(canvas.get(0, 0), Some(expected));
            }
        }
    }
}
