use crate::error::CliError;
use crate::resources::{CheckedSchedulingWeight, ResolvedResourcePolicy, ResourceLedgerError};
use std::sync::{Arc, Condvar, Mutex};
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::time::Duration;

const MIB: u64 = 1024 * 1024;
#[cfg(feature = "svg")]
const SVG_ALLOCATION_MULTIPLIER: u64 = 2;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const RAW_SVG_ALLOCATION_MULTIPLIER: u64 = 3;
const SOURCE_ALLOCATION_MULTIPLIER: u64 = 2;
const MODEL_TEXT_ALLOCATION_MULTIPLIER: u64 = 2;
const MODEL_ITEM_WEIGHT_BYTES: u64 = 256;
#[cfg(feature = "svg")]
const LAYOUT_WORK_UNIT_WEIGHT_BYTES: u64 = 64;
#[cfg(feature = "ascii")]
const ASCII_GRID_CELL_WEIGHT_BYTES: u64 = 64;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const EMBEDDED_PIXEL_BYTES: u64 = 8;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const ENCODER_AND_STACK_OVERHEAD_BYTES: u64 = 9 * MIB;
const BASIC_BACKEND_OVERHEAD_BYTES: u64 = MIB;

#[derive(Clone)]
pub(super) struct BackendAdmission {
    budget: Arc<BackendAdmissionBudget>,
    weight: u64,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    actual_prefix_weight: u64,
    enforce_actual_bound: bool,
}

impl BackendAdmission {
    #[cfg(feature = "svg")]
    pub(super) fn for_svg(resources: &ResolvedResourcePolicy) -> Result<Self, CliError> {
        let Some(_) = resources.batch().scheduling_weight_bytes else {
            return Self::unbounded(resources);
        };
        let svg = required_svg_limit(resources)?;
        let weight = checked_sum(&[
            svg_mermaid_phase_weight(resources)?,
            checked_mul(svg, SVG_ALLOCATION_MULTIPLIER)?,
            BASIC_BACKEND_OVERHEAD_BYTES,
        ])?;
        Self::bounded(resources, weight)
    }

