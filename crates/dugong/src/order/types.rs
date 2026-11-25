//! Node ordering / crossing minimization.
//!
//! Ported from Dagre's `order` pipeline: barycenters, conflict resolution, and a sweep heuristic
//! that attempts to minimize edge crossings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    InEdges,
    OutEdges,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerGraphLabel {
    pub root: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeightLabel {
    pub weight: f64,
}

pub trait OrderEdgeWeight {
    fn weight(&self) -> f64;
}

impl OrderEdgeWeight for WeightLabel {
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl OrderEdgeWeight for crate::EdgeLabel {
    fn weight(&self) -> f64 {
        self.weight
    }
}

pub trait OrderNodeRange {
    fn rank(&self) -> Option<i32>;
    fn min_rank(&self) -> Option<i32>;
    fn max_rank(&self) -> Option<i32>;
    fn has_min_rank(&self) -> bool {
        self.min_rank().is_some()
    }
    fn border_left_at(&self, _rank: i32) -> Option<String> {
        None
    }
    fn border_right_at(&self, _rank: i32) -> Option<String> {
        None
    }
    fn subgraph_layer_label(&self, _rank: i32) -> Self
    where
        Self: Sized + Default,
    {
        Self::default()
    }
}

impl OrderNodeRange for crate::NodeLabel {
    fn rank(&self) -> Option<i32> {
        self.rank
    }

    fn min_rank(&self) -> Option<i32> {
        self.compound
            .as_deref()
            .and_then(|compound| compound.min_rank)
    }

    fn max_rank(&self) -> Option<i32> {
        self.compound
            .as_deref()
            .and_then(|compound| compound.max_rank)
    }

    fn border_left_at(&self, rank: i32) -> Option<String> {
        self.compound
            .as_deref()?
            .border_left
            .get(rank as usize)?
            .clone()
    }

    fn border_right_at(&self, rank: i32) -> Option<String> {
        self.compound
            .as_deref()?
            .border_right
            .get(rank as usize)?
            .clone()
    }

    fn subgraph_layer_label(&self, rank: i32) -> Self {
        let left = self.border_left_at(rank);
        let right = self.border_right_at(rank);

        Self {
            compound: Some(Box::new(crate::CompoundNodeLabel {
                border_left: vec![left],
                border_right: vec![right],
                ..Default::default()
            })),
            ..Default::default()
        }
    }
}

pub trait OrderNodeLabel: OrderNodeRange {
    fn order(&self) -> Option<usize>;
    fn set_order(&mut self, order: usize);

    fn border_left(&self) -> Option<&str> {
        None
    }

    fn border_right(&self) -> Option<&str> {
        None
    }
}

impl OrderNodeLabel for crate::NodeLabel {
    fn order(&self) -> Option<usize> {
        self.order
    }

    fn set_order(&mut self, order: usize) {
        self.order = Some(order);
    }

    fn border_left(&self) -> Option<&str> {
        self.compound.as_deref()?.border_left.first()?.as_deref()
    }

    fn border_right(&self) -> Option<&str> {
        self.compound.as_deref()?.border_right.first()?.as_deref()
    }
}
