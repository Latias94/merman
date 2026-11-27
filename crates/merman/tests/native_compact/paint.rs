use roxmltree::Node;

pub(super) fn assert_transparent_scene(root: Node<'_, '_>) {
    assert!(!root.descendants().any(|node| {
        node.attribute("data-scene-role") == Some("canvas")
            || node.has_tag_name("pattern")
            || node.has_tag_name("radialGradient")
            || node.has_tag_name("filter")
    }));
}

pub(super) fn endpoint_glyph<'a, 'input>(root: Node<'a, 'input>) -> Node<'a, 'input> {
    let marker_id = root
        .descendants()
        .find_map(|node| node.attribute("marker-end"))
        .and_then(|reference| reference.strip_prefix("url(#"))
        .and_then(|reference| reference.strip_suffix(')'))
        .expect("expected a same-document endpoint marker reference");
    root.descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
        .and_then(|marker| marker.first_element_child())
        .expect("expected the referenced marker glyph")
}

pub(super) fn assert_cyberpunk_shadows(root: Node<'_, '_>, surfaces: usize) {
    let depth = root
        .children()
        .find(|node| node.has_tag_name("g"))
        .expect("expected a depth layer before the foreground");
    assert_eq!(depth.attribute("data-scene-role"), Some("depth"));
    assert_eq!(depth.attribute("aria-hidden"), Some("true"));
    assert_eq!(depth.attribute("pointer-events"), Some("none"));
    let mut foreground: Vec<_> = root
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .parent()
                    .and_then(|parent| parent.attribute("data-scene-role"))
                    == Some("node")
        })
        .map(rect_bounds)
        .collect();
    assert_eq!(foreground.len(), surfaces);
    let layers: Vec<_> = depth.children().filter(Node::is_element).collect();
    assert_eq!(layers.len(), 2 * surfaces);
    let view_box: Vec<f64> = root
        .attribute("viewBox")
        .unwrap()
        .split_ascii_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    let [left, top, width, height] = view_box[..] else {
        panic!("invalid scene viewBox {view_box:?}");
    };
    for pair in layers.chunks_exact(2) {
        for (&layer, fill) in pair.iter().zip(["#FF4564", "#29F3EC"]) {
            assert!(layer.has_tag_name("rect"));
            assert_eq!(layer.attribute("fill"), Some(fill));
            assert_eq!(layer.attribute("fill-opacity"), Some("0.85"));
            assert_eq!(layer.attribute("stroke"), None);
            let [x, y, w, h] = rect_bounds(layer);
            assert!(x >= left - 1e-6 && y >= top - 1e-6);
            assert!(x + w <= left + width + 1e-6 && y + h <= top + height + 1e-6);
        }
        let [x, y, w, h] = rect_bounds(pair[0]);
        let matching_surface = foreground
            .iter()
            .position(|&bounds| same_bounds(bounds, [x + 3.0, y, w, h]));
        foreground.swap_remove(matching_surface.expect("shadow must match a foreground rectangle"));
        assert!(same_bounds(rect_bounds(pair[1]), [x + 7.0, y + 4.0, w, h]));
    }
}

fn rect_bounds(node: Node<'_, '_>) -> [f64; 4] {
    ["x", "y", "width", "height"]
        .map(|attribute| node.attribute(attribute).unwrap().parse().unwrap())
}

fn same_bounds(a: [f64; 4], b: [f64; 4]) -> bool {
    a.into_iter()
        .zip(b)
        .all(|(first, second)| (first - second).abs() < 1e-6)
}
