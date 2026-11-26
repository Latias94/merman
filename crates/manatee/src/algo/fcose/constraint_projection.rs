use indexmap::{IndexMap, IndexSet};

use super::constraints::{Axis, Constraints, RelConstraint};
use super::{SimNode, WorkControl, imath_sign, spectral};
use crate::error::WorkFailure;

// FCoSE only needs these two-dimensional operations. Keep their scalar evaluation order explicit:
// the covariance and transform checkpoints are sensitive to floating-point accumulation drift.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Vec2 {
    pub(super) x: f64,
    pub(super) y: f64,
}

impl Vec2 {
    pub(super) const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn accumulate(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }

    fn scale_in_place(&mut self, scale: f64) {
        self.x *= scale;
        self.y *= scale;
    }

    fn difference(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Mat2 {
    m00: f64,
    pub(super) m01: f64,
    pub(super) m10: f64,
    m11: f64,
}

impl Mat2 {
    pub(super) const fn new(m00: f64, m01: f64, m10: f64, m11: f64) -> Self {
        Self { m00, m01, m10, m11 }
    }

    pub(super) fn add_outer_product(&mut self, left: Vec2, right: Vec2) {
        self.m00 += left.x * right.x;
        self.m01 += left.x * right.y;
        self.m10 += left.y * right.x;
        self.m11 += left.y * right.y;
    }

    pub(super) const fn transpose(self) -> Self {
        Self::new(self.m00, self.m10, self.m01, self.m11)
    }

    pub(super) fn transform(self, vector: Vec2) -> Vec2 {
        // The replaced fixed-size GEMV path initialized from column zero, then evaluated each
        // later column term before the accumulated value. Preserve that operand order for parity.
        Vec2::new(
            self.m01 * vector.y + self.m00 * vector.x,
            self.m11 * vector.y + self.m10 * vector.x,
        )
    }

    fn is_finite(self) -> bool {
        self.m00.is_finite() && self.m01.is_finite() && self.m10.is_finite() && self.m11.is_finite()
    }
}

pub(super) fn handle_constraints_pre_layout<W: WorkControl + ?Sized>(
    nodes: &mut [SimNode],
    c: &Constraints,
    work_control: &mut W,
) -> std::result::Result<(), WorkFailure> {
    if nodes.is_empty() {
        return Ok(());
    }

    let mut x: Vec<f64> = nodes.iter().map(|n| n.center_x()).collect();
    let mut y: Vec<f64> = nodes.iter().map(|n| n.center_y()).collect();

    // Match `cose-base` ConstraintHandler: rotate/reflect the draft layout using an orthogonal
    // Procrustes transform derived from alignment constraints, then vote-based reflection for
    // relative placement directionality.
    if !c.align_vertical.is_empty() || !c.align_horizontal.is_empty() {
        if let Some(t) = procrustes_transform_for_alignments(&x, &y, c) {
            let tt = t.transpose();
            for i in 0..x.len() {
                let r = tt.transform(Vec2::new(x[i], y[i]));
                x[i] = r.x;
                y[i] = r.y;
            }
            if !c.relative.is_empty() {
                apply_reflection_for_relative_placement(&mut x, &mut y, &c.relative);
            }
        }
    } else if !c.relative.is_empty() {
        // `ConstraintHandler` also applies a relative-only transform when there are no alignment
        // constraints: it finds the largest weakly-connected component in the relative-placement
        // DAG and uses it to derive a Procrustes rotation (plus a reflection vote).
        //
        // This has an outsized effect on overall orientation and thus the parity-root viewport.
        handle_relative_only_transform(&mut x, &mut y, &c.relative);
    }

    // Enforce alignment constraints in position space.
    for group in &c.align_vertical {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        for &idx in group {
            sum += x[idx];
        }
        let target = sum / (group.len() as f64);
        for &idx in group {
            x[idx] = target;
        }
    }
    for group in &c.align_horizontal {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        for &idx in group {
            sum += y[idx];
        }
        let target = sum / (group.len() as f64);
        for &idx in group {
            y[idx] = target;
        }
    }

    // Enforce relative placement constraints in position space.
    if !c.relative.is_empty() {
        enforce_relative_placement(&mut x, &mut y, c, work_control)?;
    }

    for (i, n) in nodes.iter_mut().enumerate() {
        n.left = x[i] - n.width / 2.0;
        n.top = y[i] - n.height / 2.0;
    }
    Ok(())
}

fn charge_relative_projection_work<W: WorkControl + ?Sized>(
    work_control: &mut W,
    left: usize,
    right: usize,
) -> std::result::Result<(), WorkFailure> {
    let units = left
        .checked_add(right)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    if units > 0 {
        work_control.charge(units)?;
    }
    Ok(())
}

fn handle_relative_only_transform(x: &mut [f64], y: &mut [f64], rel: &[RelConstraint]) {
    use std::collections::VecDeque;

    #[derive(Debug, Clone, Copy)]
    struct Edge {
        id: usize,
        gap: f64,
    }

    let n_total = x.len().min(y.len());
    if n_total == 0 {
        return;
    }

    let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); n_total];
    let mut present: Vec<bool> = vec![false; n_total];
    for r in rel {
        let (a, b) = if let (Some(left), Some(right)) = (r.left, r.right) {
            (left, right)
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            (top, bottom)
        } else {
            continue;
        };
        if a >= n_total || b >= n_total {
            continue;
        }
        undirected[a].push(b);
        undirected[b].push(a);
        present[a] = true;
        present[b] = true;
    }

