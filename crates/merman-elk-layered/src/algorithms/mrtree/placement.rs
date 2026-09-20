//! Source DFS treeification and Walker's two walks, with explicit stacks instead of recursion.

use super::*;

pub(super) fn place(
    component: &mut Component,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let count = component.nodes.len();
    work.charge(checked_add(
        checked_mul(count, 6)?,
        checked_n_log_n(component.edges.len())?,
    )?)?;
    let mut outgoing = vec![Vec::new(); count];
    let mut edge_indices: Vec<_> = (0..component.edges.len()).collect();
    edge_indices.sort_by_key(|&i| component.edges[i].input);
    for &i in &edge_indices {
        outgoing[component.edges[i].source].push(i);
    }
    let mut visited = vec![0u8; count];
    let mut removed = vec![false; component.edges.len()];
    for start in 0..count {
        if visited[start] != 0 {
            continue;
        }
        visited[start] = 1;
        let mut stack = vec![(start, 0)];
        while let Some((n, cursor)) = stack.pop() {
            work.charge(1)?;
            if cursor == outgoing[n].len() {
                continue;
            }
            stack.push((n, cursor + 1));
            let e = outgoing[n][cursor];
            let target = component.edges[e].target;
            match visited[target] {
                1 => removed[e] = true,
                2 => visited[target] = 1,
                _ => {
                    visited[target] = 1;
                    stack.push((target, 0));
                }
            }
        }
        visited[start] = 2;
    }
    for n in 0..count {
        for &e in &outgoing[n] {
            if !removed[e] {
                let target = component.edges[e].target;
                component.nodes[n].children.push(target);
                component.nodes[target].parent = Some(n);
            }
        }
    }
    let roots: Vec<_> = (0..count)
        .filter(|&n| component.nodes[n].parent.is_none())
        .collect();
    let root = if roots.len() == 1 {
        roots[0]
    } else {
        let dummy = component.nodes.len();
        for &child in &roots {
            component.nodes[child].parent = Some(dummy);
            component.edge_order.push(component.edges.len());
            component.edges.push(TreeEdge {
                input: None,
                source: dummy,
                target: child,
                points: Vec::new(),
            });
        }
        component.nodes.push(TreeNode {
            input: None,
            id: 0,
            identity: "n_SUPER_ROOT".into(),
            super_root: true,
            width: 0.0,
            height: 0.0,
            position: Point::default(),
            parent: None,
            children: roots,
            left_sibling: None,
            left_neighbor: None,
            level: 0,
            level_height: 0.0,
            level_min: 0.0,
            level_max: 0.0,
            prelim: 0.0,
            modifier: 0.0,
        });
        component.priority = component.priority.wrapping_add(1);
        dummy
    };
    set_levels_and_neighbors(component, root, options.direction, work)?;
    first_walk(component, root, options, work)?;
    second_walk(component, root, options, work)?;
    level_coordinates_and_bounds(component, options.direction, work)?;
    Ok(())
}

fn set_levels_and_neighbors(
    c: &mut Component,
    root: usize,
    d: Direction,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut level = vec![root];
    let mut depth = 0;
    while !level.is_empty() {
        work.charge(checked_mul(level.len(), 2)?)?;
        let height = level
            .iter()
            .map(|&n| {
                if d.horizontal() {
                    c.nodes[n].width
                } else {
                    c.nodes[n].height
                }
            })
            .fold(0.0, f64::max);
        let mut next = Vec::new();
        let mut previous = None;
        for n in level {
            c.nodes[n].level = depth;
            c.nodes[n].level_height = height;
            c.nodes[n].left_neighbor = previous;
            if let Some(p) = previous {
                if c.nodes[n].parent == c.nodes[p].parent {
                    c.nodes[n].left_sibling = Some(p);
                }
            }
            previous = Some(n);
            next.extend_from_slice(&c.nodes[n].children);
        }
        depth += 1;
        level = next;
    }
    // RootProcessor gives the synthetic root id 0. LevelProcessor keys its map by id,
    // so the real first node's later assignment also becomes the dummy's level.
    if root >= c.nodes.len() - 1 && c.nodes[root].input.is_none() {
        c.nodes[root].level = c.nodes[0].level;
    }
    Ok(())
}

fn mean_width(c: &Component, a: usize, b: usize, d: Direction) -> f64 {
    if d.horizontal() {
        c.nodes[a].height / 2.0 + c.nodes[b].height / 2.0
    } else {
        c.nodes[a].width / 2.0 + c.nodes[b].width / 2.0
    }
}

fn first_walk(
    c: &mut Component,
    root: usize,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut stack = vec![(root, false)];
    while let Some((n, after)) = stack.pop() {
        work.charge(1)?;
        if !after {
            c.nodes[n].modifier = 0.0;
            stack.push((n, true));
            for &child in c.nodes[n].children.iter().rev() {
                stack.push((child, false));
            }
            continue;
        }
        let left = c.nodes[n].left_sibling;
        let first = c.nodes[n].children.first().copied();
        if let Some(first) = first {
            let last = *c.nodes[n].children.last().ok_or(Error::NumericRange)?;
            let midpoint = (c.nodes[first].prelim + c.nodes[last].prelim) / 2.0;
            if let Some(left) = left {
                c.nodes[n].prelim = c.nodes[left].prelim
                    + options.spacing
                    + mean_width(c, left, n, options.direction);
                c.nodes[n].modifier = c.nodes[n].prelim - midpoint;
                apportion(c, n, options, work)?;
            } else {
                c.nodes[n].prelim = midpoint;
            }
        } else if let Some(left) = left {
            c.nodes[n].prelim =
                c.nodes[left].prelim + options.spacing + mean_width(c, left, n, options.direction);
        } else {
            c.nodes[n].prelim = 0.0;
        }
    }
    Ok(())
}

