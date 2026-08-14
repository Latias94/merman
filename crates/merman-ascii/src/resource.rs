use crate::error::{AsciiError, Result};
use merman_core::resources::{GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, ResourceProfile};
use merman_core::{OperationControl, OperationPhase};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

pub const ASCII_RESOURCE_LIMIT_COUNT: usize = 6;

pub const MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID: &str = "max_ascii_grid_cells";
pub const MAX_ASCII_LAYOUT_WORK_UNITS_RESOURCE_LIMIT_ID: &str = "max_ascii_layout_work_units";
pub const MAX_ASCII_DOCUMENT_CELLS_RESOURCE_LIMIT_ID: &str = "max_ascii_document_cells";
pub const MAX_ASCII_OUTPUT_BYTES_RESOURCE_LIMIT_ID: &str = "max_ascii_output_bytes";
pub const MAX_ASCII_GRAPHEME_BYTES_RESOURCE_LIMIT_ID: &str = "max_ascii_grapheme_bytes";
pub const MAX_ASCII_NESTING_DEPTH_RESOURCE_LIMIT_ID: &str = "max_ascii_nesting_depth";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AsciiResourceLimitPhase {
    Layout,
    LayoutWork,
    Document,
    Output,
    Grapheme,
    Nesting,
}

/// Stable reason why an ASCII resource check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AsciiResourceLimitCause {
    /// The requested work exceeded the configured policy ceiling.
    Ceiling,
    /// Computing cumulative work overflowed the platform counter.
    ArithmeticOverflow,
}

impl AsciiResourceLimitCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
        }
    }
}

impl fmt::Display for AsciiResourceLimitCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsciiResourceLimitPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "ascii_layout",
            Self::LayoutWork => "ascii_layout_work",
            Self::Document => "ascii_document",
            Self::Output => "ascii_output",
            Self::Grapheme => "ascii_grapheme",
            Self::Nesting => "ascii_nesting",
        }
    }
}

impl fmt::Display for AsciiResourceLimitPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AsciiResourceLimitId {
    MaxGridCells,
    MaxLayoutWorkUnits,
    MaxDocumentCells,
    MaxOutputBytes,
    MaxGraphemeBytes,
    MaxNestingDepth,
}

impl AsciiResourceLimitId {
    pub const ALL: [Self; ASCII_RESOURCE_LIMIT_COUNT] = [
        Self::MaxGridCells,
        Self::MaxLayoutWorkUnits,
        Self::MaxDocumentCells,
        Self::MaxOutputBytes,
        Self::MaxGraphemeBytes,
        Self::MaxNestingDepth,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn descriptor(self) -> &'static AsciiResourceLimitDescriptor {
        &ASCII_RESOURCE_LIMIT_DESCRIPTORS[self.index()]
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }

