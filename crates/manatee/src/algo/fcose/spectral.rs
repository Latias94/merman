use nalgebra::{DMatrix, DVector};

use super::{SimEdge, SimNode, XorShift64Star};

const INFINITY_HOPS: f64 = 100_000_000.0;
const SMALL: f64 = 1e-9;

// Mermaid `@11.12.2` defaults (cytoscape-fcose).
const DEFAULT_SAMPLE_SIZE: usize = 25;
const DEFAULT_NODE_SEPARATION: f64 = 75.0;
const DEFAULT_PI_TOL: f64 = 1e-7;

const MAX_POWER_ITERATIONS: usize = 10_000;

pub(super) fn apply_spectral_start_positions(
    nodes: &mut [SimNode],
    edges: &[SimEdge],
    random_seed: u64,
) -> bool {
    if nodes.is_empty() {
        return false;
    }

    let mut rng = XorShift64Star::new(random_seed);

    let n_real = nodes.len();
    let (adjacency, node_size) = build_transformed_adjacency(n_real, edges);
    if node_size <= 1 {
        return false;
    }

    // Upstream skips spectral when the transformed graph has 1 or 2 nodes.
    if node_size == 2 {
        if n_real != 2 {
            return false;
        }
        // Place the second node to the right of the first node using an ideal edge length.
        // This matches upstream spectral.js' fallback path.
        let ideal = edges
            .iter()
            .map(|e| e.ideal_length)
            .find(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(50.0);

        let (first, second) = (&nodes[0], &nodes[1]);
        let x1 = first.center_x();
        let y1 = first.center_y();
        let x2 = x1 + first.width / 2.0 + second.width / 2.0 + ideal;

        nodes[1].left = x2 - nodes[1].width / 2.0;
        nodes[1].top = y1 - nodes[1].height / 2.0;
        return true;
    }

    let sample_size = node_size.min(DEFAULT_SAMPLE_SIZE);
    if sample_size <= 1 {
        return false;
    }

    // Column sampling matrix (squared shortest-path distances).
    let mut c = DMatrix::<f64>::zeros(node_size, sample_size);
    let mut samples: Vec<usize> = vec![0; sample_size];
    let mut min_dist: Vec<f64> = vec![INFINITY_HOPS; node_size];

    // Greedy sampling (Mermaid default): pick a random first sample, then repeatedly pick the node
    // that maximizes the minimum distance to the already-sampled set.
    let mut sample = rng.next_usize(node_size);
    for v in &mut min_dist {
        *v = INFINITY_HOPS;
    }
    for col in 0..sample_size {
        samples[col] = sample;
        sample = bfs_fill_column(
            sample,
            col,
            &adjacency,
            DEFAULT_NODE_SEPARATION,
            &mut c,
            Some(&mut min_dist),
        );
    }

    // Square distances for C.
    for i in 0..node_size {
        for j in 0..sample_size {
            let v = c[(i, j)];
            c[(i, j)] = v * v;
        }
    }

    // PHI is the intersection of sampled rows/columns.
    let mut phi = DMatrix::<f64>::zeros(sample_size, sample_size);
    for i in 0..sample_size {
        for j in 0..sample_size {
            phi[(i, j)] = c[(samples[j], i)];
        }
    }

    let inv = match regularized_inverse_from_svd(&phi) {
        Some(m) => m,
        None => return false,
    };

    let (x_coords, y_coords) = match power_iteration(&mut rng, &c, &inv, DEFAULT_PI_TOL) {
        Some(v) => v,
        None => return false,
    };

    for i in 0..n_real {
        let x = x_coords[i];
        let y = y_coords[i];
        if !(x.is_finite() && y.is_finite()) {
            return false;
        }
        nodes[i].left = x - nodes[i].width / 2.0;
        nodes[i].top = y - nodes[i].height / 2.0;
    }

    true
}

fn build_transformed_adjacency(n_real: usize, edges: &[SimEdge]) -> (Vec<Vec<usize>>, usize) {
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n_real];
    for e in edges {
        if e.a < n_real && e.b < n_real {
            adjacency[e.a].push(e.b);
            adjacency[e.b].push(e.a);
        }
    }
    for neigh in &mut adjacency {
        neigh.sort_unstable();
        neigh.dedup();
    }

    let components = connected_components(&adjacency);
    if components.len() <= 1 {
        return (adjacency, n_real);
    }

    // Mimic `aux.connectComponents(...)` by inserting a dummy node connected to one minimum-degree
    // representative per component. This makes the transformed graph connected for BFS sampling.
    let dummy_idx = adjacency.len();
    adjacency.push(Vec::new());
    for comp in components {
        let mut best = comp[0];
        let mut best_deg = adjacency[best].len();
        for &v in &comp {
            let deg = adjacency[v].len();
            if deg < best_deg || (deg == best_deg && v < best) {
                best = v;
                best_deg = deg;
            }
        }
        adjacency[dummy_idx].push(best);
        adjacency[best].push(dummy_idx);
    }

    for neigh in &mut adjacency {
        neigh.sort_unstable();
        neigh.dedup();
    }

    let node_size = adjacency.len();
    (adjacency, node_size)
}

fn connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adjacency.len();
    let mut visited = vec![false; n];
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    for start in 0..n {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        q.push_back(start);
        let mut comp: Vec<usize> = Vec::new();

        while let Some(v) = q.pop_front() {
            comp.push(v);
            for &u in &adjacency[v] {
                if !visited[u] {
                    visited[u] = true;
                    q.push_back(u);
                }
            }
        }

        comp.sort_unstable();
        out.push(comp);
    }

    out
}

