use super::{DiagnosticContext, LanguageSession};
use crate::client_profile::ClientProtocolProfile;
use crate::diagnostics::unavailable_document_diagnostic_round_trip;
use crate::sync::lock_recovering_poison;
use futures::FutureExt;
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use tokio::sync::Notify;
use tower_lsp_server::Client;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{Diagnostic, MessageType, Uri};

type ClientEffect = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub(crate) const LSP_CLIENT_EFFECT_QUEUE_LIMIT: usize = 64;
pub(crate) const MAX_CLIENT_LOG_MESSAGE_BYTES: usize = 4 * 1024;
pub(crate) const CLIENT_LOG_TRUNCATION_SUFFIX: &str = " [truncated]";

pub(crate) fn bounded_client_log_message(message: impl Into<String>) -> String {
    let message = message.into();
    let (prefix, suffix) = if message.len() <= MAX_CLIENT_LOG_MESSAGE_BYTES {
        (message.as_str(), "")
    } else {
        let mut end = MAX_CLIENT_LOG_MESSAGE_BYTES - CLIENT_LOG_TRUNCATION_SUFFIX.len();
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        (&message[..end], CLIENT_LOG_TRUNCATION_SUFFIX)
    };
    let mut bounded = String::with_capacity(prefix.len() + suffix.len());
    bounded.push_str(prefix);
    bounded.push_str(suffix);
    bounded
}

/// Domain-facing client effects. The queue and refresh lanes remain private collaborators.
#[derive(Debug, Clone)]
pub(crate) struct ClientEffects {
    client: Client,
    session: LanguageSession,
    client_profile: Arc<OnceLock<ClientProtocolProfile>>,
    #[cfg(test)]
    diagnostic_pre_admission_gate: DiagnosticPreAdmissionTestGate,
}

impl ClientEffects {
    pub(crate) fn new(
        client: Client,
        session: LanguageSession,
        client_profile: Arc<OnceLock<ClientProtocolProfile>>,
    ) -> Self {
        Self {
            client,
            session,
            client_profile,
            #[cfg(test)]
            diagnostic_pre_admission_gate: DiagnosticPreAdmissionTestGate::default(),
        }
    }

    pub(crate) async fn push_diagnostics(&self, uri: Uri) {
        let Some(publisher) = self.diagnostic_publisher() else {
            return;
        };
        publisher.enqueue_uri(uri).await;
    }

    pub(crate) async fn republish_diagnostics(&self) {
        let Some(publisher) = self.diagnostic_publisher() else {
            return;
        };
        publisher.enqueue_all().await;
    }

    pub(crate) async fn log_message(&self, kind: MessageType, message: impl Into<String>) {
        let client = self.client.clone();
        let message = bounded_client_log_message(message);
        self.session
            .enqueue_latest_client_effect(ClientEffectKey::LogMessage, async move {
                client.log_message(kind, message).await;
            })
            .await;
    }

    pub(crate) fn refresh_semantic_tokens(&self) {
        let profile = self.profile();
        if profile.semantic_tokens.is_some() && profile.semantic_tokens_refresh {
            self.session.request_semantic_tokens_refresh();
        }
    }

    pub(crate) fn refresh_diagnostics(&self) {
        let profile = self.profile();
        if profile.diagnostic_pull && profile.diagnostic_refresh {
            self.session.request_diagnostic_refresh();
        }
    }

    pub(crate) async fn configuration_changed(
        &self,
        change: super::documents::ConfigurationUpdateOutcome,
    ) {
        if change.affects_snapshots() {
            self.refresh_semantic_tokens();
        }
        if change.affects_diagnostics() {
            self.refresh_diagnostics();
            self.republish_diagnostics().await;
        }
    }

    #[cfg(test)]
    pub(crate) async fn diagnostics_for_context(
        &self,
        context: &DiagnosticContext,
    ) -> Result<Option<Vec<Diagnostic>>> {
        let Some(publisher) = self.diagnostic_publisher() else {
            return Ok(None);
        };
        publisher.diagnostics_for_current_context(context).await
    }

    #[cfg(test)]
    pub(crate) fn client(&self) -> Client {
        self.client.clone()
    }

    fn diagnostic_publisher(&self) -> Option<DiagnosticPublisher> {
        let profile = self.profile();
        if profile.diagnostic_pull {
            return None;
        }
        Some(DiagnosticPublisher {
            client: self.client.clone(),
            session: self.session.clone(),
            profile,
            #[cfg(test)]
            diagnostic_pre_admission_gate: self.diagnostic_pre_admission_gate.clone(),
        })
    }

