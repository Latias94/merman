#[cfg(feature = "resources-class")]
use super::ClassComplexity;
#[cfg(feature = "resources-sequence")]
use super::SequenceComplexity;
use super::{
    RenderResourceLimitId, RenderResourcePolicy, RenderResourceProfile, ResourceLimitCause,
    ResourceLimitExceeded, ResourceLimitId, ResourceLimitOverride, ResourceLimitPhase,
};
#[cfg(feature = "resources-class")]
use merman_model::families::class::ClassDiagram;
use merman_model::presentation::PresentationDiagram;
use merman_model::{
    OperationCancelled, OperationControl, OperationLedgerError, OperationPhase,
    OperationResourceDomain, OperationResourceLimitExceeded, OperationResourceOverride,
    OperationResourceProvenance,
};

/// Failure returned by controlled render work accounting.
///
/// Cancellation is deliberately kept separate from a resource rejection. Adapter kernels may
/// expose only a neutral interruption signal, so the owning work control retains this structured
/// cause until the outer renderer maps it to its public error type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperationWorkError {
    #[error(transparent)]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[error(transparent)]
    ForeignResourceTerminal(OperationLedgerError),
}

/// One cumulative derived-geometry budget shared by layout and SVG emission.
#[derive(Debug)]
pub struct OperationWorkMeter {
    policy: RenderResourcePolicy,
    control: OperationControl,
    used: std::sync::atomic::AtomicUsize,
    projected_svg_bytes: std::sync::atomic::AtomicUsize,
    prepared_text_retained_bytes: std::sync::atomic::AtomicUsize,
    prepared_text_retained_bytes_peak: std::sync::atomic::AtomicUsize,
}

/// Immutable operation-owned admission for retained SVG bytes, separate from producer charges.
///
/// A document writer may reuse admitted capacity while observing cancellation at bounded work
/// intervals. The ceiling and its error provenance share the originating meter's lifetime.
pub struct SvgByteBudget<'a> {
    meter: &'a OperationWorkMeter,
    maximum: Option<usize>,
}

impl SvgByteBudget<'_> {
    pub fn observe_operation(&self) -> Result<(), OperationWorkError> {
        self.meter.checkpoint(OperationPhase::Emit)
    }

    /// Checks exact serialized length without charging a producer contribution or observing twice.
    fn appended_len(&self, current: usize, additional: usize) -> Result<usize, OperationWorkError> {
        let required = current.checked_add(additional).ok_or_else(|| {
            OperationWorkError::ResourceLimitExceeded(accumulation_overflow(
                self.meter.policy,
                ResourceLimitPhase::SvgOutput,
                RenderResourceLimitId::MaxSvgBytes,
            ))
        })?;
        if self.maximum.is_some_and(|maximum| required > maximum) {
            self.meter
                .policy
                .check_svg_byte_count(required, ResourceLimitPhase::SvgOutput)?;
        }
        Ok(required)
    }

    /// Caps optional backing storage and skips it after a cancellation request.
    /// The normal append path still owns terminal observation and precedence.
    pub fn capacity_hint_limit(&self) -> Option<usize> {
        (!self.meter.control().is_cancelled()).then_some(self.maximum.unwrap_or(usize::MAX))
    }

    /// Admits a geometrically grown capacity after the caller has observed operation control.
    ///
    /// Writes fitting the existing admitted capacity need neither a new reservation nor another
    /// policy lookup. Checking the requested length first preserves exact ceiling/overflow errors.
    pub fn reservation_capacity(
        &self,
        current_len: usize,
        current_capacity: usize,
        additional: usize,
    ) -> Result<usize, OperationWorkError> {
        let required = self.appended_len(current_len, additional)?;
        let target = required.max(current_capacity.saturating_mul(2));
        Ok(target.min(self.maximum.unwrap_or(usize::MAX)))
    }
}

pub struct SvgByteReservation {
    pub additional_bytes: usize,
    pub limit_error: Option<ResourceLimitExceeded>,
}

