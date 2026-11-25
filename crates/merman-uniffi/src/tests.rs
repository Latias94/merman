use super::*;
#[cfg(feature = "ascii")]
use merman_ascii_test_contracts::ascii_resource_boundary_contract;
#[cfg(feature = "analysis")]
use merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION;
use serde_json::Value;
#[cfg(feature = "svg")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(feature = "svg")]
use std::sync::{Barrier, Condvar, Mutex as StdMutex, Weak, mpsc};
use std::thread;

fn engine() -> Arc<Merman> {
    Merman::new()
}

#[test]
fn operation_control_can_be_cancelled_from_another_thread() {
    let control = MermanOperationControl::new(None);
    let worker_control = Arc::clone(&control);
    let worker = thread::spawn(move || worker_control.cancel());
    worker.join().expect("control cancellation worker");

    assert!(control.is_cancelled());
}

#[test]
fn operation_control_timeout_is_relative_and_structured() {
    let control = MermanOperationControl::new(Some(0));
    let cancelled = control
        .clone_control()
        .checkpoint_at(merman::OperationPhase::Admission)
        .expect_err("zero timeout must cancel at the first checkpoint");

    assert_eq!(cancelled.reason, merman::CancelReason::DeadlineExceeded);
    assert_eq!(cancelled.phase, merman::OperationPhase::Admission);
}

#[test]
fn generic_request_preserves_pre_cancelled_control_details_and_no_output() {
    let control = MermanOperationControl::new(None);
    control.cancel();
    let error = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "semantic-json".to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: None,
            options_json: None,
            control: Some(control),
        })
        .expect_err("pre-cancelled UniFFI request must publish no result");

    let MermanError::Binding {
        code,
        code_name,
        resource,
        cancellation,
        ..
    } = error;
    assert_eq!(code, BindingStatus::Cancelled.code());
    assert_eq!(code_name, BindingStatus::Cancelled.code_name());
    assert_eq!(resource, None);
    assert_eq!(
        cancellation,
        Some(MermanCancelledDetails {
            reason: "requested".to_string(),
            phase: "admission".to_string(),
        })
    );
}

#[test]
fn generic_request_preserves_zero_deadline_details_and_no_output() {
    let error = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "semantic-json".to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: None,
            options_json: None,
            control: Some(MermanOperationControl::new(Some(0))),
        })
        .expect_err("expired UniFFI request must publish no result");

    let MermanError::Binding {
        code,
        resource,
        cancellation,
        ..
    } = error;
    assert_eq!(code, BindingStatus::Cancelled.code());
    assert_eq!(resource, None);
    assert_eq!(
        cancellation,
        Some(MermanCancelledDetails {
            reason: "deadline_exceeded".to_string(),
            phase: "admission".to_string(),
        })
    );
}

#[test]
fn data_execution_preserves_full_error_and_cancellation_order() {
    let api = engine();
    let reusable = reusable_engine(None);
    let cancelled = MermanOperationControl::new(None);
    cancelled.cancel();
    for (operation_id, uri, control, status, reason) in [
        (
            "unknown-operation",
            None,
            Some(Arc::clone(&cancelled)),
            BindingStatus::UnsupportedOperation,
            None,
        ),
        (
            "document-analysis-json",
            None,
            Some(Arc::clone(&cancelled)),
            BindingStatus::InvalidArgument,
            None,
        ),
        (
            "semantic-json",
            Some("file:///diagram.mmd"),
            Some(Arc::clone(&cancelled)),
            BindingStatus::InvalidArgument,
            None,
        ),
        (
            "semantic-json",
            None,
            None,
            BindingStatus::OptionsJsonError,
            None,
        ),
        (
            "semantic-json",
            None,
            Some(Arc::clone(&cancelled)),
            BindingStatus::Cancelled,
            Some("requested"),
        ),
        (
            "semantic-json",
            None,
            Some(MermanOperationControl::new(Some(0))),
            BindingStatus::Cancelled,
            Some("deadline_exceeded"),
        ),
    ] {
        let request = MermanOperationRequestV4 {
            operation_id: operation_id.to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: uri.map(str::to_string),
            options_json: Some("{".to_string()),
            control,
        };
        for (data, full) in [
            (
                api.execute_data(request.clone()),
                api.execute(request.clone()),
            ),
            (
                reusable.execute_data(request.clone()),
                reusable.execute(request),
            ),
        ] {
            let data_error = data.expect_err("data request must publish no output");
            let full_error = full.expect_err("full request must publish no output");
            assert_eq!(format!("{data_error:?}"), format!("{full_error:?}"));
            let MermanError::Binding {
                code,
                resource,
                cancellation,
                ..
            } = data_error;
            assert_eq!(code, status.code(), "{operation_id}");
            assert_eq!(resource, None);
            assert_eq!(
                cancellation,
                reason.map(|reason| MermanCancelledDetails {
                    reason: reason.to_string(),
                    phase: "admission".to_string(),
                }),
                "{operation_id}"
            );
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn data_execution_preserves_resource_limits_and_reusable_output() {
    let api = engine();
    let reusable = reusable_engine(Some(
        r#"{"resources":{"limits":{"max_source_bytes":64}}}"#.to_string(),
    ));
    let source = "flowchart TD\nA --> B".to_string();
    let options = Some(r#"{"resources":{"limits":{"max_source_bytes":1}}}"#.to_string());
    let control = MermanOperationControl::new(None);
    let mut request =
        operation_request(OperationKey::SemanticJson, source.clone(), options.clone());
    request.control = Some(Arc::clone(&control));
    for (data, full, named) in [
        (
            api.execute_data(request.clone()),
            api.execute(request.clone()),
            api.parse_json(source.clone(), options.clone()),
        ),
        (
            reusable.execute_data(request.clone()),
            reusable.execute(request.clone()),
            reusable.parse_json(source.clone(), options),
        ),
    ] {
        let data_error = data.unwrap_err();
        let full_error = full.unwrap_err();
        let named_error = named.unwrap_err();
        assert_eq!(format!("{data_error:?}"), format!("{full_error:?}"));
        assert_eq!(format!("{named_error:?}"), format!("{full_error:?}"));
        let MermanError::Binding { code, resource, .. } = data_error;
        assert_eq!(code, BindingStatus::ResourceLimitExceeded.code());
        let resource = resource.expect("structured source resource limit");
        assert_eq!(resource.limit_id, "max_source_bytes");
        assert_eq!(resource.max, 1);
    }

    control
        .clone_control()
        .checkpoint()
        .expect("caller control remains usable");
    request.options_json = None;
    for (data, full, named) in [
        (
            api.execute_data(request.clone()),
            api.execute(request.clone()),
            api.parse_json(source.clone(), None),
        ),
        (
            reusable.execute_data(request.clone()),
            reusable.execute(request),
            reusable.parse_json(source, None),
        ),
    ] {
        let full = full.expect("resource failure must leave the engine usable");
        assert_eq!(data.unwrap(), full.data);
        assert_eq!(named.unwrap().as_bytes(), full.data);
        assert_eq!(full.metadata.operation_id, "semantic-json");
        assert_eq!(full.metadata.byte_length, full.data.len() as u64);
        let metadata: Value = serde_json::from_str(&full.metadata.raw_json).unwrap();
        assert_eq!(metadata["byte_length"], full.metadata.byte_length);
    }
}

#[cfg(feature = "svg")]
#[test]
fn data_execution_preserves_admission_and_service_error_order() {
    let measurer = CountingTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());
    let cancelled = MermanOperationControl::new(None);
    cancelled.cancel();
    let mut request = operation_request(
        OperationKey::SemanticJson,
        "flowchart TD\nA --> B".to_string(),
        Some(r#"{"environment":{"text_measurement":"deterministic"}}"#.to_string()),
    );
    for (control, status) in [
        (None, BindingStatus::InvalidArgument),
        (Some(Arc::clone(&cancelled)), BindingStatus::Cancelled),
    ] {
        request.control = control;
        let data_error = reusable.execute_data(request.clone()).unwrap_err();
        let full_error = reusable.execute(request.clone()).unwrap_err();
        assert_eq!(format!("{data_error:?}"), format!("{full_error:?}"));
        let MermanError::Binding { code, message, .. } = data_error;
        assert_eq!(code, status.code());
        if status == BindingStatus::InvalidArgument {
            assert!(message.contains("host-text-measurement"));
        }
    }
    assert_eq!(measurer.calls(), 0);

    request.operation_id = "unknown-operation".to_string();
    request.options_json = Some("{".to_string());
    {
        let _operation = reusable.admission.enter_operation().unwrap();
        for callback_active in [false, true] {
            let _callback = callback_active.then(|| reusable.admission.enter_callback().unwrap());
            let data_error = reusable.execute_data(request.clone()).unwrap_err();
            let full_error = reusable.execute(request.clone()).unwrap_err();
            assert_eq!(format!("{data_error:?}"), format!("{full_error:?}"));
            if callback_active {
                assert_reentrant_error(&data_error);
            } else {
                assert_busy_error(&data_error);
            }
        }
    }
    reusable.close().unwrap();
    let data_error = reusable.execute_data(request.clone()).unwrap_err();
    let full_error = reusable.execute(request).unwrap_err();
    assert_eq!(format!("{data_error:?}"), format!("{full_error:?}"));
    let MermanError::Binding { code, message, .. } = data_error;
    assert_eq!(code, BindingStatus::InvalidArgument.code());
    assert!(message.contains("closed"));
    assert_eq!(measurer.calls(), 0);
}

fn reusable_engine(options_json: Option<String>) -> Arc<MermanEngine> {
    MermanEngine::new(options_json, None).expect("reusable engine")
}

#[cfg(feature = "svg")]
fn callback_engine<T>(options_json: Option<String>, measurer: Arc<T>) -> Arc<MermanEngine>
where
    T: MermanTextMeasurer + 'static,
{
    let measurer: Arc<dyn MermanTextMeasurer> = measurer;
    let services = MermanEngineServices::new().with_text_measurer(measurer);
    MermanEngine::new(options_json, Some(services)).expect("callback engine")
}

fn assert_missing_capability(error: &MermanError, expected_capability_id: &str) {
    let MermanError::Binding {
        code,
        code_name,
        kind,
        capability_id,
        ..
    } = error;
    assert_eq!(*code, BindingStatus::UnsupportedOperation.code());
    assert_eq!(
        code_name.as_str(),
        BindingStatus::UnsupportedOperation.code_name()
    );
    assert_eq!(*kind, MermanErrorKind::MissingCapability);
    assert_eq!(capability_id.as_deref(), Some(expected_capability_id));
}

#[cfg(feature = "ascii")]
fn assert_ascii_resource_error(
    error: MermanError,
    expected_limit_id: &str,
    expected_phase: &str,
    expected_max: u64,
) -> MermanResourceErrorDetails {
    let MermanError::Binding {
        code,
        code_name,
        capability_id,
        resource,
        ..
    } = error;
    assert_eq!(code, BindingStatus::ResourceLimitExceeded.code());
    assert_eq!(
        code_name.as_str(),
        BindingStatus::ResourceLimitExceeded.code_name()
    );
    assert_eq!(capability_id, None);
    let details = resource.expect("ASCII resource errors must expose typed details");
    assert_eq!(details.limit_id, expected_limit_id);
    assert_eq!(details.phase, expected_phase);
    assert_eq!(details.max, expected_max);
    assert_eq!(details.profile, "interactive");
    assert!(details.actual > details.max);
    details
}

#[cfg(feature = "ascii")]
fn uniffi_ascii_options(id: MermanResourceOverrideId, value: u64) -> String {
    resource_options_json(None, vec![MermanResourceLimitOverride { id, value }])
        .expect("generated UniFFI ASCII override must produce valid options JSON")
}

#[cfg(feature = "svg")]
fn assert_reentrant_error(error: &MermanError) {
    let MermanError::Binding {
        code,
        code_name,
        kind,
        message,
        ..
    } = error;
    assert_eq!(*code, BindingStatus::InvalidArgument.code());
    assert_eq!(
        code_name.as_str(),
        BindingStatus::InvalidArgument.code_name()
    );
    assert_eq!(*kind, MermanErrorKind::ReentrantCall);
    assert!(message.contains("cannot be re-entered from its callback"));
}

#[cfg(feature = "svg")]
fn assert_busy_error(error: &MermanError) {
    let MermanError::Binding {
        code,
        code_name,
        kind,
        message,
        ..
    } = error;
    assert_eq!(*code, BindingStatus::Busy.code());
    assert_eq!(code_name.as_str(), BindingStatus::Busy.code_name());
    assert_eq!(*kind, MermanErrorKind::Busy);
    assert!(message.contains("reusable engine is busy"));
}

#[cfg(feature = "svg")]
struct CountingTextMeasurer {
    calls: AtomicUsize,
    font_styles: StdMutex<Vec<String>>,
    operations: StdMutex<Vec<MermanTextMeasurementOperation>>,
}

#[cfg(feature = "svg")]
struct FailingTextMeasurer {
    calls: AtomicUsize,
}

#[cfg(feature = "svg")]
struct MissingTextMeasurer {
    calls: AtomicUsize,
}

#[cfg(feature = "svg")]
struct FixedTextMeasurer {
    result: MermanTextMeasureResult,
}

#[cfg(feature = "svg")]
struct BlockingFailingTextMeasurer {
    state: (StdMutex<BlockingFailingTextMeasurerState>, Condvar),
}

#[cfg(feature = "svg")]
struct ReentrantRenderTextMeasurer {
    engine: StdMutex<Option<Weak<MermanEngine>>>,
}

#[cfg(feature = "svg")]
struct CrossThreadReentrantTextMeasurer {
    engine: StdMutex<Option<Weak<MermanEngine>>>,
}

#[cfg(feature = "svg")]
struct ReentrantCloseTextMeasurer {
    engine: StdMutex<Option<Weak<MermanEngine>>>,
}

#[cfg(feature = "svg")]
struct DropCountingTextMeasurer {
    drops: Arc<AtomicUsize>,
}

#[cfg(feature = "svg")]
struct ReentrantDropTextMeasurer {
    engine: StdMutex<Option<Weak<MermanEngine>>>,
    dropped: Arc<AtomicBool>,
}

#[cfg(feature = "svg")]
struct BlockingFailingTextMeasurerState {
    entered: bool,
    released: bool,
}

#[cfg(not(feature = "svg"))]
struct UnavailableTextMeasurer;

#[cfg(not(feature = "svg"))]
impl MermanTextMeasurer for UnavailableTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        Ok(None)
    }
}

#[cfg(feature = "svg")]
impl CountingTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            font_styles: StdMutex::new(Vec::new()),
            operations: StdMutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn saw_font_style(&self, expected: &str) -> bool {
        self.font_styles
            .lock()
            .unwrap()
            .iter()
            .any(|font_style| font_style == expected)
    }

    fn saw_operation(&self, expected: MermanTextMeasurementOperation) -> bool {
        self.operations.lock().unwrap().contains(&expected)
    }
}

#[cfg(feature = "svg")]
impl FailingTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "svg")]
impl MissingTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "svg")]
impl BlockingFailingTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: (
                StdMutex::new(BlockingFailingTextMeasurerState {
                    entered: false,
                    released: false,
                }),
                Condvar::new(),
            ),
        })
    }

    fn wait_until_entered(&self) {
        let (lock, cvar) = &self.state;
        let mut state = lock.lock().unwrap();
        while !state.entered {
            state = cvar.wait(state).unwrap();
        }
    }

    fn release(&self) {
        let (lock, cvar) = &self.state;
        let mut state = lock.lock().unwrap();
        state.released = true;
        cvar.notify_all();
    }
}