    #[cfg(feature = "ascii")]
    pub(super) fn for_text(
        resources: &ResolvedResourcePolicy,
        ascii_resources: merman::ascii::AsciiResourcePolicy,
    ) -> Result<Self, CliError> {
        let Some(_) = resources.batch().scheduling_weight_bytes else {
            return Self::unbounded(resources);
        };
        let Some(grid_cells) =
            ascii_resources.value(merman::ascii::AsciiResourceLimitId::MaxGridCells)
        else {
            return Self::exclusive_unmeasured(resources);
        };
        let Some(document_cells) =
            ascii_resources.value(merman::ascii::AsciiResourceLimitId::MaxDocumentCells)
        else {
            return Self::exclusive_unmeasured(resources);
        };
        let Some(output_bytes) =
            ascii_resources.value(merman::ascii::AsciiResourceLimitId::MaxOutputBytes)
        else {
            return Self::exclusive_unmeasured(resources);
        };
        let surface_cells = u64::try_from(grid_cells.max(document_cells)).map_err(|_| {
            CliError::InvalidInput("ASCII surface admission weight does not fit u64".to_string())
        })?;
        let output_bytes = u64::try_from(output_bytes).map_err(|_| {
            CliError::InvalidInput("ASCII output admission weight does not fit u64".to_string())
        })?;
        let weight = checked_sum(&[
            semantic_phase_weight(resources)?,
            checked_mul(surface_cells, ASCII_GRID_CELL_WEIGHT_BYTES)?,
            output_bytes,
            BASIC_BACKEND_OVERHEAD_BYTES,
        ])?;
        Self::bounded(resources, weight)
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(super) fn for_raster(
        resources: &ResolvedResourcePolicy,
        options: &merman::svg::export::RasterOptions,
        bytes_per_pixel: u64,
        raw_svg: bool,
    ) -> Result<Self, CliError> {
        let Some(_) = resources.batch().scheduling_weight_bytes else {
            return Self::unbounded(resources);
        };
        let svg = required_svg_limit(resources)?;
        let mermaid = if raw_svg {
            0
        } else {
            svg_mermaid_phase_weight(resources)?
        };
        let svg_multiplier = if raw_svg {
            RAW_SVG_ALLOCATION_MULTIPLIER
        } else {
            SVG_ALLOCATION_MULTIPLIER
        };
        let prefix = checked_sum(&[mermaid, checked_mul(svg, svg_multiplier)?])?;
        let output_pixels = options.size_limit.max_pixels;
        let embedded_pixels = options.embedded_image_limit.max_total_pixels;
        let embedded_bytes = options
            .embedded_image_limit
            .max_total_bytes
            .map(|bytes| bytes.min(svg));
        let (Some(output_pixels), Some(embedded_pixels), Some(embedded_bytes)) =
            (output_pixels, embedded_pixels, embedded_bytes)
        else {
            return Self::exclusive(resources, prefix, embedded_bytes.unwrap_or(svg));
        };
        let encoding = checked_sum(&[
            embedded_bytes,
            checked_mul(output_pixels, bytes_per_pixel)?,
            checked_mul(embedded_pixels, EMBEDDED_PIXEL_BYTES)?,
            ENCODER_AND_STACK_OVERHEAD_BYTES,
        ])?;
        Self::bounded_encoding(resources, prefix, encoding)
    }

    #[cfg(feature = "pdf")]
    pub(super) fn for_pdf(
        resources: &ResolvedResourcePolicy,
        options: &merman::svg::export::PdfOptions,
        raw_svg: bool,
    ) -> Result<Self, CliError> {
        let Some(_) = resources.batch().scheduling_weight_bytes else {
            return Self::unbounded(resources);
        };
        let svg = required_svg_limit(resources)?;
        let mermaid = if raw_svg {
            0
        } else {
            svg_mermaid_phase_weight(resources)?
        };
        let svg_multiplier = if raw_svg {
            RAW_SVG_ALLOCATION_MULTIPLIER
        } else {
            SVG_ALLOCATION_MULTIPLIER
        };
        let prefix = checked_sum(&[mermaid, checked_mul(svg, svg_multiplier)?])?;
        let filter_pixels = options.filter_image_limit.max_total_pixels;
        let embedded_pixels = options.embedded_image_limit.max_total_pixels;
        let embedded_bytes = options
            .embedded_image_limit
            .max_total_bytes
            .map(|bytes| bytes.min(svg));
        let (Some(filter_pixels), Some(embedded_pixels), Some(embedded_bytes)) =
            (filter_pixels, embedded_pixels, embedded_bytes)
        else {
            return Self::exclusive(resources, prefix, embedded_bytes.unwrap_or(svg));
        };
        let encoding = checked_sum(&[
            embedded_bytes,
            checked_mul(filter_pixels, 8)?,
            checked_mul(embedded_pixels, EMBEDDED_PIXEL_BYTES)?,
            ENCODER_AND_STACK_OVERHEAD_BYTES,
        ])?;
        Self::bounded_encoding(resources, prefix, encoding)
    }

    fn bounded(resources: &ResolvedResourcePolicy, weight: u64) -> Result<Self, CliError> {
        let ledger = resources.checked_scheduling_weight();
        ledger.check_single(weight)?;
        Ok(Self {
            budget: Arc::new(BackendAdmissionBudget::new(ledger)),
            weight,
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            actual_prefix_weight: 0,
            enforce_actual_bound: true,
        })
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn bounded_encoding(
        resources: &ResolvedResourcePolicy,
        prefix: u64,
        encoding: u64,
    ) -> Result<Self, CliError> {
        let mut admission = Self::bounded(resources, checked_sum(&[prefix, encoding])?)?;
        admission.actual_prefix_weight = prefix;
        Ok(admission)
    }

    fn unbounded(resources: &ResolvedResourcePolicy) -> Result<Self, CliError> {
        let mut admission = Self::bounded(resources, 1)?;
        admission.enforce_actual_bound = false;
        Ok(admission)
    }

    #[cfg(feature = "ascii")]
    fn exclusive_unmeasured(resources: &ResolvedResourcePolicy) -> Result<Self, CliError> {
        let ledger = resources.checked_scheduling_weight();
        let weight = ledger.max().ok_or_else(|| {
            CliError::InvalidInput(
                "exclusive backend admission requires a finite scheduling budget".to_string(),
            )
        })?;
        Ok(Self {
            budget: Arc::new(BackendAdmissionBudget::new(ledger)),
            weight,
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            actual_prefix_weight: 0,
            enforce_actual_bound: false,
        })
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn exclusive(
        resources: &ResolvedResourcePolicy,
        prefix: u64,
        preparation_bytes: u64,
    ) -> Result<Self, CliError> {
        let ledger = resources.checked_scheduling_weight();
        let weight = ledger.max().ok_or_else(|| {
            CliError::InvalidInput(
                "exclusive backend admission requires a finite scheduling budget".to_string(),
            )
        })?;
        ledger.check_single(checked_sum(&[
            prefix,
            preparation_bytes,
            ENCODER_AND_STACK_OVERHEAD_BYTES,
        ])?)?;
        ledger.check_single(weight)?;
        Ok(Self {
            budget: Arc::new(BackendAdmissionBudget::new(ledger)),
            weight,
            actual_prefix_weight: prefix,
            enforce_actual_bound: true,
        })
    }

    /// Admits one backend operation while observing its cooperative control at the blocking
    /// boundary. The short timed waits keep cancellation and deadlines responsive without
    /// introducing an asynchronous executor into the synchronous CLI path.
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(super) fn acquire_controlled(
        &self,
        control: &merman::OperationControl,
    ) -> Result<BackendPermit, CliError> {
        self.budget
            .acquire_controlled(self.weight, control)
            .map_err(|error| match error {
                ControlledAcquireError::Cancelled(error) => {
                    CliError::Render(merman::RenderError::Cancelled(error))
                }
                ControlledAcquireError::Resource(error) => CliError::Resource(error),
            })
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) fn ensure_actual_weight(&self, actual_encoding: u64) -> Result<(), CliError> {
        let actual = checked_sum(&[self.actual_prefix_weight, actual_encoding])?;
        if self.enforce_actual_bound && actual > self.weight {
            if self
                .budget
                .maximum()
                .is_some_and(|maximum| actual > maximum)
            {
                self.budget.check_single(actual)?;
            }
            return Err(CliError::InvalidOutput(format!(
                "backend admission estimate was too small: actual={actual} reserved={}",
                self.weight
            )));
        }
        Ok(())
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn actual_raster_weight(
    plan: merman::svg::export::RasterPlan,
    embedded: merman::svg::export::EmbeddedImagePlan,
    bytes_per_pixel: u64,
) -> Result<u64, CliError> {
    let output_pixels = checked_mul(u64::from(plan.width_px), u64::from(plan.height_px))?;
    checked_encoding_weight(output_pixels, bytes_per_pixel, embedded)
}

#[cfg(feature = "pdf")]
pub(super) fn actual_pdf_weight(
    filter_plan: merman::svg::export::PdfFilterImagePlan,
    embedded: merman::svg::export::EmbeddedImagePlan,
) -> Result<u64, CliError> {
    checked_encoding_weight(filter_plan.effective_image_pixels, 8, embedded)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn checked_encoding_weight(
    primary_pixels: u64,
    primary_bytes_per_pixel: u64,
    embedded: merman::svg::export::EmbeddedImagePlan,
) -> Result<u64, CliError> {
    checked_sum(&[
        checked_mul(primary_pixels, primary_bytes_per_pixel)?,
        embedded.total_data_bytes,
        checked_mul(embedded.total_pixels, EMBEDDED_PIXEL_BYTES)?,
        ENCODER_AND_STACK_OVERHEAD_BYTES,
    ])
}

struct BackendAdmissionBudget {
    in_flight: Mutex<CheckedSchedulingWeight>,
    capacity_changed: Condvar,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
enum ControlledAcquireError {
    Cancelled(merman::OperationCancelled),
    Resource(ResourceLedgerError),
}

impl BackendAdmissionBudget {
    fn new(in_flight: CheckedSchedulingWeight) -> Self {
        Self {
            in_flight: Mutex::new(in_flight),
            capacity_changed: Condvar::new(),
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn maximum(&self) -> Option<u64> {
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .max()
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn check_single(&self, requested: u64) -> Result<(), ResourceLedgerError> {
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .check_single(requested)
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    fn acquire_controlled(
        self: &Arc<Self>,
        requested: u64,
        control: &merman::OperationControl,
    ) -> Result<BackendPermit, ControlledAcquireError> {
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        in_flight
            .check_single(requested)
            .map_err(ControlledAcquireError::Resource)?;
        loop {
            control
                .checkpoint_at(merman::OperationPhase::Admission)
                .map_err(ControlledAcquireError::Cancelled)?;
            match in_flight.try_acquire(requested) {
                Ok(()) => break,
                Err(
                    ResourceLedgerError::LimitExceeded { .. }
                    | ResourceLedgerError::ArithmeticOverflow { .. },
                ) if in_flight.max().is_some() => {
                    let (next, _) = self
                        .capacity_changed
                        .wait_timeout(in_flight, Duration::from_millis(25))
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    in_flight = next;
                }
                Err(error) => return Err(ControlledAcquireError::Resource(error)),
            }
        }
        Ok(BackendPermit {
            budget: Arc::clone(self),
            weight: requested,
        })
    }
}

pub(super) struct BackendPermit {
    budget: Arc<BackendAdmissionBudget>,
    weight: u64,
}

impl Drop for BackendPermit {
    fn drop(&mut self) {
        let mut in_flight = self
            .budget
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        in_flight
            .release(self.weight)
            .expect("a live backend permit must own its charged weight");
        self.budget.capacity_changed.notify_all();
    }
}

fn semantic_phase_weight(resources: &ResolvedResourcePolicy) -> Result<u64, CliError> {
    use merman::resources::InputResourceLimitId;

    let source = required_input_limit(
        resources,
        InputResourceLimitId::MaxSourceBytes,
        "max_source_bytes",
    )?;
    let model_items = required_input_limit(
        resources,
        InputResourceLimitId::MaxModelItems,
        "max_model_items",
    )?;
    let model_text = required_input_limit(
        resources,
        InputResourceLimitId::MaxModelTextBytes,
        "max_model_text_bytes",
    )?;
    checked_sum(&[
        checked_mul(source, SOURCE_ALLOCATION_MULTIPLIER)?,
        checked_mul(model_items, MODEL_ITEM_WEIGHT_BYTES)?,
        checked_mul(model_text, MODEL_TEXT_ALLOCATION_MULTIPLIER)?,
    ])
}

#[cfg(feature = "svg")]
fn svg_mermaid_phase_weight(resources: &ResolvedResourcePolicy) -> Result<u64, CliError> {
    checked_sum(&[
        semantic_phase_weight(resources)?,
        checked_mul(
            required_render_limit(
                resources,
                merman::svg::ResourceLimitId::MaxLayoutWorkUnits,
                "max_layout_work_units",
            )?,
            LAYOUT_WORK_UNIT_WEIGHT_BYTES,
        )?,
    ])
}

fn required_input_limit(
    resources: &ResolvedResourcePolicy,
    id: merman::resources::InputResourceLimitId,
    name: &'static str,
) -> Result<u64, CliError> {
    required_usize_bound(resources.input_policy().value(id), name)
}

#[cfg(feature = "svg")]
fn required_svg_limit(resources: &ResolvedResourcePolicy) -> Result<u64, CliError> {
    required_render_limit(
        resources,
        merman::svg::ResourceLimitId::MaxSvgBytes,
        "max_svg_bytes",
    )
}

#[cfg(feature = "svg")]
fn required_render_limit(
    resources: &ResolvedResourcePolicy,
    id: merman::svg::ResourceLimitId,
    name: &'static str,
) -> Result<u64, CliError> {
    required_usize_bound(resources.render_policy().value(id), name)
}

fn required_usize_bound(value: Option<usize>, name: &'static str) -> Result<u64, CliError> {
    let value = value.ok_or_else(|| {
        CliError::InvalidInput(format!(
            "{name} must remain bounded while max_scheduling_weight_bytes is bounded"
        ))
    })?;
    u64::try_from(value)
        .map_err(|_| CliError::InvalidInput(format!("{name} does not fit admission accounting")))
}

fn checked_mul(value: u64, multiplier: u64) -> Result<u64, CliError> {
    value.checked_mul(multiplier).ok_or_else(|| {
        CliError::InvalidInput("backend admission weight arithmetic overflow".to_string())
    })
}

fn checked_sum(values: &[u64]) -> Result<u64, CliError> {
    values.iter().try_fold(0_u64, |total, value| {
        total.checked_add(*value).ok_or_else(|| {
            CliError::InvalidInput("backend admission weight arithmetic overflow".to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::resources::ResourceProfile;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    #[test]
    fn admission_blocks_until_the_owned_permit_is_released() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        policy
            .apply_override("max_scheduling_weight_bytes", 10)
            .unwrap();
        let admission = BackendAdmission::bounded(&policy, 6).unwrap();
        let first = admission
            .acquire_controlled(&merman::OperationControl::new())
            .unwrap();
        let worker_admission = admission.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _second = worker_admission
                .acquire_controlled(&merman::OperationControl::new())
                .unwrap();
            acquired_tx.send(()).unwrap();
        });
        ready_rx.recv().unwrap();
        assert!(matches!(
            acquired_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ));
        drop(first);
        acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("waiting backend should resume after permit release");
        worker.join().unwrap();
    }

    #[test]
    fn oversized_backend_is_rejected_during_preparation() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        policy
            .apply_override("max_scheduling_weight_bytes", 10)
            .unwrap();
        let error = match BackendAdmission::bounded(&policy, 11) {
            Ok(_) => panic!("oversized backend should be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("max_scheduling_weight_bytes"));
    }

    #[test]
    fn admission_estimates_reject_integer_overflow() {
        let error = checked_mul(u64::MAX, 2).expect_err("overflow must be explicit");
        assert!(error.to_string().contains("arithmetic overflow"));

        let error = checked_sum(&[u64::MAX, 1]).expect_err("overflow must be explicit");
        assert!(error.to_string().contains("arithmetic overflow"));
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn exclusive_admission_checks_fixed_and_actual_working_sets_together() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        let maximum = ENCODER_AND_STACK_OVERHEAD_BYTES + 100;
        policy
            .apply_override("max_scheduling_weight_bytes", maximum)
            .unwrap();
        let admission = BackendAdmission::exclusive(&policy, 40, 0).unwrap();

        admission
            .ensure_actual_weight(ENCODER_AND_STACK_OVERHEAD_BYTES + 60)
            .expect("the combined working set exactly fits");
        let error = admission
            .ensure_actual_weight(ENCODER_AND_STACK_OVERHEAD_BYTES + 61)
            .expect_err("the combined working set exceeds the finite budget");
        assert!(error.to_string().contains("max_scheduling_weight_bytes"));
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn exclusive_admission_checks_the_known_backend_floor_during_preparation() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        policy
            .apply_override(
                "max_scheduling_weight_bytes",
                ENCODER_AND_STACK_OVERHEAD_BYTES,
            )
            .unwrap();

        let error = match BackendAdmission::exclusive(&policy, 1, 0) {
            Ok(_) => panic!("the fixed prefix plus backend floor exceeds the budget"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("max_scheduling_weight_bytes"));
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn exclusive_admission_checks_bounded_embedded_bytes_during_preparation() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        let maximum = ENCODER_AND_STACK_OVERHEAD_BYTES + 100;
        policy
            .apply_override("max_scheduling_weight_bytes", maximum)
            .unwrap();

        let error = match BackendAdmission::exclusive(&policy, 40, 61) {
            Ok(_) => panic!("bounded embedded bytes must be part of the preparation floor"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("max_scheduling_weight_bytes"));
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    #[test]
    fn unbounded_embedded_images_use_the_svg_envelope_during_preparation() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        policy.apply_override("max_svg_bytes", 10).unwrap();
        policy
            .apply_override(
                "max_scheduling_weight_bytes",
                ENCODER_AND_STACK_OVERHEAD_BYTES + 39,
            )
            .unwrap();
        let options = merman::svg::export::RasterOptions {
            embedded_image_limit: merman::svg::export::EmbeddedImageLimit::unbounded(),
            ..Default::default()
        };

        let error = match BackendAdmission::for_raster(&policy, &options, 8, true) {
            Ok(_) => panic!("raw SVG bytes must bound embedded-image preparation"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("max_scheduling_weight_bytes"));
    }

    #[test]
    fn scheduling_overflow_waits_for_capacity_instead_of_failing() {
        let mut policy = ResolvedResourcePolicy::for_profile(ResourceProfile::Constrained);
        policy
            .apply_override("max_scheduling_weight_bytes", u64::MAX)
            .unwrap();
        let admission = BackendAdmission::bounded(&policy, u64::MAX).unwrap();
        let first = admission
            .acquire_controlled(&merman::OperationControl::new())
            .unwrap();
        let worker_admission = BackendAdmission::bounded(&policy, 1).unwrap();
        let shared_budget = Arc::clone(&admission.budget);
        let worker_admission = BackendAdmission {
            budget: shared_budget,
            ..worker_admission
        };
        let (ready_tx, ready_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _second = worker_admission
                .acquire_controlled(&merman::OperationControl::new())
                .unwrap();
            acquired_tx.send(()).unwrap();
        });
        ready_rx.recv().unwrap();
        assert!(matches!(
            acquired_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ));
        drop(first);
        acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("overflowing in-flight sum should resume after capacity is released");
        worker.join().unwrap();
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn actual_backend_weight_rejects_integer_overflow() {
        let embedded = merman::svg::export::EmbeddedImagePlan {
            data_resources: 1,
            raster_images: 1,
            largest_data_bytes: u64::MAX,
            total_data_bytes: u64::MAX,
            largest_raster_pixels: 1,
            total_pixels: 1,
        };
        let error = checked_encoding_weight(1, 1, embedded)
            .expect_err("actual backend accounting must not saturate");
        assert!(error.to_string().contains("arithmetic overflow"));
    }
}
