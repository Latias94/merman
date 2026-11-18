//! Deterministic orthogonal tracks. Crossings never silently become ancestry junctions.

use crate::error::AsciiError;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, charge_text_layout, terminal_single_line_text_requires_normalization,
};
use crate::text::display_width_with_profile;
use crate::{AsciiCharset, AsciiRenderOptions, Result};
use merman_model::families::git_graph::GitGraphDiagramModel;
use std::collections::HashMap;

const NONE: usize = usize::MAX;
const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const UP: u8 = 4;
const DOWN: u8 = 8;

pub(super) enum Admission {
    Spatial(Tracks),
    Records(&'static str),
}

/// A common source can split away from a commit. Different sources can join only at
/// their actual child marker, never at a shared approach channel before that child.
#[derive(Clone, Copy)]
struct Owners([usize; 2]);

impl Owners {
    fn intersects(self, other: Self) -> bool {
        self.0.iter().any(|&id| id != NONE && other.0.contains(&id))
    }

    fn connects(self, other: Self, node: usize) -> bool {
        if node != NONE {
            self.0.contains(&node) && other.0.contains(&node)
        } else {
            self.0[0] != NONE && self.0[0] == other.0[0]
        }
    }

    fn join(&mut self, other: Self, node: usize) -> bool {
        if self.0 == [NONE; 2] {
            *self = other;
            return true;
        }
        if !self.connects(other, node) {
            return false;
        }
        if node != NONE {
            self.0 = [node, NONE];
        } else if self.0[1] != other.0[1] {
            self.0[1] = NONE;
        }
        true
    }
}

#[derive(Clone, Copy)]
struct Cell {
    directions: u8,
    horizontal: Owners,
    vertical: Owners,
    label: Option<char>,
    node: usize,
    arrow: u8,
}

impl Cell {
    const EMPTY: Self = Self {
        directions: 0,
        horizontal: Owners([NONE; 2]),
        vertical: Owners([NONE; 2]),
        label: None,
        node: NONE,
        arrow: 0,
    };

    fn crossing(self) -> bool {
        self.directions & (LEFT | RIGHT) != 0
            && self.directions & (UP | DOWN) != 0
            && !self.horizontal.connects(self.vertical, self.node)
    }

    fn glyph(self, charset: AsciiCharset) -> char {
        if let Some(label) = self.label {
            return label;
        }
        if self.arrow != 0 {
            return match self.arrow {
                LEFT => '<',
                RIGHT => '>',
                UP => '^',
                _ => 'v',
            };
        }
        if self.crossing() {
            return match charset {
                AsciiCharset::Ascii => 'x',
                AsciiCharset::Unicode => '╳',
            };
        }
        if charset == AsciiCharset::Ascii {
            return match (self.directions & 3 != 0, self.directions & 12 != 0) {
                (true, true) => '+',
                (true, false) => '-',
                (false, true) => '|',
                (false, false) => ' ',
            };
        }
        match self.directions {
            0 => ' ',
            1..=3 => '─',
            4 | 8 | 12 => '│',
            5 => '┘',
            6 => '└',
            9 => '┐',
            10 => '┌',
            7 => '┴',
            11 => '┬',
            13 => '┤',
            14 => '├',
            _ => '┼',
        }
    }
}

pub(super) struct Tracks {
    cells: Vec<Cell>,
    width: usize,
    height: usize,
}

#[derive(Clone, Copy)]
enum Direction {
    Lr,
    Tb,
    Bt,
}

struct Coordinates {
    direction: Direction,
    time_pitch: usize,
    branch_pitch: usize,
    end: usize,
}

impl Coordinates {
    fn point(&self, time: usize, branch: usize) -> (usize, usize) {
        match self.direction {
            Direction::Lr => (time, branch),
            Direction::Tb => (branch, time + 2),
            Direction::Bt => (branch, self.end - time + 2),
        }
    }

