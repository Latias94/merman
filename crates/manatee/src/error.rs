#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkFailure {
    #[error("layout work was interrupted by the caller")]
    Interrupted,
    #[error("layout work arithmetic overflowed")]
    ArithmeticOverflow,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("graph contains an edge with a missing endpoint: {edge_id}")]
    MissingEndpoint { edge_id: String },
    #[error("FCoSE {axis} constraints are infeasible")]
    InfeasibleConstraints { axis: &'static str },
    #[error("FCoSE produced non-finite layout geometry: {field}")]
    NonFiniteLayout { field: &'static str },
    #[error(transparent)]
    WorkFailure(#[from] WorkFailure),
}

pub type Result<T> = std::result::Result<T, Error>;