    fn profile(&self) -> ClientProtocolProfile {
        self.client_profile
            .get()
            .cloned()
            .unwrap_or_else(ClientProtocolProfile::conservative)
    }

    #[cfg(test)]
    pub(crate) fn admission_count(&self) -> usize {
        self.session.client_effect_admission_count()
    }

    #[cfg(test)]
    pub(crate) async fn block_serial_lane_for_test(&self) -> tokio::sync::oneshot::Sender<()> {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        self.session
            .enqueue_latest_client_effect(
                ClientEffectKey::document_for_test("controlled-blocker"),
                async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                },
            )
            .await;
        started_rx
            .await
            .expect("controlled client effect should start");
        release_tx
    }

    #[cfg(test)]
    pub(crate) fn refresh_request_counts(&self) -> (usize, usize) {
        self.session.refresh_request_counts()
    }

    #[cfg(test)]
    pub(crate) async fn wait_idle(&self) {
        self.session.wait_client_effects_idle().await;
    }

    #[cfg(test)]
    pub(crate) fn block_next_diagnostic_pre_admission_for_test(
        &self,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        self.diagnostic_pre_admission_gate.block_next()
    }

    #[cfg(test)]
    pub(crate) async fn saturate_serial_lane_for_test(&self) -> tokio::sync::oneshot::Sender<()> {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        self.session
            .enqueue_latest_client_effect(
                ClientEffectKey::document_for_test("blocker"),
                async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                },
            )
            .await;
        started_rx
            .await
            .expect("blocking client effect should start");
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            self.session
                .enqueue_latest_client_effect(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }
        release_tx
    }
}

#[derive(Clone)]
struct DiagnosticPublisher {
    client: Client,
    session: LanguageSession,
    profile: ClientProtocolProfile,
    #[cfg(test)]
    diagnostic_pre_admission_gate: DiagnosticPreAdmissionTestGate,
}

impl DiagnosticPublisher {
    async fn enqueue_uri(&self, uri: Uri) {
        let publisher = self.clone();
        self.session
            .enqueue_latest_client_effect_with_transport_admission(
                ClientEffectKey::Document(uri.clone()),
                move |admission| async move {
                    publisher.synchronize_uri(uri, &admission).await;
                },
            )
            .await;
    }

    async fn enqueue_all(&self) {
        let mut uris = self.session.diagnostic_uris().await;
        uris.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        for uri in uris {
            self.enqueue_uri(uri).await;
        }
    }

    async fn diagnostics_for_current_context(
        &self,
        context: &DiagnosticContext,
    ) -> Result<Option<Vec<Diagnostic>>> {
        self.session
            .query_push_diagnostics(context, |context, analysis| {
                unavailable_document_diagnostic_round_trip(context)
                    .map(|round_trip| round_trip.diagnostics_with_profile(&self.profile))
                    .unwrap_or_else(|| {
                        analysis
                            .expect("available documents require an analysis context")
                            .diagnostic_round_trip()
                            .diagnostics_with_profile(&self.profile)
                    })
            })
            .await
    }

    async fn synchronize_uri(&self, uri: Uri, admission: &ClientEffectTransportAdmission) {
        match self.session.diagnostic_context(&uri).await {
            Some(context) => self.publish_current(&context, admission).await,
            None => self.clear(uri, admission).await,
        }
    }

    async fn publish_current(
        &self,
        context: &DiagnosticContext,
        admission: &ClientEffectTransportAdmission,
    ) {
        match self.diagnostics_for_current_context(context).await {
            Ok(Some(diagnostics)) => {
                self.publish(
                    context.document.uri.clone(),
                    diagnostics,
                    self.profile
                        .diagnostics
                        .version
                        .then_some(context.document.version),
                    admission,
                )
                .await;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(
                    uri = %context.document.uri.as_str(),
                    code = ?error.code,
                    message = %error.message,
                    "failed to compute push diagnostics"
                );
            }
        }
    }

    async fn clear(&self, uri: Uri, admission: &ClientEffectTransportAdmission) {
        self.publish(uri, Vec::new(), None, admission).await;
    }

