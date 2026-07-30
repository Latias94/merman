use crate::graphlib::Graph;
use crate::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SepNodeMetrics {
    pub(super) width: f64,
    pub(super) labelpos: Option<LabelPos>,
    pub(super) is_dummy: bool,
}

impl From<&NodeLabel> for SepNodeMetrics {
    fn from(label: &NodeLabel) -> Self {
        Self {
            width: label.width,
            labelpos: label.labelpos,
            is_dummy: label.dummy.is_some(),
        }
    }
}

pub(super) fn sep(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    w: &str,
    reverse_sep: bool,
) -> f64 {
    let v_metrics = g.node(v).map(SepNodeMetrics::from).unwrap_or_default();
    let w_metrics = g.node(w).map(SepNodeMetrics::from).unwrap_or_default();
    sep_metrics(
        v_metrics,
        w_metrics,
        g.graph().nodesep,
        g.graph().edgesep,
        reverse_sep,
    )
}

pub(super) fn sep_metrics(
    v: SepNodeMetrics,
    w: SepNodeMetrics,
    node_sep: f64,
    edge_sep: f64,
    reverse_sep: bool,
) -> f64 {
    let mut sum: f64 = 0.0;
    let mut delta: f64 = 0.0;

    sum += v.width / 2.0;
    if let Some(labelpos) = v.labelpos {
        delta = match labelpos {
            LabelPos::L => -v.width / 2.0,
            LabelPos::R => v.width / 2.0,
            LabelPos::C => 0.0,
        };
    }
    if delta != 0.0 {
        sum += if reverse_sep { delta } else { -delta };
    }
    delta = 0.0;

    sum += if v.is_dummy { edge_sep } else { node_sep } / 2.0;
    sum += if w.is_dummy { edge_sep } else { node_sep } / 2.0;

    sum += w.width / 2.0;
    if let Some(labelpos) = w.labelpos {
        delta = match labelpos {
            LabelPos::L => w.width / 2.0,
            LabelPos::R => -w.width / 2.0,
            LabelPos::C => 0.0,
        };
    }
    if delta != 0.0 {
        sum += if reverse_sep { delta } else { -delta };
    }

    sum
}

pub(super) fn width(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str) -> f64 {
    g.node(v).map(|n| n.width).unwrap_or(0.0)
}