    pub fn from_stable_id(stable_id: &str) -> Option<Self> {
        ASCII_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.stable_id == stable_id)
            .map(|descriptor| descriptor.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiResourceLimitDescriptor {
    pub id: AsciiResourceLimitId,
    pub stable_id: &'static str,
    pub phase: AsciiResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub minimum_value: usize,
}

macro_rules! ascii_limit_descriptors {
    ($($id:ident => ($stable:ident, $phase:ident, $description:literal)),+ $(,)?) => {
        pub static ASCII_RESOURCE_LIMIT_DESCRIPTORS:
            [AsciiResourceLimitDescriptor; ASCII_RESOURCE_LIMIT_COUNT] = [
                $(AsciiResourceLimitDescriptor {
                    id: AsciiResourceLimitId::$id,
                    stable_id: $stable,
                    phase: AsciiResourceLimitPhase::$phase,
                    description: $description,
                    overridable: true,
                    minimum_value: 1,
                }),+
            ];
    };
}

ascii_limit_descriptors! {
    MaxGridCells => (MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID, Layout, "Maximum terminal grid cells allocated by ASCII layout"),
    MaxLayoutWorkUnits => (MAX_ASCII_LAYOUT_WORK_UNITS_RESOURCE_LIMIT_ID, LayoutWork, "Maximum deterministic ASCII layout and planning work units"),
    MaxDocumentCells => (MAX_ASCII_DOCUMENT_CELLS_RESOURCE_LIMIT_ID, Document, "Maximum aggregate terminal display cells in an ASCII document"),
    MaxOutputBytes => (MAX_ASCII_OUTPUT_BYTES_RESOURCE_LIMIT_ID, Output, "Maximum encoded ASCII output bytes"),
    MaxGraphemeBytes => (MAX_ASCII_GRAPHEME_BYTES_RESOURCE_LIMIT_ID, Grapheme, "Maximum UTF-8 bytes in one terminal grapheme cluster"),
    MaxNestingDepth => (MAX_ASCII_NESTING_DEPTH_RESOURCE_LIMIT_ID, Nesting, "Maximum semantic nesting depth traversed by ASCII rendering"),
}

const PROFILE_VALUES: [[Option<usize>; 4]; ASCII_RESOURCE_LIMIT_COUNT] = [
    [Some(250_000), Some(125_000), Some(1_000_000), None],
    [Some(2_000_000), Some(1_000_000), Some(8_000_000), None],
    [Some(250_000), Some(125_000), Some(1_000_000), None],
    [Some(16 * MIB), Some(8 * MIB), Some(64 * MIB), None],
    [Some(4 * KIB), Some(2 * KIB), Some(64 * KIB), None],
    [Some(256), Some(128), Some(1_024), None],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiResourcePolicy {
    profile: ResourceProfile,
    base_values: [Option<usize>; ASCII_RESOURCE_LIMIT_COUNT],
    effective_values: [Option<usize>; ASCII_RESOURCE_LIMIT_COUNT],
    explicit_overrides: [Option<usize>; ASCII_RESOURCE_LIMIT_COUNT],
}

impl Default for AsciiResourcePolicy {
    fn default() -> Self {
        Self::for_profile(GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE)
    }
}

impl AsciiResourcePolicy {
    pub const fn for_profile(profile: ResourceProfile) -> Self {
        let mut values = [None; ASCII_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < ASCII_RESOURCE_LIMIT_COUNT {
            values[index] = PROFILE_VALUES[index][profile as usize];
            index += 1;
        }
        Self {
            profile,
            base_values: values,
            effective_values: values,
            explicit_overrides: [None; ASCII_RESOURCE_LIMIT_COUNT],
        }
    }

    pub const fn profile(self) -> ResourceProfile {
        self.profile
    }

    pub const fn value(self, id: AsciiResourceLimitId) -> Option<usize> {
        self.effective_values[id.index()]
    }

    pub const fn base_value(self, id: AsciiResourceLimitId) -> Option<usize> {
        self.base_values[id.index()]
    }

    pub const fn explicit_override(self, id: AsciiResourceLimitId) -> Option<usize> {
        self.explicit_overrides[id.index()]
    }

    pub fn explicit_overrides(&self) -> impl Iterator<Item = (AsciiResourceLimitId, usize)> + '_ {
        AsciiResourceLimitId::ALL
            .into_iter()
            .filter_map(|id| self.explicit_override(id).map(|value| (id, value)))
    }

    #[must_use]
    pub fn with_profile(self, profile: ResourceProfile) -> Self {
        let mut rebased = Self::for_profile(profile);
        for (id, value) in self.explicit_overrides() {
            rebased.effective_values[id.index()] = Some(value);
            rebased.explicit_overrides[id.index()] = Some(value);
        }
        rebased
    }

    pub fn apply_override(
        &mut self,
        stable_id: &str,
        value: usize,
    ) -> std::result::Result<(), AsciiResourceLimitOverrideError> {
        let id = AsciiResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| AsciiResourceLimitOverrideError::UnknownLimit(stable_id.to_string()))?;
        self.apply_limit(id, value)
    }

    pub fn apply_limit(
        &mut self,
        id: AsciiResourceLimitId,
        value: usize,
    ) -> std::result::Result<(), AsciiResourceLimitOverrideError> {
        if value < id.descriptor().minimum_value {
            return Err(AsciiResourceLimitOverrideError::BelowMinimum {
                limit_id: id.as_str(),
                minimum: id.descriptor().minimum_value,
            });
        }
        self.effective_values[id.index()] = Some(value);
        self.explicit_overrides[id.index()] = Some(value);
        Ok(())
    }

    pub fn with_limit(
        mut self,
        id: AsciiResourceLimitId,
        value: usize,
    ) -> std::result::Result<Self, AsciiResourceLimitOverrideError> {
        self.apply_limit(id, value)?;
        Ok(self)
    }

    pub(crate) fn check(self, id: AsciiResourceLimitId, actual: usize) -> Result<()> {
        if let Some(max) = self.value(id)
            && actual > max
        {
            return Err(AsciiResourceLimitExceeded {
                cause: AsciiResourceLimitCause::Ceiling,
                limit: id,
                actual,
                max,
                profile: self.profile,
            }
            .into());
        }
        Ok(())
    }

    pub(crate) fn overflow(self, id: AsciiResourceLimitId) -> AsciiError {
        // The mathematical result is larger than `usize::MAX`. Saturate the public five-field
        // projection to the same truthful ordering used by the SVG resource meter: `actual` is the
        // largest representable value and `max` remains strictly smaller, including for an
        // otherwise-unbounded policy or an explicit `usize::MAX` override.
        let max = self.value(id).unwrap_or(usize::MAX - 1).min(usize::MAX - 1);
        AsciiResourceLimitExceeded {
            cause: AsciiResourceLimitCause::ArithmeticOverflow,
            limit: id,
            actual: usize::MAX,
            max,
            profile: self.profile,
        }
        .into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AsciiResourceLimitOverrideError {
    #[error("unknown ASCII resource limit `{0}`")]
    UnknownLimit(String),
    #[error("ASCII resource limit `{limit_id}` must be at least {minimum}")]
    BelowMinimum {
        limit_id: &'static str,
        minimum: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiResourceLimitExceeded {
    pub cause: AsciiResourceLimitCause,
    pub limit: AsciiResourceLimitId,
    pub actual: usize,
    pub max: usize,
    pub profile: ResourceProfile,
}

impl AsciiResourceLimitExceeded {
    pub const fn phase(self) -> AsciiResourceLimitPhase {
        self.limit.descriptor().phase
    }
}

impl fmt::Display for AsciiResourceLimitExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ASCII resource limit `{}` exceeded during `{}`: actual {}, maximum {} (profile `{}`)",
            self.limit.as_str(),
            self.phase().as_str(),
            self.actual,
            self.max,
            self.profile.id()
        )?;
        if self.cause == AsciiResourceLimitCause::ArithmeticOverflow {
            write!(formatter, " (cause `{}`)", self.cause)?;
        }
        Ok(())
    }
}

impl std::error::Error for AsciiResourceLimitExceeded {}

pub fn ascii_resource_profile_value(profile: ResourceProfile, stable_id: &str) -> Option<usize> {
    AsciiResourceLimitId::from_stable_id(stable_id)
        .and_then(|id| AsciiResourcePolicy::for_profile(profile).value(id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogicalExtent {
    width: usize,
    height: usize,
    cells: usize,
}

impl LogicalExtent {
    pub(crate) fn checked(
        width: usize,
        height: usize,
        policy: AsciiResourcePolicy,
    ) -> Result<Self> {
        let cells = width
            .checked_mul(height)
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxGridCells))?;
        policy.check(AsciiResourceLimitId::MaxGridCells, cells)?;
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    pub(crate) const fn width(self) -> usize {
        self.width
    }

    pub(crate) const fn height(self) -> usize {
        self.height
    }

    pub(crate) const fn cells(self) -> usize {
        self.cells
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResourceContext {
    policy: AsciiResourcePolicy,
    layout_work_used: Rc<Cell<usize>>,
    document_cells_used: Rc<Cell<usize>>,
    operation: Option<ResourceOperation>,
}

#[derive(Debug, Clone)]
struct ResourceOperation {
    control: OperationControl,
    phase: OperationPhase,
}

impl ResourceContext {
    pub(crate) fn new(policy: AsciiResourcePolicy) -> Self {
        Self {
            policy,
            layout_work_used: Rc::new(Cell::new(0)),
            document_cells_used: Rc::new(Cell::new(0)),
            operation: None,
        }
    }

    /// Creates a ledger-sharing view whose resource admissions observe operation cancellation.
    pub(crate) fn controlled(&self, control: OperationControl, phase: OperationPhase) -> Self {
        Self {
            policy: self.policy,
            layout_work_used: Rc::clone(&self.layout_work_used),
            document_cells_used: Rc::clone(&self.document_cells_used),
            operation: Some(ResourceOperation { control, phase }),
        }
    }

    /// Starts a new document/grid scope while preserving the render-wide work ledger.
    pub(crate) fn scoped(&self) -> Self {
        Self {
            policy: self.policy,
            layout_work_used: Rc::clone(&self.layout_work_used),
            document_cells_used: Rc::new(Cell::new(0)),
            operation: self.operation.clone(),
        }
    }

    pub(crate) const fn policy(&self) -> AsciiResourcePolicy {
        self.policy
    }

    pub(crate) fn check(&self, id: AsciiResourceLimitId, actual: usize) -> Result<()> {
        self.checkpoint()?;
        self.policy.check(id, actual)
    }

    pub(crate) fn overflow(&self, id: AsciiResourceLimitId) -> AsciiError {
        self.checkpoint()
            .err()
            .unwrap_or_else(|| self.policy.overflow(id))
    }

    pub(crate) fn layout_work_used(&self) -> usize {
        self.layout_work_used.get()
    }

    pub(crate) fn document_cells_used(&self) -> usize {
        self.document_cells_used.get()
    }

    pub(crate) fn grid_extent(&self, width: usize, height: usize) -> Result<LogicalExtent> {
        self.checkpoint()?;
        LogicalExtent::checked(width, height, self.policy)
    }

    pub(crate) fn checked_grid_add(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_add(right)
            .ok_or_else(|| self.overflow(AsciiResourceLimitId::MaxGridCells))
    }

    pub(crate) fn checked_grid_mul(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_mul(right)
            .ok_or_else(|| self.overflow(AsciiResourceLimitId::MaxGridCells))
    }

    pub(crate) fn checked_work_add(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_add(right).ok_or_else(|| self.work_overflow())
    }

    pub(crate) fn checked_work_mul(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_mul(right).ok_or_else(|| self.work_overflow())
    }

    pub(crate) fn charge_layout_work(&self, delta: usize) -> Result<()> {
        self.checkpoint()?;
        let actual = self.checked_total(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.layout_work_used.get(),
            delta,
        )?;
        self.layout_work_used.set(actual);
        Ok(())
    }

    pub(crate) fn charge_layout_work_product(&self, left: usize, right: usize) -> Result<()> {
        self.checkpoint()?;
        let work = left.checked_mul(right).ok_or_else(|| {
            self.policy
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        let actual = self.checked_total(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.layout_work_used.get(),
            work,
        )?;
        self.layout_work_used.set(actual);
        Ok(())
    }

    pub(crate) fn charge_document_cells(&self, delta: usize) -> Result<()> {
        self.checkpoint()?;
        let actual = self.checked_total(
            AsciiResourceLimitId::MaxDocumentCells,
            self.document_cells_used.get(),
            delta,
        )?;
        self.document_cells_used.set(actual);
        Ok(())
    }

    /// Checks one compound work/document admission without mutating either shared ledger.
    pub(crate) fn check_usage(
        &self,
        layout_work_delta: usize,
        document_cells_delta: usize,
    ) -> Result<()> {
        self.checkpoint()?;
        self.checked_total(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.layout_work_used.get(),
            layout_work_delta,
        )?;
        self.checked_total(
            AsciiResourceLimitId::MaxDocumentCells,
            self.document_cells_used.get(),
            document_cells_delta,
        )?;
        Ok(())
    }

    /// Commits one compound work/document admission after both totals have been checked.
    ///
    /// Keeping the writes together prevents a document failure from leaving work debited, or a
    /// work failure from leaving document cells debited, when a caller materializes from a plan.
    pub(crate) fn charge_usage(
        &self,
        layout_work_delta: usize,
        document_cells_delta: usize,
    ) -> Result<()> {
        self.checkpoint()?;
        let layout_work_used = self.checked_total(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.layout_work_used.get(),
            layout_work_delta,
        )?;
        let document_cells_used = self.checked_total(
            AsciiResourceLimitId::MaxDocumentCells,
            self.document_cells_used.get(),
            document_cells_delta,
        )?;
        self.layout_work_used.set(layout_work_used);
        self.document_cells_used.set(document_cells_used);
        Ok(())
    }

    /// Runs one planning or materialization phase atomically against the shared ledgers.
    ///
    /// Resource accounting is intentionally incremental inside many scanners so the reported
    /// failure points remain precise. A later dimension (for example output bytes) can still
    /// reject the phase after work or document cells have been charged. This boundary restores
    /// both ledgers to their entry values on any error while preserving successful charges.
    /// Nested transactions are safe because each call restores only its own checkpoint.
    pub(crate) fn transaction<T, E>(
        &self,
        operation: impl FnOnce(&Self) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E> {
        let layout_work_checkpoint = self.layout_work_used.get();
        let document_cells_checkpoint = self.document_cells_used.get();
        match operation(self) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.layout_work_used.set(layout_work_checkpoint);
                self.document_cells_used.set(document_cells_checkpoint);
                Err(error)
            }
        }
    }

    /// Runs a recoverable planning phase while retaining work already performed.
    ///
    /// Semantic fallbacks may discard speculative document rows, but the CPU work used to reach
    /// that decision remains part of the successful render-wide budget. Callers that need a
    /// resource failure to restore both ledgers should wrap this boundary in [`Self::transaction`]
    /// and propagate the resource error through the outer transaction.
    pub(crate) fn transaction_preserving_layout_work<T, E>(
        &self,
        operation: impl FnOnce(&Self) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E> {
        let document_cells_checkpoint = self.document_cells_used.get();
        match operation(self) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.document_cells_used.set(document_cells_checkpoint);
                Err(error)
            }
        }
    }

    pub(crate) fn check_grapheme_bytes(&self, bytes: usize) -> Result<()> {
        self.checkpoint()?;
        self.policy
            .check(AsciiResourceLimitId::MaxGraphemeBytes, bytes)
    }

    pub(crate) fn check_nesting_depth(&self, depth: usize) -> Result<()> {
        self.checkpoint()?;
        self.policy
            .check(AsciiResourceLimitId::MaxNestingDepth, depth)
    }

    pub(crate) fn grid_overflow(&self) -> AsciiError {
        self.overflow(AsciiResourceLimitId::MaxGridCells)
    }

    pub(crate) fn work_overflow(&self) -> AsciiError {
        self.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    }

    pub(crate) fn nesting_overflow(&self) -> AsciiError {
        self.overflow(AsciiResourceLimitId::MaxNestingDepth)
    }

    fn checkpoint(&self) -> Result<()> {
        self.operation.as_ref().map_or(Ok(()), |operation| {
            operation
                .control
                .checkpoint_at(operation.phase)
                .map_err(AsciiError::Cancelled)
        })
    }

    fn checked_total(
        &self,
        id: AsciiResourceLimitId,
        current: usize,
        delta: usize,
    ) -> Result<usize> {
        let actual = current
            .checked_add(delta)
            .ok_or_else(|| self.policy.overflow(id))?;
        self.policy.check(id, actual)?;
        Ok(actual)
    }
}

#[derive(Debug)]
pub(crate) struct CheckedOutput {
    policy: AsciiResourcePolicy,
    output: String,
}

impl CheckedOutput {
    pub(crate) fn new(policy: AsciiResourcePolicy) -> Self {
        Self {
            policy,
            output: String::new(),
        }
    }

    pub(crate) fn push_str(&mut self, value: &str) -> Result<()> {
        let actual = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        self.policy
            .check(AsciiResourceLimitId::MaxOutputBytes, actual)?;
        self.output
            .try_reserve(value.len())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Output.as_str(),
            })?;
        self.output.push_str(value);
        Ok(())
    }

    pub(crate) fn push_char(&mut self, value: char) -> Result<()> {
        let mut encoded = [0; 4];
        self.push_str(value.encode_utf8(&mut encoded))
    }

    pub(crate) fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        struct Adapter<'a> {
            output: &'a mut CheckedOutput,
            error: Option<AsciiError>,
        }

        impl fmt::Write for Adapter<'_> {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                match self.output.push_str(value) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        self.error = Some(error);
                        Err(fmt::Error)
                    }
                }
            }
        }

