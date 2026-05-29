use super::super::charset::GraphCharset;
use super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeStroke};
use crate::canvas::Canvas;
use std::collections::HashSet;

pub(crate) type RouteCells = HashSet<(usize, usize)>;

pub(super) fn set_route_cell(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    x: usize,
    y: usize,
    ch: char,
) {
    let Some(existing) = canvas.get(x, y) else {
        return;
    };
    let merged = if route_cells.contains(&(x, y)) || is_arrow(existing) {
        merge_route_chars(existing, ch)
    } else {
        ch
    };
    canvas.set(x, y, merged);
    route_cells.insert((x, y));
}

fn merge_route_chars(existing: char, incoming: char) -> char {
    if existing == ' ' || existing == incoming {
        return incoming;
    }
    if incoming == ' ' {
        return existing;
    }
    if is_arrow(incoming) {
        return incoming;
    }
    if is_arrow(existing) {
        return existing;
    }
    if is_ascii_route_char(existing) || is_ascii_route_char(incoming) {
        return merge_ascii_route_chars(existing, incoming);
    }

    let existing_dirs = unicode_route_dirs(existing);
    let incoming_dirs = unicode_route_dirs(incoming);
    if existing_dirs == 0 || incoming_dirs == 0 {
        return incoming;
    }
    unicode_route_char(existing_dirs | incoming_dirs)
}

fn is_arrow(ch: char) -> bool {
    matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼')
}

fn is_ascii_route_char(ch: char) -> bool {
    matches!(ch, '-' | '|' | '+' | '=' | '#')
}

fn merge_ascii_route_chars(existing: char, incoming: char) -> char {
    match (existing, incoming) {
        (' ', ch) | (ch, ' ') => ch,
        ('-', '-') => '-',
        ('|', '|') => '|',
        ('+' | '-' | '|' | '=' | '#', '+' | '-' | '|' | '=' | '#') => '+',
        (_, ch) => ch,
    }
}

const DIR_UP: u8 = 1;
const DIR_RIGHT: u8 = 2;
const DIR_DOWN: u8 = 4;
const DIR_LEFT: u8 = 8;

fn unicode_route_dirs(ch: char) -> u8 {
    match ch {
        '─' | '┄' | '━' => DIR_LEFT | DIR_RIGHT,
        '│' | '┆' | '┃' => DIR_UP | DIR_DOWN,
        '┌' | '╭' => DIR_RIGHT | DIR_DOWN,
        '┐' | '╮' => DIR_LEFT | DIR_DOWN,
        '└' | '╰' => DIR_UP | DIR_RIGHT,
        '┘' | '╯' => DIR_UP | DIR_LEFT,
        '├' => DIR_UP | DIR_RIGHT | DIR_DOWN,
        '┤' => DIR_UP | DIR_DOWN | DIR_LEFT,
        '┬' => DIR_RIGHT | DIR_DOWN | DIR_LEFT,
        '┴' => DIR_UP | DIR_RIGHT | DIR_LEFT,
        '┼' => DIR_UP | DIR_RIGHT | DIR_DOWN | DIR_LEFT,
        _ => 0,
    }
}

fn unicode_route_char(dirs: u8) -> char {
    match dirs {
        dirs if dirs == (DIR_LEFT | DIR_RIGHT) => '─',
        dirs if dirs == (DIR_UP | DIR_DOWN) => '│',
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

pub(super) fn edge_line_char(
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    direction: GraphDirection,
) -> char {
    match (edge.stroke, direction) {
        (GraphEdgeStroke::Normal, GraphDirection::LeftRight) => charset.horizontal,
        (GraphEdgeStroke::Normal, GraphDirection::TopDown) => charset.vertical,
        (GraphEdgeStroke::Dotted, GraphDirection::LeftRight) => charset.dotted_horizontal,
        (GraphEdgeStroke::Dotted, GraphDirection::TopDown) => charset.dotted_vertical,
        (GraphEdgeStroke::Thick, GraphDirection::LeftRight) => charset.thick_horizontal,
        (GraphEdgeStroke::Thick, GraphDirection::TopDown) => charset.thick_vertical,
    }
}