    fn commit(&self, index: usize, lane: usize) -> (usize, usize) {
        self.point(1 + index * self.time_pitch, lane * self.branch_pitch)
    }
}

pub(super) fn plan(
    model: &GitGraphDiagramModel,
    config: &merman_model::MermaidConfig,
    resources: &ResourceContext,
) -> Result<Admission> {
    // Preflight input-sized indexing before any allocation or hash work.
    resources.charge_layout_work(
        resources.checked_work_add(model.commits.len(), model.branches.len())?,
    )?;
    if config
        .as_value()
        .get("gitGraph")
        .and_then(|config| config.get("parallelCommits"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return Ok(Admission::Records(
            "parallelCommits requires a different event axis; complete records follow.",
        ));
    }
    for warning in &model.warning_facts {
        charge_text_layout(resources, &warning.rule_id)?;
        if warning.rule_id == merman_model::warnings::GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID {
            return Ok(Admission::Records(
                "source commit IDs were overwritten; remaining model facts and warnings are preserved.",
            ));
        }
    }
    if model.commits.is_empty() {
        return Ok(Admission::Records(
            "no commits; complete branch records follow.",
        ));
    }
    let direction = match model.direction.as_str() {
        "LR" => Direction::Lr,
        "TB" => Direction::Tb,
        "BT" => Direction::Bt,
        _ => {
            return Ok(Admission::Records(
                "unknown source direction; model order is preserved.",
            ));
        }
    };
    let mut branches = HashMap::new();
    branches
        .try_reserve(model.branches.len())
        .map_err(|_| allocation_error())?;
    for (index, branch) in model.branches.iter().enumerate() {
        charge_text_layout(resources, &branch.name)?;
        if branches.insert(branch.name.as_str(), index).is_some() {
            return Ok(Admission::Records(
                "repeated branch names are ambiguous; branch occurrences are preserved.",
            ));
        }
    }
    let mut ids = HashMap::new();
    ids.try_reserve(model.commits.len())
        .map_err(|_| allocation_error())?;
    let mut lanes = reserved(model.commits.len())?;
    for (index, commit) in model.commits.iter().enumerate() {
        charge_text_layout(resources, &commit.id)?;
        charge_text_layout(resources, &commit.branch)?;
        if ids.insert(commit.id.as_str(), index).is_some() {
            return Ok(Admission::Records(
                "repeated commit IDs are ambiguous; C keys identify occurrences.",
            ));
        }
        if index > 0 && commit.seq <= model.commits[index - 1].seq {
            return Ok(Admission::Records(
                "commit sequence is not chronological; model order is preserved.",
            ));
        }
        let Some(&lane) = branches.get(commit.branch.as_str()) else {
            return Ok(Admission::Records(
                "a commit has no unique declared branch; literal branch names are preserved.",
            ));
        };
        lanes.push(lane);
    }
    let mut latest = reserved(model.branches.len())?;
    latest.resize(model.branches.len(), None);
    let mut edges = reserved(resources.checked_work_mul(model.commits.len(), 2)?)?;
    for (child, commit) in model.commits.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if commit.parents.len() > 2
            || (commit.commit_type != 4
                && commit.parents.len() == 2
                && commit.parents[0] == commit.parents[1])
        {
            return Ok(Admission::Records(
                "parent multiplicity requires explicit ordered references.",
            ));
        }
        let lane = lanes[child];
        let first_on_branch = latest[lane].is_none();
        for (number, parent) in commit.parents.iter().enumerate() {
            charge_text_layout(resources, parent)?;
            let Some(&parent) = ids.get(parent.as_str()).filter(|&&parent| parent < child) else {
                return Ok(Admission::Records(
                    "some parents are external or out of order; explicit references are preserved.",
                ));
            };
            if commit.commit_type == 4 && number == 1 {
                // Mermaid stores the picked source in parents[1], but it is not ancestry.
                continue;
            }
            let source_lane = lanes[parent];
            if source_lane == lane && latest[lane] != Some(parent) {
                return Ok(Admission::Records(
                    "non-tip ancestry on one branch requires explicit references.",
                ));
            }
            if source_lane != lane && !first_on_branch && latest[source_lane] != Some(parent) {
                return Ok(Admission::Records(
                    "cross-branch non-tip ancestry requires explicit references.",
                ));
            }
            edges.push((
                parent,
                child,
                source_lane != lane && first_on_branch && number == 0,
            ));
        }
        if !first_on_branch && commit.parents.is_empty() {
            return Ok(Admission::Records(
                "a branch contains multiple roots; explicit records preserve the disconnected history.",
            ));
        }
        latest[lane] = Some(child);
    }

    let key_width = model.commits.len().to_string().len() + 1;
    let branch_key_width = model.branches.len().to_string().len() + 1;
    let coords = Coordinates {
        direction,
        time_pitch: if matches!(direction, Direction::Lr) {
            key_width + 4
        } else {
            4
        },
        branch_pitch: if matches!(direction, Direction::Lr) {
            3
        } else {
            key_width.max(branch_key_width) + 4
        },
        end: resources.checked_grid_add(
            1,
            resources.checked_grid_mul(
                model.commits.len() - 1,
                if matches!(direction, Direction::Lr) {
                    key_width + 4
                } else {
                    4
                },
            )?,
        )?,
    };
    let branch_end =
        resources.checked_grid_mul(model.branches.len().saturating_sub(1), coords.branch_pitch)?;
    let (width, height) = match direction {
        Direction::Lr => (
            resources.checked_grid_add(coords.end, key_width + 2)?,
            resources.checked_grid_add(branch_end, 2)?,
        ),
        _ => (
            resources.checked_grid_add(branch_end, key_width.max(branch_key_width) + 2)?,
            resources.checked_grid_add(coords.end, 5)?,
        ),
    };
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut cells = reserved(extent.cells())?;
    cells.resize(extent.cells(), Cell::EMPTY);
    let mut tracks = Tracks {
        cells,
        width,
        height,
    };
    for (index, &lane) in lanes.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let (x, y) = coords.commit(index, lane);
        tracks.cells[y * width + x].node = index;
    }
    for (parent, child, fork) in edges {
        let start_time = 1 + parent * coords.time_pitch;
        let end_time = 1 + child * coords.time_pitch;
        let source_lane = lanes[parent] * coords.branch_pitch;
        let target_lane = lanes[child] * coords.branch_pitch;
        let channel = if fork {
            start_time
                + if matches!(direction, Direction::Lr) {
                    key_width + 1
                } else {
                    1
                }
        } else {
            end_time
        };
        let points = [
            coords.point(start_time, source_lane),
            coords.point(channel, source_lane),
            coords.point(channel, target_lane),
            coords.point(end_time, target_lane),
        ];
        for pair in points.windows(2) {
            if !tracks.segment(pair[0], pair[1], Owners([parent, child]), resources)? {
                return Ok(Admission::Records(
                    "overlapping routes have no unambiguous shared endpoint; complete parents follow.",
                ));
            }
        }
        let target = points[3];
        let previous = points
            .iter()
            .rev()
            .copied()
            .find(|&point| point != target)
            .unwrap_or(points[0]);
        let (x, y, arrow) = if previous.0 < target.0 {
            (target.0 - 1, target.1, RIGHT)
        } else if previous.0 > target.0 {
            (target.0 + 1, target.1, LEFT)
        } else if previous.1 < target.1 {
            (target.0, target.1 - 1, DOWN)
        } else {
            (target.0, target.1 + 1, UP)
        };
        let cell = &mut tracks.cells[y * width + x];
        if cell.arrow != 0 && cell.arrow != arrow {
            return Ok(Admission::Records(
                "opposing endpoint arrows require explicit parent records.",
            ));
        }
        cell.arrow = arrow;
    }
    for cell in &tracks.cells {
        resources.charge_layout_work(1)?;
        if cell.crossing() && cell.directions != LEFT | RIGHT | UP | DOWN {
            return Ok(Admission::Records(
                "a crossing would obscure a route endpoint; complete parents follow.",
            ));
        }
        if cell.arrow != 0
            && (cell.crossing() || (cell.directions & 3 != 0 && cell.directions & 12 != 0))
        {
            return Ok(Admission::Records(
                "an endpoint arrow would cover a junction; complete parents follow.",
            ));
        }
    }
    for (index, commit) in model.commits.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let (x, y) = coords.commit(index, lanes[index]);
        let node = &mut tracks.cells[y * width + x];
        let identity = Owners([index, NONE]);
        if (node.directions & 3 != 0 && !node.horizontal.intersects(identity))
            || (node.directions & 12 != 0 && !node.vertical.intersects(identity))
        {
            return Ok(Admission::Records(
                "a route crosses an unrelated commit; complete parents follow.",
            ));
        }
        if commit
            .custom_type
            .is_some_and(|kind| !(0..=2).contains(&kind))
        {
            return Ok(Admission::Records(
                "unknown custom commit appearance; numeric override is preserved.",
            ));
        }
        node.label = Some(match commit.custom_type.unwrap_or(commit.commit_type) {
            1 => 'X',
            2 => '*',
            3 => 'M',
            4 => 'c',
            _ => 'o',
        });
        let (label_x, label_y) = match direction {
            Direction::Lr => (x + 1, y + 1),
            _ => (x + 1, y + 2),
        };
        if !tracks.label(label_x, label_y, &format!("C{}", index + 1), resources)? {
            return Ok(Admission::Records(
                "commit labels would cover routes; complete records follow.",
            ));
        }
    }
    if !matches!(direction, Direction::Lr) {
        for lane in 0..model.branches.len() {
            if !tracks.label(
                lane * coords.branch_pitch,
                0,
                &format!("B{}", lane + 1),
                resources,
            )? {
                return Ok(Admission::Records(
                    "branch labels would cover routes; complete records follow.",
                ));
            }
        }
    }
    Ok(Admission::Spatial(tracks))
}