        let mut adapter = Adapter {
            output: self,
            error: None,
        };
        if fmt::write(&mut adapter, arguments).is_err() {
            return Err(adapter.error.unwrap_or(AsciiError::InvalidOption {
                field: "output",
                message: "formatting failed",
            }));
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> String {
        self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::CancelReason;

    #[test]
    fn descriptors_and_profile_matrix_are_total() {
        assert_eq!(
            ASCII_RESOURCE_LIMIT_DESCRIPTORS.len(),
            ASCII_RESOURCE_LIMIT_COUNT
        );
        let expected = [
            [Some(250_000), Some(125_000), Some(1_000_000), None],
            [Some(2_000_000), Some(1_000_000), Some(8_000_000), None],
            [Some(250_000), Some(125_000), Some(1_000_000), None],
            [Some(16 * MIB), Some(8 * MIB), Some(64 * MIB), None],
            [Some(4 * KIB), Some(2 * KIB), Some(64 * KIB), None],
            [Some(256), Some(128), Some(1_024), None],
        ];
        for (index, id) in AsciiResourceLimitId::ALL.into_iter().enumerate() {
            assert_eq!(id.index(), index);
            assert_eq!(AsciiResourceLimitId::from_stable_id(id.as_str()), Some(id));
            assert_eq!(id.descriptor().id, id);
            for (profile_index, profile) in ResourceProfile::ALL.into_iter().enumerate() {
                let policy = AsciiResourcePolicy::for_profile(profile);
                assert_eq!(
                    ascii_resource_profile_value(profile, id.as_str()),
                    expected[index][profile_index]
                );
                assert_eq!(policy.value(id), expected[index][profile_index]);
            }
        }
    }

    #[test]
    fn every_limit_accepts_exact_value_and_rejects_limit_plus_one() {
        for id in AsciiResourceLimitId::ALL {
            let policy = AsciiResourcePolicy::default()
                .with_limit(id, 3)
                .expect("valid override");
            policy.check(id, 3).expect("exact limit should pass");
            assert_eq!(
                policy.check(id, 4),
                Err(AsciiError::ResourceLimitExceeded(
                    AsciiResourceLimitExceeded {
                        cause: AsciiResourceLimitCause::Ceiling,
                        limit: id,
                        actual: 4,
                        max: 3,
                        profile: ResourceProfile::Interactive,
                    }
                ))
            );
        }
    }

    #[test]
    fn profile_rebase_preserves_explicit_overrides() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 42)
            .expect("valid override")
            .with_profile(ResourceProfile::Constrained);

        assert_eq!(policy.profile(), ResourceProfile::Constrained);
        assert_eq!(policy.value(AsciiResourceLimitId::MaxGridCells), Some(42));
        assert_eq!(
            policy.value(AsciiResourceLimitId::MaxDocumentCells),
            Some(125_000)
        );
    }

