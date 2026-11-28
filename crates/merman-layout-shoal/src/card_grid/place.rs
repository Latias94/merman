use super::CardGridOptions;
use crate::grid::{Hierarchy, Metrics, Projection, Scope, rect};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Graph, GroupGeometry, Insets, LayoutError, LayoutOutput, Options, PackingPolicy, Point, Rect,
    Size, TitlePosition, WidthPolicy, WorkError,
};

const PRECISION: LayoutError =
    LayoutError::InvalidInput("card-grid coordinates exceed finite precision");

type Gutters = [Vec<f64>; 2];

#[derive(Clone, Copy)]
pub(super) struct CaptionDemand {
    slots: [usize; 2],
    size: Size,
}

#[derive(Clone, Copy, Default)]
struct CaptionRoom {
    span: f64,
    shelf: f64,
    count: usize,
}

impl CaptionRoom {
    fn include(&mut self, span: f64, shelf: f64) {
        self.span = self.span.max(span);
        self.shelf = self.shelf.max(shelf);
        self.count = self.count.saturating_add(1);
    }

    fn gap(self, normal: f64, runway: f64) -> f64 {
        if self.count == 0 {
            normal
        } else {
            // Bound demand for neighboring caption shelves independently of multiplicity.
            normal.max(self.span + runway + self.count.saturating_sub(1).min(3) as f64 * self.shelf)
        }
    }
}

pub(super) fn group_items(hierarchy: &mut Hierarchy, work: &mut Work<'_>) -> LayoutResult<()> {
    let mut ordered = Vec::new();
    for scope in &mut hierarchy.scopes {
        let items = &mut scope.items;
        work.visit(items.len())?;
        let is_card = |item| item < hierarchy.node_count;
        let cards = items.iter().filter(|&&item| is_card(item)).count();
        if cards == 0 || cards == items.len() {
            continue;
        }
        work.charge(items.len())?;
        crate::scratch::clear_reserve(&mut ordered, items.len())?;
        for cards_first in [true, false] {
            work.visit(items.len())?;
            ordered.extend(
                items
                    .iter()
                    .copied()
                    .filter(|&item| is_card(item) == cards_first),
            );
        }
        std::mem::swap(items, &mut ordered);
    }
    Ok(())
}

