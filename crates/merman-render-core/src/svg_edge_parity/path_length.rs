/// Returns the rendered centerline length for an SVG path.
///
/// Mermaid derives Neo edge masking from `SVGPathElement.getTotalLength()`. Kurbo parses the same
/// SVG path grammar, including arcs, and evaluates Bezier arc length without flattening the path
/// into a local polyline approximation.
pub fn svg_path_length_from_d(d: &str) -> Option<f64> {
    use kurbo::Shape;

    let path = kurbo::BezPath::from_svg(d).ok()?;
    // SVGGeometryElement.getTotalLength() exposes a Web IDL `float`, promoted to a JS Number
    // before Mermaid applies marker offsets. Preserve that boundary instead of exposing the
    // platform-dependent low bits of Kurbo's f64 arc-length calculation in SVG styles.
    // https://www.w3.org/TR/SVG/types.html#InterfaceSVGGeometryElement
    let length = f64::from(path.perimeter(crate::route_types::SVG_LENGTH_ACCURACY) as f32);
    length.is_finite().then_some(length)
}
