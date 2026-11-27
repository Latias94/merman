//! Mermaid 12 ray intersections for adapter endpoint projection.

use crate::{LayoutNode, LayoutPoint as P};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    Rect,
    Ellipse,
    Diamond,
}

/// Intersects the ray from the node center through `point` with its source outline.
pub fn intersect_node_outline(node: &LayoutNode, outline: Outline, point: &P) -> P {
    let dx = point.x - node.x;
    let dy = point.y - node.y;
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    if outline == Outline::Ellipse {
        let det = (w * w * dy * dy + h * h * dx * dx).sqrt();
        return P {
            x: node.x + w * h * dx / det,
            y: node.y + w * h * dy / det,
        };
    }
    if outline == Outline::Diamond {
        // Mermaid 12 polygon intersection uses floating division without the old
        // half-pixel rounding bias. A diamond's center ray has this closed form.
        let scale = dx.abs() / w + dy.abs() / h;
        return if scale == 0.0 {
            P {
                x: node.x,
                y: node.y,
            }
        } else {
            P {
                x: node.x + dx / scale,
                y: node.y + dy / scale,
            }
        };
    }
    if dy.abs() * w > dx.abs() * h {
        let h = if dy < 0.0 { -h } else { h };
        P {
            x: node.x + if dy == 0.0 { 0.0 } else { h * dx / dy },
            y: node.y + h,
        }
    } else {
        let w = if dx < 0.0 { -w } else { w };
        P {
            x: node.x + w,
            y: node.y + if dx == 0.0 { 0.0 } else { w * dy / dx },
        }
    }
}