    async fn publish(
        &self,
        uri: Uri,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
        admission: &ClientEffectTransportAdmission,
    ) {
        #[cfg(test)]
        self.diagnostic_pre_admission_gate.wait_if_blocked().await;
        let client = self.client.clone();
        let _ = admission
            .send(async move {
                client.publish_diagnostics(uri, diagnostics, version).await;
            })
            .await;
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
struct DiagnosticPreAdmissionTestGate {
    pending: Arc<Mutex<Option<PendingDiagnosticPreAdmissionTestGate>>>,
}

#[cfg(test)]
struct PendingDiagnosticPreAdmissionTestGate {
    started: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

#[cfg(test)]
impl DiagnosticPreAdmissionTestGate {
    fn block_next(
        &self,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let previous =
            lock_recovering_poison(&self.pending).replace(PendingDiagnosticPreAdmissionTestGate {
                started: started_tx,
                release: release_rx,
            });
        assert!(
            previous.is_none(),
            "only one diagnostic pre-admission test gate may be installed at a time"
        );
        (started_rx, release_tx)
    }

    async fn wait_if_blocked(&self) {
        let pending = lock_recovering_poison(&self.pending).take();
        if let Some(pending) = pending {
            let _ = pending.started.send(());
            let _ = pending.release.await;
        }
    }
}

#[cfg(test)]
impl std::fmt::Debug for DiagnosticPreAdmissionTestGate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiagnosticPreAdmissionTestGate")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ClientEffectKey {
    Document(Uri),
    LogMessage,
}

impl ClientEffectKey {
    #[cfg(test)]
    pub(crate) fn document_for_test(name: impl AsRef<str>) -> Self {
        Self::Document(
            format!("file:///client-effects/{}", name.as_ref())
                .parse()
                .expect("client-effect test URI must be valid"),
        )
    }
}

struct QueuedClientEffect {
    key: ClientEffectKey,
    intent: u64,
    effect: ClientEffect,
}

fn drop_client_effect_safely(effect: QueuedClientEffect) {
    if std::panic::catch_unwind(AssertUnwindSafe(|| drop(effect))).is_err() {
        tracing::error!("LSP client effect panicked while being dropped");
    }
}

fn drop_client_effects_safely(effects: VecDeque<QueuedClientEffect>) {
    for effect in effects {
        drop_client_effect_safely(effect);
    }
}

struct ActiveClientEffect {
    key: ClientEffectKey,
    intent: u64,
    abort: AbortHandle,
}

enum ClientEffectAdmission {
    Cancelled(QueuedClientEffect),
    Superseded(QueuedClientEffect),
    Pending(QueuedClientEffect),
    Admitted {
        replaced: Option<QueuedClientEffect>,
    },
}

enum ClientEffectClaim {
    Cancelled,
    Empty,
    Current(QueuedClientEffect),
    Superseded(QueuedClientEffect),
}

/// Serializes client-visible side effects with bounded, latest-intent admission.
#[derive(Clone)]
pub(super) struct ClientEffectDispatcher {
    inner: Arc<ClientEffectDispatcherInner>,
}

struct ClientEffectDispatcherInner {
    state: Mutex<ClientEffectState>,
    transport_admission: Mutex<()>,
    changed: Notify,
    idle: Notify,
}

#[derive(Clone)]
pub(super) struct ClientEffectTransportAdmission {
    inner: Arc<ClientEffectDispatcherInner>,
    key: ClientEffectKey,
    intent: u64,
}

impl ClientEffectTransportAdmission {
    fn new(inner: Arc<ClientEffectDispatcherInner>, key: ClientEffectKey, intent: u64) -> Self {
        Self { inner, key, intent }
    }

    fn is_current(&self) -> bool {
        let state = lock_recovering_poison(&self.inner.state);
        !state.cancelled && state.latest_intents.get(&self.key) == Some(&self.intent)
    }

    async fn send(&self, transport: impl Future<Output = ()> + Send + 'static) -> bool {
        ClientEffectTransportFuture {
            admission: self.clone(),
            transport: Box::pin(transport),
        }
        .await
    }
}

struct ClientEffectTransportFuture {
    admission: ClientEffectTransportAdmission,
    transport: ClientEffect,
}

impl Future for ClientEffectTransportFuture {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let admission = self.admission.clone();
        // Intent registration and the one synchronous poll that may call the transport's
        // irreversible `start_send` share this gate. The gate is never held across `.await`.
        let _transport_admission = lock_recovering_poison(&admission.inner.transport_admission);
        if !admission.is_current() {
            return Poll::Ready(false);
        }
        self.transport.as_mut().poll(context).map(|()| true)
    }
}

#[derive(Default)]
struct ClientEffectState {
    queue: VecDeque<QueuedClientEffect>,
    latest_intents: HashMap<ClientEffectKey, u64>,
    active_effect: Option<ActiveClientEffect>,
    cancelled: bool,
    next_intent_id: u64,
    next_worker_id: u64,
    active_worker: Option<ActiveClientEffectWorker>,
    #[cfg(test)]
    admitted_count: usize,
}

impl ClientEffectState {
    fn register_intent(&mut self, key: ClientEffectKey) -> Option<u64> {
        if self.cancelled {
            return None;
        }
        self.next_intent_id = self
            .next_intent_id
            .checked_add(1)
            .expect("client effect intent sequence exhausted");
        let intent = self.next_intent_id;
        let active_abort = self
            .active_effect
            .as_ref()
            .filter(|active| active.key == key && matches!(&key, ClientEffectKey::Document(_)))
            .map(|active| active.abort.clone());
        self.latest_intents.insert(key, intent);
        if let Some(active_abort) = active_abort {
            active_abort.abort();
        }
        Some(intent)
    }