pub(super) fn caption_demands(
    graph: &Graph,
    hierarchy: &Hierarchy,
    projections: &[Projection],
    metrics: Metrics,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Vec<CaptionDemand>>> {
    work.charge(hierarchy.scopes.len())?;
    let mut grouped = reserved(hierarchy.scopes.len())?;
    grouped.resize_with(hierarchy.scopes.len(), Vec::new);
    work.visit(hierarchy.scopes.len())?;
    if hierarchy
        .scopes
        .iter()
        .all(|scope| scope.caption == Size::default())
    {
        return Ok(grouped);
    }
    work.charge(hierarchy.parent.len())?;
    let mut slots = filled(hierarchy.parent.len(), 0)?;
    for scope in &hierarchy.scopes {
        work.visit(scope.items.len())?;
        for (slot, &item) in scope.items.iter().enumerate() {
            slots[item] = slot;
        }
    }
    for (edge, projection) in graph.edges.iter().zip(projections) {
        work.visit(1)?;
        let [Some(a), Some(b)] = projection.items else {
            continue;
        };
        if a == b {
            continue;
        }
        let mut size = Size::default();
        let clearance = 2.0 * metrics.label_clearance();
        for label in [edge.label, edge.start_label, edge.end_label]
            .into_iter()
            .flatten()
        {
            size.width = size.width.max(label.width + clearance);
            size.height = size.height.max(label.height + clearance);
        }
        if size != Size::default() {
            push(
                &mut grouped[projection.scope],
                CaptionDemand {
                    slots: [slots[a], slots[b]],
                    size,
                },
            )?;
        }
    }
    Ok(grouped)
}

fn caption_gaps(
    items: usize,
    columns: usize,
    demands: &[CaptionDemand],
    gap: Size,
    runway: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Gutters> {
    let nx = columns - 1;
    let ny = items.div_ceil(columns) - 1;
    let len = nx.checked_add(ny).ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(len.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut room = [
        filled(nx, CaptionRoom::default())?,
        filled(ny, CaptionRoom::default())?,
    ];
    work.visit(demands.len())?;
    for &CaptionDemand {
        slots: [a, b],
        size,
    } in demands
    {
        for (axis, a, b, span, shelf) in [
            (0, a % columns, b % columns, size.width, size.height),
            (1, a / columns, b / columns, size.height, size.height),
        ] {
            if a != b {
                // Each caption reserves a nearby crossing in its own scope. No caption can
                // enlarge more than one row gap and one column gap.
                room[axis][a.min(b) + (a.abs_diff(b) - 1) / 2].include(span, shelf);
            }
        }
    }
    let mut gaps = [reserved(nx)?, reserved(ny)?];
    work.visit(len)?;
    for (axis, normal) in [gap.width, gap.height].into_iter().enumerate() {
        gaps[axis].extend(room[axis].iter().map(|room| room.gap(normal, runway)));
    }
    Ok(gaps)
}

fn horizontal_anchor(graph: &Graph, item: usize, height: f64) -> f64 {
    graph
        .nodes
        .get(item)
        .and_then(|node| node.anchor_y)
        .unwrap_or(height / 2.0)
}

fn checked_rect(rect: Rect) -> LayoutResult<Rect> {
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    if !right.is_finite()
        || !bottom.is_finite()
        || (rect.width > 0.0 && right <= rect.x)
        || (rect.height > 0.0 && bottom <= rect.y)
    {
        return Err(PRECISION);
    }
    Ok(rect)
}

fn item_rect_mut(drawing: &mut LayoutOutput, item: usize) -> &mut Rect {
    if item < drawing.nodes.len() {
        &mut drawing.nodes[item]
    } else {
        &mut drawing.groups[item - drawing.nodes.len()].rect
    }
}

fn bottom_title(position: TitlePosition) -> bool {
    matches!(
        position,
        TitlePosition::BottomLeft | TitlePosition::BottomCenter | TitlePosition::BottomRight
    )
}

fn padding(graph: &Graph, scope: usize, margin: f64) -> Insets {
    let mut padding = Insets {
        left: margin,
        top: margin,
        right: margin,
        bottom: margin,
    };
    if let Some(group) = graph.groups.get(scope) {
        padding.left = padding.left.max(group.padding.left);
        padding.top = padding.top.max(group.padding.top);
        padding.right = padding.right.max(group.padding.right);
        padding.bottom = padding.bottom.max(group.padding.bottom);
        if let Some(title) = group.title {
            let runway = if bottom_title(group.title_position) {
                &mut padding.bottom
            } else {
                &mut padding.top
            };
            *runway = runway.max(title.height + 2.0 * margin);
        }
    }
    padding
}

#[allow(
    clippy::too_many_arguments,
    reason = "Each scope uses borrowed placement and demand inputs."
)]
fn cells(
    graph: &Graph,
    items: &[usize],
    scope_caption: Size,
    demands: &[CaptionDemand],
    drawing: &mut LayoutOutput,
    gap: Size,
    margin: f64,
    available: f64,
    options: &Options,
    hints: CardGridOptions,
    work: &mut Work<'_>,
) -> LayoutResult<(Size, Gutters)> {
    if items.is_empty() {
        return Ok((Size::default(), Gutters::default()));
    }
    let count = items.len();
    let mut average = Size::default();
    let mut widest: f64 = 0.0;
    let mut present = [false; 2];
    for &item in items {
        work.visit(1)?;
        let card = rect(drawing, item);
        average.width += card.width / count as f64;
        average.height += card.height / count as f64;
        widest = widest.max(card.width);
        present[usize::from(item >= graph.nodes.len())] = true;
    }
    let target = match options.packing {
        PackingPolicy::AspectTargeted { target_ratio }
        | PackingPolicy::BalancedAspect { target_ratio } => target_ratio,
        _ => 1.6,
    };
    let limit = [hints.card_columns, hints.boundary_columns]
        .into_iter()
        .zip(present)
        .filter_map(|(limit, present)| limit.filter(|_| present))
        .fold(count, usize::min)
        .max(1);
    let natural = if hints
        .preferred
        .into_iter()
        .zip(present)
        .any(|(preferred, present)| preferred && present)
    {
        limit
    } else {
        let columns =
            ((average.height + gap.height) / (average.width + gap.width) * target * count as f64)
                .sqrt()
                .round() as usize;
        columns.clamp(1, limit)
    };
    // The widest card gives a fitting column count without repeated trial layouts.
    // An oversized card gets its own row.
    let mut gaps = caption_gaps(count, natural, demands, gap, margin, work)?;
    let (mut columns, mut total_width) = widths(items, natural, &gaps[0], drawing, work)?;
    if total_width > available && natural > 1 {
        let max_gap = gap
            .width
            .max(scope_caption.width + margin + 3.0 * scope_caption.height);
        let guaranteed_fit =
            (((available + max_gap) / (widest + max_gap)).floor() as usize).clamp(1, natural);
        if guaranteed_fit < natural {
            gaps = caption_gaps(count, guaranteed_fit, demands, gap, margin, work)?;
            (columns, total_width) = widths(items, guaranteed_fit, &gaps[0], drawing, work)?;
        }
    }
    let count_columns = columns.len();
    let count_rows = count.div_ceil(count_columns);
    let gutter_count = |tracks: usize| tracks.checked_add(1).ok_or(WorkError::ArithmeticOverflow);
    let x_gutters = gutter_count(count_columns)?;
    let y_gutters = gutter_count(count_rows)?;
    work.charge(
        x_gutters
            .checked_add(y_gutters)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let mut gutters = [reserved(x_gutters)?, reserved(y_gutters)?];
    gutters[0].push(0.0);
    gutters[1].push(0.0);
    let mut x = 0.0;
    for (&column, &gap) in columns.iter().zip(&gaps[0]) {
        work.visit(1)?;
        gutters[0].push(x + column + gap / 2.0);
        x += column + gap;
    }
    let mut y = 0.0;
    for (row, items) in items.chunks(count_columns).enumerate() {
        let (mut above, mut below) = (0.0_f64, 0.0_f64);
        for &item in items {
            work.visit(1)?;
            let height = rect(drawing, item).height;
            let anchor = horizontal_anchor(graph, item, height);
            above = above.max(anchor);
            below = below.max(height - anchor);
        }
        if row > 0 {
            let gap = gaps[1][row - 1];
            gutters[1].push(y + gap / 2.0);
            y += gap;
        }
        let mut x = 0.0;
        for (index, (&item, &column)) in items.iter().zip(&columns).enumerate() {
            work.visit(1)?;
            let slot = item_rect_mut(drawing, item);
            slot.x = x + (column - slot.width) / 2.0;
            slot.y = y + (above - horizontal_anchor(graph, item, slot.height));
            x += column + gaps[0].get(index).copied().unwrap_or_default();
            if let Some(rank) = drawing.node_ranks.get_mut(item) {
                *rank = row as u32;
            }
        }
        y += above + below;
    }
    for tracks in &mut gutters {
        tracks.push(0.0);
    }
    Ok((
        Size {
            width: total_width,
            height: y,
        },
        gutters,
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "The scope shares the measured placement inputs across its rows."
)]
fn scope_cells(
    graph: &Graph,
    scope: &Scope,
    demands: &[CaptionDemand],
    drawing: &mut LayoutOutput,
    metrics: Metrics,
    available: f64,
    options: &Options,
    hints: CardGridOptions,
    work: &mut Work<'_>,
) -> LayoutResult<(Size, Gutters)> {
    let margin = metrics.margin();
    // One track beyond the opposing marker approaches leaves their elbows clear.
    let min_gap = margin + options.spacing.edge;
    let gap = Size {
        width: options.spacing.node.max(min_gap),
        height: options.spacing.rank.max(min_gap),
    };
    if !gap.width.is_finite() || !gap.height.is_finite() {
        return Err(PRECISION);
    }
    let mut place_rows = |items: &[usize], demands: &[CaptionDemand], work: &mut Work<'_>| {
        cells(
            graph,
            items,
            scope.caption,
            demands,
            drawing,
            gap,
            margin,
            available,
            options,
            hints,
            work,
        )
    };
    let items = scope.items.as_slice();
    if !hints.has_preferred_columns()
        || items.first().is_none_or(|&item| item >= graph.nodes.len())
        || items[items.len() - 1] < graph.nodes.len()
    {
        return place_rows(items, demands, work);
    }

    work.visit(items.len().ilog2() as usize + 1)?;
    let split = items.partition_point(|&item| item < graph.nodes.len());
    let (cards, boundaries) = items.split_at(split);
    let mut local = [Vec::new(), Vec::new()];
    let mut seam = CaptionRoom::default();
    for &demand in demands {
        work.visit(1)?;
        let [a, b] = demand.slots;
        match (a < split, b < split) {
            (true, true) => push(&mut local[0], demand)?,
            (false, false) => push(
                &mut local[1],
                CaptionDemand {
                    slots: [a - split, b - split],
                    ..demand
                },
            )?,
            _ => seam.include(demand.size.height, demand.size.height),
        }
    }
    let (cards_size, [mut x, mut y]) = place_rows(cards, &local[0], work)?;
    let (boundary_size, [bx, by]) = place_rows(boundaries, &local[1], work)?;
    let separation = seam.gap(gap.height, margin);
    let offset = cards_size.height + separation;
    for &item in boundaries {
        work.visit(1)?;
        item_rect_mut(drawing, item).y += offset;
    }
    // Column gutters can differ between the two grids. The scoped router still checks
    // each candidate against every wall, and the row gutter joins the grids below the cards.
    x.pop();
    work.charge(bx.len())?;
    x.try_reserve(bx.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    x.extend_from_slice(&bx[1..bx.len() - 1]);
    work.charge_sort(x.len() - 1)?;
    x[1..].sort_unstable_by(f64::total_cmp);
    work.check()?;
    work.visit(x.len())?;
    x.dedup();
    push(&mut x, 0.0)?;
    y.pop();
    work.charge(by.len())?;
    y.try_reserve(by.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    y.push(cards_size.height + separation / 2.0);
    y.extend(by[1..by.len() - 1].iter().map(|track| track + offset));
    y.push(0.0);
    Ok((
        Size {
            width: cards_size.width.max(boundary_size.width),
            height: offset + boundary_size.height,
        },
        [x, y],
    ))
}

fn widths(
    items: &[usize],
    count: usize,
    gaps: &[f64],
    drawing: &LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<f64>, f64)> {
    work.charge(count)?;
    let mut widths = filled(count, 0.0_f64)?;
    for (slot, &item) in items.iter().enumerate() {
        work.visit(1)?;
        let column = &mut widths[slot % count];
        *column = column.max(rect(drawing, item).width);
    }
    work.visit(
        count
            .checked_add(gaps.len())
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let total = widths.iter().chain(gaps).sum::<f64>();
    Ok((widths, total))
}

pub(super) fn place(
    graph: &Graph,
    hierarchy: &Hierarchy,
    caption_demands: &[Vec<CaptionDemand>],
    metrics: Metrics,
    options: &Options,
    hints: CardGridOptions,
    work: &mut Work<'_>,
) -> LayoutResult<(LayoutOutput, Vec<Gutters>)> {
    let mut drawing = LayoutOutput {
        direction: graph.direction,
        ..Default::default()
    };
    if graph.nodes.is_empty() && graph.groups.is_empty() {
        return Ok((drawing, Vec::new()));
    }
    let scope_count = hierarchy.scopes.len();
    work.charge(
        graph
            .nodes
            .len()
            .checked_add(scope_count)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    drawing.nodes = reserved(graph.nodes.len())?;
    for node in &graph.nodes {
        work.visit(1)?;
        drawing.nodes.push(Rect {
            width: node.size.width,
            height: node.size.height,
            ..Default::default()
        });
    }
    drawing.groups = reserved(graph.groups.len())?;
    for group in &graph.groups {
        work.visit(1)?;
        drawing.groups.push(GroupGeometry {
            parent: group.parent,
            rect: Rect::default(),
            title_center: None,
            title_struck: false,
            effective_direction: group.direction.unwrap_or(graph.direction),
        });
    }
    if options.capture_node_ranks {
        work.charge(graph.nodes.len())?;
        drawing.node_ranks = filled(graph.nodes.len(), 0)?;
    }

    let margin = metrics.margin();
    let max_width = match options.width_policy {
        WidthPolicy::FitWidth { max_width, .. } => max_width,
        WidthPolicy::Natural => match options.packing {
            PackingPolicy::FitWidth { max_width } => max_width,
            _ => f64::INFINITY,
        },
    };
    let root = hierarchy.root();
    work.charge(scope_count)?;
    let mut available = filled(scope_count, max_width)?;
    let mut gutters = reserved(scope_count)?;
    gutters.resize_with(scope_count, Gutters::default);
    for &id in &hierarchy.order {
        work.visit(1)?;
        let outer = if id == root {
            max_width
        } else {
            available[hierarchy.parent[hierarchy.node_count + id]]
        };
        let padding = padding(graph, id, margin);
        available[id] = (outer - padding.left - padding.right).max(0.0);
    }
    for &id in hierarchy.order.iter().rev() {
        work.visit(1)?;
        let scope = &hierarchy.scopes[id];
        let padding = padding(graph, id, margin);
        let (content, tracks) = scope_cells(
            graph,
            scope,
            &caption_demands[id],
            &mut drawing,
            metrics,
            available[id],
            options,
            hints,
            work,
        )?;
        gutters[id] = tracks;
        let width = content.width.max(scope.caption.width) + padding.left + padding.right;
        let height = content.height + padding.top + padding.bottom;
        if let Some(spec) = graph.groups.get(id) {
            let mut rect = Rect {
                width: width.max(spec.min_size.width),
                height: height.max(spec.min_size.height),
                ..Default::default()
            };
            if let Some(title) = spec.title {
                rect.width = rect.width.max(title.width + 4.0 * margin);
            }
            drawing.groups[id].rect = checked_rect(rect)?;
        } else {
            checked_rect(Rect {
                width,
                height,
                ..Default::default()
            })?;
            drawing.bounds = Size { width, height };
        }
    }

    for &id in &hierarchy.order {
        work.visit(1)?;
        let padding = padding(graph, id, margin);
        let mut scope = Rect {
            width: drawing.bounds.width,
            height: drawing.bounds.height,
            ..Default::default()
        };
        if let Some(group) = drawing.groups.get_mut(id) {
            let rect = group.rect;
            scope = rect;
            let spec = &graph.groups[id];
            group.title_center = spec.title.map(|title| {
                let x = match spec.title_position {
                    TitlePosition::TopLeft | TitlePosition::BottomLeft => {
                        rect.x + margin + title.width / 2.0
                    }
                    TitlePosition::TopRight | TitlePosition::BottomRight => {
                        rect.x + rect.width - margin - title.width / 2.0
                    }
                    _ => rect.center().x,
                };
                let y = if bottom_title(spec.title_position) {
                    rect.y + rect.height - margin - title.height / 2.0
                } else {
                    rect.y + margin + title.height / 2.0
                };
                Point { x, y }
            });
        }
        let x = scope.x + padding.left;
        let y = scope.y + padding.top;
        let [columns, rows] = &mut gutters[id];
        for (tracks, origin, start, length) in [
            (columns, x, scope.x, scope.width),
            (rows, y, scope.y, scope.height),
        ] {
            if let [first, interior @ .., last] = tracks.as_mut_slice() {
                *first = start + margin / 2.0;
                *last = start + length - margin / 2.0;
                for track in interior {
                    work.visit(1)?;
                    *track += origin;
                }
            }
        }
        for &item in &hierarchy.scopes[id].items {
            work.visit(1)?;
            let rect = item_rect_mut(&mut drawing, item);
            rect.x += x;
            rect.y += y;
            checked_rect(*rect)?;
        }
    }
    Ok((drawing, gutters))
}