#[cfg(feature = "svg")]
impl ReentrantRenderTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            engine: StdMutex::new(None),
        })
    }

    fn set_engine(&self, engine: &Arc<MermanEngine>) {
        *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
    }
}

#[cfg(feature = "svg")]
impl CrossThreadReentrantTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            engine: StdMutex::new(None),
        })
    }

    fn set_engine(&self, engine: &Arc<MermanEngine>) {
        *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
    }
}

#[cfg(feature = "svg")]
impl ReentrantCloseTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            engine: StdMutex::new(None),
        })
    }

    fn set_engine(&self, engine: &Arc<MermanEngine>) {
        *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
    }
}

#[cfg(feature = "svg")]
impl ReentrantDropTextMeasurer {
    fn new(dropped: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self {
            engine: StdMutex::new(None),
            dropped,
        })
    }

    fn set_engine(&self, engine: &Arc<MermanEngine>) {
        *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for CountingTextMeasurer {
    fn measure(
        &self,
        request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.font_styles
            .lock()
            .unwrap()
            .push(request.font_style.clone());
        self.operations.lock().unwrap().push(request.operation);
        assert!(request.font_size.is_finite());
        assert!(request.line_height.is_finite());
        let width = (request.text.chars().count() as f64 * 9.0).max(1.0);
        let height = request.line_height.max(1.0);
        let mut result = MermanTextMeasureResult {
            result_kind: MermanTextMeasurementResultKind::Metrics,
            width: 0.0,
            height: 0.0,
            length: 0.0,
            line_count: 0,
            bbox_left: None,
            bbox_right: None,
            raw_width: None,
        };
        match request.operation {
            MermanTextMeasurementOperation::Measure
            | MermanTextMeasurementOperation::Wrapped
            | MermanTextMeasurementOperation::MermaidCalculateTextDimensions => {
                result.width = width;
                result.height = height;
                result.line_count = 1;
            }
            MermanTextMeasurementOperation::ComputedLength
            | MermanTextMeasurementOperation::SimpleBBoxWidth
            | MermanTextMeasurementOperation::RawBBoxWidth
            | MermanTextMeasurementOperation::BoundingClientRectWidth
            | MermanTextMeasurementOperation::TspanBBoxWidth
            | MermanTextMeasurementOperation::WrapProbeBBoxWidth
            | MermanTextMeasurementOperation::CanvasMeasureTextWidth => {
                result.result_kind = MermanTextMeasurementResultKind::Length;
                result.length = width;
            }
            MermanTextMeasurementOperation::TspanBBoxHeight
            | MermanTextMeasurementOperation::HtmlLineHeight
            | MermanTextMeasurementOperation::SimpleBBoxHeight
            | MermanTextMeasurementOperation::RawBBoxHeight => {
                result.result_kind = MermanTextMeasurementResultKind::Length;
                result.length = height;
            }
            MermanTextMeasurementOperation::CreateTextBBoxYOffset
            | MermanTextMeasurementOperation::RawBBoxYOffset
            | MermanTextMeasurementOperation::RawHangingBBoxYOffset => {
                result.result_kind = MermanTextMeasurementResultKind::Length;
                result.length = -1.0;
            }
            MermanTextMeasurementOperation::CreateTextMiddleBBoxYOffset
            | MermanTextMeasurementOperation::RawMiddleBBoxYOffset => {
                result.result_kind = MermanTextMeasurementResultKind::Length;
                result.length = -2.0;
            }
            MermanTextMeasurementOperation::BBoxX
            | MermanTextMeasurementOperation::BBoxXWithAsciiOverhang
            | MermanTextMeasurementOperation::TitleBBoxX => {
                result.result_kind = MermanTextMeasurementResultKind::HorizontalExtents;
                result.bbox_left = Some(width / 2.0);
                result.bbox_right = Some(width / 2.0);
            }
            MermanTextMeasurementOperation::WrappedWithRawWidth => {
                result.result_kind = MermanTextMeasurementResultKind::WrappedWithRawWidth;
                result.width = width;
                result.height = height;
                result.line_count = 1;
                result.raw_width = Some(width);
            }
        }
        Ok(Some(result))
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for FailingTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(MermanError::internal("test host measurer failed"))
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for MissingTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for FixedTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        Ok(Some(self.result))
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for BlockingFailingTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        let (lock, cvar) = &self.state;
        let mut state = lock.lock().unwrap();
        state.entered = true;
        cvar.notify_all();
        while !state.released {
            state = cvar.wait(state).unwrap();
        }
        Err(MermanError::internal("blocked host measurer failed"))
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for ReentrantRenderTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        let engine = self
            .engine
            .lock()
            .unwrap()
            .as_ref()
            .expect("reentrant render measurer should have an engine")
            .upgrade()
            .expect("reentrant engine should remain alive during its callback");
        let error = engine
            .render_svg("flowchart TD\nNested[Call]".to_string(), None)
            .expect_err("same-engine callback reentry must be rejected");
        assert_reentrant_error(&error);
        Err(error)
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for CrossThreadReentrantTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        let engine = self
            .engine
            .lock()
            .unwrap()
            .as_ref()
            .expect("cross-thread reentrant measurer should have an engine")
            .upgrade()
            .expect("reentrant engine should remain alive during its callback");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = engine.render_svg("flowchart TD\nNested[Call]".to_string(), None);
            tx.send(result).ok();
        });

        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Err(error)) => {
                assert_reentrant_error(&error);
                Err(error)
            }
            Ok(Ok(_)) => Err(MermanError::internal(
                "cross-thread reentrant render succeeded",
            )),
            Err(_) => Err(MermanError::internal(
                "cross-thread reentrant render did not finish",
            )),
        }
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for ReentrantCloseTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        let engine = self
            .engine
            .lock()
            .unwrap()
            .as_ref()
            .expect("reentrant close measurer should have an engine")
            .upgrade()
            .expect("engine should remain alive during its callback");
        let error = engine
            .close()
            .expect_err("close from the active callback must be retryable");
        assert_reentrant_error(&error);
        Err(error)
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for DropCountingTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        Ok(None)
    }
}

#[cfg(feature = "svg")]
impl Drop for DropCountingTextMeasurer {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(feature = "svg")]
impl MermanTextMeasurer for ReentrantDropTextMeasurer {
    fn measure(
        &self,
        _request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
        Ok(None)
    }
}

