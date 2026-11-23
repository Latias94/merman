use super::{Edge, Shaft};
use kurbo::BezPath;

#[derive(Debug, Default)]
pub(super) struct Deviation {
    pub(super) group_box: f64,
    pub(super) endpoint_to_outline: f64,
    pub(super) border_crossing: f64,
}

pub(super) fn stroke_passes(edge: &Edge<'_>) -> Result<(usize, Vec<Shaft>), String> {
    let path = edge.transform * BezPath::from_svg(edge.d).map_err(|e| e.to_string())?;
    let mut passes = [BezPath::new(), BezPath::new()];
    let mut pieces = path.subpaths();
    let mut count = 0;
    while let Some(first) = pieces.next() {
        let second = pieces.next().ok_or("unpaired RoughJS segment stroke")?;
        count += 2;
        for (pass, piece) in passes.iter_mut().zip([first, second]) {
            for &element in piece {
                pass.push(element);
            }
        }
    }
    let shafts = passes
        .into_iter()
        .map(|path| Shaft::from_path(path, edge.cross_axis))
        .collect::<Result<_, _>>()?;
    Ok((count, shafts))
}

#[test]
fn only_the_moved_rough_stroke_hits_the_foreign_body() {
    use super::{Document, draw};
    use kurbo::ParamCurve;
    let drawing = draw(include_str!(
        "../../../../../../../fixtures/flowchart/upstream_cypress_flowchart_handdrawn_spec_fhd11_should_render_styled_subgraphs_011.mmd"
    ));
    let document = Document::parse(&drawing.svg).unwrap();
    let before = drawing.capture(&document);
    assert!(before.failures.is_empty(), "{}", before.failures.join("\n"));
    assert_eq!(before.counts.stroke_passes, 2 * before.counts.shafts);
    let index = drawing
        .layout
        .edges
        .iter()
        .position(|e| e.from == "A" && e.to == "B")
        .unwrap();
    let edge = &before.scene.edges[index];
    let foreign = before.body_names.iter().position(|id| id == "G").unwrap();
    let obstacle = before.scene.bodies[foreign].bounds;
    let (_, passes) = stroke_passes(edge).unwrap();
    assert!(passes.iter().all(|shaft| !shaft.hits(obstacle)));
    let path = document
        .descendants()
        .find(|n| {
            n.attribute("data-et") == Some("edge")
                && n.attribute("data-id") == Some(&drawing.layout.edges[index].id)
        })
        .unwrap();
    let original = BezPath::from_svg(edge.d).unwrap();
    let mut changed = BezPath::new();
    for (piece, elements) in original.subpaths().enumerate() {
        if piece == 1 {
            let curve = BezPath::from_vec(elements.to_vec());
            let start = curve.segments().next().unwrap().start();
            let end = curve.segments().last().unwrap().end();
            let midpoint = start.midpoint(end);
            let point = edge.transform.inverse() * obstacle.center();
            changed.move_to(start);
            changed.quad_to(point + (point - midpoint), end);
        } else {
            for &element in elements {
                changed.push(element);
            }
        }
    }
    let mut svg = drawing.svg.clone();
    svg.replace_range(
        path.attribute_node("d").unwrap().range_value(),
        &changed.to_svg(),
    );
    let document = Document::parse(&svg).unwrap();
    let after = drawing.capture(&document);
    let hits: Vec<_> = after
        .failures
        .iter()
        .filter(|failure| failure.contains("A → B") && failure.contains("enters G: ForeignNodeHit"))
        .collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].contains("stroke 2"));
}