    fn remove_intent_if_current(&mut self, key: &ClientEffectKey, intent: u64) -> bool {
        if self.latest_intents.get(key) != Some(&intent) {
            return false;
        }
        self.latest_intents.remove(key);
        true
    }

    fn try_admit(&mut self, queued: QueuedClientEffect) -> ClientEffectAdmission {
        if self.cancelled {
            return ClientEffectAdmission::Cancelled(queued);
        }
        if self.latest_intents.get(&queued.key) != Some(&queued.intent) {
            return ClientEffectAdmission::Superseded(queued);
        }

        let replacement = self
            .queue
            .iter()
            .position(|candidate| candidate.key == queued.key);
        if replacement.is_none() && self.queue.len() >= LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            return ClientEffectAdmission::Pending(queued);
        }

        let replaced = replacement.and_then(|index| self.queue.remove(index));
        self.queue.push_back(queued);
        #[cfg(test)]
        {
            self.admitted_count += 1;
        }
        ClientEffectAdmission::Admitted { replaced }
    }

    fn claim_next(&mut self) -> ClientEffectClaim {
        if self.cancelled {
            return ClientEffectClaim::Cancelled;
        }
        let Some(queued) = self.queue.pop_front() else {
            return ClientEffectClaim::Empty;
        };
        if self.latest_intents.get(&queued.key) != Some(&queued.intent) {
            return ClientEffectClaim::Superseded(queued);
        }

        ClientEffectClaim::Current(queued)
    }

    fn begin_effect(&mut self, key: ClientEffectKey, intent: u64, abort: AbortHandle) -> bool {
        if self.cancelled
            || self.latest_intents.get(&key) != Some(&intent)
            || self.active_effect.is_some()
        {
            return false;
        }
        self.active_effect = Some(ActiveClientEffect { key, intent, abort });
        true
    }

    fn finish_effect(&mut self, key: &ClientEffectKey, intent: u64) {
        if self
            .active_effect
            .as_ref()
            .is_some_and(|active| active.key == *key && active.intent == intent)
        {
            self.active_effect = None;
        }
        let _ = self.remove_intent_if_current(key, intent);
    }

    fn abandon_active_effect(&mut self) {
        let Some(active) = self.active_effect.take() else {
            return;
        };
        let _ = self.remove_intent_if_current(&active.key, active.intent);
    }

    #[cfg(test)]
    fn has_active_effect(&self) -> bool {
        self.active_effect.is_some()
    }
}

struct ActiveClientEffectWorker {
    id: u64,
    abort: AbortHandle,
}

struct ClientEffectWorkerGuard {
    dispatcher: ClientEffectDispatcher,
    worker_id: u64,
}

impl Drop for ClientEffectWorkerGuard {
    fn drop(&mut self) {
        self.dispatcher.worker_finished(self.worker_id);
    }
}

struct ClientEffectIntentGuard {
    inner: Arc<ClientEffectDispatcherInner>,
    key: ClientEffectKey,
    intent: u64,
    armed: bool,
}

impl ClientEffectIntentGuard {
    fn register(inner: Arc<ClientEffectDispatcherInner>, key: ClientEffectKey) -> Option<Self> {
        let intent = {
            let _transport_admission = lock_recovering_poison(&inner.transport_admission);
            let intent = lock_recovering_poison(&inner.state).register_intent(key.clone())?;
            inner.changed.notify_waiters();
            intent
        };
        Some(Self {
            inner,
            key,
            intent,
            armed: true,
        })
    }

    fn intent(&self) -> u64 {
        self.intent
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ClientEffectIntentGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // This guard only rolls back an intent that has not been admitted into
        // the queue. Registration already linearizes against transport polls;
        // taking the admission lock again here would only add a destructor
        // blocking/reentrancy hazard.
        let removed = lock_recovering_poison(&self.inner.state)
            .remove_intent_if_current(&self.key, self.intent);
        if removed {
            self.inner.changed.notify_waiters();
        }
    }
}

impl std::fmt::Debug for ClientEffectDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = lock_recovering_poison(&self.inner.state);
        formatter
            .debug_struct("ClientEffectDispatcher")
            .field("queued", &state.queue.len())
            .field("running", &state.active_worker.is_some())
            .field("cancelled", &state.cancelled)
            .finish()
    }
}