#[cfg(feature = "svg")]
impl Drop for ReentrantDropTextMeasurer {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.lock().unwrap().as_ref().and_then(Weak::upgrade) {
            engine
                .close()
                .expect("destructor reentry observes an idempotently closed engine");
        }
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[test]
fn engine_render_svg_matches_the_uniffi_feature_surface() {
    let result = engine().render_svg("flowchart TD\nA[Hello] --> B[World]".to_string(), None);

    if native_artifact_contract()
        .runtime_capabilities()
        .has_operation("svg")
    {
        let svg = result.unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    } else {
        let MermanError::Binding {
            code,
            code_name,
            kind,
            capability_id,
            resource,
            message,
            ..
        } = result.unwrap_err();
        assert_eq!(code, BindingStatus::UnsupportedOperation.code());
        assert_eq!(code_name, BindingStatus::UnsupportedOperation.code_name());
        assert_eq!(kind, MermanErrorKind::MissingCapability);
        assert_eq!(capability_id.as_deref(), Some("svg"));
        assert_eq!(resource, None);
        assert_eq!(
            message,
            "operation `svg` requires capability `svg`, which is not exposed by target `native`"
        );
    }
}

#[test]
fn generic_operation_uses_the_descriptor_owned_semantic_path() {
    let result = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "semantic-json".to_string(),
            source: "flowchart TD\nA[Hello] --> B[World]".to_string(),
            uri: None,
            options_json: Some(r#"{"runtime_policy":"deterministic"}"#.to_string()),
            control: None,
        })
        .unwrap();

    assert_eq!(result.operation_id, "semantic-json");
    assert_eq!(result.media_type, "application/json");
    assert!(
        String::from_utf8(result.data)
            .unwrap()
            .contains("flowchart-v2")
    );
    let metadata: Value = serde_json::from_str(&result.metadata.raw_json).unwrap();
    assert_eq!(metadata["operation_id"], "semantic-json");
    assert_eq!(metadata["version"], 1);
    assert_eq!(metadata["runtime_policy"], "deterministic");
    assert_eq!(result.metadata.version, 1);
    assert_eq!(result.metadata.operation_id, "semantic-json");
    assert_eq!(result.metadata.media_type, "application/json");
    assert_eq!(result.metadata.runtime_policy, "deterministic");
    assert_eq!(result.metadata.byte_length, metadata["byte_length"]);
    assert_eq!(result.metadata.output_plan, None);
}

#[cfg(feature = "svg")]
#[test]
fn generic_new_family_svg_error_keeps_reusable_engine_usable() {
    let engine = reusable_engine(None);
    let error = engine
        .execute(MermanOperationRequestV4 {
            operation_id: "svg".to_string(),
            source: "agentflow-beta\nend\n".to_string(),
            uri: None,
            options_json: None,
            control: None,
        })
        .expect_err("invalid Agentflow must return an error without an output result");
    let MermanError::Binding {
        code, code_name, ..
    } = error;
    assert_eq!(code, BindingStatus::ParseError.code());
    assert_eq!(code_name, BindingStatus::ParseError.code_name());

    let result = engine
        .execute(MermanOperationRequestV4 {
            operation_id: "svg".to_string(),
            source: "usecase-beta\nactor User(\"Customer\")\nTask(\"Checkout\")\nUser --> Task\n"
                .to_string(),
            uri: None,
            options_json: None,
            control: None,
        })
        .expect("the same engine must render Usecase after a failed Agentflow operation");
    assert_eq!(result.operation_id, "svg");
    assert_eq!(result.media_type, "image/svg+xml");
    let svg = String::from_utf8(result.data).expect("UTF-8 SVG");
    assert!(svg.contains("<svg"));
    assert!(svg.contains(r#"data-usecase-id="Task""#));
    assert!(svg.contains("Checkout"));
    assert_eq!(result.metadata.operation_id, "svg");
    assert_eq!(result.metadata.byte_length, svg.len() as u64);
}

#[test]
fn typed_metadata_preserves_unknown_future_output_plans() {
    let metadata = BindingOperationMetadata::from_json_bytes(
        br#"{
                "version":1,
                "operation_id":"future-binary",
                "media_type":"application/octet-stream",
                "runtime_policy":"deterministic",
                "byte_length":7,
                "output_plan":{"kind":"future-plan","nested":{"value":3}}
            }"#,
    )
    .unwrap();
    let metadata = uniffi_operation_metadata(metadata).unwrap();

    let Some(plan) = metadata.output_plan else {
        panic!("future output plan must remain open and lossless");
    };
    assert_eq!(plan.kind, "future-plan");
    assert_eq!(plan.raster, None);
    assert_eq!(plan.pdf_filter_images, None);
    let raw: Value = serde_json::from_str(&plan.raw_json).unwrap();
    assert_eq!(raw["nested"]["value"], 3);
    assert_eq!(metadata.byte_length, 7);
}

#[cfg(feature = "svg")]
#[test]
fn generic_operation_exposes_the_svg_capability_plan() {
    let result = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "svg-plan-json".to_string(),
            source: "flowchart TD\nA[Hello] --> B[World]".to_string(),
            uri: None,
            options_json: None,
            control: None,
        })
        .unwrap();

    assert_eq!(result.operation_id, "svg-plan-json");
    assert_eq!(result.media_type, "application/json");
    let plan: Value = serde_json::from_slice(&result.data).unwrap();
    assert_eq!(plan["planned_operation_id"], "svg");
    assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
    assert_eq!(plan["ready"], true);

    let named: Value = serde_json::from_str(
        &engine()
            .svg_plan_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap(),
    )
    .unwrap();
    let reusable: Value = serde_json::from_str(
        &reusable_engine(None)
            .svg_plan_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(named, plan);
    assert_eq!(reusable, plan);
}

