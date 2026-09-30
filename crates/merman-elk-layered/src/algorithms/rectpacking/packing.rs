//! Modified EPL-2.0 translations of InitialPlacement, Compaction, RowFillingAndCompaction,
//! RectangleExpansion and their Block, BlockRow, BlockStack, RectRow helpers.
//! Arena indices preserve the source's shared block identities without cyclic ownership.

use super::{Error, Options, Placement, Rectangle, charge};
use crate::work::WorkControl;

#[derive(Clone, Default)]
struct Subrow {
    nodes: Vec<usize>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Default)]
struct Block {
    nodes: Vec<usize>,
    rows: Vec<Subrow>,
    parent: usize,
    stack: Option<usize>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
    max_height: f64,
    smallest_height: f64,
    average_height: f64,
    fixed: bool,
    position_fixed: bool,
}

#[derive(Clone, Default)]
struct Row {
    blocks: Vec<usize>,
    stacks: Vec<usize>,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Default)]
struct Stack {
    blocks: Vec<usize>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
pub(super) struct Changes {
    pub increase_min: f64,
    pub decrease_min: f64,
    increase_max: f64,
    decrease_max: f64,
}

impl Default for Changes {
    fn default() -> Self {
        Self {
            increase_min: f64::INFINITY,
            decrease_min: f64::INFINITY,
            increase_max: 0.0,
            decrease_max: 0.0,
        }
    }
}

#[derive(Default)]
pub(super) struct Drawing {
    pub nodes: Vec<Placement>,
    pub width: f64,
    pub height: f64,
    pub changes: Changes,
    blocks: Vec<Block>,
    rows: Vec<Row>,
    stacks: Vec<Stack>,
    order: Vec<usize>,
    additional_height: f64,
}

struct Context<'a> {
    drawing: &'a mut Drawing,
    options: &'a Options,
    work: &'a mut dyn WorkControl,
}