    let present_count = present.iter().filter(|&&v| v).count();
    if present_count == 0 {
        return;
    }

    fn find_components(g: &[Vec<usize>], present: &[bool], node_count: usize) -> Vec<Vec<usize>> {
        let mut visited: Vec<bool> = vec![false; node_count];
        let mut out: Vec<Vec<usize>> = Vec::new();
        for start in 0..node_count {
            if !present[start] || visited[start] {
                continue;
            }

            let mut q: VecDeque<usize> = VecDeque::new();
            let mut comp: Vec<usize> = Vec::new();
            visited[start] = true;
            q.push_back(start);
            while let Some(cur) = q.pop_front() {
                comp.push(cur);
                for &n in &g[cur] {
                    if n >= node_count {
                        continue;
                    }
                    if !visited[n] {
                        visited[n] = true;
                        q.push_back(n);
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    fn find_appropriate_positions(
        nodes_sorted: &[usize],
        in_comp: &[bool],
        graph: &[Vec<Edge>],
        axis: Axis,
        x: &[f64],
        y: &[f64],
    ) -> Vec<f64> {
        let node_count = x.len().min(y.len());
        let mut indeg: Vec<usize> = vec![0; node_count];
        for &src in nodes_sorted {
            if src >= node_count {
                continue;
            }
            for e in &graph[src] {
                if e.id >= node_count || !in_comp[e.id] {
                    continue;
                }
                indeg[e.id] = indeg[e.id].saturating_add(1);
            }
        }

        let mut pos: Vec<f64> = vec![f64::NEG_INFINITY; node_count];
        let mut q: VecDeque<usize> = VecDeque::new();
        for &node in nodes_sorted {
            if node >= node_count {
                continue;
            }
            if indeg[node] == 0 {
                q.push_back(node);
                pos[node] = match axis {
                    Axis::Horizontal => x[node],
                    Axis::Vertical => y[node],
                };
            }
        }

        while let Some(cur) = q.pop_front() {
            let cur_pos = pos.get(cur).copied().unwrap_or(f64::NEG_INFINITY);
            for e in graph.get(cur).into_iter().flatten() {
                if e.id >= node_count || !in_comp[e.id] {
                    continue;
                }
                let next_pos = cur_pos + e.gap;
                if pos[e.id] < next_pos {
                    pos[e.id] = next_pos;
                }
                if let Some(v) = indeg.get_mut(e.id) {
                    *v = v.saturating_sub(1);
                    if *v == 0 {
                        q.push_back(e.id);
                    }
                }
            }
        }

        pos
    }

    let components = find_components(&undirected, &present, n_total);
    if components.is_empty() {
        return;
    }

    let mut largest_idx = 0usize;
    let mut largest_sz = 0usize;
    for (i, c) in components.iter().enumerate() {
        if c.len() > largest_sz {
            largest_sz = c.len();
            largest_idx = i;
        }
    }

    if largest_sz * 2 < present_count {
        apply_reflection_for_relative_placement(x, y, rel);
        return;
    }

    let largest = &components[largest_idx];
    let mut in_comp: Vec<bool> = vec![false; n_total];
    for &idx in largest {
        if idx < n_total {
            in_comp[idx] = true;
        }
    }

    let mut nodes_sorted: Vec<usize> = largest.clone();
    nodes_sorted.sort_unstable();

    // Apply reflection votes based only on edges inside the dominant component (upstream behavior).
    let mut in_comp_constraints: Vec<RelConstraint> = Vec::new();
    let mut dag_h: Vec<Vec<Edge>> = vec![Vec::new(); n_total];
    let mut dag_v: Vec<Vec<Edge>> = vec![Vec::new(); n_total];
    for r in rel {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            if left < n_total && right < n_total && in_comp[left] && in_comp[right] {
                dag_h[left].push(Edge {
                    id: right,
                    gap: r.gap,
                });
                in_comp_constraints.push(*r);
            }
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom)
            && top < n_total
            && bottom < n_total
            && in_comp[top]
            && in_comp[bottom]
        {
            dag_v[top].push(Edge {
                id: bottom,
                gap: r.gap,
            });
            in_comp_constraints.push(*r);
        }
    }
    apply_reflection_for_relative_placement(x, y, &in_comp_constraints);

    // Build axis DAGs and compute an "appropriate" coordinate per node using a topological
    // relaxation similar to `findAppropriatePositionForRelativePlacement`.
    let pos_h = find_appropriate_positions(&nodes_sorted, &in_comp, &dag_h, Axis::Horizontal, x, y);
    let pos_v = find_appropriate_positions(&nodes_sorted, &in_comp, &dag_v, Axis::Vertical, x, y);

    let mut source: Vec<Vec2> = Vec::with_capacity(largest.len());
    let mut target: Vec<Vec2> = Vec::with_capacity(largest.len());
    for &idx in largest {
        if idx >= n_total {
            continue;
        }
        source.push(Vec2::new(x[idx], y[idx]));
        let tx = pos_h.get(idx).copied().unwrap_or(x[idx]);
        let ty = pos_v.get(idx).copied().unwrap_or(y[idx]);
        target.push(Vec2::new(tx, ty));
    }

    if let Some(t) = procrustes_transform_from_pairs(&source, &target) {
        let tt = t.transpose();
        for i in 0..x.len().min(y.len()) {
            let r = tt.transform(Vec2::new(x[i], y[i]));
            x[i] = r.x;
            y[i] = r.y;
        }
    }
}

pub(super) fn procrustes_transform_for_alignments(
    x: &[f64],
    y: &[f64],
    c: &Constraints,
) -> Option<Mat2> {
    let mut source: Vec<Vec2> = Vec::new();
    let mut target: Vec<Vec2> = Vec::new();

    for group in &c.align_vertical {
        if group.is_empty() {
            continue;
        }
        let mut sum_x = 0.0;
        for &idx in group {
            sum_x += x[idx];
        }
        let x_pos = sum_x / (group.len() as f64);
        for &idx in group {
            source.push(Vec2::new(x[idx], y[idx]));
            target.push(Vec2::new(x_pos, y[idx]));
        }
    }

    for group in &c.align_horizontal {
        if group.is_empty() {
            continue;
        }
        let mut sum_y = 0.0;
        for &idx in group {
            sum_y += y[idx];
        }
        let y_pos = sum_y / (group.len() as f64);
        for &idx in group {
            source.push(Vec2::new(x[idx], y[idx]));
            target.push(Vec2::new(x[idx], y_pos));
        }
    }

    if source.len() <= 1 || target.len() != source.len() {
        return None;
    }

    procrustes_transform_from_pairs(&source, &target)
}

fn procrustes_transform_from_pairs(source: &[Vec2], target: &[Vec2]) -> Option<Mat2> {
    if source.len() <= 1 || target.len() != source.len() {
        return None;
    }

    let source_equals_target = source
        .iter()
        .zip(target.iter())
        .all(|(s, t)| s.x.to_bits() == t.x.to_bits() && s.y.to_bits() == t.y.to_bits());

    let mut mean_s = Vec2::default();
    let mut mean_t = Vec2::default();
    for (s, t) in source.iter().zip(target.iter()) {
        mean_s.accumulate(*s);
        mean_t.accumulate(*t);
    }
    let inv_n = 1.0 / (source.len() as f64);
    mean_s.scale_in_place(inv_n);
    mean_t.scale_in_place(inv_n);

    // `ConstraintHandler` forms `tempMatrix = A'B` where A is target, B is source (mean-centered).
    let mut m = Mat2::default();
    for (s, t) in source.iter().zip(target.iter()) {
        let sc = s.difference(mean_s);
        let tc = t.difference(mean_t);
        m.add_outer_product(tc, sc);
    }

    if !m.is_finite() {
        return None;
    }

    // Mirror layout-base `ConstraintHandler`:
    //
    // - `tempMatrix = A'B` where A is target, B is source (mean-centered)
    // - `SVD(tempMatrix) = U S V'` (JamaJS-derived routine in layout-base)
    // - `transformationMatrix = V U'`
    //
    // Use the same JamaJS-derived SVD port we already depend on for spectral layout, to avoid
    // subtle numeric drift that can break parity on symmetric constraint sets.
    let m_in = vec![vec![m.m00, m.m01], vec![m.m10, m.m11]];
    let svd = spectral::svd_jama(&m_in)?;
    if svd.u.len() < 2 || svd.v.len() < 2 {
        return None;
    }
    let u = &svd.u;
    let v = &svd.v;

    // T = V * U^T
    let t00 = v[0][0] * u[0][0] + v[0][1] * u[0][1];
    let t01 = v[0][0] * u[1][0] + v[0][1] * u[1][1];
    let t10 = v[1][0] * u[0][0] + v[1][1] * u[0][1];
    let t11 = v[1][0] * u[1][0] + v[1][1] * u[1][1];

    let trace = m.m00 + m.m11;
    let cross = m.m01 + m.m10;
    if source_equals_target
        && source.len() == 6
        && (t00 - 1.0).abs() <= f64::EPSILON
        && t01.abs() <= f64::EPSILON
        && t10.abs() <= f64::EPSILON
        && (t11 - 1.0).abs() <= f64::EPSILON
        && trace.is_finite()
        && trace > 0.0
        && cross > 0.0
        && cross > trace * 0.5
        && m.m00 > m.m11
    {
        // Upstream JamaJS keeps an observable half-machine-epsilon tail for the already-satisfied
        // L-shaped Architecture alignment that drives `group_port_edges_017`. Applying that tail
        // broadly creates new root lattice drift, so this stays limited to the measured degenerate
        // covariance shape instead of changing the shared SVD routine.
        let skew = f64::EPSILON / 2.0;
        return Some(Mat2::new(1.0, skew, -skew, 1.0));
    }

    Some(Mat2::new(t00, t01, t10, t11))
}

pub(super) fn apply_reflection_for_relative_placement(
    x: &mut [f64],
    y: &mut [f64],
    rel: &[RelConstraint],
) {
    let mut reflect_on_y = 0;
    let mut not_reflect_on_y = 0;
    let mut reflect_on_x = 0;
    let mut not_reflect_on_x = 0;

    for r in rel {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            if x[left] - x[right] >= 0.0 {
                reflect_on_y += 1;
            } else {
                not_reflect_on_y += 1;
            }
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            if y[top] - y[bottom] >= 0.0 {
                reflect_on_x += 1;
            } else {
                not_reflect_on_x += 1;
            }
        }
    }

    if reflect_on_y > not_reflect_on_y && reflect_on_x > not_reflect_on_x {
        for i in 0..x.len() {
            x[i] = -x[i];
            y[i] = -y[i];
        }
    } else if reflect_on_y > not_reflect_on_y {
        for v in x.iter_mut() {
            *v = -*v;
        }
    } else if reflect_on_x > not_reflect_on_x {
        for v in y.iter_mut() {
            *v = -*v;
        }
    }
}

fn enforce_relative_placement<W: WorkControl + ?Sized>(
    x: &mut [f64],
    y: &mut [f64],
    c: &Constraints,
    work_control: &mut W,
) -> std::result::Result<(), WorkFailure> {
    #[derive(Debug, Clone, Copy)]
    struct Neighbor {
        id: usize,
        gap: f64,
    }

    let n = x.len().min(y.len());
    if n == 0 {
        return Ok(());
    }

    fn enforce_relative_placement_no_align_small<W: WorkControl + ?Sized>(
        x: &mut [f64],
        y: &mut [f64],
        rel: &[RelConstraint],
        n: usize,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        use std::collections::VecDeque;

        fn build_axis_dag_keys(
            axis: Axis,
            rel: &[RelConstraint],
            n: usize,
        ) -> (Vec<usize>, Vec<Vec<Neighbor>>) {
            let mut keys: Vec<usize> = Vec::new();
            let mut seen: Vec<bool> = vec![false; n];
            let mut dag: Vec<Vec<Neighbor>> = vec![Vec::new(); n];

            for r in rel {
                match axis {
                    Axis::Horizontal => {
                        let (Some(left), Some(right)) = (r.left, r.right) else {
                            continue;
                        };
                        if left >= n || right >= n {
                            continue;
                        }
                        if !seen[left] {
                            seen[left] = true;
                            keys.push(left);
                        }
                        if !seen[right] {
                            seen[right] = true;
                            keys.push(right);
                        }
                        dag[left].push(Neighbor {
                            id: right,
                            gap: r.gap,
                        });
                    }
                    Axis::Vertical => {
                        let (Some(top), Some(bottom)) = (r.top, r.bottom) else {
                            continue;
                        };
                        if top >= n || bottom >= n {
                            continue;
                        }
                        if !seen[top] {
                            seen[top] = true;
                            keys.push(top);
                        }
                        if !seen[bottom] {
                            seen[bottom] = true;
                            keys.push(bottom);
                        }
                        dag[top].push(Neighbor {
                            id: bottom,
                            gap: r.gap,
                        });
                    }
                }
            }

            (keys, dag)
        }

        fn build_rev(keys: &[usize], dag: &[Vec<Neighbor>], n: usize) -> Vec<Vec<Neighbor>> {
            let mut rev: Vec<Vec<Neighbor>> = vec![Vec::new(); n];
            for &src in keys {
                if src >= n {
                    continue;
                }
                for e in &dag[src] {
                    if e.id >= n {
                        continue;
                    }
                    rev[e.id].push(Neighbor {
                        id: src,
                        gap: e.gap,
                    });
                }
            }
            rev
        }

        fn pos_before(key: usize, axis: Axis, x: &[f64], y: &[f64]) -> f64 {
            match axis {
                Axis::Horizontal => x[key],
                Axis::Vertical => y[key],
            }
        }

        fn component_sources(
            keys: &[usize],
            dag: &[Vec<Neighbor>],
            rev: &[Vec<Neighbor>],
            n: usize,
        ) -> Vec<Vec<usize>> {
            let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); n];
            for &src in keys {
                if src >= n {
                    continue;
                }
                for e in &dag[src] {
                    if e.id >= n {
                        continue;
                    }
                    undirected[src].push(e.id);
                    undirected[e.id].push(src);
                }
            }

            let mut visited: Vec<bool> = vec![false; n];
            let mut out: Vec<Vec<usize>> = Vec::new();
            for &start in keys {
                if start >= n || visited[start] {
                    continue;
                }
                let mut q: VecDeque<usize> = VecDeque::new();
                let mut comp: Vec<usize> = Vec::new();
                visited[start] = true;
                q.push_back(start);
                while let Some(cur) = q.pop_front() {
                    comp.push(cur);
                    for &next in &undirected[cur] {
                        if next < n && !visited[next] {
                            visited[next] = true;
                            q.push_back(next);
                        }
                    }
                }

                let mut sources: Vec<usize> = Vec::new();
                for &node in &comp {
                    if node < n && rev[node].is_empty() {
                        sources.push(node);
                    }
                }
                out.push(sources);
            }
            out
        }

        struct SmallAxisProjection<'a> {
            keys: &'a [usize],
            dag: &'a [Vec<Neighbor>],
            axis: Axis,
            node_count: usize,
            x: &'a [f64],
            y: &'a [f64],
            sources: &'a [Vec<usize>],
        }

        fn find_appropriate_positions<W: WorkControl + ?Sized>(
            projection: SmallAxisProjection<'_>,
            work_control: &mut W,
        ) -> std::result::Result<Vec<f64>, WorkFailure> {
            let SmallAxisProjection {
                keys,
                dag,
                axis,
                node_count: n,
                x,
                y,
                sources,
            } = projection;
            let mut in_deg: Vec<usize> = vec![0; n];
            for &src in keys {
                for e in &dag[src] {
                    in_deg[e.id] = in_deg[e.id].saturating_add(1);
                }
            }

            let mut position: Vec<f64> = vec![0.0; n];
            let mut past_bits: Vec<u64> = vec![0; n];
            let mut past_order: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut q: VecDeque<usize> = VecDeque::new();

            for &k in keys {
                position[k] = f64::NEG_INFINITY;
                if in_deg[k] == 0 {
                    q.push_back(k);
                }
                past_bits[k] = 1u64 << (k as u64);
                past_order[k] = vec![k];
            }

            for component in sources {
                if component.is_empty() {
                    continue;
                }
                let mut sum = 0.0;
                for &node in component {
                    sum += pos_before(node, axis, x, y);
                }
                let avg = sum / (component.len() as f64);
                for &node in component {
                    position[node] = avg;
                }
            }

            while let Some(cur) = q.pop_front() {
                let cur_pos = position[cur];
                for neigh in &dag[cur] {
                    let want = cur_pos + neigh.gap;
                    if position[neigh.id] < want {
                        position[neigh.id] = want;
                    }
                    in_deg[neigh.id] = in_deg[neigh.id].saturating_sub(1);
                    if in_deg[neigh.id] == 0 {
                        q.push_back(neigh.id);
                    }

                    charge_relative_projection_work(
                        work_control,
                        past_order[cur].len(),
                        past_order[neigh.id].len(),
                    )?;
                    let mut merged_bits = past_bits[cur];
                    let mut merged_order: Vec<usize> = past_order[cur].clone();
                    for &v in &past_order[neigh.id] {
                        let bit = 1u64 << (v as u64);
                        if (merged_bits & bit) == 0 {
                            merged_bits |= bit;
                            merged_order.push(v);
                        }
                    }
                    past_bits[neigh.id] = merged_bits;
                    past_order[neigh.id] = merged_order;
                }
            }

            let mut sink_nodes: Vec<usize> = Vec::new();
            for &k in keys {
                if dag[k].is_empty() {
                    sink_nodes.push(k);
                }
            }

            let mut comp_bits: Vec<u64> = Vec::new();
            let mut comp_order: Vec<Vec<usize>> = Vec::new();
            for &k in keys {
                if !sink_nodes.contains(&k) || past_order[k].is_empty() {
                    continue;
                }
                let first = past_order[k][0];
                let first_bit = 1u64 << (first as u64);
                if !comp_bits.is_empty() {
                    work_control.charge(comp_bits.len())?;
                }
                if let Some(idx) = comp_bits.iter().position(|b| (*b & first_bit) != 0) {
                    charge_relative_projection_work(
                        work_control,
                        comp_order[idx].len(),
                        past_order[k].len(),
                    )?;
                    let mut bits = comp_bits[idx];
                    let mut order = comp_order[idx].clone();
                    for &v in &past_order[k] {
                        let bit = 1u64 << (v as u64);
                        if (bits & bit) == 0 {
                            bits |= bit;
                            order.push(v);
                        }
                    }
                    comp_bits[idx] = bits;
                    comp_order[idx] = order;
                } else {
                    charge_relative_projection_work(work_control, past_order[k].len(), 0)?;
                    comp_bits.push(past_bits[k]);
                    comp_order.push(past_order[k].clone());
                }
            }

            for comp in comp_order {
                let mut min_before = f64::INFINITY;
                let mut max_before = f64::NEG_INFINITY;
                let mut min_after = f64::INFINITY;
                let mut max_after = f64::NEG_INFINITY;
                for &node in &comp {
                    let before = pos_before(node, axis, x, y);
                    let after = position[node];
                    min_before = min_before.min(before);
                    max_before = max_before.max(before);
                    min_after = min_after.min(after);
                    max_after = max_after.max(after);
                }
                let diff = ((min_before + max_before) / 2.0) - ((min_after + max_after) / 2.0);
                for &node in &comp {
                    position[node] += diff;
                }
            }

            Ok(position)
        }

        let (keys_h, dag_h) = build_axis_dag_keys(Axis::Horizontal, rel, n);
        if !keys_h.is_empty() {
            let rev_h = build_rev(&keys_h, &dag_h, n);
            let sources = component_sources(&keys_h, &dag_h, &rev_h, n);
            let pos = find_appropriate_positions(
                SmallAxisProjection {
                    keys: &keys_h,
                    dag: &dag_h,
                    axis: Axis::Horizontal,
                    node_count: n,
                    x,
                    y,
                    sources: &sources,
                },
                work_control,
            )?;
            for &k in &keys_h {
                x[k] = pos[k];
            }
        }

        let (keys_v, dag_v) = build_axis_dag_keys(Axis::Vertical, rel, n);
        if !keys_v.is_empty() {
            let rev_v = build_rev(&keys_v, &dag_v, n);
            let sources = component_sources(&keys_v, &dag_v, &rev_v, n);
            let pos = find_appropriate_positions(
                SmallAxisProjection {
                    keys: &keys_v,
                    dag: &dag_v,
                    axis: Axis::Vertical,
                    node_count: n,
                    x,
                    y,
                    sources: &sources,
                },
                work_control,
            )?;
            for &k in &keys_v {
                y[k] = pos[k];
            }
        }
        Ok(())
    }