#[cfg(all(
    feature = "svg",
    not(feature = "layout-cytoscape"),
    not(feature = "layout-dagre"),
    not(feature = "layout-elk"),
    not(feature = "math")
))]
#[test]
fn ambient_render_dependencies_do_not_widen_the_uniffi_owner_contract() {
    let facade = engine();
    let catalog: Value = serde_json::from_str(&facade.runtime_catalog_json().unwrap()).unwrap();
    let capability_ids = catalog["capabilities"]["capability_ids"]
        .as_array()
        .expect("capability IDs");
    assert!(capability_ids.iter().any(|id| id == "svg"));

    for (capability_id, source) in [
        (
            "layout-cytoscape",
            "architecture-beta\n  service api(server)[API]",
        ),
        (
            "layout-dagre",
            "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B",
        ),
        ("math", "sequenceDiagram\nA->>B: $$x^2$$"),
    ] {
        assert!(!capability_ids.iter().any(|id| id == capability_id));

        let plan: Value = serde_json::from_str(
            &facade
                .svg_plan_json(source.to_string(), None)
                .expect("SVG plan"),
        )
        .unwrap();
        assert_eq!(
            plan["required_capability_ids"],
            serde_json::json!([capability_id])
        );
        assert_eq!(
            plan["missing_capability_ids"],
            serde_json::json!([capability_id])
        );
        assert_eq!(plan["ready"], false);

        let error = facade
            .render_svg(source.to_string(), None)
            .expect_err("the UniFFI owner contract must reject ambient render capabilities");
        assert_missing_capability(&error, capability_id);
    }

    let error = facade
        .render_svg(
            "flowchart TD\nA --> B".to_string(),
            Some(r#"{"environment":{"math_renderer":"ratex"}}"#.to_string()),
        )
        .expect_err("explicit ratex selection requires owner-selected math");
    assert_missing_capability(&error, "math");
}

#[test]
fn generic_one_shot_native_policy_matches_the_owner_adapter_probe() {
    let request = MermanOperationRequestV4 {
        operation_id: "semantic-json".to_string(),
        source: "flowchart TD\nA --> B".to_string(),
        uri: None,
        options_json: Some(r#"{"runtime_policy":"native"}"#.to_string()),
        control: None,
    };
    let artifact_contract = native_artifact_contract();
    assert_eq!(
        artifact_contract.runtime_policy_exposure(),
        merman_bindings_core::RuntimePolicyExposure::BindingOptions
    );
    let capabilities = artifact_contract.runtime_capabilities();
    let missing_adapter = ["system-clock", "system-timezone", "system-random"]
        .into_iter()
        .find(|adapter| {
            !capabilities
                .system_adapter_ids
                .iter()
                .any(|id| id == adapter)
        });

    if let Some(missing_adapter) = missing_adapter {
        assert!(capabilities.system_adapter_ids.is_empty());
        for adapter_id in ["system-clock", "system-random", "system-timezone"] {
            assert!(
                !capabilities
                    .capability_ids
                    .iter()
                    .any(|id| id == &adapter_id)
            );
        }
        let error = engine().execute(request).unwrap_err();
        assert_missing_capability(&error, missing_adapter);
    } else {
        let result = engine().execute(request).unwrap();
        assert_eq!(
            capabilities.system_adapter_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
        let metadata: Value = serde_json::from_str(&result.metadata.raw_json).unwrap();
        assert_eq!(metadata["runtime_policy"], "native");
    }
}

#[test]
fn generic_operation_distinguishes_unknown_operation_from_missing_capability() {
    let error = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "not-an-operation".to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: None,
            options_json: None,
            control: None,
        })
        .expect_err("unknown operation must fail before dispatch");

    let MermanError::Binding {
        kind,
        capability_id,
        ..
    } = error;
    assert_eq!(kind, MermanErrorKind::UnknownOperation);
    assert_eq!(capability_id, None);
}

#[cfg(feature = "png")]
#[test]
fn engine_exposes_real_png_output() {
    let source = "flowchart TD\nA[Hello] --> B[World]".to_string();
    let bytes = engine().render_png(source.clone(), None).unwrap();
    let result = engine().render_png_result(source.clone(), None).unwrap();
    let reusable = reusable_engine(None);
    assert_eq!(reusable.render_png(source.clone(), None).unwrap(), bytes);
    let reusable_result = reusable.render_png_result(source, None).unwrap();
    assert_eq!(reusable_result, result);
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(result.data, bytes);
    assert_eq!(result.metadata.byte_length, result.data.len() as u64);
    let plan = result
        .metadata
        .output_plan
        .expect("PNG metadata must include a raster plan");
    assert_eq!(plan.kind, "raster");
    assert!(plan.raster.is_some());
    assert_eq!(plan.pdf_filter_images, None);
    let raw: Value = serde_json::from_str(&plan.raw_json).unwrap();
    assert_eq!(raw["kind"], "raster");
}

#[cfg(feature = "jpeg")]
#[test]
fn engine_exposes_real_jpeg_output() {
    let bytes = engine()
        .render_jpeg("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
        .unwrap();
    assert_eq!(&bytes[..3], b"\xff\xd8\xff");
}

#[cfg(feature = "pdf")]
#[test]
fn engine_exposes_real_pdf_output() {
    let source = "flowchart TD\nA[Hello] --> B[World]".to_string();
    let bytes = engine().render_pdf(source.clone(), None).unwrap();
    let result = engine().render_pdf_result(source, None).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(result.data, bytes);
    let plan = result
        .metadata
        .output_plan
        .expect("PDF metadata must include a filter-images plan");
    assert_eq!(plan.kind, "pdf-filter-images");
    assert_eq!(plan.raster, None);
    assert!(plan.pdf_filter_images.is_some());
    let raw: Value = serde_json::from_str(&plan.raw_json).unwrap();
    assert_eq!(raw["kind"], "pdf-filter-images");
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn complete_uniffi_build_renders_architecture() {
    let svg = engine()
        .render_svg(
            "architecture-beta\n  service api(server)[API service]\n".to_string(),
            None,
        )
        .unwrap();

    assert!(svg.contains("<svg"));
}

#[test]
fn engine_exposes_transport_owned_versions() {
    let engine = engine();

    assert_eq!(UNIFFI_BINDING_API_VERSION, 8);
    assert_eq!(engine.binding_api_version_v8(), UNIFFI_BINDING_API_VERSION);
    assert_eq!(engine.package_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn api_v7_bindings_are_rejected_before_record_decoding() {
    for checksum in UNIFFI_BINDING_API_OLD_VERSION_METHOD_CHECKSUMS {
        assert_ne!(
            uniffi_merman_uniffi_checksum_method_merman_binding_api_version_v8(),
            checksum
        );
    }
    let generated_header = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../platforms/apple/Sources/Merman/Generated/MermanFFI.h"
    ));
    assert!(
        generated_header.contains("uniffi_merman_uniffi_fn_method_merman_binding_api_version_v8")
    );
    assert!(
        generated_header
            .contains("uniffi_merman_uniffi_checksum_method_merman_binding_api_version_v8")
    );
    assert!(
        !generated_header.contains("uniffi_merman_uniffi_fn_method_merman_binding_api_version_v7"),
        "old probe must be absent before record decoding"
    );
    assert!(
        !generated_header
            .contains("uniffi_merman_uniffi_checksum_method_merman_binding_api_version_v7"),
        "old checksum must be absent before record decoding"
    );
}

#[cfg(feature = "svg")]
#[test]
fn uniffi_host_request_preserves_tspan_whitespace_and_wrap_contract() {
    use MermanTextWhiteSpace::{BreakSpaces, Normal, Nowrap};
    use merman_bindings_core::{
        HostTextMeasurementRequest,
        TextMeasurementOperation::{
            CanvasMeasureTextWidth, Measure, TspanBBoxHeight, TspanBBoxWidth,
        },
        TextMeasurementPhase, TextStyle, WrapMode,
    };

    #[derive(Default)]
    struct CapturingTextMeasurer {
        request: StdMutex<Option<MermanTextMeasureRequest>>,
    }

    impl MermanTextMeasurer for CapturingTextMeasurer {
        fn measure(
            &self,
            request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            *self.request.lock().unwrap() = Some(request);
            Ok(None)
        }
    }

    let callback = Arc::new(CapturingTextMeasurer::default());
    let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
    let _operation = admission.enter_operation().expect("operation admission");
    let host = UniffiHostTextMeasurer::new(callback.clone(), admission);
    let style = TextStyle {
        font_family: Some("Test Font".to_string()),
        font_size: 16.0,
        font_weight: Some("600".to_string()),
        font_style: Some("italic".to_string()),
    };

    for (operation, wrap_mode, max_width, white_space) in [
        (TspanBBoxWidth, WrapMode::HtmlLike, None, BreakSpaces),
        (TspanBBoxHeight, WrapMode::HtmlLike, None, BreakSpaces),
        (TspanBBoxWidth, WrapMode::HtmlLike, Some(80.0), BreakSpaces),
        (Measure, WrapMode::HtmlLike, None, Nowrap),
        (Measure, WrapMode::HtmlLike, Some(80.0), BreakSpaces),
        (CanvasMeasureTextWidth, WrapMode::HtmlLike, None, Nowrap),
        (TspanBBoxWidth, WrapMode::SvgLike, None, Normal),
        (Measure, WrapMode::SvgLike, Some(80.0), Normal),
        (TspanBBoxWidth, WrapMode::SvgLikeSingleRun, None, Normal),
        (Measure, WrapMode::SvgLikeSingleRun, Some(80.0), Normal),
    ] {
        assert!(
            host.call_host(HostTextMeasurementRequest {
                operation,
                phase: TextMeasurementPhase::SvgBBox,
                text: "a  b",
                style: &style,
                max_width,
                wrap_mode,
            })
            .expect("callback transport")
            .is_none()
        );
        let captured = callback
            .request
            .lock()
            .unwrap()
            .take()
            .expect("callback received the projected request");
        let (expected_wrap_mode, line_height) = match wrap_mode {
            WrapMode::HtmlLike => (MermanTextWrapMode::HtmlLike, 24.0),
            WrapMode::SvgLike => (MermanTextWrapMode::SvgLike, 17.6),
            WrapMode::SvgLikeSingleRun => (MermanTextWrapMode::SvgLikeSingleRun, 17.6),
        };
        assert_eq!(
            captured,
            MermanTextMeasureRequest {
                operation: uniffi_measurement_operation(operation),
                phase: MermanTextMeasurementPhase::SvgBBox,
                text: "a  b".to_string(),
                font_family: Some("Test Font".to_string()),
                font_size: 16.0,
                font_weight: Some("600".to_string()),
                font_style: "italic".to_string(),
                max_width,
                line_height,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                wrap_mode: expected_wrap_mode,
                direction: MermanTextDirection::Auto,
                white_space,
            },
            "{operation:?}, {wrap_mode:?}, max_width={max_width:?}"
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn uniffi_preserves_exact_host_measurement_operations() {
    for (operation, expected) in [
        (
            merman_bindings_core::TextMeasurementOperation::RawBBoxYOffset,
            MermanTextMeasurementOperation::RawBBoxYOffset,
        ),
        (
            merman_bindings_core::TextMeasurementOperation::RawMiddleBBoxYOffset,
            MermanTextMeasurementOperation::RawMiddleBBoxYOffset,
        ),
        (
            merman_bindings_core::TextMeasurementOperation::RawHangingBBoxYOffset,
            MermanTextMeasurementOperation::RawHangingBBoxYOffset,
        ),
    ] {
        assert_eq!(uniffi_measurement_operation(operation), expected);
        assert!(operation.accepts_signed_length());
    }
    assert_eq!(
        uniffi_measurement_operation(
            merman_bindings_core::TextMeasurementOperation::MermaidCalculateTextDimensions,
        ),
        MermanTextMeasurementOperation::MermaidCalculateTextDimensions
    );
    assert_eq!(
        uniffi_measurement_operation(
            merman_bindings_core::TextMeasurementOperation::CanvasMeasureTextWidth,
        ),
        MermanTextMeasurementOperation::CanvasMeasureTextWidth
    );
    assert_eq!(
        uniffi_measurement_operation(
            merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
        ),
        MermanTextMeasurementOperation::CreateTextMiddleBBoxYOffset
    );
    assert_eq!(
        uniffi_measurement_operation(merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,),
        MermanTextMeasurementOperation::RawBBoxHeight
    );
    assert_eq!(
        merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
            merman_bindings_core::TextMeasurementOperation::MermaidCalculateTextDimensions,
        ),
        merman_bindings_core::HostTextMeasurementResultKind::Metrics
    );
    assert_eq!(
        merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
            merman_bindings_core::TextMeasurementOperation::CanvasMeasureTextWidth,
        ),
        merman_bindings_core::HostTextMeasurementResultKind::Length
    );
    assert_eq!(
        merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
            merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
        ),
        merman_bindings_core::HostTextMeasurementResultKind::Length
    );
    assert_eq!(
        merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
            merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
        ),
        merman_bindings_core::HostTextMeasurementResultKind::Length
    );

    let callback = CountingTextMeasurer::new();
    let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
    let _operation = admission.enter_operation().expect("operation admission");
    let host = UniffiHostTextMeasurer::new(callback, admission);
    let style = merman_bindings_core::TextStyle::default();
    let result = host
        .call_host(merman_bindings_core::HostTextMeasurementRequest {
            operation: merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
            phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
            text: "middle",
            style: &style,
            max_width: None,
            wrap_mode: merman_bindings_core::WrapMode::SvgLike,
        })
        .expect("callback transport")
        .expect("handled middle y-offset");
    let merman_bindings_core::HostTextMeasurement::Length(result) = result else {
        panic!("middle y-offset must use the length result shape");
    };
    assert_eq!(result, -2.0);

    let raw_height = host
        .call_host(merman_bindings_core::HostTextMeasurementRequest {
            operation: merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
            phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
            text: "raw-height",
            style: &style,
            max_width: None,
            wrap_mode: merman_bindings_core::WrapMode::SvgLike,
        })
        .expect("callback transport")
        .expect("handled raw bbox height");
    let merman_bindings_core::HostTextMeasurement::Length(raw_height) = raw_height else {
        panic!("raw bbox height must use the length result shape");
    };
    assert!(raw_height > 0.0);
}

#[cfg(feature = "svg")]
#[test]
fn uniffi_checked_decoder_rejects_oversized_counts_and_half_extents() {
    let style = merman_bindings_core::TextStyle::default();
    let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
    let _operation = admission.enter_operation().expect("operation admission");
    let request = |operation| merman_bindings_core::HostTextMeasurementRequest {
        operation,
        phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
        text: "x",
        style: &style,
        max_width: None,
        wrap_mode: merman_bindings_core::WrapMode::SvgLike,
    };

    let oversized = UniffiHostTextMeasurer::new(
        Arc::new(FixedTextMeasurer {
            result: MermanTextMeasureResult {
                result_kind: MermanTextMeasurementResultKind::Metrics,
                width: 1.0,
                height: 1.0,
                length: 0.0,
                line_count: u64::MAX,
                bbox_left: None,
                bbox_right: None,
                raw_width: None,
            },
        }),
        Arc::clone(&admission),
    );
    assert!(
        oversized
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::Measure,
            ))
            .is_err()
    );

    let half_extents = UniffiHostTextMeasurer::new(
        Arc::new(FixedTextMeasurer {
            result: MermanTextMeasureResult {
                result_kind: MermanTextMeasurementResultKind::HorizontalExtents,
                width: 0.0,
                height: 0.0,
                length: 0.0,
                line_count: 0,
                bbox_left: Some(1.0),
                bbox_right: None,
                raw_width: None,
            },
        }),
        admission,
    );
    assert!(
        half_extents
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::BBoxX,
            ))
            .is_err()
    );
}

#[cfg(feature = "svg")]
#[test]
fn engine_accepts_options_json() {
    let svg = engine()
        .render_svg(
            "flowchart TD\nA[Hello]".to_string(),
            Some(
                r#"{
                        "environment": { "text_measurement": "deterministic" },
                        "svg": { "diagram_id": "uniffi diagram", "pipeline": "readable" }
                    }"#
                .to_string(),
            ),
        )
        .unwrap();

    assert!(svg.contains("id=\"uniffi-diagram\""));
    assert!(svg.contains("data-merman-foreignobject"));
}

