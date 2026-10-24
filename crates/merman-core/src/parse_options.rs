/// Parser behavior switches shared by semantic and presentation model facades.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Return an `error` diagram model when a detected diagram's parser fails.
    ///
    /// Preprocessing failures remain errors. An unrecognized source produces no diagram.
    pub suppress_errors: bool,
}

impl ParseOptions {
    /// Strict parsing: return detection and diagram-parser failures.
    pub fn strict() -> Self {
        Self {
            suppress_errors: false,
        }
    }

    /// Lenient model parsing: retain detected parser failures as `error` diagrams.
    pub fn lenient() -> Self {
        Self {
            suppress_errors: true,
        }
    }
}
