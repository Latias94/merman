//! Validate the measured graph and layout options shared by each placement kernel.

use crate::work::LayoutResult;
use crate::{
    EdgeSpec, GroupSpec, LayoutError, NodeSpec, Options, PackingPolicy, PortSideHint, Side, Size,
    WidthPolicy,
};

fn length(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}
pub(crate) fn measured(s: Size) -> bool {
    [s.width, s.height]
        .into_iter()
        .all(|extent| length(extent) && (extent == 0.0 || extent * 0.5 > 0.0))
}

pub(crate) fn measured_group(group: &GroupSpec) -> bool {
    let padding = group.padding;
    [padding.top, padding.right, padding.bottom, padding.left]
        .into_iter()
        .all(length)
        && measured(group.min_size)
        && group.title.is_none_or(measured)
}

pub(crate) fn options(options: &Options) -> LayoutResult<()> {
    let s = options.spacing;
    if ![
        s.node,
        s.rank,
        s.component,
        s.edge,
        s.loop_gap,
        s.corner_clearance,
        s.port_pitch,
        s.min_port_pitch,
        s.end_label_across,
        s.end_label_along,
        s.port_sep,
    ]
    .into_iter()
    .all(length)
    {
        return Err(LayoutError::InvalidInput(
            "spacing must be finite and nonnegative",
        ));
    }
    let positive = |v: f64| v.is_finite() && v > 0.0;
    match options.packing {
        PackingPolicy::AspectTargeted { target_ratio }
        | PackingPolicy::BalancedAspect { target_ratio }
            if !positive(target_ratio) =>
        {
            return Err(LayoutError::InvalidInput("invalid aspect target"));
        }
        PackingPolicy::FitWidth { max_width } if !positive(max_width) => {
            return Err(LayoutError::InvalidInput("invalid packing width"));
        }
        _ => {}
    }
    if let WidthPolicy::FitWidth {
        max_width,
        turn_breakpoint,
        ..
    } = options.width_policy
        && (!positive(max_width) || !length(turn_breakpoint))
    {
        return Err(LayoutError::InvalidInput("invalid width policy"));
    }
    if let Some(q) = options.grid_quantum {
        if !positive(q) {
            return Err(LayoutError::InvalidInput("invalid grid quantum"));
        }
        return Err(LayoutError::Unsupported("grid layout"));
    }
    Ok(())
}

pub(crate) fn node(node: &NodeSpec) -> LayoutResult<()> {
    if !measured(node.size)
        || matches!(node.contour, crate::ShapeContourHint::Rounded { rx, ry } if !length(rx) || !length(ry))
        || matches!(
            node.contour,
            crate::ShapeContourHint::OffsetDiamond { x, y }
                if !x.is_finite()
                    || !y.is_finite()
                    || x.abs() > node.size.width * 0.5
                    || y.abs() > node.size.height * 0.5
        )
        || node.port_span.is_some_and(|v| !length(v))
        || node.perpendicular_port_span.is_some_and(|v| !length(v))
        || node
            .anchor_y
            .is_some_and(|v| !length(v) || v > node.size.height)
        || (node.flow_relative_axis.is_some() && node.anchor_y.is_some())
        || node.order_hint.is_some_and(|h| !h.cross_key.is_finite())
        || !(-1..=1).contains(&node.pin)
    {
        return Err(LayoutError::InvalidInput(
            "invalid node geometry, parent or order hint",
        ));
    }
    Ok(())
}

pub(crate) fn port(size: Size, port: PortSideHint) -> LayoutResult<()> {
    if let PortSideHint::FixedOffset { side, offset } = port {
        let face = match side {
            Side::Top | Side::Bottom => size.width,
            Side::Left | Side::Right => size.height,
        };
        if !offset.is_finite() || offset.abs() > face * 0.5 {
            return Err(LayoutError::InvalidInput("port offset outside face"));
        }
    }
    Ok(())
}

pub(crate) fn edge(edge: &EdgeSpec) -> LayoutResult<()> {
    if !edge.weight.is_finite()
        || edge.weight <= 0.0
        || ![edge.label, edge.start_label, edge.end_label]
            .into_iter()
            .flatten()
            .all(measured)
        || ![edge.start_marker, edge.end_marker]
            .into_iter()
            .all(|m| length(m.length) && length(m.width))
        || !edge.raw_marker_runways().into_iter().all(length)
    {
        return Err(LayoutError::InvalidInput(
            "invalid edge label, marker or weight",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Endpoint, NodeId};

    #[test]
    fn edge_validation_rejects_non_finite_or_negative_marker_runways() {
        let ends = (Endpoint::Node(NodeId(0)), Endpoint::Node(NodeId(1)));
        for bad in [[f64::NAN, 0.0], [0.0, f64::INFINITY], [-1.0, 0.0]] {
            let mut spec = EdgeSpec::new(ends.0, ends.1);
            spec.set_marker_runways(bad);
            assert!(edge(&spec).is_err(), "{bad:?}");
        }
        let mut ok = EdgeSpec::new(ends.0, ends.1);
        ok.set_marker_runways([0.0, 14.0]);
        assert!(edge(&ok).is_ok());
    }

    #[test]
    fn node_validation_bounds_offset_diamond_to_node_half_extents() {
        let mut spec = NodeSpec::new(
            Size {
                width: 40.0,
                height: 20.0,
            },
            0,
        );
        spec.contour = crate::ShapeContourHint::OffsetDiamond { x: 20.0, y: -10.0 };
        assert!(node(&spec).is_ok());
        spec.contour = crate::ShapeContourHint::OffsetDiamond { x: 20.1, y: 0.0 };
        assert!(node(&spec).is_err());
        spec.contour = crate::ShapeContourHint::OffsetDiamond { x: 0.0, y: -10.1 };
        assert!(node(&spec).is_err());
    }
}