impl ClientEffectDispatcher {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(ClientEffectDispatcherInner {
                state: Mutex::new(ClientEffectState::default()),
                transport_admission: Mutex::new(()),
                changed: Notify::new(),
                idle: Notify::new(),
            }),
        }
    }

    pub(super) async fn enqueue_latest(
        &self,
        key: ClientEffectKey,
        effect: impl Future<Output = ()> + Send + 'static,
    ) {
        self.enqueue_latest_with_transport_admission(key, |_| effect)
            .await;
    }

    pub(super) async fn enqueue_latest_with_transport_admission<F, Fut>(
        &self,
        key: ClientEffectKey,
        effect: F,
    ) where
        F: FnOnce(ClientEffectTransportAdmission) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let Some(mut intent) =
            ClientEffectIntentGuard::register(Arc::clone(&self.inner), key.clone())
        else {
            return;
        };
        let transport_admission = ClientEffectTransportAdmission::new(
            Arc::clone(&self.inner),
            key.clone(),
            intent.intent(),
        );
        let mut queued = Some(QueuedClientEffect {
            key,
            intent: intent.intent(),
            effect: Box::pin(effect(transport_admission)),
        });

        let registration = loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (admission, registration) = {
                let mut state = lock_recovering_poison(&self.inner.state);
                let admission = state.try_admit(
                    queued
                        .take()
                        .expect("a client effect admission always returns ownership"),
                );
                let registration = if matches!(&admission, ClientEffectAdmission::Admitted { .. })
                    && state.active_worker.is_none()
                {
                    Some(Self::register_worker(&mut state))
                } else {
                    None
                };
                (admission, registration)
            };

            match admission {
                ClientEffectAdmission::Cancelled(cancelled)
                | ClientEffectAdmission::Superseded(cancelled) => {
                    drop_client_effect_safely(cancelled);
                    return;
                }
                ClientEffectAdmission::Pending(pending) => {
                    queued = Some(pending);
                    changed.as_mut().await;
                }
                ClientEffectAdmission::Admitted { replaced } => {
                    intent.disarm();
                    if let Some(replaced) = replaced {
                        drop_client_effect_safely(replaced);
                    }
                    break registration;
                }
            }
        };

        self.inner.changed.notify_waiters();
        if let Some((worker_id, registration)) = registration {
            self.spawn_worker(worker_id, registration);
        }
    }

    pub(super) fn cancel(&self) {
        let (worker_abort, active_effect, queued) = {
            let _transport_admission = lock_recovering_poison(&self.inner.transport_admission);
            let mut state = lock_recovering_poison(&self.inner.state);
            state.cancelled = true;
            let queued = std::mem::take(&mut state.queue);
            state.latest_intents.clear();
            let active_effect = state.active_effect.take();
            let worker_abort = state
                .active_worker
                .as_ref()
                .map(|worker| worker.abort.clone());
            (worker_abort, active_effect, queued)
        };

        if let Some(active_effect) = active_effect {
            active_effect.abort.abort();
        }
        if let Some(worker_abort) = worker_abort {
            worker_abort.abort();
        }
        drop_client_effects_safely(queued);
        self.inner.changed.notify_waiters();
        self.inner.idle.notify_waiters();
    }

    #[cfg(test)]
    pub(super) fn admission_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state).admitted_count
    }

    #[cfg(test)]
    pub(super) fn latest_intent_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state)
            .latest_intents
            .len()
    }

    #[cfg(test)]
    pub(super) fn has_active_effect(&self) -> bool {
        lock_recovering_poison(&self.inner.state).has_active_effect()
    }

    #[cfg(test)]
    pub(super) async fn wait_idle(&self) {
        loop {
            let idle = self.inner.idle.notified();
            tokio::pin!(idle);
            idle.as_mut().enable();
            let is_idle = {
                let state = lock_recovering_poison(&self.inner.state);
                state.active_worker.is_none() && state.queue.is_empty()
            };
            if is_idle {
                return;
            }
            idle.as_mut().await;
        }
    }

    async fn drain(&self) {
        loop {
            let claim = lock_recovering_poison(&self.inner.state).claim_next();
            match claim {
                ClientEffectClaim::Cancelled | ClientEffectClaim::Empty => return,
                ClientEffectClaim::Superseded(superseded) => {
                    self.inner.changed.notify_waiters();
                    drop_client_effect_safely(superseded);
                }
                ClientEffectClaim::Current(current) => {
                    let key = current.key.clone();
                    let intent = current.intent;
                    let (abort, registration) = AbortHandle::new_pair();
                    let is_current = lock_recovering_poison(&self.inner.state).begin_effect(
                        key.clone(),
                        intent,
                        abort,
                    );
                    if !is_current {
                        let _ = lock_recovering_poison(&self.inner.state)
                            .remove_intent_if_current(&key, intent);
                        self.inner.changed.notify_waiters();
                        drop_client_effect_safely(current);
                        continue;
                    }
                    self.inner.changed.notify_waiters();
                    let outcome = Abortable::new(
                        AssertUnwindSafe(current.effect).catch_unwind(),
                        registration,
                    )
                    .await;
                    lock_recovering_poison(&self.inner.state).finish_effect(&key, intent);
                    self.inner.changed.notify_waiters();
                    if matches!(outcome, Ok(Err(_))) {
                        tracing::error!("LSP client effect panicked");
                    }
                }
            }
        }
    }

    fn register_worker(state: &mut ClientEffectState) -> (u64, AbortRegistration) {
        state.next_worker_id = state
            .next_worker_id
            .checked_add(1)
            .expect("client effect worker sequence exhausted");
        let worker_id = state.next_worker_id;
        let (abort, registration) = AbortHandle::new_pair();
        state.active_worker = Some(ActiveClientEffectWorker {
            id: worker_id,
            abort,
        });
        (worker_id, registration)
    }

    fn spawn_worker(&self, worker_id: u64, registration: AbortRegistration) {
        let dispatcher = self.clone();
        tokio::spawn(async move {
            let _worker = ClientEffectWorkerGuard {
                dispatcher: dispatcher.clone(),
                worker_id,
            };
            let outcome = Abortable::new(
                AssertUnwindSafe(dispatcher.drain()).catch_unwind(),
                registration,
            )
            .await;
            if matches!(outcome, Ok(Err(_))) {
                tracing::error!("LSP client effect worker panicked");
            }
        });
    }

    fn worker_finished(&self, worker_id: u64) {
        let replacement = {
            let mut state = lock_recovering_poison(&self.inner.state);
            if state
                .active_worker
                .as_ref()
                .is_none_or(|worker| worker.id != worker_id)
            {
                return;
            }
            state.active_worker = None;
            state.abandon_active_effect();
            if !state.cancelled && !state.queue.is_empty() {
                Some(Self::register_worker(&mut state))
            } else {
                None
            }
        };

        self.inner.changed.notify_waiters();
        if let Some((worker_id, registration)) = replacement {
            self.spawn_worker(worker_id, registration);
        } else {
            self.inner.idle.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::task::{Context, Poll, Wake, Waker};

    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("intentional client-effect drop panic");
        }
    }

    #[test]
    fn registering_new_intent_before_claim_supersedes_queued_effect() {
        let key = ClientEffectKey::document_for_test("same");
        let mut state = ClientEffectState::default();
        let older = state.register_intent(key.clone()).unwrap();
        let admission = state.try_admit(QueuedClientEffect {
            key: key.clone(),
            intent: older,
            effect: Box::pin(async {}),
        });
        assert!(matches!(
            admission,
            ClientEffectAdmission::Admitted { replaced: None }
        ));

        let _newer = state.register_intent(key).unwrap();
        assert!(matches!(
            state.claim_next(),
            ClientEffectClaim::Superseded(_)
        ));
    }

    #[test]
    fn dropping_new_intent_does_not_resurrect_queued_old_effect() {
        let key = ClientEffectKey::document_for_test("same");
        let mut state = ClientEffectState::default();
        let older = state.register_intent(key.clone()).unwrap();
        let admission = state.try_admit(QueuedClientEffect {
            key: key.clone(),
            intent: older,
            effect: Box::pin(async {}),
        });
        assert!(matches!(
            admission,
            ClientEffectAdmission::Admitted { replaced: None }
        ));

        let newer = state.register_intent(key.clone()).unwrap();
        assert!(state.remove_intent_if_current(&key, newer));
        assert!(matches!(
            state.claim_next(),
            ClientEffectClaim::Superseded(_)
        ));
    }

    #[test]
    fn registering_superseding_intent_wakes_existing_waiter() {
        #[derive(Default)]
        struct WakeCount(AtomicUsize);

        impl Wake for WakeCount {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }

            fn wake_by_ref(self: &Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let _older =
            ClientEffectIntentGuard::register(Arc::clone(&dispatcher.inner), key.clone()).unwrap();
        let mut changed = Box::pin(dispatcher.inner.changed.notified());
        let wake_count = Arc::new(WakeCount::default());
        let waker = Waker::from(Arc::clone(&wake_count));
        let mut context = Context::from_waker(&waker);
        assert!(matches!(changed.as_mut().poll(&mut context), Poll::Pending));

        let _newer = ClientEffectIntentGuard::register(Arc::clone(&dispatcher.inner), key).unwrap();

        assert_eq!(wake_count.0.load(Ordering::Relaxed), 1);
        assert!(matches!(
            changed.as_mut().poll(&mut context),
            Poll::Ready(())
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_effects_finish_in_enqueue_order() {
        let dispatcher = ClientEffectDispatcher::new();
        let (release_first_tx, release_first_rx) = tokio::sync::oneshot::channel();
        let (first_done_tx, first_done_rx) = tokio::sync::oneshot::channel();
        let (second_done_tx, mut second_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("first"), async move {
                let _ = release_first_rx.await;
                let _ = first_done_tx.send(());
            })
            .await;
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("second"), async move {
                let _ = second_done_tx.send(());
            })
            .await;

        assert!(matches!(futures::poll!(&mut second_done_rx), Poll::Pending));
        release_first_tx.send(()).unwrap();
        first_done_rx.await.unwrap();
        second_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn panicking_client_effect_does_not_stall_queued_or_later_effects() {
        let dispatcher = ClientEffectDispatcher::new();
        let (panic_started_tx, panic_started_rx) = tokio::sync::oneshot::channel();
        let (release_panic_tx, release_panic_rx) = tokio::sync::oneshot::channel();
        let (queued_done_tx, queued_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("panic"), async move {
                let _ = panic_started_tx.send(());
                let _ = release_panic_rx.await;
                panic!("intentional client-effect panic");
            })
            .await;
        panic_started_rx
            .await
            .expect("panicking client effect should start");

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("queued"), async move {
                let _ = queued_done_tx.send(());
            })
            .await;
        release_panic_tx
            .send(())
            .expect("panicking client effect should still be active");
        queued_done_rx
            .await
            .expect("a queued effect should run after the panic is isolated");

        let (later_done_tx, later_done_rx) = tokio::sync::oneshot::channel();
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("later"), async move {
                let _ = later_done_tx.send(());
            })
            .await;
        later_done_rx
            .await
            .expect("a later enqueue should use the surviving dispatcher worker");
        dispatcher.wait_idle().await;
        assert!(!dispatcher.has_active_effect());
        assert_eq!(dispatcher.latest_intent_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pending_document_effects_keep_only_the_latest_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let superseded_ran = Arc::new(AtomicBool::new(false));
        let latest_ran = Arc::new(AtomicBool::new(false));

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("blocker"), async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        started_rx.await.unwrap();

        let superseded_marker = Arc::clone(&superseded_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("same"), async move {
                superseded_marker.store(true, Ordering::Release);
            })
            .await;
        let latest_marker = Arc::clone(&latest_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("same"), async move {
                latest_marker.store(true, Ordering::Release);
            })
            .await;

        release_tx.send(()).unwrap();
        dispatcher.wait_idle().await;

        assert!(!superseded_ran.load(Ordering::Acquire));
        assert!(latest_ran.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn superseded_effect_drop_panic_does_not_stall_the_serial_lane() {
        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let (blocker_started_tx, blocker_started_rx) = tokio::sync::oneshot::channel();
        let (release_blocker_tx, release_blocker_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("blocker"), async move {
                let _ = blocker_started_tx.send(());
                let _ = release_blocker_rx.await;
            })
            .await;
        blocker_started_rx.await.unwrap();
        let panic_on_drop = PanicOnDrop;
        dispatcher
            .enqueue_latest(key.clone(), async move {
                let _panic_on_drop = panic_on_drop;
                std::future::pending::<()>().await;
            })
            .await;

        let (latest_done_tx, latest_done_rx) = tokio::sync::oneshot::channel();
        dispatcher
            .enqueue_latest(key, async move {
                let _ = latest_done_tx.send(());
            })
            .await;

        release_blocker_tx.send(()).unwrap();
        latest_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
        assert!(!dispatcher.has_active_effect());
        assert_eq!(dispatcher.latest_intent_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn saturated_effect_queue_keeps_the_newest_same_key_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (release_active_tx, release_active_rx) = tokio::sync::oneshot::channel();
        let (queued_started_tx, queued_started_rx) = tokio::sync::oneshot::channel();
        let (release_queued_tx, release_queued_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("active"), async move {
                let _ = active_started_tx.send(());
                let _ = release_active_rx.await;
            })
            .await;
        active_started_rx.await.unwrap();
        dispatcher
            .enqueue_latest(
                ClientEffectKey::document_for_test("queued-blocker"),
                async move {
                    let _ = queued_started_tx.send(());
                    let _ = release_queued_rx.await;
                },
            )
            .await;
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT - 1 {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let older_ran = Arc::new(AtomicBool::new(false));
        let older_marker = Arc::clone(&older_ran);
        let older = dispatcher.enqueue_latest(ClientEffectKey::LogMessage, async move {
            older_marker.store(true, Ordering::Release);
        });
        tokio::pin!(older);
        assert!(futures::poll!(&mut older).is_pending());

        let newer_ran = Arc::new(AtomicBool::new(false));
        let newer_marker = Arc::clone(&newer_ran);
        let newer = dispatcher.enqueue_latest(ClientEffectKey::LogMessage, async move {
            newer_marker.store(true, Ordering::Release);
        });
        tokio::pin!(newer);
        assert!(futures::poll!(&mut newer).is_pending());
        assert!(futures::poll!(&mut older).is_ready());

        release_active_tx.send(()).unwrap();
        queued_started_rx.await.unwrap();

        assert!(futures::poll!(&mut newer).is_ready());

        release_queued_tx.send(()).unwrap();
        dispatcher.wait_idle().await;

        assert!(!older_ran.load(Ordering::Acquire));
        assert!(newer_ran.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_waiting_effects_release_their_latest_intents() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("active"), async move {
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        started_rx.await.unwrap();
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let baseline = dispatcher.latest_intent_count();
        for index in 0..4 {
            let mut waiting = Box::pin(dispatcher.enqueue_latest(
                ClientEffectKey::document_for_test(format!("dropped-{index}")),
                async {},
            ));
            assert!(futures::poll!(&mut waiting).is_pending());
            assert_eq!(dispatcher.latest_intent_count(), baseline + 1);
            drop(waiting);
            assert_eq!(dispatcher.latest_intent_count(), baseline);
        }

        dispatcher.cancel();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelling_client_effects_waits_for_the_active_future_to_drop() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (effect_alive_tx, effect_dropped_rx) = tokio::sync::oneshot::channel::<()>();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("active"), async move {
                let _effect_alive = effect_alive_tx;
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        started_rx.await.expect("client effect should start");

        dispatcher.cancel();
        dispatcher.wait_idle().await;

        assert!(
            effect_dropped_rx.await.is_err(),
            "idle must mean the aborted effect future has actually been dropped"
        );
        assert!(!dispatcher.has_active_effect());
        assert_eq!(dispatcher.latest_intent_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn distinct_client_effects_apply_backpressure_at_the_queue_limit() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("blocker"), async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        started_rx.await.unwrap();

        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let overflow_dispatcher = dispatcher.clone();
        let (admitted_tx, mut admitted_rx) = tokio::sync::oneshot::channel();
        let overflow = tokio::spawn(async move {
            overflow_dispatcher
                .enqueue_latest(ClientEffectKey::document_for_test("overflow"), async {})
                .await;
            let _ = admitted_tx.send(());
        });
        assert!(matches!(futures::poll!(&mut admitted_rx), Poll::Pending));

        release_tx.send(()).unwrap();
        admitted_rx.await.unwrap();
        overflow.await.unwrap();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn full_queue_admits_latest_after_cancelling_active_same_key() {
        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (active_alive_tx, active_dropped_rx) = tokio::sync::oneshot::channel::<()>();

        dispatcher
            .enqueue_latest(key.clone(), async move {
                let _active_alive = active_alive_tx;
                let _ = active_started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        active_started_rx.await.unwrap();
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let (latest_done_tx, latest_done_rx) = tokio::sync::oneshot::channel();
        let latest = dispatcher.enqueue_latest(key, async move {
            let _ = latest_done_tx.send(());
        });
        tokio::pin!(latest);
        assert!(futures::poll!(&mut latest).is_pending());
        assert!(
            active_dropped_rx.await.is_err(),
            "queue saturation must not keep the superseded active effect alive"
        );

        latest.await;
        latest_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
        assert!(!dispatcher.has_active_effect());
        assert_eq!(dispatcher.latest_intent_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn new_same_key_intent_cancels_active_effect_before_running_latest() {
        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (active_alive_tx, active_dropped_rx) = tokio::sync::oneshot::channel::<()>();
        let (queued_done_tx, queued_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(key.clone(), async move {
                let _active_alive = active_alive_tx;
                let _ = active_started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        active_started_rx.await.unwrap();

        dispatcher
            .enqueue_latest(key, async move {
                let _ = queued_done_tx.send(());
            })
            .await;
        assert!(
            active_dropped_rx.await.is_err(),
            "superseding intent must drop the active transport future"
        );
        queued_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
        assert!(!dispatcher.has_active_effect());
        assert_eq!(dispatcher.latest_intent_count(), 0);
    }
}