fn leftmost_at_depth(
    c: &Component,
    node: usize,
    depth: usize,
    work: &mut dyn WorkControl,
) -> Result<Option<usize>, Error> {
    let mut stack = vec![(node, 0)];
    while let Some((n, d)) = stack.pop() {
        work.charge(1)?;
        if d == depth {
            return Ok(Some(n));
        }
        for &child in c.nodes[n].children.iter().rev() {
            stack.push((child, d + 1));
        }
    }
    Ok(None)
}

fn apportion(
    c: &mut Component,
    node: usize,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut leftmost = c.nodes[node].children.first().copied();
    let mut depth = 1;
    while let Some(left) = leftmost {
        let Some(neighbor) = c.nodes[left].left_neighbor else {
            break;
        };
        work.charge(checked_add(depth, 1)?)?;
        let mut left_ancestor = left;
        let mut neighbor_ancestor = neighbor;
        let mut left_mod = 0.0;
        let mut right_mod = 0.0;
        for _ in 0..depth {
            left_ancestor = c.nodes[left_ancestor].parent.ok_or(Error::NumericRange)?;
            neighbor_ancestor = c.nodes[neighbor_ancestor]
                .parent
                .ok_or(Error::NumericRange)?;
            right_mod += c.nodes[left_ancestor].modifier;
            left_mod += c.nodes[neighbor_ancestor].modifier;
        }
        let mut distance = c.nodes[neighbor].prelim
            + left_mod
            + options.spacing
            + mean_width(c, left, neighbor, options.direction)
            - c.nodes[left].prelim
            - right_mod;
        if distance > 0.0 {
            let mut sibling = Some(node);
            let mut count = 0;
            while sibling.is_some() && sibling != Some(neighbor_ancestor) {
                work.charge(1)?;
                count += 1;
                sibling = c.nodes[sibling.ok_or(Error::NumericRange)?].left_sibling;
            }
            if sibling.is_none() {
                return Ok(());
            }
            let portion = distance / count as f64;
            sibling = Some(node);
            while sibling != Some(neighbor_ancestor) {
                work.charge(1)?;
                let n = sibling.ok_or(Error::NumericRange)?;
                c.nodes[n].prelim += distance;
                c.nodes[n].modifier += distance;
                distance -= portion;
                sibling = c.nodes[n].left_sibling;
            }
        }
        depth += 1;
        leftmost = if c.nodes[left].children.is_empty() {
            leftmost_at_depth(c, node, depth, work)?
        } else {
            c.nodes[left].children.first().copied()
        };
    }
    Ok(())
}

fn source_round(value: f64) -> Result<f64, Error> {
    let rounded = (value + 0.5).floor();
    // ELK stores Java int coordinates. Reject overflow instead of producing wrapped geometry.
    if !rounded.is_finite() || rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(Error::NumericRange);
    }
    Ok(rounded)
}

fn second_walk(
    c: &mut Component,
    root: usize,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut stack = vec![(root, -c.nodes[root].level_height / 2.0, 0.0)];
    while let Some((n, previous_y, modifier)) = stack.pop() {
        work.charge(1)?;
        let x = source_round(c.nodes[n].prelim + modifier)?;
        let y = source_round(previous_y + c.nodes[n].level_height / 2.0)?;
        let point = match options.direction {
            Direction::Down => Point { x, y },
            Direction::Up => Point { x, y: -y },
            Direction::Right => Point { x: y, y: x },
            Direction::Left => Point { x: -y, y: x },
        };
        c.nodes[n].position = Point {
            x: point.x - c.nodes[n].width / 2.0,
            y: point.y - c.nodes[n].height / 2.0,
        };
        for &child in c.nodes[n].children.iter().rev() {
            stack.push((
                child,
                previous_y + c.nodes[n].level_height + options.spacing,
                modifier + c.nodes[n].modifier,
            ));
        }
    }
    Ok(())
}

fn level_coordinates_and_bounds(
    c: &mut Component,
    d: Direction,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    work.charge(checked_mul(c.nodes.len(), 2)?)?;
    let mut levels = vec![(f64::MAX, -f64::MAX); c.nodes.len()];
    c.min = Point {
        x: f64::INFINITY,
        y: f64::INFINITY,
    };
    c.max = Point {
        x: f64::NEG_INFINITY,
        y: f64::NEG_INFINITY,
    };
    for n in &c.nodes {
        let start = d.along(n.position);
        let end = start + d.along(n.size());
        levels[n.level].0 = levels[n.level].0.min(start);
        levels[n.level].1 = levels[n.level].1.max(end);
        c.min.x = c.min.x.min(n.position.x);
        c.min.y = c.min.y.min(n.position.y);
        c.max.x = c.max.x.max(n.position.x + n.width);
        c.max.y = c.max.y.max(n.position.y + n.height);
    }
    for n in &mut c.nodes {
        (n.level_min, n.level_max) = levels[n.level];
    }
    Ok(())
}
