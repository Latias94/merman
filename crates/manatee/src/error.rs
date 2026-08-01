#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("graph contains an edge with a missing endpoint: {edge_id}")]
    MissingEndpoint { edge_id: String },
    #[error("FCoSE {axis} constraints are infeasible")]
    InfeasibleConstraints { axis: &'static str },
    #[error("FCoSE produced non-finite layout geometry: {field}")]
    NonFiniteLayout { field: &'static str },
}

pub type Result<T> = std::result::Result<T, Error>;
