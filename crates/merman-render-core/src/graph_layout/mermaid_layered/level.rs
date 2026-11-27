//! Borrowed interface to the flat Layered solver.

use super::{Point, Routing, Size};

#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub node: f64,
    pub rank: f64,
    pub edge: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelOptions {
    pub spacing: Spacing,
    pub port_spacing: f64,
    pub head_room: f64,
    pub routing: Routing,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NodeKind {
    #[default]
    Node,
    StartPort,
    EndPort,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LevelNode {
    /// Cross-flow width and along-flow height.
    pub size: Size,
    pub kind: NodeKind,
    pub span: f64,
    pub pin: i8,
    pub key: Option<f64>,
    pub hint: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Attachment {
    pub dx: f64,
    pub fixed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelEdge {
    pub tail: usize,
    pub head: usize,
    pub min_len: u32,
    /// Cross-flow width and along-flow height.
    pub label: Size,
    pub tail_attachment: Attachment,
    pub head_attachment: Attachment,
}

impl LevelEdge {
    pub(crate) fn new(tail: usize, head: usize, min_len: u32) -> Self {
        Self {
            tail,
            head,
            min_len,
            label: Size::default(),
            tail_attachment: Attachment::default(),
            head_attachment: Attachment::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Membership<'a> {
    pub of: &'a [Option<usize>],
    pub parent: &'a [Option<usize>],
}

#[derive(Clone, Copy, Debug)]
pub struct LevelPlan<'a> {
    pub nodes: &'a [LevelNode],
    pub edges: &'a [LevelEdge],
    pub groups: Option<Membership<'a>>,
    pub options: LevelOptions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Orient,
    OrderOnly,
    Place,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankPoint {
    pub rank: usize,
    pub cross: f64,
}

#[derive(Debug)]
pub struct OrderedLevel {
    pub nodes: Vec<RankPoint>,
    /// One route per edge in input order. Knots run from lower to higher solved rank.
    pub edges: Vec<Vec<RankPoint>>,
    pub crossings: f64,
    pub declared_crossings: f64,
}

#[derive(Debug)]
pub struct PlacedLevelEdge {
    /// One route per input edge. Points run from original tail to original head.
    pub points: Vec<Point>,
    /// The reference's zero-initialized label position, even without a label dummy.
    pub label: Point,
    pub reversed_for_ranking: bool,
}

#[derive(Debug)]
pub struct PlacedLevel {
    pub size: Size,
    pub centers: Vec<Point>,
    pub edges: Vec<PlacedLevelEdge>,
}

#[derive(Debug)]
pub enum LevelResult {
    Oriented(Vec<bool>),
    Ordered(OrderedLevel),
    Placed(PlacedLevel),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Node(usize),
    Group(usize),
    Edge(usize),
    Graph,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("edge occurrence {0} has an unknown source")]
    UnknownSource(usize),
    #[error("edge occurrence {0} has an unknown target")]
    UnknownTarget(usize),
    #[error("{field} at {owner:?} must be finite and nonnegative")]
    InvalidNumber { owner: Owner, field: &'static str },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CompoundError<E> {
    #[error(transparent)]
    Work(E),
    #[error("Layered layout arithmetic overflow")]
    ArithmeticOverflow,
    #[error("Layered layout storage could not be allocated")]
    AllocationFailed,
    #[error("invalid Layered input: {0}")]
    InvalidInput(#[source] InputError),
    #[error("inconsistent Layered level: {0}")]
    InconsistentLevel(&'static str),
    #[error("Layered layout produced non-finite geometry")]
    NonFiniteGeometry,
}

impl<E> CompoundError<E> {
    /// The outer provider still has to admit the Dagre retry with the same meter.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_) | Self::InconsistentLevel(_) | Self::NonFiniteGeometry
        )
    }
}

pub type Result<T, E> = std::result::Result<T, CompoundError<E>>;
pub type Charge<'a, E> = dyn FnMut(usize) -> std::result::Result<(), E> + 'a;
pub type Solve<'a, E> =
    dyn FnMut(LevelPlan<'_>, Mode, &mut Charge<'_, E>) -> Result<LevelResult, E> + 'a;
