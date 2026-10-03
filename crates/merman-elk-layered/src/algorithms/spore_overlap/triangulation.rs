//! Source Bowyer-Watson triangulation and elkjs hash-bucket traversal order.
//! elkjs 0.9.3's GWT Double hash truncates to i32; KVector adds the reversed y bits.
//! Its InternalHashCodeMap iterates insertion-ordered JS Map buckets and then each collision
//! chain. Preserving this order matters when NaiveMinST's stable sort sees equal edge costs.

use super::*;
fn vector_hash(point: Point) -> i32 {
    (point.x as i32).wrapping_add((point.y as i32).reverse_bits())
}
fn insert<T>(
    items: &mut Vec<(i32, T)>,
    hash: i32,
    value: T,
    equal: impl Fn(&T, &T) -> bool,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    charge(work, checked_add(items.len(), 1)?)?;
    let mut after = None;
    for (i, (h, item)) in items.iter().enumerate() {
        if *h == hash {
            if equal(item, &value) {
                return Ok(());
            }
            after = Some(i + 1);
        }
    }
    items.insert(after.unwrap_or(items.len()), (hash, value));
    Ok(())
}
fn same_edge(a: &[usize; 2], b: &[usize; 2]) -> bool {
    a == b || (a[0] == b[1] && a[1] == b[0])
}
pub(super) fn add_edge(
    edges: &mut Vec<(i32, [usize; 2])>,
    vertices: &[Vertex],
    edge: [usize; 2],
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let hash = vector_hash(vertices[edge[0]].original)
        .wrapping_add(vector_hash(vertices[edge[1]].original));
    insert(edges, hash, edge, same_edge, work)
}
#[derive(Clone)]
struct Triangle {
    indices: [usize; 3],
    center: Point,
    radius: f64,
}
impl Triangle {
    fn new(indices: [usize; 3], points: &[Point]) -> Self {
        let [a, b, c] = indices.map(|i| points[i]);
        let ab = Point {
            x: b.x - a.x,
            y: b.y - a.y,
        };
        let ac = Point {
            x: c.x - a.x,
            y: c.y - a.y,
        };
        let bc = Point {
            x: c.x - b.x,
            y: c.y - b.y,
        };
        let e = ab.x * (a.x + b.x) + ab.y * (a.y + b.y);
        let f = ac.x * (a.x + c.x) + ac.y * (a.y + c.y);
        let g = 2. * (ab.x * bc.y - ab.y * bc.x);
        let center = Point {
            x: (ac.y * e - ab.y * f) / g,
            y: (ab.x * f - ac.x * e) / g,
        };
        Self {
            indices,
            center,
            radius: distance(center, a),
        }
    }
    fn edges(&self) -> [[usize; 2]; 3] {
        let [a, b, c] = self.indices;
        [[a, b], [b, c], [c, a]]
    }
}
pub(super) fn triangulate(
    vertices: &[Vertex],
    edges: &mut Vec<(i32, [usize; 2])>,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    if vertices.is_empty() {
        return Ok(());
    }
    charge(work, checked_add(vertices.len(), 3)?)?;
    let n = vertices.len();
    let mut points: Vec<_> = vertices.iter().map(|v| v.original).collect();
    let min_x = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let min_y = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let max_y = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let width = max_x - min_x;
    let height = max_y - min_y;
    points.extend([
        Point {
            x: min_x - 50.,
            y: min_y - width - 50.,
        },
        Point {
            x: min_x - 50.,
            y: max_y + width + 50.,
        },
        Point {
            x: max_x + height / 2. + 50.,
            y: min_y + height / 2.,
        },
    ]);
    if points.iter().any(|&p| !finite(p)) {
        return Err(Error::NonFiniteGeometry);
    }
    let hash = |ids: [usize; 3]| {
        ids.into_iter()
            .fold(0i32, |sum, i| sum.wrapping_add(vector_hash(points[i])))
    };
    let mut triangles = vec![(
        hash([n, n + 1, n + 2]),
        Triangle::new([n, n + 1, n + 2], &points),
    )];
    for node in 0..n {
        charge(work, triangles.len())?;
        let invalid: Vec<_> = triangles
            .iter()
            .enumerate()
            .filter_map(|(i, (_, t))| {
                fuzzy(distance(t.center, points[node]), t.radius, 0.0001)
                    .is_lt()
                    .then_some(i)
            })
            .collect();
        charge(
            work,
            checked_mul(checked_mul(invalid.len(), invalid.len())?, 3)?,
        )?;
        let mut boundary = Vec::new();
        for &i in &invalid {
            for edge in triangles[i].1.edges() {
                if !invalid.iter().any(|&j| {
                    i != j
                        && triangles[j]
                            .1
                            .edges()
                            .iter()
                            .any(|other| same_edge(&edge, other))
                }) {
                    boundary.push(edge);
                }
            }
        }
        let mut next_invalid = invalid.iter().copied().peekable();
        let mut index = 0;
        triangles.retain(|_| {
            let keep = next_invalid.peek() != Some(&index);
            if !keep {
                next_invalid.next();
            }
            index += 1;
            keep
        });
        for [u, v] in boundary {
            let indices = [node, u, v];
            let triangle = Triangle::new(indices, &points);
            // Zero-area triangles retain source NaN circle behavior: they never remove later
            // triangles but their boundary edges still participate in the final graph.
            insert(
                &mut triangles,
                hash(indices),
                triangle,
                |a, b| a.indices.iter().all(|i| b.indices.contains(i)),
                work,
            )?;
        }
    }
    let mut result = Vec::new();
    for (_, triangle) in triangles {
        for edge in triangle.edges() {
            let hash = vector_hash(points[edge[0]]).wrapping_add(vector_hash(points[edge[1]]));
            insert(&mut result, hash, edge, same_edge, work)?;
        }
    }
    for (_, edge) in result {
        if edge[0] < n && edge[1] < n {
            add_edge(edges, vertices, edge, work)?;
        }
    }
    Ok(())
}
