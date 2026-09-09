//! Admission events and errors shared by fallible geometry producers.

use crate::core::Op;
use euclid::Trig;
use num_traits::Float;

/// Work admission precedes scratch growth; output operations are delivered individually.
#[derive(Debug)]
pub enum GenerationEvent<F: Float + Trig> {
    Work(usize),
    Op(Op<F>),
}

#[derive(Debug)]
pub enum GenerationError<E> {
    Consumer(E),
    Allocation(std::collections::TryReserveError),
    InvalidGeometry,
    UnsupportedFillStyle,
}

impl<E: std::fmt::Display> std::fmt::Display for GenerationError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Consumer(error) => {
                write!(formatter, "geometry consumer rejected generation: {error}")
            }
            Self::Allocation(error) => write!(formatter, "geometry allocation failed: {error}"),
            Self::InvalidGeometry => formatter.write_str("invalid rough geometry"),
            Self::UnsupportedFillStyle => {
                formatter.write_str("expected hachure or crosshatch fill")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for GenerationError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Consumer(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::InvalidGeometry | Self::UnsupportedFillStyle => None,
        }
    }
}