/// Operation-local ownership of prepared-text bytes admitted by the shared resource meter.
#[derive(Debug)]
#[must_use = "keep the reservation alive while its prepared-text bytes are retained"]
pub struct PreparedTextRetainedReservation {
    meter: std::sync::Arc<OperationWorkMeter>,
    retained_bytes: usize,
}

impl PreparedTextRetainedReservation {
    #[cfg(any(test, feature = "test-support"))]
    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Releases the unused portion of a conservative reservation.
    pub fn reconcile_downward(&mut self, retained_bytes: usize) {
        assert!(
            retained_bytes <= self.retained_bytes,
            "prepared-text retained-byte reconciliation cannot grow a reservation"
        );
        let released = self.retained_bytes - retained_bytes;
        self.meter.release_prepared_text_retained_bytes(released);
        self.retained_bytes = retained_bytes;
    }
}

impl Drop for PreparedTextRetainedReservation {
    fn drop(&mut self) {
        self.meter
            .release_prepared_text_retained_bytes(self.retained_bytes);
    }
}

impl OperationWorkMeter {
    pub fn new(policy: RenderResourcePolicy) -> Self {
        Self::new_with_control(policy, OperationControl::new())
    }

    pub fn new_with_control(policy: RenderResourcePolicy, control: OperationControl) -> Self {
        Self {
            policy,
            control,
            used: std::sync::atomic::AtomicUsize::new(0),
            projected_svg_bytes: std::sync::atomic::AtomicUsize::new(0),
            prepared_text_retained_bytes: std::sync::atomic::AtomicUsize::new(0),
            prepared_text_retained_bytes_peak: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub const fn policy(&self) -> RenderResourcePolicy {
        self.policy
    }

    pub fn control(&self) -> &OperationControl {
        &self.control
    }

    /// Replays the operation's first cancellation, ceiling, or overflow terminal.
    pub fn checkpoint(&self, phase: OperationPhase) -> Result<(), OperationWorkError> {
        self.control
            .terminal_checkpoint_at(phase)
            .map_err(|error| self.map_terminal_error(error))
    }

    fn resource_checkpoint(&self, phase: OperationPhase) -> Result<(), OperationWorkError> {
        self.checkpoint(phase)
    }

    pub fn preflight(&self, additional: usize) -> Result<(), OperationWorkError> {
        let phase = OperationPhase::Layout;
        self.resource_checkpoint(phase)?;
        if additional == 0 {
            return Ok(());
        }
        let used = self.used.load(std::sync::atomic::Ordering::Relaxed);
        let Some(next) = used.checked_add(additional) else {
            return Err(self.terminate_resource_error(
                self.layout_work_overflow_details(),
                phase,
                used,
                additional,
            ));
        };
        match self.policy.check_layout_work_units(next) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.terminate_resource_error(error, phase, used, additional)),
        }
    }

    /// Charges work atomically. A rejected charge leaves the cumulative usage unchanged.
    pub fn charge(&self, additional: usize) -> Result<(), OperationWorkError> {
        self.charge_work(additional, OperationPhase::Layout)
    }

    /// Charges render-time geometry work while preserving the shared layout-work ceiling.
    ///
    /// RoughJS generation happens during SVG emission, but it still consumes the same
    /// `max_layout_work_units` budget as other derived geometry. Keeping the operation phase as
    /// `Emit` makes cancellation and resource failures point at the stage that actually ran.
    pub fn charge_emit_work(&self, additional: usize) -> Result<(), OperationWorkError> {
        self.charge_work(additional, OperationPhase::Emit)
    }

    /// Charges the shared work ceiling while retaining the caller's operation phase.
    pub fn charge_work(
        &self,
        additional: usize,
        phase: OperationPhase,
    ) -> Result<(), OperationWorkError> {
        self.charge_work_inner(additional, phase, None).map(|_| ())
    }

    #[cfg(feature = "layout-shoal")]
    pub fn try_charge_optional_layout_work(
        &self,
        additional: usize,
        headroom: usize,
    ) -> Result<bool, OperationWorkError> {
        self.charge_work_inner(additional, OperationPhase::Layout, Some(headroom))
    }