fn bfs_fill_column(
    pivot: usize,
    col: usize,
    adjacency: &[Vec<usize>],
    node_separation: f64,
    c: &mut DMatrix<f64>,
    mut min_dist: Option<&mut [f64]>,
) -> usize {
    let node_size = adjacency.len();
    let mut dist: Vec<i32> = vec![-1; node_size];
    let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    dist[pivot] = 0;
    q.push_back(pivot);

    while let Some(v) = q.pop_front() {
        for &u in &adjacency[v] {
            if dist[u] == -1 {
                dist[u] = dist[v].saturating_add(1);
                q.push_back(u);
            }
        }
    }

    let mut max_dist = 0.0;
    let mut max_idx = 0usize;
    for i in 0..node_size {
        let d = if dist[i] == -1 {
            INFINITY_HOPS
        } else {
            (dist[i] as f64) * node_separation
        };
        c[(i, col)] = d;

        if let Some(min_dist) = min_dist.as_deref_mut() {
            if d < min_dist[i] {
                min_dist[i] = d;
            }
            if min_dist[i] > max_dist {
                max_dist = min_dist[i];
                max_idx = i;
            }
        }
    }

    if min_dist.is_some() { max_idx } else { pivot }
}

fn regularized_inverse_from_svd(phi: &DMatrix<f64>) -> Option<DMatrix<f64>> {
    let svd = nalgebra::linalg::SVD::new(phi.clone(), true, true);
    let u = svd.u?;
    let v_t = svd.v_t?;
    let s = svd.singular_values;
    if s.len() == 0 {
        return None;
    }

    let max_s = s[0] * s[0] * s[0];

    let k = s.len();
    let mut sig = DMatrix::<f64>::zeros(k, k);
    for i in 0..k {
        let si = s[i];
        let si2 = si * si;
        let denom = if si2 == 0.0 {
            f64::INFINITY
        } else {
            si2 + (max_s / si2)
        };
        sig[(i, i)] = if denom.is_finite() && denom != 0.0 {
            si / denom
        } else {
            0.0
        };
    }

    let v = v_t.transpose();
    Some(v * sig * u.transpose())
}

fn power_iteration(
    rng: &mut XorShift64Star,
    c: &DMatrix<f64>,
    inv: &DMatrix<f64>,
    pi_tol: f64,
) -> Option<(DVector<f64>, DVector<f64>)> {
    let n = c.nrows();
    if n == 0 {
        return None;
    }

    let mut y1 = DVector::<f64>::from_fn(n, |_, _| rng.next_f64_unit());
    let mut y2 = DVector::<f64>::from_fn(n, |_, _| rng.next_f64_unit());
    normalize_in_place(&mut y1);
    normalize_in_place(&mut y2);

    let (v1, theta1) = dominant_eigenvector(c, inv, y1, pi_tol)?;
    let (v2, theta2) = second_eigenvector(c, inv, &v1, y2, pi_tol)?;

    let x = v1 * theta1.abs().sqrt();
    let y = v2 * theta2.abs().sqrt();
    Some((x, y))
}

fn dominant_eigenvector(
    c: &DMatrix<f64>,
    inv: &DMatrix<f64>,
    mut y: DVector<f64>,
    pi_tol: f64,
) -> Option<(DVector<f64>, f64)> {
    let mut previous = SMALL;
    let mut theta = 0.0;

    for _ in 0..MAX_POWER_ITERATIONS {
        let v = y.clone();
        let t = mult_gamma(&v);
        let t = mult_l(&t, c, inv);
        let mut next = mult_gamma(&t);
        theta = v.dot(&next);
        normalize_in_place(&mut next);

        let current = v.dot(&next);
        let denom = if previous.abs() < SMALL {
            SMALL
        } else {
            previous
        };
        let ratio = (current / denom).abs();

        y = next;
        if ratio <= 1.0 + pi_tol && ratio >= 1.0 {
            return Some((y, theta));
        }
        previous = current;
    }

    Some((y, theta))
}

fn second_eigenvector(
    c: &DMatrix<f64>,
    inv: &DMatrix<f64>,
    v1: &DVector<f64>,
    mut y: DVector<f64>,
    pi_tol: f64,
) -> Option<(DVector<f64>, f64)> {
    let mut previous = SMALL;
    let mut theta = 0.0;

    for _ in 0..MAX_POWER_ITERATIONS {
        let mut v = y.clone();
        let proj = v1.dot(&v);
        v -= v1 * proj;

        let t = mult_gamma(&v);
        let t = mult_l(&t, c, inv);
        let mut next = mult_gamma(&t);
        theta = v.dot(&next);
        normalize_in_place(&mut next);

        let current = v.dot(&next);
        let denom = if previous.abs() < SMALL {
            SMALL
        } else {
            previous
        };
        let ratio = (current / denom).abs();

        y = next;
        if ratio <= 1.0 + pi_tol && ratio >= 1.0 {
            return Some((y, theta));
        }
        previous = current;
    }

    Some((y, theta))
}

fn mult_gamma(v: &DVector<f64>) -> DVector<f64> {
    let n = v.len();
    if n == 0 {
        return v.clone();
    }
    let mean = v.iter().sum::<f64>() / (n as f64);
    DVector::<f64>::from_fn(n, |i, _| v[i] - mean)
}

fn mult_l(v: &DVector<f64>, c: &DMatrix<f64>, inv: &DMatrix<f64>) -> DVector<f64> {
    // Nyström-style multiplication:
    // L = -0.5 * C * INV * C^T
    let t = c.transpose() * v;
    let t = inv * t;
    let out = c * t;
    out * -0.5
}

fn normalize_in_place(v: &mut DVector<f64>) {
    let norm = v.norm();
    if norm.is_finite() && norm > 0.0 {
        *v /= norm;
    }
}