impl Tracks {
    fn segment(
        &mut self,
        a: (usize, usize),
        b: (usize, usize),
        owners: Owners,
        resources: &ResourceContext,
    ) -> Result<bool> {
        if a == b {
            return Ok(true);
        }
        let horizontal = a.1 == b.1;
        let start = if horizontal {
            a.0.min(b.0)
        } else {
            a.1.min(b.1)
        };
        let end = if horizontal {
            a.0.max(b.0)
        } else {
            a.1.max(b.1)
        };
        for position in start..=end {
            resources.charge_layout_work(1)?;
            let (x, y) = if horizontal {
                (position, a.1)
            } else {
                (a.0, position)
            };
            let cell = &mut self.cells[y * self.width + x];
            let group = if horizontal {
                &mut cell.horizontal
            } else {
                &mut cell.vertical
            };
            if !group.join(owners, cell.node) {
                return Ok(false);
            }
            if position > start {
                cell.directions |= if horizontal { LEFT } else { UP };
            }
            if position < end {
                cell.directions |= if horizontal { RIGHT } else { DOWN };
            }
        }
        Ok(true)
    }

    fn label(
        &mut self,
        x: usize,
        y: usize,
        label: &str,
        resources: &ResourceContext,
    ) -> Result<bool> {
        if y >= self.height || resources.checked_grid_add(x, label.len())? > self.width {
            return Ok(false);
        }
        for (offset, ch) in label.chars().enumerate() {
            resources.charge_layout_work(1)?;
            let cell = &mut self.cells[y * self.width + x + offset];
            if cell.directions != 0 || cell.label.is_some() || cell.node != NONE {
                return Ok(false);
            }
            cell.label = Some(ch);
        }
        Ok(true)
    }