    fn charge_work_inner(
        &self,
        additional: usize,
        phase: OperationPhase,
        headroom: Option<usize>,
    ) -> Result<bool, OperationWorkError> {
        self.resource_checkpoint(phase)?;
        if additional == 0 && headroom.is_none() {
            return Ok(true);
        }
        let mut used = self.used.load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let Some(next) = used.checked_add(additional) else {
                if headroom.is_some() {
                    return Ok(false);
                }
                return Err(self.terminate_resource_error(
                    self.layout_work_overflow_details(),
                    phase,
                    used,
                    additional,
                ));
            };
            if let Some(headroom) = headroom {
                let maximum = self
                    .policy
                    .value(ResourceLimitId::MaxLayoutWorkUnits)
                    .unwrap_or(usize::MAX);
                if next
                    .checked_add(headroom)
                    .is_none_or(|projected| projected > maximum)
                {
                    return Ok(false);
                }
                if additional == 0 {
                    return Ok(true);
                }
            } else if let Err(error) = self.policy.check_layout_work_units(next) {
                return Err(self.terminate_resource_error(error, phase, used, additional));
            }
            match self.used.compare_exchange_weak(
                used,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(true),
                Err(actual) => {
                    self.resource_checkpoint(phase)?;
                    used = actual;
                }
            }
        }
    }

    fn layout_work_overflow_details(&self) -> ResourceLimitExceeded {
        accumulation_overflow(
            self.policy,
            ResourceLimitPhase::LayoutModel,
            RenderResourceLimitId::MaxLayoutWorkUnits,
        )
    }

    /// Atomically reserves operation-local bytes retained by one prepared-text owner.
    pub fn reserve_prepared_text_retained_bytes(
        self: &std::sync::Arc<Self>,
        retained_bytes: usize,
    ) -> Result<PreparedTextRetainedReservation, ResourceLimitExceeded> {
        let mut current = self
            .prepared_text_retained_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let next = current.checked_add(retained_bytes).ok_or_else(|| {
                accumulation_overflow(
                    self.policy,
                    ResourceLimitPhase::LayoutModel,
                    RenderResourceLimitId::MaxPreparedTextRetainedBytes,
                )
            })?;
            self.policy.check_prepared_text_retained_bytes(next)?;
            match self.prepared_text_retained_bytes.compare_exchange_weak(
                current,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.prepared_text_retained_bytes_peak
                        .fetch_max(next, std::sync::atomic::Ordering::Relaxed);
                    return Ok(PreparedTextRetainedReservation {
                        meter: std::sync::Arc::clone(self),
                        retained_bytes,
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Checks whether additional prepared-artifact bytes fit without retaining them.
    ///
    /// Prepared producers use this before invoking or copying an opaque backend result. The
    /// owning sidecar still performs the atomic reservation once the exact artifact exists.
    pub fn preflight_prepared_text_retained_bytes(
        &self,
        additional_bytes: usize,
    ) -> Result<(), OperationWorkError> {
        self.checkpoint(OperationPhase::Layout)?;
        let current = self
            .prepared_text_retained_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        let next = current.checked_add(additional_bytes).ok_or_else(|| {
            OperationWorkError::ResourceLimitExceeded(accumulation_overflow(
                self.policy,
                ResourceLimitPhase::LayoutModel,
                RenderResourceLimitId::MaxPreparedTextRetainedBytes,
            ))
        })?;
        self.policy
            .check_prepared_text_retained_bytes(next)
            .map_err(OperationWorkError::ResourceLimitExceeded)
    }

    /// Returns the currently unreserved prepared-artifact budget for bounded policies.
    pub fn remaining_prepared_text_retained_bytes(&self) -> Option<usize> {
        let maximum = self
            .policy
            .value(ResourceLimitId::MaxPreparedTextRetainedBytes)?;
        Some(maximum.saturating_sub(self.prepared_text_retained_bytes()))
    }

    fn release_prepared_text_retained_bytes(&self, released_bytes: usize) {
        if released_bytes == 0 {
            return;
        }
        let previous = self
            .prepared_text_retained_bytes
            .fetch_sub(released_bytes, std::sync::atomic::Ordering::Relaxed);
        debug_assert!(previous >= released_bytes);
    }

    /// Checks whether one exact append fits the whole-document SVG ceiling without charging it.
    ///
    /// Producer-owned contribution accounting remains separate because `current_svg_bytes`
    /// already includes earlier emitted contributions. Callers that also precharge a disjoint
    /// contribution must do so exactly once after this append check succeeds.
    pub fn check_svg_append(
        &self,
        current_svg_bytes: usize,
        additional_svg_bytes: usize,
    ) -> Result<(), OperationWorkError> {
        let budget = self.svg_byte_budget();
        budget.observe_operation()?;
        budget
            .appended_len(current_svg_bytes, additional_svg_bytes)
            .map(|_| ())
    }

    /// Prepares immutable whole-document admission used to cap backing-buffer growth.
    ///
    /// This is intentionally independent of the producer-owned projected-byte ledger: the
    /// bounded document sink validates its retained bytes directly and must not subtract or charge
    /// separately owned contributions.
    pub fn svg_byte_budget(&self) -> SvgByteBudget<'_> {
        SvgByteBudget {
            meter: self,
            maximum: self.policy.value(ResourceLimitId::MaxSvgBytes),
        }
    }

    pub fn arithmetic_overflow(&self) -> OperationWorkError {
        self.terminate_resource_error(
            self.layout_work_overflow_details(),
            OperationPhase::Layout,
            self.used(),
            1,
        )
    }

    /// Charges one producer-owned, disjoint projected contribution to the SVG byte ledger.
    ///
    /// The final whole-document SVG check remains authoritative. This earlier cumulative charge
    /// prevents independently prepared expansions, such as icons or repeated source-style payloads,
    /// from allocating their complete strings before the operation-level output policy can reject
    /// them. Callers must not charge bytes already owned by another contribution.
    pub fn charge_svg_bytes(&self, additional: usize) -> Result<(), OperationWorkError> {
        let phase = OperationPhase::Emit;
        self.resource_checkpoint(phase)?;
        let mut used = self
            .projected_svg_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let Some(next) = used.checked_add(additional) else {
                return Err(self.terminate_resource_error(
                    accumulation_overflow(
                        self.policy,
                        ResourceLimitPhase::SvgOutput,
                        RenderResourceLimitId::MaxSvgBytes,
                    ),
                    phase,
                    used,
                    additional,
                ));
            };
            if let Err(error) = self
                .policy
                .check_svg_byte_count(next, ResourceLimitPhase::SvgOutput)
            {
                return Err(self.terminate_resource_error(error, phase, used, additional));
            }
            match self.projected_svg_bytes.compare_exchange_weak(
                used,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => {
                    self.resource_checkpoint(phase)?;
                    used = actual;
                }
            }
        }
    }

    pub fn preflight_geometry_json_bytes(
        &self,
        current: usize,
        additional: usize,
    ) -> Result<(), OperationWorkError> {
        let phase = OperationPhase::Emit;
        self.resource_checkpoint(phase)?;
        let Some(actual) = current.checked_add(additional) else {
            return Err(self.terminate_resource_error(
                accumulation_overflow(
                    self.policy,
                    ResourceLimitPhase::GeometryOutput,
                    RenderResourceLimitId::MaxGeometryJsonBytes,
                ),
                phase,
                current,
                additional,
            ));
        };
        self.policy
            .check_render_limit(
                ResourceLimitPhase::GeometryOutput,
                RenderResourceLimitId::MaxGeometryJsonBytes,
                actual,
            )
            .map_err(|error| self.terminate_resource_error(error, phase, current, additional))
    }

    /// Checks an absolute SVG byte projection without consuming the cumulative icon reservation.
    ///
    /// Whole-document postprocessors use this after a streaming size pass. A rejection is still an
    /// operation terminal, but a successful preflight must not double-count icon bytes already
    /// included in the projected document.
    pub fn preflight_svg_byte_count(
        &self,
        actual: usize,
        resource_phase: ResourceLimitPhase,
        operation_phase: OperationPhase,
    ) -> Result<(), OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        self.policy
            .check_svg_byte_count(actual, resource_phase)
            .map_err(|error| self.terminate_resource_error(error, operation_phase, 0, actual))
    }

    pub fn preflight_svg_structure(
        &self,
        elements: usize,
        tree_depth: usize,
        operation_phase: OperationPhase,
    ) -> Result<(), OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        self.policy
            .check_svg_structure(elements, tree_depth)
            .map_err(|error| self.terminate_absolute_resource_error(error, operation_phase))
    }

    pub fn preflight_svg_element_count(
        &self,
        elements: usize,
        operation_phase: OperationPhase,
    ) -> Result<(), OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        self.policy
            .check_svg_element_count(elements)
            .map_err(|error| self.terminate_absolute_resource_error(error, operation_phase))
    }

    pub fn preflight_parsed_render(
        &self,
        parsed: &PresentationDiagram,
        operation_phase: OperationPhase,
    ) -> Result<(), OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        let result = self.policy.check_parsed_render(parsed);
        self.resource_checkpoint(operation_phase)?;
        result.map_err(|error| self.terminate_absolute_resource_error(error, operation_phase))
    }

    #[cfg(feature = "resources-class")]
    pub fn preflight_class_complexity(
        &self,
        model: &ClassDiagram,
        operation_phase: OperationPhase,
    ) -> Result<ClassComplexity, OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        let result = self.policy.check_class_complexity(model);
        self.resource_checkpoint(operation_phase)?;
        result.map_err(|error| self.terminate_absolute_resource_error(error, operation_phase))
    }

    #[cfg(feature = "resources-sequence")]
    pub fn preflight_sequence_complexity(
        &self,
        model: &merman_model::families::sequence::SequenceDiagramModel,
        operation_phase: OperationPhase,
    ) -> Result<SequenceComplexity, OperationWorkError> {
        self.resource_checkpoint(operation_phase)?;
        let result = self.policy.check_sequence_complexity(model);
        self.resource_checkpoint(operation_phase)?;
        result.map_err(|error| self.terminate_absolute_resource_error(error, operation_phase))
    }

    pub fn terminate_absolute_resource_error(
        &self,
        error: ResourceLimitExceeded,
        operation_phase: OperationPhase,
    ) -> OperationWorkError {
        let actual = error.actual;
        self.terminate_resource_error(error, operation_phase, 0, actual)
    }

    pub fn terminate_svg_byte_count_overflow(
        &self,
        resource_phase: ResourceLimitPhase,
        operation_phase: OperationPhase,
    ) -> OperationWorkError {
        self.terminate_resource_error(
            accumulation_overflow(
                self.policy,
                resource_phase,
                RenderResourceLimitId::MaxSvgBytes,
            ),
            operation_phase,
            0,
            usize::MAX,
        )
    }

    fn terminate_resource_error(
        &self,
        error: ResourceLimitExceeded,
        phase: OperationPhase,
        consumed: usize,
        requested: usize,
    ) -> OperationWorkError {
        let provenance = self.operation_resource_provenance();
        let terminal = match error.cause {
            ResourceLimitCause::Ceiling => {
                self.control
                    .terminate_resource_limit(OperationResourceLimitExceeded {
                        id: error.limit,
                        phase,
                        resource_phase: error.phase.as_str(),
                        limit: saturating_u64(error.max),
                        consumed: saturating_u64(consumed),
                        requested: saturating_u64(requested),
                        provenance,
                    })
            }
            ResourceLimitCause::ArithmeticOverflow => self.control.terminate_resource_overflow(
                error.limit,
                phase,
                error.phase.as_str(),
                saturating_u64(error.actual),
                saturating_u64(error.max),
                provenance,
            ),
        };
        self.map_terminal_error(terminal)
    }

    fn operation_resource_provenance(&self) -> OperationResourceProvenance {
        OperationResourceProvenance::new(
            OperationResourceDomain::Render,
            Some(self.policy.profile()),
            self.policy
                .explicit_overrides()
                .map(|(id, value)| OperationResourceOverride {
                    id: id.as_str(),
                    value: saturating_u64(value),
                }),
        )
    }

    fn map_terminal_error(&self, error: OperationLedgerError) -> OperationWorkError {
        match error {
            OperationLedgerError::Cancelled(error) => OperationWorkError::Cancelled(error),
            OperationLedgerError::ResourceLimitExceeded(error) => self
                .project_resource_limit(&error)
                .map(OperationWorkError::ResourceLimitExceeded)
                .unwrap_or_else(|| {
                    OperationWorkError::ForeignResourceTerminal(
                        OperationLedgerError::ResourceLimitExceeded(error),
                    )
                }),
            OperationLedgerError::ArithmeticOverflow { .. } => self
                .project_resource_overflow(&error)
                .map(OperationWorkError::ResourceLimitExceeded)
                .unwrap_or(OperationWorkError::ForeignResourceTerminal(error)),
        }
    }

    fn project_resource_limit(
        &self,
        error: &OperationResourceLimitExceeded,
    ) -> Option<ResourceLimitExceeded> {
        let (profile, explicit_overrides) = project_render_provenance(&error.provenance)?;
        Some(ResourceLimitExceeded {
            cause: ResourceLimitCause::Ceiling,
            phase: ResourceLimitPhase::from_stable_id(error.resource_phase)?,
            limit: error.id,
            actual: saturating_usize(error.consumed.saturating_add(error.requested)),
            max: saturating_usize(error.limit),
            profile,
            explicit_overrides,
        })
    }

    fn project_resource_overflow(
        &self,
        error: &OperationLedgerError,
    ) -> Option<ResourceLimitExceeded> {
        let OperationLedgerError::ArithmeticOverflow {
            id,
            resource_phase,
            actual,
            maximum,
            provenance,
            ..
        } = error
        else {
            return None;
        };
        let (profile, explicit_overrides) = project_render_provenance(provenance)?;
        Some(ResourceLimitExceeded {
            cause: ResourceLimitCause::ArithmeticOverflow,
            phase: ResourceLimitPhase::from_stable_id(resource_phase)?,
            limit: id,
            actual: saturating_usize(*actual),
            max: saturating_usize(*maximum),
            profile,
            explicit_overrides,
        })
    }

    /// Returns the unreserved SVG budget for bounded policies after all successful charges.
    #[cfg(test)]
    pub(super) fn remaining_svg_bytes(&self) -> Option<usize> {
        let maximum = self.policy.value(ResourceLimitId::MaxSvgBytes)?;
        let used = self
            .projected_svg_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        Some(maximum.saturating_sub(used))
    }

    /// Atomically reserves up to `requested` additional SVG bytes.
    ///
    /// Bounded policies return a deterministic limit error alongside a partial reservation when
    /// the full amount does not fit. The caller may still succeed if its bounded producer emits no
    /// more than the reserved bytes, then reconcile the conservative reservation to actual output.
    pub fn reserve_svg_bytes_up_to(
        &self,
        requested: usize,
    ) -> Result<SvgByteReservation, OperationWorkError> {
        let phase = OperationPhase::Emit;
        self.resource_checkpoint(phase)?;
        let Some(maximum) = self.policy.value(ResourceLimitId::MaxSvgBytes) else {
            self.charge_svg_bytes(requested)?;
            return Ok(SvgByteReservation {
                additional_bytes: requested,
                limit_error: None,
            });
        };

        let mut used = self
            .projected_svg_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let available = maximum.saturating_sub(used);
            let additional_bytes = requested.min(available);
            let Some(next) = used.checked_add(additional_bytes) else {
                return Err(self.terminate_resource_error(
                    accumulation_overflow(
                        self.policy,
                        ResourceLimitPhase::SvgOutput,
                        RenderResourceLimitId::MaxSvgBytes,
                    ),
                    phase,
                    used,
                    additional_bytes,
                ));
            };
            match self.projected_svg_bytes.compare_exchange_weak(
                used,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let limit_error = (additional_bytes < requested).then(|| {
                        svg_reservation_limit_error(self.policy, maximum, used, requested)
                    });
                    return Ok(SvgByteReservation {
                        additional_bytes,
                        limit_error,
                    });
                }
                Err(actual) => {
                    self.resource_checkpoint(phase)?;
                    used = actual;
                }
            }
        }
    }

    /// Reconciles an earlier conservative SVG reservation with the bytes actually retained.
    pub fn reconcile_svg_bytes(
        &self,
        reserved: usize,
        actual: usize,
    ) -> Result<(), OperationWorkError> {
        self.resource_checkpoint(OperationPhase::Emit)?;
        if actual > reserved {
            return self.charge_svg_bytes(actual - reserved);
        }

        let released = reserved - actual;
        if released != 0 {
            let previous = self
                .projected_svg_bytes
                .fetch_sub(released, std::sync::atomic::Ordering::Relaxed);
            debug_assert!(previous >= released);
        }
        Ok(())
    }

    pub fn used(&self) -> usize {
        self.used.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn prepared_text_retained_bytes(&self) -> usize {
        self.prepared_text_retained_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn prepared_text_retained_bytes_peak(&self) -> usize {
        self.prepared_text_retained_bytes_peak
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn projected_svg_bytes(&self) -> usize {
        self.projected_svg_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn project_render_provenance(
    provenance: &OperationResourceProvenance,
) -> Option<(RenderResourceProfile, Vec<ResourceLimitOverride>)> {
    if provenance.domain != OperationResourceDomain::Render {
        return None;
    }
    let explicit_overrides = provenance
        .explicit_overrides
        .iter()
        .map(|override_| {
            Some(ResourceLimitOverride {
                id: ResourceLimitId::from_stable_id(override_.id)?,
                value: saturating_usize(override_.value),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some((provenance.profile?, explicit_overrides))
}

fn saturating_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn accumulation_overflow(
    policy: RenderResourcePolicy,
    phase: ResourceLimitPhase,
    id: RenderResourceLimitId,
) -> ResourceLimitExceeded {
    let limit = ResourceLimitId::Render(id);
    ResourceLimitExceeded {
        cause: ResourceLimitCause::ArithmeticOverflow,
        phase,
        limit: limit.as_str(),
        actual: usize::MAX,
        max: policy.value(limit).unwrap_or(usize::MAX),
        profile: policy.profile(),
        explicit_overrides: policy
            .explicit_overrides()
            .map(|(id, value)| ResourceLimitOverride { id, value })
            .collect(),
    }
}

fn svg_byte_limit_error(policy: RenderResourcePolicy, maximum: usize) -> ResourceLimitExceeded {
    ResourceLimitExceeded {
        cause: ResourceLimitCause::Ceiling,
        phase: ResourceLimitPhase::SvgOutput,
        limit: ResourceLimitId::MaxSvgBytes.as_str(),
        actual: maximum.saturating_add(1),
        max: maximum,
        profile: policy.profile(),
        explicit_overrides: policy
            .explicit_overrides()
            .map(|(id, value)| ResourceLimitOverride { id, value })
            .collect(),
    }
}

fn svg_reservation_limit_error(
    policy: RenderResourcePolicy,
    maximum: usize,
    used: usize,
    requested: usize,
) -> ResourceLimitExceeded {
    if used.checked_add(requested).is_none() {
        return accumulation_overflow(
            policy,
            ResourceLimitPhase::SvgOutput,
            RenderResourceLimitId::MaxSvgBytes,
        );
    }
    svg_byte_limit_error(policy, maximum)
}
