use std::collections::BTreeMap;

use crate::wire::SpecifiedWireV1;

pub(crate) trait ContainsNonFiniteNumber {
    fn contains_non_finite_number(&self) -> bool;
}

impl ContainsNonFiniteNumber for f32 {
    fn contains_non_finite_number(&self) -> bool {
        !self.is_finite()
    }
}

impl ContainsNonFiniteNumber for f64 {
    fn contains_non_finite_number(&self) -> bool {
        !self.is_finite()
    }
}

impl<T> ContainsNonFiniteNumber for Option<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        self.as_ref()
            .is_some_and(ContainsNonFiniteNumber::contains_non_finite_number)
    }
}

impl<T> ContainsNonFiniteNumber for Vec<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        self.iter()
            .any(ContainsNonFiniteNumber::contains_non_finite_number)
    }
}

impl<K, V> ContainsNonFiniteNumber for BTreeMap<K, V>
where
    V: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        self.values()
            .any(ContainsNonFiniteNumber::contains_non_finite_number)
    }
}

impl<T> ContainsNonFiniteNumber for SpecifiedWireV1<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Value(value) => value.contains_non_finite_number(),
            Self::Unspecified | Self::Clear => false,
        }
    }
}