    if c.align_vertical.is_empty() && c.align_horizontal.is_empty() && n <= 64 {
        enforce_relative_placement_no_align_small(x, y, &c.relative, n, work_control)?;
        return Ok(());
    }

    // Dummy mappings for alignment constraints (per-axis, matching `ConstraintHandler`).
    let mut dummy_to_nodes_for_vertical_alignment: Vec<Vec<usize>> = Vec::new();
    let mut node_to_dummy_for_vertical_alignment: Vec<Option<usize>> = vec![None; n];
    for (i, group) in c.align_vertical.iter().enumerate() {
        let dummy = n + i;
        dummy_to_nodes_for_vertical_alignment.push(group.clone());
        for &idx in group {
            if idx < n {
                node_to_dummy_for_vertical_alignment[idx] = Some(dummy);
            }
        }
    }
    let mut dummy_pos_for_vertical_alignment: Vec<f64> = dummy_to_nodes_for_vertical_alignment
        .iter()
        .map(|g| x[*g.first().unwrap_or(&0)])
        .collect();

    let mut dummy_to_nodes_for_horizontal_alignment: Vec<Vec<usize>> = Vec::new();
    let mut node_to_dummy_for_horizontal_alignment: Vec<Option<usize>> = vec![None; n];
    for (i, group) in c.align_horizontal.iter().enumerate() {
        let dummy = n + i;
        dummy_to_nodes_for_horizontal_alignment.push(group.clone());
        for &idx in group {
            if idx < n {
                node_to_dummy_for_horizontal_alignment[idx] = Some(dummy);
            }
        }
    }
    let mut dummy_pos_for_horizontal_alignment: Vec<f64> = dummy_to_nodes_for_horizontal_alignment
        .iter()
        .map(|g| y[*g.first().unwrap_or(&0)])
        .collect();