    #[test]
    fn override_rejects_zero_and_unknown_ids() {
        let mut policy = AsciiResourcePolicy::default();
        assert!(matches!(
            policy.apply_limit(AsciiResourceLimitId::MaxGridCells, 0),
            Err(AsciiResourceLimitOverrideError::BelowMinimum { .. })
        ));
        assert_eq!(
            policy.apply_override("not_a_resource_limit", 1),
            Err(AsciiResourceLimitOverrideError::UnknownLimit(
                "not_a_resource_limit".to_string()
            ))
        );
    }

    #[test]
    fn checked_extent_reports_overflow_without_allocating() {
        let error = LogicalExtent::checked(usize::MAX, 2, AsciiResourcePolicy::default())
            .expect_err("overflow must fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected resource error");
        };
        assert_eq!(details.cause, AsciiResourceLimitCause::ArithmeticOverflow);
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }

    #[test]
    fn arithmetic_overflow_preserves_an_exceeded_projection_for_unbounded_policies() {
        for policy in [
            AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(AsciiResourceLimitId::MaxGridCells, usize::MAX)
                .expect("maximum usize is a valid direct-API override"),
        ] {
            let AsciiError::ResourceLimitExceeded(details) =
                policy.overflow(AsciiResourceLimitId::MaxGridCells)
            else {
                panic!("expected a resource overflow");
            };
            assert_eq!(details.cause, AsciiResourceLimitCause::ArithmeticOverflow);
            assert_eq!(details.actual, usize::MAX);
            assert_eq!(details.max, usize::MAX - 1);
            assert!(details.actual > details.max);
        }
    }

    #[test]
    fn checked_output_counts_actual_encoded_bytes_before_append() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid override");
        let mut output = CheckedOutput::new(policy);

        output.push_str("abc").expect("exact limit should pass");
        let error = output
            .push_str("d")
            .expect_err("limit plus one should fail");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxOutputBytes,
                actual: 4,
                max: 3,
                ..
            })
        ));
    }

    #[test]
    fn transaction_restores_shared_ledgers_when_a_later_limit_fails() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 10)
            .expect("valid work limit")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 10)
            .expect("valid document limit")
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 1)
            .expect("valid output limit");
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(2, 3)
            .expect("the checkpoint should start with prior usage");

        let error = resources
            .transaction(|resources| {
                resources.charge_usage(4, 5)?;
                resources.check(AsciiResourceLimitId::MaxOutputBytes, 2)
            })
            .expect_err("the output check should fail");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxOutputBytes,
                actual: 2,
                max: 1,
                ..
            })
        ));
        assert_eq!(resources.layout_work_used(), 2);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn controlled_compound_admission_prioritizes_cancellation_without_ledger_mutation() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("valid work limit")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 1)
            .expect("valid document limit");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let controlled = resources.controlled(control, OperationPhase::Emit);

        let error = controlled
            .charge_usage(2, 2)
            .expect_err("cancellation must win over both compound ceilings");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn transaction_commits_success_and_nested_failure_isolated() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 20)
            .expect("valid work limit")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 20)
            .expect("valid document limit");
        let resources = ResourceContext::new(policy);

        resources
            .transaction(|resources| {
                resources.charge_usage(2, 3)?;
                let nested = resources.transaction(|resources| {
                    resources.charge_usage(4, 5)?;
                    Err::<(), _>(resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
                });
                assert!(nested.is_err());
                assert_eq!(resources.layout_work_used(), 2);
                assert_eq!(resources.document_cells_used(), 3);
                resources.charge_usage(1, 1)
            })
            .expect("outer transaction should commit");

        assert_eq!(resources.layout_work_used(), 3);
        assert_eq!(resources.document_cells_used(), 4);
    }

    #[test]
    fn transaction_rolls_back_domain_errors_without_an_ascii_error_adapter() {
        #[derive(Debug, PartialEq, Eq)]
        enum PlanningFailure {
            RouteCollision,
        }

        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        resources
            .charge_usage(2, 3)
            .expect("the checkpoint should start with prior usage");

        let error = resources
            .transaction(|resources| {
                resources
                    .charge_usage(4, 5)
                    .expect("the speculative usage should fit");
                Err::<(), _>(PlanningFailure::RouteCollision)
            })
            .expect_err("the domain failure should roll back the transaction");

        assert_eq!(error, PlanningFailure::RouteCollision);
        assert_eq!(resources.layout_work_used(), 2);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn recoverable_transaction_keeps_work_and_restores_document_cells() {
        #[derive(Debug, PartialEq, Eq)]
        enum PlanningFailure {
            RouteCollision,
        }

        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        resources
            .charge_usage(2, 3)
            .expect("the checkpoint should start with prior usage");

        let error = resources
            .transaction_preserving_layout_work(|resources| {
                resources
                    .charge_usage(4, 5)
                    .expect("the speculative usage should fit");
                Err::<(), _>(PlanningFailure::RouteCollision)
            })
            .expect_err("the semantic failure should discard speculative document cells");

        assert_eq!(error, PlanningFailure::RouteCollision);
        assert_eq!(resources.layout_work_used(), 6);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn scoped_contexts_share_one_render_wide_layout_work_ledger() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 5)
            .expect("valid layout-work limit");
        let resources = ResourceContext::new(policy);
        let first_phase = resources.scoped();
        let second_phase = resources.scoped();

        resources
            .charge_layout_work(2)
            .expect("the root phase should fit");
        first_phase
            .charge_layout_work(3)
            .expect("the exact cumulative render work should fit");
        let error = second_phase
            .charge_layout_work(1)
            .expect_err("a later phase must observe work charged by earlier scopes");

        assert_eq!(resources.layout_work_used(), 5);
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxLayoutWorkUnits,
                actual: 6,
                max: 5,
                ..
            })
        ));
    }

    #[test]
    fn scoped_contexts_keep_document_ledgers_local() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 3)
            .expect("valid document limit");
        let resources = ResourceContext::new(policy);
        let document = resources.scoped();

        resources
            .charge_document_cells(3)
            .expect("the root document should fit exactly");
        document
            .charge_document_cells(3)
            .expect("a separate document scope should have an independent ledger");
        let error = document
            .charge_document_cells(1)
            .expect_err("the scoped document must still enforce its own cumulative limit");

        assert_eq!(resources.document_cells_used(), 3);
        assert_eq!(document.document_cells_used(), 3);
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxDocumentCells,
                actual: 4,
                max: 3,
                ..
            })
        ));
    }
}