    pub(super) fn write(
        &self,
        model: &GitGraphDiagramModel,
        options: &AsciiRenderOptions,
        record_width: usize,
        document: &mut BudgetedTextDocument,
    ) -> Result<()> {
        let vertical = model.direction != "LR";
        document.push_wrapped_prefixed_line_with("", "  ", record_width, |line| {
            line.write_fmt(format_args!(
                "Git branch tracks ({}; {})",
                model.direction,
                match model.direction.as_str() {
                    "BT" => "newest at top",
                    "TB" => "oldest at top",
                    _ => "oldest at left",
                }
            ))
        })?;
        let charset = options.structural_charset();
        let branch_digits = model.branches.len().to_string().len();
        let name_limit = record_width
            .saturating_sub(self.width.saturating_add(branch_digits + 4))
            .min(16);
        let mut name_width = 0;
        if !vertical {
            for branch in &model.branches {
                if let Some(width) =
                    branch_label_width(&branch.name, name_limit, options, document.resources_mut())?
                {
                    name_width = name_width.max(width);
                }
            }
        }
        for y in 0..self.height {
            let row = &self.cells[y * self.width..(y + 1) * self.width];
            let end = row
                .iter()
                .rposition(|cell| cell.directions != 0 || cell.label.is_some())
                .map_or(0, |index| index + 1);
            let name = if !vertical && y % 3 == 0 {
                let name = model.branches[y / 3].name.as_str();
                branch_label_width(name, name_limit, options, document.resources_mut())?
                    .map(|width| (name, width))
            } else {
                None
            };
            document.push_line_with(|line| {
                if !vertical {
                    if y % 3 == 0 {
                        line.write_fmt(format_args!(
                            "B{:<width$} ",
                            y / 3 + 1,
                            width = branch_digits
                        ))?;
                        if name_width > 0 {
                            if let Some((name, _)) = name {
                                line.push_str(name)?;
                            }
                            for _ in name.map_or(0, |(_, width)| width)..name_width {
                                line.push_str(" ")?;
                            }
                            line.push_str(" ")?;
                        }
                    } else {
                        for _ in
                            0..branch_digits + 2 + if name_width > 0 { name_width + 1 } else { 0 }
                        {
                            line.push_str(" ")?;
                        }
                    }
                }
                for cell in &row[..end] {
                    line.write_fmt(format_args!("{}", cell.glyph(charset)))?;
                }
                Ok(())
            })?;
        }
        for text in [
            "o normal; X reverse; * highlight; M merge; c cherry-pick.",
            "Arrows point to the child commit; follow them backwards to its parents.",
            "x / crossed diagonals = crossing, no join. Other junctions share ancestry.",
            "C keys identify commits; B keys identify branches. Cherry-pick sources stay in records.",
        ] {
            document.push_wrapped_prefixed_line_with("", "  ", record_width, |line| {
                line.push_str(text)
            })?;
        }
        Ok(())
    }
}

fn branch_label_width(
    name: &str,
    max: usize,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    if max == 0 || terminal_single_line_text_requires_normalization(name, resources)? {
        return Ok(None);
    }
    let width = display_width_with_profile(name, options.terminal_width_profile);
    Ok((width <= max).then_some(width))
}

fn reserved<T>(capacity: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    Ok(values)
}

fn allocation_error() -> AsciiError {
    AsciiError::allocation_failed("git_tracks")
}