#[cfg(feature = "ascii")]
#[test]
fn engine_renders_ascii() {
    let text = engine()
        .render_ascii("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
        .unwrap();

    assert!(text.contains("Hello"));
    assert!(text.contains("World"));
}

#[cfg(feature = "ascii")]
#[test]
fn engine_ascii_result_exposes_the_encoded_representation() {
    let result = engine()
        .render_ascii_result(
            "flowchart LR\nA[Hello] -- yes --> B[World]".to_string(),
            Some(
                r##"{"ascii":{"layout_profile":"auto","max_width":80,"overflow":"error","color_mode":"ansi16","theme":{"foreground":"#010101","background":"#ffffff","line":"#020202","accent":"#030303","border":"#040404"}}}"##
                    .to_string(),
            ),
        )
        .expect("ANSI16 ASCII result should render");
    assert!(result.data.windows(2).any(|window| window == b"\x1b["));
    let plan = result
        .metadata
        .output_plan
        .expect("ASCII metadata output plan");
    let ascii = plan.ascii.expect("typed ASCII output plan");
    assert_eq!(ascii.schema_version, 4);
    assert_eq!(ascii.encoding, "ansi16");
    let raw: Value = serde_json::from_str(&plan.raw_json).expect("raw output plan JSON");
    assert_eq!(raw["encoding"], "ansi16");
    assert_eq!(ascii.requested_layout_profile, "auto");
    assert_eq!(ascii.requested_graph_presentation, "strict");
    assert!(ascii.primary_width.is_some());
    assert!(ascii.primary_height.is_some());
    assert!(ascii.layout_profile.is_some());
    assert_eq!(ascii.primary_overflowed, Some(false));
    assert!(!ascii.emitted_overflowed);
    assert_eq!(
        raw["requested_layout_profile"],
        ascii.requested_layout_profile
    );
    assert_eq!(
        raw["requested_graph_presentation"],
        ascii.requested_graph_presentation
    );
    assert_eq!(raw["compact_attempted"], ascii.compact_attempted);
    assert_eq!(
        raw["layout_profile"],
        serde_json::to_value(ascii.layout_profile).unwrap()
    );
    assert_eq!(
        raw["primary_width"],
        serde_json::to_value(ascii.primary_width).unwrap()
    );
    assert_eq!(
        raw["primary_height"],
        serde_json::to_value(ascii.primary_height).unwrap()
    );
    assert_eq!(raw["primary_overflowed"], false);
    assert_eq!(raw["emitted_overflowed"], false);
    assert!(raw.get("overflowed").is_none());
}

#[cfg(feature = "ascii")]
#[test]
fn engine_ascii_result_preserves_unavailable_primary_geometry() {
    let source = format!(
        "flowchart LR\nA@{{ shape: f-circ }}\nB[Target]\n{}",
        "A o--o B\n".repeat(97)
    );
    let result = engine()
        .render_ascii_result(
            source,
            Some(
                r#"{"ascii":{"graph_presentation":"adaptive","layout_profile":"auto","max_width":80,"overflow":"fallback"}}"#
                    .to_string(),
            ),
        )
        .expect("adaptive presentation should recover graph capacity failure");
    assert_eq!(result.metadata.version, 1);
    let plan = result
        .metadata
        .output_plan
        .expect("ASCII metadata output plan");
    let ascii = plan.ascii.expect("typed ASCII output plan");
    assert_eq!(ascii.schema_version, 4);
    assert_eq!(ascii.projection, "structured_text");
    assert_eq!(ascii.primary_width, None);
    assert_eq!(ascii.primary_height, None);
    assert_eq!(ascii.layout_profile, None);
    assert_eq!(ascii.primary_overflowed, None);
    assert_eq!(ascii.requested_layout_profile, "auto");
    assert_eq!(ascii.requested_graph_presentation, "adaptive");
    assert!(ascii.compact_attempted);
    assert!(!ascii.emitted_overflowed);
    assert_eq!(ascii.outcome, "fallback");
    assert!(ascii.fallback_attempted);
    assert_eq!(
        ascii.fallback_reason.as_deref(),
        Some("port_capacity_exceeded")
    );
    assert_eq!(ascii.lossiness, "presentation_only");
    let raw: Value = serde_json::from_str(&plan.raw_json).expect("raw output plan JSON");
    for field in [
        "primary_width",
        "primary_height",
        "layout_profile",
        "primary_overflowed",
    ] {
        assert_eq!(
            raw.get(field),
            Some(&Value::Null),
            "missing primary field {field}"
        );
    }
    assert_eq!(raw["emitted_width"], ascii.emitted_width);
    assert_eq!(raw["emitted_height"], ascii.emitted_height);
    assert_eq!(raw["requested_graph_presentation"], "adaptive");
    assert_eq!(raw["emitted_overflowed"], false);
    assert!(raw.get("overflowed").is_none());
    let text = String::from_utf8(result.data).expect("structured-text UTF-8 output");
    assert!(
        text.contains("E97:"),
        "all original relations must survive fallback"
    );
}

#[cfg(feature = "ascii")]
#[test]
fn engine_ascii_result_preserves_measured_empty_primary_geometry() {
    let result = engine()
        .render_ascii_result("flowchart LR".to_string(), None)
        .expect("empty graph should have measured zero geometry");
    let plan = result
        .metadata
        .output_plan
        .expect("ASCII metadata output plan");
    let ascii = plan.ascii.expect("typed ASCII output plan");
    assert_eq!(ascii.schema_version, 4);
    assert_eq!(ascii.primary_width, Some(0));
    assert_eq!(ascii.primary_height, Some(0));
    assert_eq!(ascii.layout_profile.as_deref(), Some("canonical"));
    assert_eq!(ascii.primary_overflowed, Some(false));
    assert!(!ascii.emitted_overflowed);
    assert_eq!(ascii.requested_graph_presentation, "strict");
    assert_eq!(ascii.outcome, "empty");
    let raw: Value = serde_json::from_str(&plan.raw_json).expect("raw output plan JSON");
    assert_eq!(raw["primary_width"], 0);
    assert_eq!(raw["primary_height"], 0);
    assert_eq!(raw["layout_profile"], "canonical");
    assert_eq!(raw["primary_overflowed"], false);
}

#[cfg(feature = "ascii")]
#[test]
fn engine_applies_flowchart_node_label_wrap_width() {
    let text = engine()
        .render_ascii(
            "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]".to_string(),
            Some(r#"{ "ascii": { "flowchartNodeLabelWrapWidth": 8 } }"#.to_string()),
        )
        .unwrap();

    for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert!(text.contains(expected), "missing {expected:?}:\n{text}");
    }
    assert!(!text.contains("Alpha Beta Gamma Delta"), "{text}");
}

#[cfg(feature = "ascii")]
#[test]
fn uniffi_ascii_operation_preserves_a_typed_exact_resource_boundary() {
    let id = MermanResourceOverrideId::MaxAsciiOutputBytes;
    let case = ascii_resource_boundary_contract()
        .transport_representatives
        .uniffi_interactive;
    assert_eq!(case.id, id.id());
    let expected = case.exact;
    let engine = engine();

    let output = engine
        .render_ascii(
            case.source.clone(),
            Some(uniffi_ascii_options(id, expected)),
        )
        .unwrap_or_else(|error| panic!("exact {} boundary failed: {error:?}", case.id));
    assert!(!output.is_empty(), "{} produced no output", case.id);

    let error = engine
        .render_ascii(case.source, Some(uniffi_ascii_options(id, expected - 1)))
        .expect_err("one-below UniFFI ASCII boundary must fail");
    let details = assert_ascii_resource_error(error, &case.id, &case.phase, expected - 1);
    assert_eq!(details.actual, expected);
}

#[cfg(feature = "svg")]
#[test]
fn engine_returns_semantic_json() {
    let json: Value = serde_json::from_str(
        &engine()
            .parse_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        json.get("type").and_then(Value::as_str),
        Some("flowchart-v2")
    );
}

#[cfg(feature = "svg")]
#[test]
fn engine_returns_layout_json() {
    let json: Value = serde_json::from_str(
        &engine()
            .layout_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap(),
    )
    .unwrap();

    assert!(json.get("meta").is_some());
    assert!(json.get("layout").is_some());
}

#[cfg(feature = "svg")]
#[test]
fn engine_returns_edge_geometry_json() {
    let json: Value = serde_json::from_str(
        &engine()
            .edge_geometry_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["diagram_type"], "flowchart-v2");

    let edges = json["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty());
    assert!(edges.iter().all(|edge| edge["id"].as_str().is_some()));
}

#[cfg(feature = "analysis")]
#[test]
fn engine_returns_analysis_json() {
    let json: Value = serde_json::from_str(
        &engine()
            .analyze_json("flowchart TD\nA[Hello]".to_string(), None)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["valid"], true);
}

#[cfg(feature = "analysis")]
#[test]
fn named_analysis_facts_helpers_match_generic_execution() {
    let source = "flowchart TD\nA[Hello]".to_string();
    let named: Value =
        serde_json::from_str(&engine().analysis_facts_json(source.clone(), None).unwrap()).unwrap();
    let reusable: Value = serde_json::from_str(
        &reusable_engine(None)
            .analysis_facts_json(source.clone(), None)
            .unwrap(),
    )
    .unwrap();
    let generic = engine()
        .execute(MermanOperationRequestV4 {
            operation_id: "analysis-facts-json".to_string(),
            source,
            uri: None,
            options_json: None,
            control: None,
        })
        .unwrap();
    let generic: Value = serde_json::from_slice(&generic.data).unwrap();

    assert_eq!(named, generic);
    assert_eq!(reusable, generic);
    assert_eq!(named["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
}

#[cfg(feature = "analysis")]
#[test]
fn engine_returns_document_analysis_json() {
    let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
    let json: Value = serde_json::from_str(
        &engine()
            .analyze_document_json(
                source.to_string(),
                "file:///tmp/example.md".to_string(),
                None,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["source"]["kind"], "markdown");
    assert_eq!(json["valid"], true);
}

#[cfg(feature = "analysis")]
#[test]
fn engine_returns_document_facts_json() {
    let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
    let json: Value = serde_json::from_str(
        &engine()
            .analyze_document_facts_json(
                source.to_string(),
                "file:///tmp/example.md".to_string(),
                None,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
    assert_eq!(json["source"]["kind"], "markdown");
    assert_eq!(json["diagrams"][0]["source_id"], "mermaid-fence-1");
    assert_eq!(
        json["diagrams"][0]["syntax"]["fact_source"],
        "parser_complete"
    );
    assert!(
        json["diagrams"][0]["syntax"]["semantic_items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| {
                item["name"] == "A" && item["rename_policy"] == "flowchart_node_id"
            }))
    );

    let unavailable: Value = serde_json::from_str(
        &engine()
            .analyze_document_facts_json(
                "```mermaid\nunknownDiagram\nPretendNode --> OtherNode\n```\n".to_string(),
                "file:///tmp/unknown.md".to_string(),
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(unavailable["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
    assert_eq!(
        unavailable["diagrams"][0]["syntax"]["fact_source"],
        "unavailable"
    );
    assert_eq!(
        unavailable["diagrams"][0]["syntax"]["semantic_items"],
        serde_json::json!([])
    );
}

#[cfg(feature = "analysis")]
#[test]
fn engine_validates_source() {
    let api = engine();
    let reusable = reusable_engine(None);
    for (source, valid, status) in [
        ("flowchart TD\nA[Hello]", true, BindingStatus::Ok),
        ("", false, BindingStatus::NoDiagram),
    ] {
        let result = api.validate(source.to_string(), None).unwrap();
        assert_eq!(result.valid, valid);
        assert_eq!(result.code_name, status.code_name());
        if !valid {
            assert!(
                result
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("no Mermaid diagram")
            );
        }
        assert_eq!(reusable.validate(source.to_string(), None).unwrap(), result);
        for full in [
            api.execute(operation_request(
                OperationKey::ValidationJson,
                source.to_string(),
                None,
            )),
            reusable.execute(operation_request(
                OperationKey::ValidationJson,
                source.to_string(),
                None,
            )),
        ] {
            let full = full.unwrap();
            let payload: Value = serde_json::from_slice(&full.data).unwrap();
            assert_eq!(payload["valid"], result.valid);
            assert_eq!(payload["code"], result.code);
            assert_eq!(payload["code_name"], result.code_name);
            assert_eq!(full.metadata.operation_id, "validation-json");
            assert_eq!(full.metadata.byte_length, full.data.len() as u64);
        }
    }
}

#[test]
fn engine_exposes_metadata() {
    let engine = engine();
    let runtime_catalog_value: serde_json::Value =
        serde_json::from_str(&engine.runtime_catalog_json().unwrap()).unwrap();
    let runtime_capability_ids = runtime_catalog_value["capabilities"]["capability_ids"]
        .as_array()
        .expect("runtime capability IDs");
    let has_ascii = runtime_capability_ids.iter().any(|id| id == "ascii");
    let has_analysis = runtime_capability_ids.iter().any(|id| id == "analysis");
    let has_svg = runtime_capability_ids.iter().any(|id| id == "svg");
    assert_eq!(has_ascii, cfg!(feature = "ascii"));
    assert_eq!(has_analysis, cfg!(feature = "analysis"));
    assert_eq!(has_svg, cfg!(feature = "svg"));
    let operation_ids = runtime_catalog_value["capabilities"]["operation_ids"]
        .as_array()
        .unwrap();
    for (operation_id, expected) in [
        ("jpeg", cfg!(feature = "jpeg")),
        ("pdf", cfg!(feature = "pdf")),
        ("png", cfg!(feature = "png")),
        ("semantic-json", true),
    ] {
        assert_eq!(
            operation_ids.iter().any(|id| id == operation_id),
            expected,
            "operation {operation_id} must follow merman-uniffi features"
        );
    }
    for (capability_id, expected) in [
        ("layout-cytoscape", cfg!(feature = "layout-cytoscape")),
        ("layout-dagre", cfg!(feature = "layout-dagre")),
        ("layout-elk", cfg!(feature = "layout-elk")),
        ("math", cfg!(feature = "math")),
    ] {
        assert_eq!(
            runtime_capability_ids.iter().any(|id| id == capability_id),
            expected,
            "capability {capability_id} must follow merman-uniffi features"
        );
    }
    let exposes_system_adapters = cfg!(feature = "native-runtime");
    let system_adapter_ids = runtime_catalog_value["capabilities"]["system_adapter_ids"]
        .as_array()
        .unwrap();
    for adapter_id in ["system-clock", "system-random", "system-timezone"] {
        assert_eq!(
            system_adapter_ids.iter().any(|id| id == adapter_id),
            exposes_system_adapters,
            "system adapter {adapter_id} must follow merman-uniffi features"
        );
    }

    assert!(
        engine
            .supported_diagrams()
            .contains(&"flowchart".to_string())
    );
    let ascii_capabilities = engine.ascii_capabilities();
    if has_ascii {
        let sequence = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "sequence")
            .expect("expected UniFFI ASCII capabilities to include sequence");
        assert_eq!(sequence.semantic_coverage.as_deref(), Some("partial"));
        assert_eq!(sequence.primary_projection, "diagrammatic");
        assert_eq!(sequence.support_level, "partial");
        assert_eq!(sequence.layout_profiles, ["canonical", "compact", "auto"]);
        assert_eq!(sequence.graph_presentations, ["strict"]);
        assert_eq!(sequence.width_profiles, ["unicode", "cjk"]);
        assert_eq!(
            sequence.encodings,
            ["plain", "ansi16", "ansi256", "truecolor", "html"]
        );
        assert_eq!(sequence.fallback_encodings, ["plain"]);

        let gantt = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "gantt")
            .expect("expected UniFFI ASCII capabilities to include gantt");
        assert_eq!(gantt.support_level, "partial");
        assert_eq!(gantt.semantic_coverage.as_deref(), Some("partial"));
        assert_eq!(gantt.primary_projection, "diagrammatic");
        assert!(gantt.structured_text_fallback);

        let class = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "class")
            .expect("expected UniFFI ASCII capabilities to include class");
        assert_eq!(class.support_level, "partial");
        assert!(class.structured_text_fallback);
        assert_eq!(class.graph_presentations, ["strict"]);

        for canonical in merman_bindings_core::ascii_capabilities() {
            let projected = ascii_capabilities
                .iter()
                .find(|capability| capability.diagram_type == canonical.diagram_type)
                .expect("canonical ASCII family must have a UniFFI projection");
            assert_eq!(
                projected.graph_presentations, canonical.graph_presentations,
                "graph presentation admission must follow the canonical family contract"
            );
        }

        let mut ascii_diagrams = ascii_capabilities
            .iter()
            .map(|capability| capability.diagram_type.clone())
            .collect::<Vec<_>>();
        ascii_diagrams.sort();
        let mut declared_diagrams = merman_bindings_core::ascii_capabilities()
            .into_iter()
            .map(|capability| capability.diagram_type.to_string())
            .collect::<Vec<_>>();
        declared_diagrams.sort();
        assert_eq!(ascii_diagrams, declared_diagrams);
        let zenuml = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "zenuml")
            .expect("expected UniFFI ASCII capabilities to include ZenUML");
        assert_eq!(zenuml.semantic_coverage.as_deref(), Some("partial"));
        assert_eq!(zenuml.primary_projection, "diagrammatic");
        assert_eq!(zenuml.support_level, "partial");
        assert_eq!(zenuml.layout_profiles, ["canonical"]);
        assert_eq!(zenuml.graph_presentations, ["strict"]);
        assert_eq!(zenuml.width_profiles, ["unicode", "cjk"]);
        assert_eq!(
            zenuml.encodings,
            ["plain", "ansi16", "ansi256", "truecolor", "html"]
        );
        assert_eq!(zenuml.fallback_encodings, ["plain"]);
    } else {
        assert!(ascii_capabilities.is_empty());
    }
    assert!(engine.supported_themes().contains(&"default".to_string()));
    let theme_catalog: serde_json::Value =
        serde_json::from_str(&engine.theme_catalog_json().unwrap()).unwrap();
    assert_eq!(theme_catalog["schema_version"], 2);
    assert_eq!(
        theme_catalog["presets"].as_array().unwrap().len(),
        if has_svg { 13 } else { 0 }
    );
    let capabilities = engine.diagram_family_capabilities();
    assert!(capabilities.iter().any(|capability| {
        capability.diagram_type == "flowchart"
            && capability.family_id == "flowchart"
            && capability.metadata_id.as_deref() == Some("flowchart")
            && !capability.has_detector
            && capability.has_semantic_parser
            && capability.has_editor_parser
            && capability.has_combined_parser
            && capability.has_render_parser
            && !capability.has_header
            && capability.config_namespace.as_deref() == Some("flowchart")
    }));
    if has_analysis {
        let lint_rules = engine.lint_rule_catalog().unwrap();
        assert!(lint_rules.iter().any(|rule| {
            rule.id == "merman.authoring.flowchart.explicit_direction"
                && rule.origin == "merman_authoring"
                && rule.default_profile == "recommended"
                && rule
                    .evidence
                    .contains(&"docs/adr/0072-lint-rule-governance.md".to_string())
        }));
        assert!(lint_rules.iter().any(|rule| {
            rule.id == "merman.compatibility.config.deprecated_flowchart_html_labels"
                && rule.tags == vec!["deprecated"]
        }));
        assert!(
            engine
                .configurable_lint_rule_catalog()
                .unwrap()
                .iter()
                .all(|rule| rule.configurable && rule.category != "internal")
        );
    } else {
        let lint_error = engine.lint_rule_catalog().unwrap_err();
        assert_missing_capability(&lint_error, "analysis");
        let configurable_error = engine.configurable_lint_rule_catalog().unwrap_err();
        assert_missing_capability(&configurable_error, "analysis");
    }
    let runtime_catalog = runtime_catalog_value
        .as_object()
        .expect("runtime catalog must be an object");
    assert_eq!(
        runtime_catalog
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "capabilities",
            "constructor_service_contracts",
            "constructor_service_ids",
            "metadata_ids",
            "option_group_ids",
            "options_schema_versions",
            "output_contracts",
            "package_version",
            "payload_schemas",
            "registry",
            "resources",
            "schema_version",
            "transport_api_version",
        ])
    );
    assert_eq!(
        runtime_catalog["schema_version"],
        merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION
    );
    assert_eq!(
        runtime_catalog["transport_api_version"],
        UNIFFI_BINDING_API_VERSION
    );
    assert_eq!(
        runtime_catalog["capabilities"],
        serde_json::to_value(native_artifact_contract().runtime_capabilities()).unwrap()
    );
    assert!(
        !runtime_catalog["capabilities"]["system_adapter_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "system-timing")
    );
    assert_eq!(
        runtime_catalog["capabilities"]["capability_ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == "svg")),
        has_svg
    );
    assert!(runtime_catalog.get("features").is_none());
    assert_eq!(
        runtime_catalog["options_schema_versions"],
        serde_json::json!([merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION])
    );
    assert_eq!(
        runtime_catalog["option_group_ids"],
        serde_json::json!(
            native_artifact_contract()
                .option_group_keys()
                .map(merman_bindings_core::BindingOptionGroupKey::id)
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        runtime_catalog["constructor_service_ids"],
        serde_json::json!(
            native_artifact_contract()
                .constructor_service_keys()
                .map(merman_bindings_core::ConstructorServiceKey::id)
                .collect::<Vec<_>>()
        )
    );
    #[cfg(feature = "svg")]
    assert_eq!(
        runtime_catalog["constructor_service_ids"],
        serde_json::json!(["host-text-measurement", "icon-registry"])
    );
    #[cfg(not(feature = "svg"))]
    assert_eq!(
        runtime_catalog["constructor_service_ids"],
        serde_json::json!([])
    );
    assert!(
        runtime_catalog["payload_schemas"]
            .as_array()
            .is_some_and(|schemas| schemas
                .iter()
                .any(|schema| schema["id"] == "binding-result"))
    );
    assert!(
        runtime_catalog["metadata_ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == "diagram-family-capabilities"))
    );
    assert_eq!(
        runtime_catalog["resources"]["general_binding_default_profile"],
        "interactive"
    );
}

#[test]
fn typed_resource_options_builder_uses_the_shared_descriptor() {
    let json = resource_options_json(
        Some(MermanResourceProfile::Constrained),
        vec![MermanResourceLimitOverride {
            id: MermanResourceOverrideId::MaxSourceBytes,
            value: 4096,
        }],
    )
    .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["version"],
        merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION
    );
    assert_eq!(value["resources"]["profile"], "constrained");
    assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

    let geometry = resource_options_json(
        None,
        vec![MermanResourceLimitOverride {
            id: MermanResourceOverrideId::MaxGeometryJsonBytes,
            value: 8192,
        }],
    )
    .unwrap();
    let geometry: Value = serde_json::from_str(&geometry).unwrap();
    assert_eq!(
        geometry["resources"]["limits"]["max_geometry_json_bytes"],
        8192
    );

    let inherited = resource_options_json(None, Vec::new()).unwrap();
    assert_eq!(inherited, r#"{"version":4}"#);
}

#[test]
fn resource_override_wire_ordinals_remain_append_only() {
    let variants = [
        MermanResourceOverrideId::MaxSourceBytes,
        MermanResourceOverrideId::MaxModelItems,
        MermanResourceOverrideId::MaxModelTextBytes,
        MermanResourceOverrideId::MaxModelNestingDepth,
        MermanResourceOverrideId::MaxLayoutWorkUnits,
        MermanResourceOverrideId::MaxSvgBytes,
        MermanResourceOverrideId::MaxSvgElements,
        MermanResourceOverrideId::MaxDocumentDiagrams,
        MermanResourceOverrideId::MaxAsciiGridCells,
        MermanResourceOverrideId::MaxRasterWidth,
        MermanResourceOverrideId::MaxRasterHeight,
        MermanResourceOverrideId::MaxRasterPixels,
        MermanResourceOverrideId::MaxEmbeddedImageBytes,
        MermanResourceOverrideId::MaxTotalEmbeddedImageBytes,
        MermanResourceOverrideId::MaxEmbeddedImagePixels,
        MermanResourceOverrideId::MaxTotalEmbeddedImagePixels,
        MermanResourceOverrideId::MaxPdfFilterImagePixels,
        MermanResourceOverrideId::MaxAsciiLayoutWorkUnits,
        MermanResourceOverrideId::MaxAsciiDocumentCells,
        MermanResourceOverrideId::MaxAsciiOutputBytes,
        MermanResourceOverrideId::MaxAsciiGraphemeBytes,
        MermanResourceOverrideId::MaxAsciiNestingDepth,
        MermanResourceOverrideId::MaxPreparedTextRetainedBytes,
        MermanResourceOverrideId::MaxGeometryJsonBytes,
    ];

    for (ordinal, variant) in variants.into_iter().enumerate() {
        assert_eq!(variant as usize, ordinal);
    }

    #[cfg(feature = "ascii")]
    {
        let mut transport_ids = [
            MermanResourceOverrideId::MaxAsciiGridCells,
            MermanResourceOverrideId::MaxAsciiLayoutWorkUnits,
            MermanResourceOverrideId::MaxAsciiDocumentCells,
            MermanResourceOverrideId::MaxAsciiOutputBytes,
            MermanResourceOverrideId::MaxAsciiGraphemeBytes,
            MermanResourceOverrideId::MaxAsciiNestingDepth,
        ]
        .map(MermanResourceOverrideId::id);
        transport_ids.sort_unstable();

        let shared_cases = ascii_resource_boundary_contract().binding_core_interactive;
        let mut shared_ids = shared_cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        shared_ids.sort_unstable();

        assert_eq!(transport_ids.as_slice(), shared_ids.as_slice());
    }
}

#[test]
fn resource_override_host_width_conversion_is_a_caller_error() {
    let host_max = usize::MAX as u64;
    assert_eq!(resource_override_host_value(host_max).unwrap(), usize::MAX);

    let error = resource_override_value::<u32>(u64::from(u32::MAX) + 1, "u32").unwrap_err();
    let MermanError::Binding {
        code,
        code_name,
        kind,
        capability_id,
        resource,
        message,
        ..
    } = error;
    assert_eq!(code, BindingStatus::InvalidArgument.code());
    assert_eq!(code_name, BindingStatus::InvalidArgument.code_name());
    assert_eq!(kind, MermanErrorKind::Generic);
    assert_eq!(capability_id, None);
    assert_eq!(resource, None);
    assert_eq!(message, "resource override exceeds u32");
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_reuses_options() {
    let reusable = reusable_engine(Some(
        r#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "uniffi reusable", "pipeline": "readable" }
            }"#
        .to_string(),
    ));

    let svg = reusable
        .render_svg("flowchart TD\nA[Hello]".to_string(), None)
        .unwrap();
    assert!(svg.contains("id=\"uniffi-reusable\""));
    assert!(svg.contains("data-merman-foreignobject"));

    let request_svg = reusable
        .render_svg(
            "flowchart TD\nA[Hello]".to_string(),
            Some(r#"{"svg":{"diagram_id":"request override"}}"#.to_string()),
        )
        .unwrap();
    assert!(request_svg.contains("id=\"request-override\""));
    assert!(request_svg.contains("data-merman-foreignobject"));

    let baseline_svg = reusable
        .render_svg("flowchart TD\nA[Hello]".to_string(), None)
        .unwrap();
    assert!(baseline_svg.contains("id=\"uniffi-reusable\""));
}

#[test]
fn reusable_engine_rejects_request_runtime_policy() {
    let reusable = reusable_engine(None);
    let error = reusable
        .execute(MermanOperationRequestV4 {
            operation_id: "semantic-json".to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: None,
            options_json: Some(r#"{"runtime_policy":"deterministic"}"#.to_string()),
            control: None,
        })
        .unwrap_err();
    let MermanError::Binding {
        code,
        code_name,
        message,
        ..
    } = error;
    assert_eq!(code, BindingStatus::OptionsJsonError.code());
    assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
    assert!(message.contains("cannot set runtime_policy"));
}

#[cfg(feature = "analysis")]
#[test]
fn reusable_engine_returns_document_analysis_json() {
    let reusable = reusable_engine(Some(
        r#"{ "version": 4, "analysis": { "lint": { "profile": "strict" } } }"#.to_string(),
    ));
    let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
    let json: Value = serde_json::from_str(
        &reusable
            .analyze_document_json(
                source.to_string(),
                "file:///tmp/example.md".to_string(),
                None,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(json["source"]["kind"], "markdown");
    assert_eq!(json["valid"], true);
}

#[cfg(feature = "svg")]
#[test]
fn engine_services_builder_is_immutable_and_order_independent() {
    let registry = MermanIconRegistry::from_packs(Vec::new()).unwrap();
    let measurer = CountingTextMeasurer::new();
    let empty = MermanEngineServices::new();

    let icon_only = empty.with_icon_registry(registry.clone());
    let text_only = empty.with_text_measurer(measurer.clone());
    assert!(empty.icon_registry.is_none());
    assert!(empty.text_measurer.is_none());
    assert!(icon_only.icon_registry.is_some());
    assert!(icon_only.text_measurer.is_none());
    assert!(text_only.icon_registry.is_none());
    assert!(text_only.text_measurer.is_some());

    let icon_then_text = icon_only.with_text_measurer(measurer.clone());
    let text_then_icon = text_only.with_icon_registry(registry);
    assert!(icon_then_text.icon_registry.is_some());
    assert!(icon_then_text.text_measurer.is_some());
    assert!(text_then_icon.icon_registry.is_some());
    assert!(text_then_icon.text_measurer.is_some());

    MermanEngine::new(None, Some(icon_then_text)).unwrap();
    MermanEngine::new(None, Some(text_then_icon)).unwrap();
    assert_eq!(
        measurer.calls(),
        0,
        "service composition must not call the host"
    );
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_uses_host_text_measurer() {
    let measurer = CountingTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());
    assert_eq!(
        measurer.calls(),
        0,
        "construction must not invoke callbacks"
    );

    let svg = reusable
        .render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]\nclassDef emphasized font-style:italic\nclass A emphasized"
                .to_string(),
            Some(r#"{"svg":{"diagram_id":"request-measured"}}"#.to_string()),
        )
        .unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("id=\"request-measured\""));
    assert!(measurer.calls() > 0);
    assert!(measurer.saw_font_style("italic"));
    assert!(measurer.saw_operation(MermanTextMeasurementOperation::Wrapped));
}

#[cfg(feature = "svg")]
#[test]
fn constructor_services_reject_explicit_text_measurement_without_calling_the_host() {
    let measurer = CountingTextMeasurer::new();
    let services = MermanEngineServices::new().with_text_measurer(measurer.clone());
    let error = match MermanEngine::new(
        Some(r#"{"environment":{"text_measurement":"deterministic"}}"#.to_string()),
        Some(services),
    ) {
        Ok(_) => panic!("explicit selector and callback service must conflict"),
        Err(error) => error,
    };
    let MermanError::Binding { code, message, .. } = error;
    assert_eq!(code, BindingStatus::InvalidArgument.code());
    assert!(message.contains("host-text-measurement"));
    assert!(message.contains("environment.text_measurement"));
    assert_eq!(measurer.calls(), 0);
}

#[cfg(feature = "svg")]
#[test]
fn immutable_icon_registry_is_reused_across_engines() {
    let registry = MermanIconRegistry::from_packs(vec![MermanIconPack::new(
        r#"{
                "icons":{
                    "rocket":{
                        "body":"<path data-icon=\"uniffi-registry\" d=\"M0 0H16V16H0z\"/>"
                    }
                }
            }"#
        .to_string(),
        Some("test".to_string()),
    )])
    .unwrap();
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());

    let services = MermanEngineServices::new().with_icon_registry(registry);
    let first = MermanEngine::new(None, Some(services.clone())).unwrap();
    let second = MermanEngine::new(None, Some(services)).unwrap();
    for engine in [first, second] {
        let svg = engine
            .render_svg(
                r#"flowchart TD
A@{ icon: "test:rocket", label: "A" }"#
                    .to_string(),
                None,
            )
            .unwrap();
        assert!(svg.contains(r#"data-icon="uniffi-registry""#), "{svg}");
    }
}

#[test]
fn empty_icon_registry_is_normalized_to_no_service() {
    let registry = MermanIconRegistry::from_packs(Vec::new()).unwrap();
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());

    let services = MermanEngineServices::new().with_icon_registry(registry);
    let engine = MermanEngine::new(None, Some(services)).unwrap();
    let semantic = engine
        .parse_json("flowchart TD\nA --> B".to_string(), None)
        .unwrap();
    assert!(semantic.contains("flowchart-v2"), "{semantic}");

    #[cfg(feature = "svg")]
    {
        let source = "flowchart TD\nA --> B".to_string();
        let without_services = MermanEngine::new(None, None).unwrap();
        assert_eq!(
            engine.svg_plan_json(source.clone(), None).unwrap(),
            without_services.svg_plan_json(source, None).unwrap()
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn icon_registry_errors_preserve_structured_details() {
    let error = match MermanIconRegistry::from_packs(vec![MermanIconPack::new(
        r#"{"prefix":"test","icons":{"bad":{"body":"<path>"}}}"#.to_string(),
        None,
    )]) {
        Ok(_) => panic!("invalid icon XML must be rejected transactionally"),
        Err(error) => error,
    };
    let MermanError::Binding {
        code,
        icon_registry,
        ..
    } = error;
    assert_eq!(code, BindingStatus::InvalidArgument.code());
    assert_eq!(
        icon_registry,
        Some(MermanIconRegistryErrorDetails {
            kind_id: "invalid_xml".to_string(),
            pack_index: Some(0),
            registration_name: None,
        })
    );
}

#[cfg(feature = "svg")]
#[test]
fn callback_free_reusable_engine_admits_concurrent_operations() {
    let reusable = reusable_engine(None);
    let entered = Arc::new(Barrier::new(3));
    let release = Arc::new(Barrier::new(3));
    let handles = (0..2)
        .map(|_| {
            let reusable = reusable.clone();
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                reusable.with_reusable_operation(|_| {
                    entered.wait();
                    release.wait();
                    Ok(())
                })
            })
        })
        .collect::<Vec<_>>();

    entered.wait();
    release.wait();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
}

#[cfg(feature = "svg")]
#[test]
fn busy_close_preserves_the_complete_engine_for_retry() {
    let engine = reusable_engine(None);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let active_engine = engine.clone();
    let active_entered = entered.clone();
    let active_release = release.clone();
    let active = thread::spawn(move || {
        active_engine.with_reusable_operation(|_| {
            active_entered.wait();
            active_release.wait();
            Ok(())
        })
    });

    entered.wait();
    assert_busy_error(
        &engine
            .close()
            .expect_err("active operation makes close retryable"),
    );
    release.wait();
    active.join().unwrap().unwrap();

    engine.close().unwrap();
    engine.close().unwrap();
    let error = engine
        .parse_json("flowchart TD\nA --> B".to_string(), None)
        .expect_err("post-close operations must fail");
    let MermanError::Binding {
        code,
        kind,
        message,
        ..
    } = error;
    assert_eq!(code, BindingStatus::InvalidArgument.code());
    assert_eq!(kind, MermanErrorKind::Generic);
    assert!(message.contains("closed"));
}

#[cfg(feature = "svg")]
#[test]
fn reentrant_close_preserves_the_engine_for_retry() {
    let measurer = ReentrantCloseTextMeasurer::new();
    let engine = callback_engine(None, measurer.clone());
    measurer.set_engine(&engine);

    let svg = engine
        .render_svg("flowchart TD\nA[Measured] --> B[Done]".to_string(), None)
        .unwrap();
    assert!(svg.contains("<svg"));
    engine
        .parse_json("flowchart TD\nA --> B".to_string(), None)
        .expect("failed reentrant close must leave the engine intact");
    engine.close().unwrap();
}

#[cfg(feature = "svg")]
#[test]
fn concurrent_close_is_idempotent_and_drops_services_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let measurer = Arc::new(DropCountingTextMeasurer {
        drops: drops.clone(),
    });
    let engine = callback_engine(None, measurer.clone());
    drop(measurer);

    let first = engine.clone();
    let second = engine.clone();
    let first = thread::spawn(move || first.close());
    let second = thread::spawn(move || second.close());
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    engine.close().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "svg")]
#[test]
fn callback_destruction_may_reenter_close_without_deadlock() {
    let dropped = Arc::new(AtomicBool::new(false));
    let measurer = ReentrantDropTextMeasurer::new(dropped.clone());
    let engine = callback_engine(None, measurer.clone());
    measurer.set_engine(&engine);
    drop(measurer);

    engine.close().unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    engine.close().unwrap();
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_falls_back_when_host_text_measurer_errors() {
    let measurer = FailingTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());

    let svg = reusable
        .render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
        .unwrap();

    assert!(svg.contains("<svg"));
    assert!(measurer.calls() > 0);
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_layout_falls_back_when_host_text_measurer_errors() {
    let measurer = FailingTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());

    let layout = reusable
        .layout_json(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
        .unwrap();

    let layout: Value = serde_json::from_str(&layout).unwrap();
    assert!(layout.get("layout").is_some());
    assert!(measurer.calls() > 0);
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_falls_back_when_host_text_measurer_returns_none() {
    let measurer = MissingTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());

    let svg = reusable
        .render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
        .unwrap();

    assert!(svg.contains("<svg"));
    assert!(measurer.calls() > 0);
}

#[cfg(feature = "svg")]
#[test]
fn callback_reusable_engine_serializes_operation_admission() {
    let reusable = callback_engine(None, CountingTextMeasurer::new());
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let first = reusable.clone();
    let first_entered = Arc::clone(&entered);
    let first_release = Arc::clone(&release);
    let first_handle = thread::spawn(move || {
        first.with_reusable_operation(|_| {
            first_entered.wait();
            first_release.wait();
            Ok(())
        })
    });

    entered.wait();
    let error = reusable
        .parse_json("flowchart TD\nA --> B".to_string(), None)
        .expect_err("a callback engine must reject a competing operation");
    assert_busy_error(&error);
    release.wait();
    first_handle.join().unwrap().unwrap();
    reusable
        .parse_json("flowchart TD\nA --> B".to_string(), None)
        .expect("operation admission must be released when the first call returns");
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engines_isolate_active_host_callbacks() {
    let blocking_measurer = BlockingFailingTextMeasurer::new();
    let first = callback_engine(None, blocking_measurer.clone());
    let first_render = first.clone();
    let first_handle = thread::spawn(move || {
        first_render.render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
    });

    blocking_measurer.wait_until_entered();

    let second = reusable_engine(None);
    let (second_done_tx, second_done_rx) = mpsc::channel();
    let second_handle = thread::spawn(move || {
        second_done_tx
            .send(second.render_svg(
                "flowchart TD\nC[Independent] --> D[Engine]".to_string(),
                None,
            ))
            .unwrap();
    });
    let second_result = second_done_rx.recv_timeout(Duration::from_secs(1));

    blocking_measurer.release();

    let first_svg = first_handle.join().unwrap().unwrap();
    second_handle.join().unwrap();
    let second_svg = second_result
        .expect("independent engine render must not wait for another engine callback")
        .expect("independent engine render must not be rejected by another engine callback");
    assert!(first_svg.contains("<svg"));
    assert!(second_svg.contains("<svg"));
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_conservatively_rejects_independent_call_during_active_callback() {
    let blocking_measurer = BlockingFailingTextMeasurer::new();
    let reusable = callback_engine(None, blocking_measurer.clone());
    let first_render = reusable.clone();
    let first_handle = thread::spawn(move || {
        first_render.render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
    });

    blocking_measurer.wait_until_entered();

    let independent_call = reusable.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let independent_handle = thread::spawn(move || {
        done_tx
            .send(independent_call.render_svg(
                "flowchart TD\nC[Independent] --> D[Same engine]".to_string(),
                None,
            ))
            .unwrap();
    });
    let independent_result = done_rx.recv_timeout(Duration::from_secs(1));

    blocking_measurer.release();

    let first_svg = first_handle.join().unwrap().unwrap();
    independent_handle.join().unwrap();
    let error = independent_result
        .expect("same-engine call must fail instead of waiting for the active callback")
        .expect_err("same-engine call must be conservatively rejected");
    assert_reentrant_error(&error);
    assert!(first_svg.contains("<svg"));
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_rejects_same_engine_render_reentry_from_callback() {
    let measurer = ReentrantRenderTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());
    measurer.set_engine(&reusable);

    let svg = reusable
        .render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
        .unwrap();

    assert!(svg.contains("<svg"));
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_rejects_cross_thread_same_engine_render_reentry_without_blocking() {
    let measurer = CrossThreadReentrantTextMeasurer::new();
    let reusable = callback_engine(None, measurer.clone());
    measurer.set_engine(&reusable);

    let svg = reusable
        .render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
        .unwrap();

    assert!(svg.contains("<svg"));
}

#[cfg(feature = "svg")]
#[test]
fn reusable_engine_keeps_callback_alive_until_in_flight_operation_returns() {
    let measurer = BlockingFailingTextMeasurer::new();
    let weak_measurer = Arc::downgrade(&measurer);
    let reusable = callback_engine(None, measurer.clone());
    let render_engine = reusable.clone();
    let render = thread::spawn(move || {
        render_engine.render_svg(
            "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
            None,
        )
    });

    measurer.wait_until_entered();
    drop(measurer);
    drop(reusable);
    let retained_measurer = weak_measurer
        .upgrade()
        .expect("the in-flight operation must retain its callback");
    retained_measurer.release();
    drop(retained_measurer);
    assert!(render.join().unwrap().unwrap().contains("<svg"));
    assert!(
        weak_measurer.upgrade().is_none(),
        "the callback must be released after the operation and final engine owner return"
    );
}

#[cfg(not(feature = "svg"))]
#[test]
fn text_measurer_api_remains_visible_and_reports_missing_svg_capability() {
    let services =
        MermanEngineServices::new().with_text_measurer(Arc::new(UnavailableTextMeasurer));
    let constructor_error = match MermanEngine::new(None, Some(services)) {
        Ok(_) => panic!("text-measurer service must require svg"),
        Err(error) => error,
    };
    assert_missing_capability(&constructor_error, "svg");
}

#[cfg(feature = "svg")]
#[test]
fn uniffi_error_preserves_structured_resource_details() {
    let error = MermanError::from_binding(BindingError::resource_limit(
        "embedded_image_decode",
        "max_embedded_image_bytes",
        5,
        4,
        "constrained",
        "embedded image is too large",
    ));
    let MermanError::Binding { resource, .. } = error;
    assert_eq!(
        resource,
        Some(MermanResourceErrorDetails {
            cause: "ceiling".to_string(),
            limit_id: "max_embedded_image_bytes".to_string(),
            phase: "embedded_image_decode".to_string(),
            actual: 5,
            max: 4,
            profile: "constrained".to_string(),
        })
    );

    let error = MermanError::from_binding(BindingError::resource_limit_with_cause(
        merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    ));
    let MermanError::Binding { resource, .. } = error;
    assert_eq!(
        resource.expect("resource details").cause,
        "arithmetic_overflow"
    );
}

#[test]
fn uniffi_error_preserves_structured_diagnostic_details() {
    use merman_bindings_core::{BindingDiagnosticErrorDetails, BindingDiagnosticSpan};

    let error = MermanError::from_binding(
        BindingError::new(BindingStatus::ParseError, "invalid edge").with_diagnostic_details(
            BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
                .with_span(BindingDiagnosticSpan::new(3, 8, "exact"))
                .with_field("edge")
                .with_diagram_type("flowchart"),
        ),
    );
    let MermanError::Binding { diagnostic, .. } = error;

    assert_eq!(
        diagnostic,
        Some(MermanDiagnosticErrorDetails {
            code: "flowchart.edge.invalid".to_string(),
            span: Some(MermanDiagnosticSpan {
                start: 3,
                end: 8,
                kind: "exact".to_string(),
            }),
            field: Some("edge".to_string()),
            diagram_type: Some("flowchart".to_string()),
            requested_max_width: None,
            actual_width: None,
            width_profile: None,
            fallback_reason: None,
        })
    );
}

#[test]
fn uniffi_error_preserves_structured_cancellation_details() {
    let error = MermanError::from_binding(BindingError::cancelled(merman::OperationCancelled {
        phase: merman::OperationPhase::Layout,
        reason: merman::CancelReason::DeadlineExceeded,
    }));
    let MermanError::Binding {
        code,
        code_name,
        resource,
        cancellation,
        ..
    } = error;

    assert_eq!(code, BindingStatus::Cancelled.code());
    assert_eq!(code_name, BindingStatus::Cancelled.code_name());
    assert_eq!(resource, None);
    assert_eq!(
        cancellation,
        Some(MermanCancelledDetails {
            reason: "deadline_exceeded".to_string(),
            phase: "layout".to_string(),
        })
    );
}

#[cfg(feature = "svg")]
#[test]
fn engine_error_preserves_binding_status() {
    let err = engine()
        .render_svg("flowchart TD\nA".to_string(), Some("{".to_string()))
        .unwrap_err();

    let MermanError::Binding {
        code,
        code_name,
        kind,
        capability_id,
        resource,
        message,
        ..
    } = err;
    assert_eq!(code, BindingStatus::OptionsJsonError.code());
    assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
    assert_eq!(kind, MermanErrorKind::Generic);
    assert_eq!(capability_id, None);
    assert_eq!(resource, None);
    assert!(message.contains("invalid options_json"));
}