pub(super) fn pack(
    rectangles: &[Rectangle],
    options: &Options,
    target_width: f64,
    work: &mut dyn WorkControl,
) -> Result<Drawing, Error> {
    charge(work, rectangles.len())?;
    if !target_width.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    let mut drawing = Drawing {
        nodes: rectangles
            .iter()
            .map(|r| Placement {
                width: r.width,
                height: r.height,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut context = Context {
        drawing: &mut drawing,
        options,
        work,
    };
    context.initial_placement(target_width)?;
    let mut position = 0;
    while position < context.drawing.order.len() {
        charge(context.work, 1)?;
        let row = context.drawing.order[position];
        if position > 0 {
            let previous = context.drawing.order[position - 1];
            let y = context.drawing.rows[previous].y
                + context.drawing.rows[previous].height
                + options.spacing;
            context.move_row(row, y)?;
        }
        if context.compact(position, target_width)? {
            let count = context.drawing.rows[row].blocks.len();
            charge(context.work, count)?;
            for i in 0..count {
                let block = context.drawing.rows[row].blocks[i];
                context.drawing.blocks[block].fixed = false;
                context.drawing.blocks[block].position_fixed = false;
                context.reset_block(block)?;
            }
            context.drawing.rows[row].stacks.clear();
            context.drawing.rows[row].width = target_width;
            continue;
        }
        context.adjust_row_dimensions(row)?;
        context.update_changes(position, target_width)?;
        position += 1;
    }
    let mut width = 0.0_f64;
    let mut height = 0.0;
    charge(context.work, context.drawing.order.len())?;
    for (i, &row) in context.drawing.order.iter().enumerate() {
        width = width.max(context.drawing.rows[row].width);
        height += context.drawing.rows[row].height + if i > 0 { options.spacing } else { 0.0 };
    }
    drawing.width = width.max(options.minimum_width - options.padding.left - options.padding.right);
    drawing.height =
        height.max(options.minimum_height - options.padding.top - options.padding.bottom);
    drawing.additional_height = drawing.height - height;
    if !drawing.width.is_finite() || !drawing.height.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    Ok(drawing)
}

impl Context<'_> {
    fn new_row(&mut self, y: f64) -> Result<usize, Error> {
        charge(self.work, 1)?;
        let id = self.drawing.rows.len();
        self.drawing.rows.push(Row {
            y,
            ..Default::default()
        });
        self.drawing.order.push(id);
        Ok(id)
    }

    fn new_block(&mut self, row: usize, x: f64) -> Result<usize, Error> {
        charge(self.work, 1)?;
        let id = self.drawing.blocks.len();
        self.drawing.blocks.push(Block {
            parent: row,
            x,
            y: self.drawing.rows[row].y,
            smallest_height: f64::INFINITY,
            ..Default::default()
        });
        self.add_block(row, id);
        Ok(id)
    }

    fn add_block(&mut self, row: usize, block: usize) {
        let r = &mut self.drawing.rows[row];
        r.height = r.height.max(self.drawing.blocks[block].height);
        r.width += self.drawing.blocks[block].width
            + if r.blocks.is_empty() {
                0.0
            } else {
                self.options.spacing
            };
        r.blocks.push(block);
    }

    fn remove_block(&mut self, row: usize, block: usize) -> Result<(), Error> {
        charge(self.work, self.drawing.rows[row].blocks.len())?;
        let r = &mut self.drawing.rows[row];
        // Source subtracts width even if the block was already detached; retain that behavior.
        if let Some(index) = r.blocks.iter().position(|&id| id == block) {
            r.blocks.remove(index);
        }
        r.width -= self.drawing.blocks[block].width
            + if r.blocks.is_empty() {
                0.0
            } else {
                self.options.spacing
            };
        r.height = r
            .blocks
            .iter()
            .map(|&id| self.drawing.blocks[id].height)
            .fold(f64::from_bits(1), f64::max);
        Ok(())
    }

    fn notify_row(&mut self, row: usize) -> Result<(), Error> {
        charge(self.work, self.drawing.rows[row].blocks.len())?;
        let r = &mut self.drawing.rows[row];
        r.width = 0.0;
        r.height = f64::NEG_INFINITY;
        for (index, &id) in r.blocks.iter().enumerate() {
            r.width +=
                self.drawing.blocks[id].width + if index > 0 { self.options.spacing } else { 0.0 };
            r.height = r.height.max(self.drawing.blocks[id].height);
        }
        Ok(())
    }

    fn initial_placement(&mut self, target: f64) -> Result<(), Error> {
        let mut row = self.new_row(0.0)?;
        let mut block = self.new_block(row, 0.0)?;
        let mut current_width = 0.0;
        for node in 0..self.drawing.nodes.len() {
            charge(self.work, 1)?;
            let first = self.drawing.rows[row].blocks[0];
            let potential = current_width
                + self.drawing.nodes[node].width
                + if self.drawing.blocks[first].nodes.is_empty() {
                    0.0
                } else {
                    self.options.spacing
                };
            if potential > target {
                let y =
                    self.drawing.rows[row].y + self.drawing.rows[row].height + self.options.spacing;
                row = self.new_row(y)?;
                block = self.new_block(row, 0.0)?;
            }
            if !self.drawing.blocks[block].nodes.is_empty()
                && (self.options.row_height_reevaluation || !self.similar_height(block, node))
            {
                let x = self.drawing.blocks[block].x
                    + self.drawing.blocks[block].width
                    + self.options.spacing;
                block = self.new_block(row, x)?;
            }
            self.add_child(block, node, false)?;
            current_width = self.drawing.nodes[node].x + self.drawing.nodes[node].width;
        }
        Ok(())
    }

    fn similar_height(&self, block: usize, node: usize) -> bool {
        let b = &self.drawing.blocks[block];
        let height = self.drawing.nodes[node].height;
        (height >= b.smallest_height && height <= b.min_height)
            || (b.average_height * 0.5 <= height && b.average_height * 1.5 >= height)
    }

    fn add_child(&mut self, block: usize, node: usize, new_row: bool) -> Result<(), Error> {
        charge(self.work, 1)?;
        let b = &mut self.drawing.blocks[block];
        if b.rows.is_empty() || new_row {
            let y = b
                .rows
                .last()
                .map_or(b.y, |r| r.y + r.height + self.options.spacing);
            b.rows.push(Subrow {
                x: b.x,
                y,
                ..Default::default()
            });
        }
        let last = b.rows.last_mut().ok_or(Error::NonFiniteGeometry)?;
        let rect = &mut self.drawing.nodes[node];
        let spacing = if last.nodes.is_empty() {
            0.0
        } else {
            self.options.spacing
        };
        rect.x = last.x + last.width + spacing;
        rect.y = last.y;
        last.height = last.height.max(rect.height);
        last.width += rect.width + spacing;
        last.nodes.push(node);
        b.nodes.push(node);
        b.width = b.width.max(last.width);
        let spacing = if b.nodes.len() == 1 {
            0.0
        } else {
            self.options.spacing
        };
        b.min_width = b.min_width.max(rect.width + spacing);
        b.smallest_height = b.smallest_height.min(rect.height);
        b.max_height += rect.height + spacing;
        b.min_height = b.min_height.max(rect.height);
        charge(self.work, b.rows.len())?;
        b.height = b.rows.iter().map(|r| r.height).sum::<f64>()
            + b.rows.len().saturating_sub(1) as f64 * self.options.spacing;
        b.average_height = b.max_height / b.nodes.len() as f64
            - self.options.spacing * (b.nodes.len() - 1) as f64 / b.nodes.len() as f64;
        let parent = b.parent;
        self.notify_row(parent)
    }

    fn remove_child(&mut self, block: usize, node: usize) -> Result<(), Error> {
        let b = &mut self.drawing.blocks[block];
        charge(self.work, b.nodes.len())?;
        b.nodes.retain(|&id| id != node);
        if let Some(index) = b.rows.iter().position(|r| r.nodes.contains(&node)) {
            let r = &mut b.rows[index];
            r.nodes.retain(|&id| id != node);
            let mut width = 0.0;
            let mut height = 0.0_f64;
            for &id in &r.nodes {
                let rect = &mut self.drawing.nodes[id];
                rect.x = r.x + width;
                rect.y = r.y;
                width += rect.width + self.options.spacing;
                height = height.max(rect.height + self.options.spacing);
            }
            r.width = width - self.options.spacing;
            r.height = height - self.options.spacing;
            if r.nodes.is_empty() {
                b.rows.remove(index);
            }
        }
        self.reset_block(block)
    }

    fn reset_block(&mut self, block: usize) -> Result<(), Error> {
        let b = &mut self.drawing.blocks[block];
        charge(self.work, b.nodes.len())?;
        b.width = 0.0;
        b.height = 0.0;
        for (index, row) in b.rows.iter().enumerate() {
            if !row.nodes.is_empty() {
                b.width = b.width.max(row.width);
                b.height += row.height + if index > 0 { self.options.spacing } else { 0.0 };
            }
        }
        b.rows.retain(|r| !r.nodes.is_empty());
        b.min_width = 0.0;
        b.min_height = 0.0;
        b.max_height = 0.0;
        b.smallest_height = f64::INFINITY;
        for &id in &b.nodes {
            let rect = self.drawing.nodes[id];
            b.min_width = b.min_width.max(rect.width);
            b.min_height = b.min_height.max(rect.height);
            b.smallest_height = b.smallest_height.min(rect.height);
            b.max_height += rect.height + self.options.spacing;
        }
        // ELK retains the trailing spacing in maxHeight after removal/reset.
        b.average_height = b.max_height / b.nodes.len() as f64
            - self.options.spacing * (b.nodes.len() as f64 - 1.0) / b.nodes.len() as f64;
        let parent = b.parent;
        self.notify_row(parent)
    }

    fn move_block(&mut self, block: usize, x: f64, y: f64) -> Result<(), Error> {
        let b = &mut self.drawing.blocks[block];
        charge(self.work, b.nodes.len())?;
        let dx = x - b.x;
        let dy = y - b.y;
        for &id in &b.nodes {
            self.drawing.nodes[id].x += dx;
            self.drawing.nodes[id].y += dy;
        }
        for row in &mut b.rows {
            row.x += dx;
            row.y += dy;
        }
        b.x = x;
        b.y = y;
        Ok(())
    }

    fn block_bounds(&mut self, block: usize, width: f64, place: bool) -> Result<(f64, f64), Error> {
        let b = &mut self.drawing.blocks[block];
        charge(self.work, b.nodes.len())?;
        let mut current_x = 0.0;
        let mut current_y = b.y;
        let mut current_width = 0.0_f64;
        let mut current_height = 0.0;
        let mut max_height_in_row = 0.0_f64;
        let mut width_in_row = 0.0;
        let mut index = 0;
        if place {
            b.rows.clear();
            b.rows.push(Subrow {
                x: b.x,
                y: b.y,
                ..Default::default()
            });
        }
        for &id in &b.nodes {
            let rect = &mut self.drawing.nodes[id];
            if current_x + rect.width + if index > 0 { self.options.spacing } else { 0.0 } > width
                && max_height_in_row > 0.0
            {
                current_x = 0.0;
                current_y += max_height_in_row + self.options.spacing;
                current_width = current_width.max(width_in_row);
                current_height += max_height_in_row + self.options.spacing;
                max_height_in_row = 0.0;
                width_in_row = 0.0;
                if place {
                    b.rows.push(Subrow {
                        x: b.x,
                        y: current_y,
                        ..Default::default()
                    });
                }
                index = 0;
            }
            let spacing = if index > 0 { self.options.spacing } else { 0.0 };
            width_in_row += rect.width + spacing;
            max_height_in_row = max_height_in_row.max(rect.height);
            if place {
                let row = b.rows.last_mut().ok_or(Error::NonFiniteGeometry)?;
                rect.x = row.x + row.width + spacing;
                rect.y = row.y;
                row.width += rect.width + spacing;
                row.height = row.height.max(rect.height);
                row.nodes.push(id);
            }
            current_x += rect.width + spacing;
            index += 1;
        }
        current_width = current_width.max(width_in_row);
        current_height += max_height_in_row;
        if !current_width.is_finite() || !current_height.is_finite() {
            return Err(Error::NonFiniteGeometry);
        }
        if place {
            b.width = current_width;
            b.height = current_height;
            let parent = b.parent;
            self.notify_row(parent)?;
        }
        Ok((current_width, current_height))
    }

    fn block_width_for_height(&mut self, block: usize, height: f64) -> Result<f64, Error> {
        let min = self.drawing.blocks[block].min_width;
        if self.drawing.blocks[block].max_height <= height {
            return Ok(min);
        }
        let (width, actual_height) = self.block_bounds(block, min, false)?;
        if width <= min && actual_height <= height {
            return Ok(min);
        }
        let mut upper = self.drawing.blocks[block].width;
        let mut lower = min;
        let mut viable = upper;
        while lower + 1.0 < upper {
            charge(self.work, 1)?;
            let middle = lower + (upper - lower) / 2.0;
            if middle == lower || middle == upper || !middle.is_finite() {
                return Err(Error::NumericStagnation);
            }
            let (width, actual_height) = self.block_bounds(block, middle, false)?;
            if width <= middle && actual_height <= height {
                viable = middle;
                upper = middle;
            } else {
                lower = middle;
            }
        }
        Ok(viable)
    }

    fn update_stack(&mut self, stack: usize) -> Result<(), Error> {
        let s = &mut self.drawing.stacks[stack];
        charge(self.work, s.blocks.len())?;
        s.width = 0.0;
        s.height = 0.0;
        for (index, &block) in s.blocks.iter().enumerate() {
            s.width = s.width.max(self.drawing.blocks[block].width);
            s.height += self.drawing.blocks[block].height
                + if index > 0 { self.options.spacing } else { 0.0 };
        }
        Ok(())
    }

    fn add_to_stack(&mut self, stack: usize, block: usize) {
        let s = &mut self.drawing.stacks[stack];
        s.width = s.width.max(self.drawing.blocks[block].width);
        s.height += self.drawing.blocks[block].height
            + if s.blocks.is_empty() {
                0.0
            } else {
                self.options.spacing
            };
        s.blocks.push(block);
        self.drawing.blocks[block].stack = Some(stack);
    }

    fn move_stack(&mut self, stack: usize, x: f64, y: f64) -> Result<(), Error> {
        let dx = x - self.drawing.stacks[stack].x;
        let dy = y - self.drawing.stacks[stack].y;
        let count = self.drawing.stacks[stack].blocks.len();
        charge(self.work, count)?;
        for index in 0..count {
            let block = self.drawing.stacks[stack].blocks[index];
            self.move_block(
                block,
                self.drawing.blocks[block].x + dx,
                self.drawing.blocks[block].y + dy,
            )?;
        }
        self.drawing.stacks[stack].x = x;
        self.drawing.stacks[stack].y = y;
        Ok(())
    }

    fn move_row(&mut self, row: usize, y: f64) -> Result<(), Error> {
        let dy = y - self.drawing.rows[row].y;
        let count = self.drawing.rows[row].stacks.len();
        charge(self.work, count)?;
        for index in 0..count {
            let stack = self.drawing.rows[row].stacks[index];
            self.move_stack(
                stack,
                self.drawing.stacks[stack].x,
                self.drawing.stacks[stack].y + dy,
            )?;
        }
        self.drawing.rows[row].y = y;
        Ok(())
    }

    fn stack_width_for_height(&mut self, stack: usize, height: f64) -> Result<f64, Error> {
        let count = self.drawing.stacks[stack].blocks.len();
        if count == 1 {
            return self.block_width_for_height(self.drawing.stacks[stack].blocks[0], height);
        }
        charge(self.work, count)?;
        let mut lower = self.drawing.stacks[stack]
            .blocks
            .iter()
            .map(|&id| self.drawing.blocks[id].min_width)
            .fold(0.0_f64, f64::max);
        let mut upper = self.drawing.stacks[stack].width;
        let mut viable = upper;
        while lower + 1.0 < upper {
            charge(self.work, count)?;
            let middle = lower + (upper - lower) / 2.0;
            if middle == lower || middle == upper || !middle.is_finite() {
                return Err(Error::NumericStagnation);
            }
            let mut total = 0.0;
            for index in 0..count {
                total += self
                    .block_bounds(self.drawing.stacks[stack].blocks[index], middle, false)?
                    .1;
            }
            // Source omits between-block spacing here and uses a strict height comparison.
            if total < height {
                viable = middle;
                upper = middle;
            } else {
                lower = middle;
            }
        }
        Ok(viable)
    }

    fn place_stack(&mut self, stack: usize, width: f64) -> Result<(), Error> {
        let mut y = self.drawing.stacks[stack].y;
        let x = self.drawing.stacks[stack].x;
        let count = self.drawing.stacks[stack].blocks.len();
        let mut max_width = 0.0_f64;
        charge(self.work, count)?;
        for index in 0..count {
            let block = self.drawing.stacks[stack].blocks[index];
            self.move_block(block, x, y)?;
            self.block_bounds(block, width, true)?;
            max_width = max_width.max(self.drawing.blocks[block].width);
            y += self.drawing.blocks[block].height + self.options.spacing;
        }
        self.drawing.stacks[stack].width = max_width;
        self.drawing.stacks[stack].height = y;
        Ok(())
    }

    fn stack_of(&self, block: usize) -> Result<usize, Error> {
        self.drawing.blocks[block]
            .stack
            .ok_or(Error::NonFiniteGeometry)
    }

    fn use_row_height(&mut self, row: usize, block: usize) -> Result<bool, Error> {
        let stack = self.stack_of(block)?;
        let previous = self.drawing.stacks[stack].width;
        if self.drawing.blocks[block].height < self.drawing.rows[row].height {
            let target = self.stack_width_for_height(stack, self.drawing.rows[row].height)?;
            if previous > target {
                self.place_stack(stack, target)?;
                return Ok(previous != self.drawing.stacks[stack].width);
            }
        }
        Ok(false)
    }

    fn place_child_in_block(
        &mut self,
        row: usize,
        block: usize,
        node: usize,
        target: f64,
    ) -> Result<bool, Error> {
        if !self.similar_height(block, node) {
            return Ok(false);
        }
        let b = &self.drawing.blocks[block];
        let r = &self.drawing.rows[row];
        let rect = self.drawing.nodes[node];
        let last = b.rows.last().ok_or(Error::NonFiniteGeometry)?;
        if last.x + last.width + rect.width + self.options.spacing <= target
            && (last.y - r.y + rect.height <= r.height || r.blocks.len() == 1)
        {
            self.add_child(block, node, false)?;
            return Ok(true);
        }
        if b.x + rect.width <= target
            && (b.y + b.height + rect.height + self.options.spacing <= r.height
                || r.blocks.len() == 1)
        {
            self.add_child(block, node, true)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn absorb(
        &mut self,
        row: usize,
        block: usize,
        next: usize,
        target: f64,
    ) -> Result<bool, Error> {
        let mut changed = false;
        while let Some(&node) = self.drawing.blocks[next].nodes.first() {
            charge(self.work, 1)?;
            if !self.place_child_in_block(row, block, node, target)? {
                break;
            }
            changed = true;
            self.remove_child(next, node)?;
        }
        if self.drawing.blocks[next].nodes.is_empty() {
            self.remove_block(self.drawing.blocks[next].parent, next)?;
        }
        if changed {
            self.update_stack(self.stack_of(block)?)?;
        }
        Ok(changed)
    }

    fn remove_from_next_row(&mut self, position: usize, block: usize) -> Result<(), Error> {
        if let Some(&row) = self.drawing.order.get(position + 1) {
            self.remove_block(row, block)?;
            if self.drawing.rows[row].blocks.is_empty() {
                self.drawing.order.remove(position + 1);
            }
        }
        Ok(())
    }

    fn place_below(
        &mut self,
        position: usize,
        block: usize,
        next: usize,
        from_next: bool,
        target: f64,
    ) -> Result<bool, Error> {
        let row = self.drawing.order[position];
        let remaining = target - self.drawing.blocks[block].x;
        let current_min = self.drawing.blocks[block].y - self.drawing.rows[row].y
            + self.block_bounds(block, remaining, false)?.1;
        if self.drawing.blocks[next].min_width + self.options.spacing > remaining {
            return Ok(false);
        }
        let next_min = self.block_bounds(next, remaining, false)?.1;
        if current_min + self.options.spacing + next_min > self.drawing.rows[row].height {
            return Ok(false);
        }
        self.block_bounds(block, remaining, true)?;
        self.drawing.blocks[block].fixed = true;
        self.block_bounds(next, remaining, true)?;
        self.move_block(
            next,
            self.drawing.blocks[block].x,
            self.drawing.blocks[block].y + self.drawing.blocks[block].height + self.options.spacing,
        )?;
        self.drawing.blocks[next].position_fixed = true;
        self.add_to_stack(self.stack_of(block)?, next);
        if from_next {
            self.add_block(row, next);
            self.drawing.blocks[next].parent = row;
            self.remove_from_next_row(position, next)?;
        }
        Ok(true)
    }

    fn place_beside(
        &mut self,
        position: usize,
        block: usize,
        next: usize,
        target: f64,
    ) -> Result<bool, Error> {
        let row = self.drawing.order[position];
        let stack = self.stack_of(block)?;
        let current_min = self.stack_width_for_height(
            stack,
            self.drawing.rows[row].y + self.drawing.rows[row].height - self.drawing.stacks[stack].y,
        )?;
        let reevaluate = self.drawing.blocks[next].min_height > self.drawing.rows[row].height
            && self.options.row_height_reevaluation;
        let mut next_target =
            target - (self.drawing.stacks[stack].x + current_min - self.options.spacing);
        let next_height = self.block_bounds(next, next_target, false)?.1;
        if reevaluate && next_height > self.drawing.blocks[next].min_height {
            return Ok(false);
        }
        if reevaluate {
            let count = self.drawing.rows[row].stacks.len();
            let mut potential = 0.0;
            charge(self.work, count)?;
            for index in 0..count {
                potential += self.stack_width_for_height(
                    self.drawing.rows[row].stacks[index],
                    self.drawing.blocks[next].min_height,
                )? + self.options.spacing;
            }
            next_target = target - potential;
        }
        if next_target < self.drawing.blocks[next].min_width {
            return Ok(false);
        }
        let last_row = position + 1 == self.drawing.order.len() - 1
            && next_target >= self.drawing.rows[self.drawing.order[position + 1]].width;
        if !reevaluate && next_height > self.drawing.rows[row].height && !last_row {
            return Ok(false);
        }
        if last_row && next_height > self.drawing.rows[row].height {
            self.drawing.blocks[block].height = next_height;
            let width = self.block_width_for_height(block, next_height)?;
            self.block_bounds(block, width, true)?;
        } else {
            self.place_stack(stack, current_min)?;
            self.drawing.blocks[block].fixed = true;
        }
        self.block_bounds(
            next,
            target - (self.drawing.blocks[block].x + self.drawing.blocks[block].width),
            true,
        )?;
        self.move_block(
            next,
            self.drawing.stacks[stack].x + self.drawing.stacks[stack].width,
            self.drawing.rows[row].y,
        )?;
        self.add_block(row, next);
        self.remove_from_next_row(position, next)?;
        Ok(true)
    }

    fn compact(&mut self, position: usize, target: f64) -> Result<bool, Error> {
        let row = self.drawing.order[position];
        let mut changed = false;
        let mut current_stack: Option<usize> = None;
        let mut index = 0;
        while index < self.drawing.rows[row].blocks.len() {
            charge(self.work, 1)?;
            let block = self.drawing.rows[row].blocks[index];
            if self.drawing.blocks[block].fixed {
                index += 1;
                continue;
            }
            if self.drawing.blocks[block].nodes.is_empty() {
                self.remove_block(row, block)?;
                changed = true;
                continue;
            }
            if !self.drawing.blocks[block].position_fixed {
                if let Some(stack) = current_stack {
                    self.update_stack(stack)?;
                }
                let x = current_stack.map_or(0.0, |id| {
                    self.drawing.stacks[id].x + self.drawing.stacks[id].width + self.options.spacing
                });
                let y = self.drawing.rows[row].y;
                charge(self.work, 1)?;
                let stack = self.drawing.stacks.len();
                self.drawing.stacks.push(Stack {
                    x,
                    y,
                    ..Default::default()
                });
                self.move_block(block, x, y)?;
                self.drawing.rows[row].stacks.push(stack);
                self.add_to_stack(stack, block);
                self.drawing.blocks[block].position_fixed = true;
                current_stack = Some(stack);
            }
            let next = self.drawing.rows[row]
                .blocks
                .get(index + 1)
                .copied()
                .or_else(|| {
                    self.drawing
                        .order
                        .get(position + 1)
                        .and_then(|&id| self.drawing.rows[id].blocks.first().copied())
                });
            if let Some(next) = next {
                let from_next = self.drawing.blocks[next].parent != row;
                if self.drawing.blocks[next].nodes.is_empty() {
                    self.remove_block(row, next)?;
                    break;
                }
                self.block_bounds(block, target - self.drawing.blocks[block].x, true)?;
                self.update_stack(self.stack_of(block)?)?;
                changed |= self.absorb(row, block, next, target)?;
                if self.drawing.blocks[next].nodes.is_empty() {
                    if let Some(&next_row) = self.drawing.order.get(position + 1) {
                        self.remove_block(next_row, next)?;
                    }
                    while let Some(&next_row) = self.drawing.order.get(position + 1) {
                        if !self.drawing.rows[next_row].blocks.is_empty() {
                            break;
                        }
                        self.drawing.order.remove(position + 1);
                    }
                    continue;
                }
                if self.place_below(position, block, next, from_next, target)? {
                    changed = true;
                    index += 1;
                    continue;
                }
                if from_next {
                    let old_height = self.drawing.rows[row].height;
                    let next_min_height = self.drawing.blocks[next].min_height;
                    if self.place_beside(position, block, next, target)? {
                        changed = true;
                        if old_height < next_min_height {
                            self.drawing.blocks[next].parent = row;
                            return Ok(true);
                        }
                        index += 1;
                        continue;
                    }
                }
                if self.use_row_height(row, block)? {
                    self.drawing.blocks[block].fixed = true;
                    changed = true;
                    index += 1;
                    continue;
                }
                if changed {
                    index += 1;
                    continue;
                }
            }
            if self.use_row_height(row, block)? {
                self.drawing.blocks[block].fixed = true;
                changed = true;
                if let Some(next) = next {
                    self.drawing.blocks[next].position_fixed = false;
                }
            } else {
                self.update_stack(self.stack_of(block)?)?;
            }
            index += 1;
        }
        Ok(false)
    }

    fn adjust_row_dimensions(&mut self, row: usize) -> Result<(), Error> {
        let mut height = 0.0_f64;
        let mut width = 0.0;
        let count = self.drawing.rows[row].stacks.len();
        charge(self.work, count)?;
        for index in 0..count {
            let stack = self.drawing.rows[row].stacks[index];
            self.update_stack(stack)?;
            height = height.max(self.drawing.stacks[stack].height);
            width += self.drawing.stacks[stack].width
                + if index > 0 { self.options.spacing } else { 0.0 };
        }
        self.drawing.rows[row].height = height;
        self.drawing.rows[row].width = width;
        Ok(())
    }

    fn update_changes(&mut self, position: usize, target: f64) -> Result<(), Error> {
        let row = self.drawing.order[position];
        if let Some(&next) = self.drawing.order.get(position + 1) {
            let first = *self.drawing.rows[next]
                .blocks
                .first()
                .ok_or(Error::NonFiniteGeometry)?;
            let increase = self.drawing.rows[row].width
                + self.options.spacing
                + self.drawing.blocks[first].width
                - target;
            let changes = &mut self.drawing.changes;
            // Source mixes increase and decrease extrema here; keep exact 0.9.1 behavior.
            changes.increase_max = increase.max(changes.decrease_max);
            changes.increase_min = increase.min(changes.decrease_min);
            if let Some(&last) = self.drawing.rows[row].stacks.last() {
                let decrease = self.drawing.stacks[last].width
                    + if self.drawing.rows[row].stacks.len() <= 1 {
                        0.0
                    } else {
                        self.options.spacing
                    };
                changes.decrease_max = changes.decrease_max.max(decrease);
                changes.decrease_min = changes.decrease_max.min(decrease);
            }
        }
        if self.drawing.order.len() == 1 {
            let stack = *self.drawing.rows[row]
                .stacks
                .last()
                .ok_or(Error::NonFiniteGeometry)?;
            let block = *self.drawing.stacks[stack]
                .blocks
                .last()
                .ok_or(Error::NonFiniteGeometry)?;
            let b = &self.drawing.blocks[block];
            charge(self.work, b.rows.len())?;
            for subrow in &b.rows {
                let changes = &mut self.drawing.changes;
                changes.decrease_max = changes.decrease_max.max(b.width - subrow.width);
                changes.decrease_min = changes.decrease_min.min(b.width - subrow.width);
                changes.increase_max = changes
                    .increase_max
                    .max(subrow.width + self.options.spacing);
                changes.increase_min = changes
                    .increase_min
                    .min(subrow.width + self.options.spacing);
            }
        }
        Ok(())
    }
}

pub(super) fn expand(
    drawing: &mut Drawing,
    width: f64,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let height_per_row = drawing.additional_height / drawing.order.len() as f64;
    let mut context = Context {
        drawing,
        options,
        work,
    };
    for row_index in 0..context.drawing.order.len() {
        charge(context.work, 1)?;
        let row = context.drawing.order[row_index];
        context.move_row(
            row,
            context.drawing.rows[row].y + height_per_row * row_index as f64,
        )?;
        let stack_count = context.drawing.rows[row].stacks.len();
        let extra_width = (width - context.drawing.rows[row].width) / stack_count as f64;
        for stack_index in 0..stack_count {
            charge(context.work, 1)?;
            let stack = context.drawing.rows[row].stacks[stack_index];
            let extra_height = context.drawing.rows[row].height
                - context.drawing.stacks[stack].height
                + height_per_row;
            context.move_stack(
                stack,
                context.drawing.stacks[stack].x + stack_index as f64 * extra_width,
                context.drawing.stacks[stack].y,
            )?;
            let block_count = context.drawing.stacks[stack].blocks.len();
            let height_per_block = extra_height / block_count as f64;
            for block_index in 0..block_count {
                charge(context.work, 1)?;
                let block = context.drawing.stacks[stack].blocks[block_index];
                let x = context.drawing.blocks[block].x;
                let y = context.drawing.blocks[block].y + block_index as f64 * height_per_block;
                context.move_block(block, x, y)?;
                let width_for_subrow = context.drawing.stacks[stack].width + extra_width;
                let b = &mut context.drawing.blocks[block];
                b.width = width_for_subrow;
                b.height += height_per_block;
                let height_per_subrow = height_per_block / b.rows.len() as f64;
                charge(context.work, b.nodes.len())?;
                for (subrow_index, subrow) in b.rows.iter_mut().enumerate() {
                    let width_per_rect =
                        (width_for_subrow - subrow.width) / subrow.nodes.len() as f64;
                    subrow.height += height_per_subrow;
                    subrow.width = width_for_subrow;
                    for (node_index, &id) in subrow.nodes.iter().enumerate() {
                        let node = &mut context.drawing.nodes[id];
                        node.x += node_index as f64 * width_per_rect;
                        node.y += subrow_index as f64 * height_per_subrow;
                        node.width += width_per_rect;
                        node.height = subrow.height;
                    }
                }
            }
        }
    }
    Ok(())
}
