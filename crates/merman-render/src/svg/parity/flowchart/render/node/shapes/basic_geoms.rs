//! Flowchart v2 basic geometry shapes.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::{OptionalStyleAttr, escape_attr};
use crate::svg::parity::fmt;

pub(in crate::svg::parity::flowchart::render::node) fn render_diamond(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    style: &str,
) {
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let _ = write!(
        out,
        r#"<polygon points="{},0 {},{} {},{} 0,{}" class="label-container" transform="translate({}, {})"{} />"#,
        fmt(w / 2.0),
        fmt(w),
        fmt(-h / 2.0),
        fmt(w / 2.0),
        fmt(-h),
        fmt(-h / 2.0),
        fmt(-w / 2.0 + 0.5),
        fmt(h / 2.0),
        OptionalStyleAttr(style)
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_circle(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    style: &str,
) {
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let r = (w.min(h) / 2.0).max(0.5);
    let _ = write!(
        out,
        r#"<circle class="basic label-container" style="{}" r="{}" cx="0" cy="0"/>"#,
        escape_attr(style),
        fmt(r),
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_double_circle(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    style: &str,
) {
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let r = (w.min(h) / 2.0).max(0.5);
    let inner = (r - 5.0).max(0.5);
    let _ = write!(
        out,
        r#"<g class="basic label-container" style="{}"><circle class="outer-circle" cx="0" cy="0" r="{}" style="{}"/><circle class="inner-circle" cx="0" cy="0" r="{}" style="{}"/></g>"#,
        escape_attr(style),
        fmt(r),
        escape_attr(style),
        fmt(inner),
        escape_attr(style),
    );
}