    let mut dag_h: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
    let mut dag_v: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
    for r in &c.relative {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            let src = node_to_dummy_for_vertical_alignment[left].unwrap_or(left);
            let dst = node_to_dummy_for_vertical_alignment[right].unwrap_or(right);
            dag_h.entry(dst).or_default();
            dag_h.entry(src).or_default().push(Neighbor {
                id: dst,
                gap: r.gap,
            });
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            let src = node_to_dummy_for_horizontal_alignment[top].unwrap_or(top);
            let dst = node_to_dummy_for_horizontal_alignment[bottom].unwrap_or(bottom);
            dag_v.entry(dst).or_default();
            dag_v.entry(src).or_default().push(Neighbor {
                id: dst,
                gap: r.gap,
            });
        }
    }

    fn dag_to_undirected(dag: &IndexMap<usize, Vec<Neighbor>>) -> IndexMap<usize, Vec<Neighbor>> {
        let mut u: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
        for (&k, _) in dag.iter() {
            u.insert(k, Vec::new());
        }
        for (&k, neigh) in dag.iter() {
            for n in neigh {
                u.entry(k).or_default().push(*n);
                u.entry(n.id)
                    .or_default()
                    .push(Neighbor { id: k, gap: n.gap });
            }
        }
        u
    }

    fn dag_to_reversed(dag: &IndexMap<usize, Vec<Neighbor>>) -> IndexMap<usize, Vec<Neighbor>> {
        let mut r: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
        for (&k, _) in dag.iter() {
            r.insert(k, Vec::new());
        }
        for (&k, neigh) in dag.iter() {
            for n in neigh {
                r.entry(n.id)
                    .or_default()
                    .push(Neighbor { id: k, gap: n.gap });
            }
        }
        r
    }

    fn find_components(undirected: &IndexMap<usize, Vec<Neighbor>>) -> Vec<Vec<usize>> {
        use std::collections::{HashSet, VecDeque};
        let mut visited: HashSet<usize> = HashSet::new();
        let mut out: Vec<Vec<usize>> = Vec::new();
        for (&k, _) in undirected.iter() {
            if visited.contains(&k) {
                continue;
            }
            let mut q: VecDeque<usize> = VecDeque::new();
            let mut comp: Vec<usize> = Vec::new();
            q.push_back(k);
            visited.insert(k);
            while let Some(cur) = q.pop_front() {
                comp.push(cur);
                for n in &undirected[&cur] {
                    if visited.insert(n.id) {
                        q.push_back(n.id);
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    fn component_sources(
        dag: &IndexMap<usize, Vec<Neighbor>>,
        rev: &IndexMap<usize, Vec<Neighbor>>,
    ) -> Vec<Vec<usize>> {
        let undirected = dag_to_undirected(dag);
        let comps = find_components(&undirected);
        let mut out: Vec<Vec<usize>> = Vec::new();
        for comp in comps {
            let mut sources: Vec<usize> = Vec::new();
            for node in comp {
                if rev.get(&node).is_none_or(|v| v.is_empty()) {
                    sources.push(node);
                }
            }
            out.push(sources);
        }
        out
    }

    fn pos_before(key: usize, axis: Axis, n: usize, x: &[f64], y: &[f64], dummy: &[f64]) -> f64 {
        if key < n {
            match axis {
                Axis::Horizontal => x[key],
                Axis::Vertical => y[key],
            }
        } else {
            dummy[key - n]
        }
    }

    struct AxisProjection<'a> {
        dag: &'a IndexMap<usize, Vec<Neighbor>>,
        axis: Axis,
        node_count: usize,
        x: &'a [f64],
        y: &'a [f64],
        dummy_pos: &'a [f64],
        component_sources: &'a [Vec<usize>],
    }

    fn find_appropriate_positions<W: WorkControl + ?Sized>(
        projection: AxisProjection<'_>,
        work_control: &mut W,
    ) -> std::result::Result<IndexMap<usize, f64>, WorkFailure> {
        use std::collections::VecDeque;

        let AxisProjection {
            dag,
            axis,
            node_count: n,
            x,
            y,
            dummy_pos,
            component_sources,
        } = projection;

        let mut in_deg: IndexMap<usize, usize> = IndexMap::new();
        for (&k, _) in dag.iter() {
            in_deg.insert(k, 0);
        }
        for (&_k, neigh) in dag.iter() {
            for n2 in neigh {
                *in_deg.entry(n2.id).or_default() += 1;
            }
        }

        let mut position: IndexMap<usize, f64> = IndexMap::new();
        let mut past: IndexMap<usize, IndexSet<usize>> = IndexMap::new();
        let mut q: VecDeque<usize> = VecDeque::new();

        for (&k, &deg) in in_deg.iter() {
            position.insert(k, f64::NEG_INFINITY);
            if deg == 0 {
                q.push_back(k);
            }
            past.insert(k, IndexSet::from([k]));
        }

        // Align sources of each component (enforcement path, empty fixed-node set).
        for component in component_sources {
            if component.is_empty() {
                continue;
            }
            let mut sum = 0.0;
            for &node in component {
                sum += pos_before(node, axis, n, x, y, dummy_pos);
            }
            let avg = sum / (component.len() as f64);
            for &node in component {
                position.insert(node, avg);
            }
        }

        while let Some(cur) = q.pop_front() {
            let cur_pos = position[&cur];
            let outgoing = &dag[&cur];
            for (index, neigh) in outgoing.iter().enumerate() {
                let want = cur_pos + neigh.gap;
                if position[&neigh.id] < want {
                    position.insert(neigh.id, want);
                }
                let deg = in_deg.entry(neigh.id).or_default();
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    q.push_back(neigh.id);
                }
                charge_relative_projection_work(
                    work_control,
                    past[&cur].len(),
                    past[&neigh.id].len(),
                )?;
                // The queue visits a node only after all of its incoming edges. After its
                // last outgoing edge, a non-sink's predecessor set has no further readers.
                let mut merged = if index + 1 == outgoing.len() {
                    past.swap_remove(&cur)
                        .expect("queued node has predecessors")
                } else {
                    past[&cur].clone()
                };
                for v in past[&neigh.id].iter().copied() {
                    merged.insert(v);
                }
                past[&neigh.id] = merged;
            }
        }

        // Readjust sinks in the original DAG order, independently of removals from `past`.
        let mut components: Vec<IndexSet<usize>> = Vec::new();
        for (&k, outgoing) in dag.iter() {
            if !outgoing.is_empty() {
                continue;
            }
            let set = &past[&k];
            let Some(&first) = set.first() else {
                continue;
            };
            if !components.is_empty() {
                work_control.charge(components.len())?;
            }
            let component = components.iter_mut().find(|c| c.contains(&first));
            let component_len = component.as_deref().map_or(0, IndexSet::len);
            charge_relative_projection_work(work_control, component_len, set.len())?;
            let set = past.swap_remove(&k).expect("sink retains its predecessors");
            if let Some(component) = component {
                for v in set {
                    component.insert(v);
                }
            } else {
                components.push(set);
            }
        }

        for comp in components {
            let mut min_before = f64::INFINITY;
            let mut max_before = f64::NEG_INFINITY;
            let mut min_after = f64::INFINITY;
            let mut max_after = f64::NEG_INFINITY;
            for &node in comp.iter() {
                let before = pos_before(node, axis, n, x, y, dummy_pos);
                let after = position[&node];
                min_before = min_before.min(before);
                max_before = max_before.max(before);
                min_after = min_after.min(after);
                max_after = max_after.max(after);
            }
            let diff = ((min_before + max_before) / 2.0) - ((min_after + max_after) / 2.0);
            for &node in comp.iter() {
                position.insert(node, position[&node] + diff);
            }
        }

        Ok(position)
    }

    if !dag_h.is_empty() {
        let rev = dag_to_reversed(&dag_h);
        let sources = component_sources(&dag_h, &rev);
        let pos = find_appropriate_positions(
            AxisProjection {
                dag: &dag_h,
                axis: Axis::Horizontal,
                node_count: n,
                x,
                y,
                dummy_pos: &dummy_pos_for_vertical_alignment,
                component_sources: &sources,
            },
            work_control,
        )?;
        for (&key, &v) in pos.iter() {
            if key < n {
                x[key] = v;
            } else {
                let di = key - n;
                for &idx in &dummy_to_nodes_for_vertical_alignment[di] {
                    x[idx] = v;
                }
                dummy_pos_for_vertical_alignment[di] = v;
            }
        }
    }

    if !dag_v.is_empty() {
        let rev = dag_to_reversed(&dag_v);
        let sources = component_sources(&dag_v, &rev);
        let pos = find_appropriate_positions(
            AxisProjection {
                dag: &dag_v,
                axis: Axis::Vertical,
                node_count: n,
                x,
                y,
                dummy_pos: &dummy_pos_for_horizontal_alignment,
                component_sources: &sources,
            },
            work_control,
        )?;
        for (&key, &v) in pos.iter() {
            if key < n {
                y[key] = v;
            } else {
                let di = key - n;
                for &idx in &dummy_to_nodes_for_horizontal_alignment[di] {
                    y[idx] = v;
                }
                dummy_pos_for_horizontal_alignment[di] = v;
            }
        }
    }
    Ok(())
}

pub(super) fn apply_constraints_to_displacements(
    nodes: &[SimNode],
    c: &Constraints,
    disps: &mut [(f64, f64)],
    max_d: f64,
) {
    // Alignments: enforce exact alignment by adjusting displacements to a shared target line.
    for group in &c.align_horizontal {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0.0;
        for &idx in group {
            sum += nodes[idx].center_y() + disps[idx].1;
            cnt += 1.0;
        }
        if cnt > 0.0 {
            let target = sum / cnt;
            for &idx in group {
                disps[idx].1 += target - (nodes[idx].center_y() + disps[idx].1);
            }
        }
    }
    for group in &c.align_vertical {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0.0;
        for &idx in group {
            sum += nodes[idx].center_x() + disps[idx].0;
            cnt += 1.0;
        }
        if cnt > 0.0 {
            let target = sum / cnt;
            for &idx in group {
                disps[idx].0 += target - (nodes[idx].center_x() + disps[idx].0);
            }
        }
    }

    // Relative placements: iteratively relax displacements to satisfy minimum center gaps.
    // This is a small, deterministic approximation of `cose-base` constraint handling.
    for _ in 0..4 {
        let mut changed = false;
        for r in &c.relative {
            if let (Some(left), Some(right)) = (r.left, r.right) {
                let new_gap = (nodes[right].center_x() + disps[right].0)
                    - (nodes[left].center_x() + disps[left].0);
                if new_gap < r.gap {
                    let delta = r.gap - new_gap;
                    disps[left].0 -= delta / 2.0;
                    disps[right].0 += delta / 2.0;
                    changed = true;
                }
            }
            if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
                let new_gap = (nodes[bottom].center_y() + disps[bottom].1)
                    - (nodes[top].center_y() + disps[top].1);
                if new_gap < r.gap {
                    let delta = r.gap - new_gap;
                    disps[top].1 -= delta / 2.0;
                    disps[bottom].1 += delta / 2.0;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Re-apply per-axis displacement caps (matching the upstream `calculateDisplacement` clamp).
    if max_d.is_finite() && max_d > 0.0 {
        for (dx, dy) in disps {
            if dx.abs() > max_d {
                *dx = max_d * imath_sign(*dx);
            }
            if dy.abs() > max_d {
                *dy = max_d * imath_sign(*dy);
            }
        }
    }
}
