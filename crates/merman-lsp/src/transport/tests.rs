use super::framing::FrameError;
use super::*;
use std::{
    convert::Infallible,
    future::Future,
    io::Cursor,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};
use tokio::sync::oneshot;
use tokio::time::timeout;

fn frame(body: &[u8]) -> Vec<u8> {
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(body);
    frame
}

fn request_method(frame: FrameRead) -> String {
    match frame {
        FrameRead::Message {
            message: IncomingMessage::Request(request),
            ..
        } => request.method().to_owned(),
        _ => panic!("expected JSON-RPC request"),
    }
}

struct ResponseLoopback {
    responses: mpsc::UnboundedSender<Response>,
}

struct PendingResponseLoopback {
    response_polled: oneshot::Sender<()>,
}

struct PendingResponseSink {
    response_polled: Option<oneshot::Sender<()>>,
}

impl Loopback for ResponseLoopback {
    type RequestStream = stream::Empty<Request>;
    type ResponseSink = mpsc::UnboundedSender<Response>;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        (stream::empty(), self.responses)
    }
}

impl Loopback for PendingResponseLoopback {
    type RequestStream = stream::Empty<Request>;
    type ResponseSink = PendingResponseSink;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        (
            stream::empty(),
            PendingResponseSink {
                response_polled: Some(self.response_polled),
            },
        )
    }
}

impl Sink<Response> for PendingResponseSink {
    type Error = Infallible;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Some(response_polled) = self.get_mut().response_polled.take() {
            let _ = response_polled.send(());
        }
        Poll::Pending
    }

    fn start_send(self: Pin<&mut Self>, _item: Response) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Pending
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

struct BudgetedService {
    calls: Arc<AtomicUsize>,
    first_started: Option<oneshot::Sender<()>>,
    release_first: Option<oneshot::Receiver<()>>,
    second_called: Option<oneshot::Sender<()>>,
}

struct ExitReleasesBudgetedService {
    calls: Arc<AtomicUsize>,
    main_release: Option<oneshot::Receiver<()>>,
    control_release: Option<oneshot::Receiver<()>>,
    release_main: Option<oneshot::Sender<()>>,
    release_control: Option<oneshot::Sender<()>>,
    exit_seen: Option<oneshot::Sender<()>>,
}

impl Service<Request> for ExitReleasesBudgetedService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Option<Response>, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match request.method() {
            "test/block" => {
                let release = self.main_release.take().expect("single main request");
                Box::pin(async move {
                    let _ = release.await;
                    Ok(None)
                })
            }
            "$/cancelRequest" => {
                let release = self.control_release.take().expect("single control request");
                Box::pin(async move {
                    let _ = release.await;
                    Ok(None)
                })
            }
            "exit" => {
                let _ = self
                    .release_main
                    .take()
                    .expect("single exit notification")
                    .send(());
                let _ = self
                    .release_control
                    .take()
                    .expect("single exit notification")
                    .send(());
                let _ = self
                    .exit_seen
                    .take()
                    .expect("single exit notification")
                    .send(());
                Box::pin(async { Ok(None) })
            }
            method => panic!("unexpected test method: {method}"),
        }
    }
}

struct PendingService {
    calls: Arc<AtomicUsize>,
}

struct NeverReadyService {
    calls: Arc<AtomicUsize>,
    readiness_polls: Arc<AtomicUsize>,
}

struct AdmissionProbeService {
    admission_stage: Arc<AtomicUsize>,
    ordinary_polled: Option<oneshot::Sender<()>>,
    cancel_admitted: Option<oneshot::Sender<()>>,
    exit_admitted: Option<oneshot::Sender<()>>,
}

struct ReservedControlService {
    calls: Arc<AtomicUsize>,
    cancel_seen: Option<oneshot::Sender<()>>,
    exit_seen: Option<oneshot::Sender<()>>,
}

struct SharedDeadlineWrite {
    first_flush_release: oneshot::Receiver<()>,
    first_flush_released: bool,
    flush_started: mpsc::UnboundedSender<usize>,
    active_flush: Option<usize>,
    completed_flushes: usize,
}

struct SharedDeadlineResponseSink {
    first_ready_release: oneshot::Receiver<()>,
    first_ready_released: bool,
    ready_started: mpsc::UnboundedSender<usize>,
    active_ready: Option<usize>,
    completed_sends: Arc<AtomicUsize>,
}

struct FailingResponseSink {
    send_attempts: Arc<AtomicUsize>,
}

impl Sink<Response> for SharedDeadlineResponseSink {
    type Error = Infallible;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let send = self.completed_sends.load(Ordering::SeqCst) + 1;
        if self.active_ready != Some(send) {
            self.active_ready = Some(send);
            let _ = self.ready_started.unbounded_send(send);
        }

        if send == 1 && !self.first_ready_released {
            match Pin::new(&mut self.first_ready_release).poll(cx) {
                Poll::Ready(_) => self.first_ready_released = true,
                Poll::Pending => return Poll::Pending,
            }
        }
        if send > 1 {
            return Poll::Pending;
        }

        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, _item: Response) -> Result<(), Self::Error> {
        self.completed_sends.fetch_add(1, Ordering::SeqCst);
        self.active_ready = None;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl Sink<Response> for FailingResponseSink {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, _item: Response) -> Result<(), Self::Error> {
        self.send_attempts.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("intentional response routing failure"))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for SharedDeadlineWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let flush = self.completed_flushes + 1;
        if self.active_flush != Some(flush) {
            self.active_flush = Some(flush);
            let _ = self.flush_started.unbounded_send(flush);
        }

        if flush == 1 && !self.first_flush_released {
            match Pin::new(&mut self.first_flush_release).poll(cx) {
                Poll::Ready(_) => self.first_flush_released = true,
                Poll::Pending => return Poll::Pending,
            }
        }
        if flush > 1 {
            return Poll::Pending;
        }

        self.completed_flushes = flush;
        self.active_flush = None;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Service<Request> for PendingService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: Request) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}

impl Service<Request> for NeverReadyService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.readiness_polls.fetch_add(1, Ordering::SeqCst);
        Poll::Pending
    }

    fn call(&mut self, _request: Request) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(None) })
    }
}

impl StdioService for AdmissionProbeService {
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Option<Response>, Self::Error>> + Send + 'static>>;

    fn admit(&mut self, request: Request) -> Self::Future {
        match request.method() {
            "test/pending" => {
                assert_eq!(self.admission_stage.fetch_add(1, Ordering::SeqCst), 0);
                let ordinary_polled = self
                    .ordinary_polled
                    .take()
                    .expect("single ordinary request");
                Box::pin(async move {
                    let _ = ordinary_polled.send(());
                    std::future::pending().await
                })
            }
            "$/cancelRequest" => {
                assert_eq!(self.admission_stage.fetch_add(1, Ordering::SeqCst), 1);
                let _ = self
                    .cancel_admitted
                    .take()
                    .expect("single cancellation notification")
                    .send(());
                Box::pin(std::future::pending())
            }
            "exit" => {
                assert_eq!(self.admission_stage.fetch_add(1, Ordering::SeqCst), 2);
                let _ = self
                    .exit_admitted
                    .take()
                    .expect("single exit notification")
                    .send(());
                Box::pin(std::future::poll_fn(
                    |_| -> Poll<Result<Option<Response>, Infallible>> {
                        panic!("a valid exit future must be dropped without polling")
                    },
                ))
            }
            method => panic!("unexpected admission probe method: {method}"),
        }
    }
}

impl Service<Request> for ReservedControlService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match request.method() {
            "$/cancelRequest" => {
                let cancel_seen = self.cancel_seen.take().expect("single cancel notification");
                let _ = cancel_seen.send(());
                Box::pin(std::future::pending())
            }
            "exit" => {
                let exit_seen = self.exit_seen.take().expect("single exit notification");
                let _ = exit_seen.send(());
                Box::pin(async { Ok(None) })
            }
            _ => Box::pin(std::future::pending()),
        }
    }
}

impl Service<Request> for BudgetedService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match request.id() {
            Some(Id::Number(1)) => {
                let first_started = self
                    .first_started
                    .take()
                    .expect("first request is called once");
                let release_first = self
                    .release_first
                    .take()
                    .expect("first request is called once");
                Box::pin(async move {
                    let _ = first_started.send(());
                    let _ = release_first.await;
                    Ok(None)
                })
            }
            Some(Id::Number(2)) => {
                let second_called = self
                    .second_called
                    .take()
                    .expect("second request is called once");
                let _ = second_called.send(());
                Box::pin(async { Ok(None) })
            }
            id => panic!("unexpected test request id: {id:?}"),
        }
    }
}

macro_rules! impl_stdio_service {
    ($($service:ty),+ $(,)?) => {
        $(
            impl StdioService for $service {
                type Error = <$service as Service<Request>>::Error;
                type Future = <$service as Service<Request>>::Future;

                fn admit(&mut self, request: Request) -> Self::Future {
                    Service::call(self, request)
                }
            }
        )+
    };
}

impl_stdio_service!(
    BudgetedService,
    ExitReleasesBudgetedService,
    PendingService,
    NeverReadyService,
    ReservedControlService,
);

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_preserves_pipelined_messages() {
    let mut bytes = frame(br#"{"jsonrpc":"2.0","id":9,"method":"exit"}"#);
    bytes.extend(frame(br#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#));
    let mut reader = LspFrameReader::new(Cursor::new(bytes));

    assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
    assert_eq!(
        request_method(reader.read_frame().await.unwrap()),
        "shutdown"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_recovers_after_utf8_and_json_errors() {
    let mut bytes = frame(&[0xff]);
    bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"#));
    bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
    let mut reader = LspFrameReader::new(Cursor::new(bytes));

    assert!(matches!(
        reader.read_frame().await.unwrap(),
        FrameRead::Error(FrameError::InvalidUtf8(_), Recovery::Continue)
    ));
    assert!(matches!(
        reader.read_frame().await.unwrap(),
        FrameRead::Error(FrameError::InvalidJson(_), Recovery::Continue)
    ));
    assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_reports_an_empty_body_and_recovers() {
    let mut bytes = b"Content-Length: 0\r\n\r\n".to_vec();
    bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
    let mut reader = LspFrameReader::new(Cursor::new(bytes));

    let error = reader.read_frame().await.unwrap();
    assert!(matches!(
        &error,
        FrameRead::Error(FrameError::EmptyBody, Recovery::Continue)
    ));
    let FrameRead::Error(error, _) = error else {
        unreachable!("empty frame must produce an error")
    };
    assert_eq!(
        error.jsonrpc_error().code,
        tower_lsp_server::jsonrpc::ErrorCode::ParseError
    );
    assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_discards_a_bounded_oversized_body() {
    let mut bytes = frame(&[b'x'; 65]);
    bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
    let mut reader = LspFrameReader::with_limits(Cursor::new(bytes), 64, 64);

    assert!(matches!(
        reader.read_frame().await.unwrap(),
        FrameRead::Error(
            FrameError::BodyTooLarge {
                length: 65,
                limit: 64
            },
            Recovery::Continue
        )
    ));
    assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_recovers_from_an_oversized_header_with_known_length() {
    let oversized = format!("Content-Length: 0\r\nX-Padding: {}\r\n\r\n", "x".repeat(80));
    let mut bytes = oversized.into_bytes();
    bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
    let mut reader = LspFrameReader::with_limits(Cursor::new(bytes), 64, 64);

    assert!(matches!(
        reader.read_frame().await.unwrap(),
        FrameRead::Error(FrameError::HeaderTooLarge { limit: 64 }, Recovery::Continue)
    ));
    assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_stops_scanning_an_unbounded_header() {
    let input = format!("Content-Length: 0\r\nX-Padding: {}", "x".repeat(200));
    let mut reader = LspFrameReader::with_limits(Cursor::new(input.into_bytes()), 64, 64);

    assert!(matches!(
        reader.read_frame().await.unwrap(),
        FrameRead::Error(FrameError::HeaderTooLarge { limit: 64 }, Recovery::Stop)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn frame_reader_distinguishes_clean_and_partial_eof() {
    let mut empty = LspFrameReader::new(Cursor::new(Vec::new()));
    assert!(matches!(empty.read_frame().await.unwrap(), FrameRead::Eof));

    let mut partial = LspFrameReader::new(Cursor::new(b"Content-Length: 5\r\n".to_vec()));
    let error = partial
        .read_frame()
        .await
        .err()
        .expect("partial frame should fail");
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[tokio::test(flavor = "current_thread")]
async fn pending_client_response_routing_does_not_block_cancel_or_exit() {
    let response = br#"{"jsonrpc":"2.0","id":99,"result":null}"#;
    let cancel = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
    let exit = br#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
    let (mut client_input, server_input) = tokio::io::duplex(4096);
    let calls = Arc::new(AtomicUsize::new(0));
    let (response_polled_tx, response_polled_rx) = oneshot::channel();

    let server = tokio::spawn({
        let calls = Arc::clone(&calls);
        async move {
            serve_stdio(
                server_input,
                Vec::<u8>::new(),
                PendingResponseLoopback {
                    response_polled: response_polled_tx,
                },
                PendingService { calls },
            )
            .await
        }
    });

    client_input
        .write_all(&frame(response))
        .await
        .expect("write client response frame");
    client_input
        .flush()
        .await
        .expect("flush client response frame");
    timeout(Duration::from_secs(1), response_polled_rx)
        .await
        .expect("response sink should be polled")
        .expect("response sink poll signal");

    let mut control = frame(cancel);
    control.extend(frame(exit));
    client_input
        .write_all(&control)
        .await
        .expect("write pipelined cancellation and exit frames");
    client_input
        .flush()
        .await
        .expect("flush pipelined control frames");

    let termination = timeout(Duration::from_secs(1), server)
        .await
        .expect("pending response routing must not hold the reader")
        .expect("server task should not panic");

    assert_eq!(termination, StdioTermination::ExitWithoutShutdown);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pending_client_response_routing_remains_time_bounded() {
    let response = br#"{"jsonrpc":"2.0","id":99,"result":null}"#;
    let (mut client_input, server_input) = tokio::io::duplex(4096);
    let calls = Arc::new(AtomicUsize::new(0));
    let (response_polled_tx, response_polled_rx) = oneshot::channel();
    let server = tokio::spawn({
        let calls = Arc::clone(&calls);
        async move {
            serve_stdio(
                server_input,
                Vec::<u8>::new(),
                PendingResponseLoopback {
                    response_polled: response_polled_tx,
                },
                PendingService { calls },
            )
            .await
        }
    });

    client_input
        .write_all(&frame(response))
        .await
        .expect("write client response frame");
    client_input
        .flush()
        .await
        .expect("flush client response frame");
    timeout(Duration::from_secs(1), response_polled_rx)
        .await
        .expect("response sink should be polled")
        .expect("response sink should remain pending after its first poll");

    let termination = timeout(
        CLIENT_RESPONSE_DISPATCH_TIMEOUT + Duration::from_secs(1),
        server,
    )
    .await
    .expect("pending response routing should hit its deadline")
    .expect("server task should not panic");
    assert_eq!(termination, StdioTermination::InputClosed);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pending_tower_readiness_is_not_polled_before_stdio_admission() {
    let calls = Arc::new(AtomicUsize::new(0));
    let readiness_polls = Arc::new(AtomicUsize::new(0));
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let stop = timeout(
        Duration::from_secs(1),
        stdio_server(
            Cursor::new(frame(
                br#"{"jsonrpc":"2.0","id":1,"method":"test/pending","params":null}"#,
            )),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
        )
        .serve_inner(NeverReadyService {
            calls: Arc::clone(&calls),
            readiness_polls: Arc::clone(&readiness_polls),
        }),
    )
    .await
    .expect("stdio admission must not wait for Tower readiness");

    assert_eq!(stop, TransportStop::InputClosed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(readiness_polls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn request_byte_budget_fails_closed_without_blocking_prior_client_responses() {
    let first = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
    let response = br#"{"jsonrpc":"2.0","id":99,"result":null}"#;
    let second = br#"{"jsonrpc":"2.0","id":2,"method":"test/block","params":null}"#;
    assert_eq!(first.len(), second.len());

    let mut input = frame(first);
    input.extend(frame(response));
    input.extend(frame(second));

    let calls = Arc::new(AtomicUsize::new(0));
    let (first_started_tx, _first_started_rx) = oneshot::channel();
    let (_release_first_tx, release_first_rx) = oneshot::channel();
    let (second_called_tx, second_called_rx) = oneshot::channel();
    let (responses_tx, mut responses_rx) = mpsc::unbounded();

    let server = stdio_server(
        Cursor::new(input),
        Vec::<u8>::new(),
        ResponseLoopback {
            responses: responses_tx,
        },
    )
    .request_byte_budget(first.len());
    let server_task = tokio::spawn({
        let calls = Arc::clone(&calls);
        async move {
            server
                .serve(BudgetedService {
                    calls,
                    first_started: Some(first_started_tx),
                    release_first: Some(release_first_rx),
                    second_called: Some(second_called_tx),
                })
                .await;
        }
    });

    let routed_response = timeout(Duration::from_secs(2), responses_rx.next())
        .await
        .expect("client response should bypass the request byte budget")
        .expect("client response sink should remain open");
    assert_eq!(routed_response.id(), &Id::Number(99));
    assert!(
        timeout(Duration::from_secs(2), second_called_rx)
            .await
            .expect("server should close the second request signal")
            .is_err(),
        "a request beyond the byte budget must not call the service"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    timeout(Duration::from_secs(2), server_task)
        .await
        .expect("server should stop after exhausting the request byte budget")
        .expect("server task should not panic");
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_cancel_and_exit_bypass_a_saturated_main_request_budget() {
    let main = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
    let cancel = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
    let exit = br#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
    assert!(exit.len() <= cancel.len());

    let mut input = frame(main);
    input.extend(frame(cancel));
    input.extend(frame(exit));

    let calls = Arc::new(AtomicUsize::new(0));
    let (release_main, main_release) = oneshot::channel();
    let (release_control, control_release) = oneshot::channel();
    let (exit_seen, exit_observed) = oneshot::channel();
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let stop = timeout(
        Duration::from_secs(2),
        stdio_server(
            Cursor::new(input),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
        )
        .request_byte_budget(main.len())
        .control_request_byte_budget(cancel.len(), cancel.len())
        .serve_inner(ExitReleasesBudgetedService {
            calls: Arc::clone(&calls),
            main_release: Some(main_release),
            control_release: Some(control_release),
            release_main: Some(release_main),
            release_control: Some(release_control),
            exit_seen: Some(exit_seen),
        }),
    )
    .await
    .expect("bounded control messages should remain reachable");

    assert_eq!(stop, TransportStop::ExitNotification);
    exit_observed
        .await
        .expect("the exit notification should reach the service");
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn control_messages_use_reserved_handler_capacity() {
    let mut input = Vec::new();
    for id in 0..ORDINARY_HANDLER_CAPACITY {
        input.extend(frame(
            format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"test/block"}}"#).as_bytes(),
        ));
    }
    input.extend(frame(
        br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":0}}"#,
    ));
    input.extend(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#));

    let calls = Arc::new(AtomicUsize::new(0));
    let (cancel_seen_tx, cancel_seen_rx) = oneshot::channel();
    let (exit_seen_tx, exit_seen_rx) = oneshot::channel();
    let (responses_tx, _responses_rx) = mpsc::unbounded();
    let server = tokio::spawn({
        let calls = Arc::clone(&calls);
        async move {
            serve_stdio(
                Cursor::new(input),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
                ReservedControlService {
                    calls,
                    cancel_seen: Some(cancel_seen_tx),
                    exit_seen: Some(exit_seen_tx),
                },
            )
            .await
        }
    });

    timeout(Duration::from_secs(1), cancel_seen_rx)
        .await
        .expect("cancel should use the reserved handler capacity")
        .expect("cancel service call should signal synchronously");
    timeout(Duration::from_secs(1), exit_seen_rx)
        .await
        .expect("exit should bypass all handler capacity")
        .expect("exit service call should signal synchronously");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        ORDINARY_HANDLER_CAPACITY + 2,
        "ordinary work must leave the reserved control capacity intact"
    );

    timeout(OUTPUT_WRITE_TIMEOUT + Duration::from_secs(1), server)
        .await
        .expect("pending handlers should share the absolute drain deadline")
        .expect("stdio task should not panic");
}

#[tokio::test(flavor = "current_thread")]
async fn pending_ordinary_future_does_not_block_synchronous_cancel_and_exit_admission() {
    let (mut client_input, server_input) = tokio::io::duplex(4096);
    let admission_stage = Arc::new(AtomicUsize::new(0));
    let (ordinary_polled_tx, ordinary_polled_rx) = oneshot::channel();
    let (cancel_admitted_tx, cancel_admitted_rx) = oneshot::channel();
    let (exit_admitted_tx, exit_admitted_rx) = oneshot::channel();
    let (responses_tx, _responses_rx) = mpsc::unbounded();
    let server = tokio::spawn({
        let admission_stage = Arc::clone(&admission_stage);
        async move {
            serve_stdio(
                server_input,
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
                AdmissionProbeService {
                    admission_stage,
                    ordinary_polled: Some(ordinary_polled_tx),
                    cancel_admitted: Some(cancel_admitted_tx),
                    exit_admitted: Some(exit_admitted_tx),
                },
            )
            .await
        }
    });

    client_input
        .write_all(&frame(
            br#"{"jsonrpc":"2.0","id":1,"method":"test/pending","params":null}"#,
        ))
        .await
        .expect("write ordinary request frame");
    client_input
        .flush()
        .await
        .expect("flush ordinary request frame");

    timeout(Duration::from_secs(1), ordinary_polled_rx)
        .await
        .expect("ordinary handler should be polled")
        .expect("ordinary handler poll signal");

    let mut control = frame(br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#);
    control.extend(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#));
    client_input
        .write_all(&control)
        .await
        .expect("write pipelined cancellation and exit frames");
    client_input
        .flush()
        .await
        .expect("flush pipelined control frames");

    timeout(Duration::from_secs(1), cancel_admitted_rx)
        .await
        .expect("cancellation should be admitted while ordinary work is pending")
        .expect("cancellation admission signal");
    timeout(Duration::from_secs(1), exit_admitted_rx)
        .await
        .expect("exit should be admitted while ordinary work is pending")
        .expect("exit admission signal");
    assert_eq!(admission_stage.load(Ordering::SeqCst), 3);

    assert_eq!(
        timeout(Duration::from_secs(1), server)
            .await
            .expect("server should stop after the admitted exit")
            .expect("server task should not panic"),
        StdioTermination::ExitWithoutShutdown
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handler_saturation_fails_before_service_admission() {
    let mut input = Vec::new();
    for id in 0..=ORDINARY_HANDLER_CAPACITY {
        input.extend(frame(
            format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"test/block"}}"#).as_bytes(),
        ));
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let termination = timeout(
        Duration::from_secs(1),
        serve_stdio(
            Cursor::new(input),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
            PendingService {
                calls: Arc::clone(&calls),
            },
        ),
    )
    .await
    .expect("handler saturation should fail closed without waiting for pending work");

    assert_eq!(termination, StdioTermination::InputClosed);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        ORDINARY_HANDLER_CAPACITY,
        "the rejected message must not reach synchronous stdio admission"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn exit_terminates_without_waiting_for_a_pending_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let termination = timeout(
        Duration::from_secs(1),
        serve_stdio(
            Cursor::new(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#)),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
            PendingService {
                calls: Arc::clone(&calls),
            },
        ),
    )
    .await
    .expect("a pending exit handler must not hold transport shutdown open");

    assert_eq!(termination, StdioTermination::ExitWithoutShutdown);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn control_request_byte_budget_is_bounded() {
    let first = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
    let second = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":2}}"#;
    assert_eq!(first.len(), second.len());

    let mut input = frame(first);
    input.extend(frame(second));
    let calls = Arc::new(AtomicUsize::new(0));
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let stop = timeout(
        Duration::from_secs(2),
        stdio_server(
            Cursor::new(input),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
        )
        .control_request_byte_budget(first.len(), first.len())
        .serve_inner(PendingService {
            calls: Arc::clone(&calls),
        }),
    )
    .await
    .expect("exhausting the control budget should fail closed");

    assert_eq!(stop, TransportStop::InputClosed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_cancel_cannot_bypass_the_main_request_budget() {
    let main = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
    let normal_cancel = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
    let oversized_cancel = format!(
        r#"{{"jsonrpc":"2.0","method":"$/cancelRequest","params":{{"id":1,"padding":"{}"}}}}"#,
        "x".repeat(normal_cancel.len())
    );
    assert!(oversized_cancel.len() > normal_cancel.len());

    let mut input = frame(main);
    input.extend(frame(oversized_cancel.as_bytes()));
    let calls = Arc::new(AtomicUsize::new(0));
    let (responses_tx, _responses_rx) = mpsc::unbounded();

    let stop = timeout(
        Duration::from_secs(2),
        stdio_server(
            Cursor::new(input),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
        )
        .request_byte_budget(main.len())
        .control_request_byte_budget(normal_cancel.len(), normal_cancel.len())
        .serve_inner(PendingService {
            calls: Arc::clone(&calls),
        }),
    )
    .await
    .expect("an oversized pseudo-control message should fail closed");

    assert_eq!(stop, TransportStop::InputClosed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn queued_messages_share_one_absolute_output_drain_deadline() {
    let (_drain_tx, mut drain_rx) = watch::channel(Some(Instant::now() + OUTPUT_WRITE_TIMEOUT));
    let (release_first_flush, first_flush_release) = oneshot::channel();
    let (flush_started_tx, mut flush_started_rx) = mpsc::unbounded();
    let writer = SharedDeadlineWrite {
        first_flush_release,
        first_flush_released: false,
        flush_started: flush_started_tx,
        active_flush: None,
        completed_flushes: 0,
    };

    let output = tokio::spawn(async move {
        let mut writer = writer;
        let first =
            OutgoingMessage::Response(Response::from_ok(Id::Number(1), serde_json::Value::Null));
        let second =
            OutgoingMessage::Response(Response::from_ok(Id::Number(2), serde_json::Value::Null));
        let first_result = write_message_bounded(&mut writer, &first, &mut drain_rx).await;
        let second_result = write_message_bounded(&mut writer, &second, &mut drain_rx).await;
        (first_result, second_result)
    });

    assert_eq!(
        flush_started_rx
            .next()
            .await
            .expect("the first message should begin flushing"),
        1
    );
    tokio::time::advance(Duration::from_secs(20)).await;
    release_first_flush
        .send(())
        .expect("the first message should still be draining");
    assert_eq!(
        flush_started_rx
            .next()
            .await
            .expect("the second message should begin flushing"),
        2
    );

    let (first_result, second_result) = timeout(Duration::from_secs(11), output)
        .await
        .expect("the second message must use the remaining shared deadline")
        .expect("output task should not panic");
    assert!(matches!(first_result, Ok(Ok(()))));
    assert!(second_result.is_err());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn queued_client_responses_share_one_absolute_drain_deadline() {
    let (_drain_tx, drain_rx) = watch::channel(Some(Instant::now() + OUTPUT_WRITE_TIMEOUT));
    let (release_first_ready, first_ready_release) = oneshot::channel();
    let (ready_started_tx, mut ready_started_rx) = mpsc::unbounded();
    let completed_sends = Arc::new(AtomicUsize::new(0));
    let sink = SharedDeadlineResponseSink {
        first_ready_release,
        first_ready_released: false,
        ready_started: ready_started_tx,
        active_ready: None,
        completed_sends: Arc::clone(&completed_sends),
    };
    let (mut responses_tx, responses_rx) = mpsc::channel(2);
    responses_tx
        .try_send(Response::from_ok(Id::Number(1), serde_json::Value::Null))
        .expect("queue first client response");
    responses_tx
        .try_send(Response::from_ok(Id::Number(2), serde_json::Value::Null))
        .expect("queue second client response");
    responses_tx.disconnect();
    let (routing_failed_tx, routing_failed_rx) = oneshot::channel();

    let routing = tokio::spawn(route_client_responses(
        responses_rx,
        sink,
        routing_failed_tx,
        drain_rx,
    ));

    assert_eq!(
        ready_started_rx
            .next()
            .await
            .expect("the first response should begin routing"),
        1
    );
    tokio::time::advance(Duration::from_secs(20)).await;
    release_first_ready
        .send(())
        .expect("the first response should still be routing");
    assert_eq!(
        ready_started_rx
            .next()
            .await
            .expect("the second response should begin routing"),
        2
    );

    timeout(Duration::from_secs(11), routing)
        .await
        .expect("the second response must use the remaining shared deadline")
        .expect("response routing task should not panic");
    routing_failed_rx
        .await
        .expect("the shared response routing deadline should report failure");
    assert_eq!(completed_sends.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn client_response_send_error_notifies_waiter_and_stops_routing() {
    let (_drain_tx, drain_rx) = watch::channel(None);
    let send_attempts = Arc::new(AtomicUsize::new(0));
    let sink = FailingResponseSink {
        send_attempts: Arc::clone(&send_attempts),
    };
    let (mut responses_tx, responses_rx) = mpsc::channel(2);
    responses_tx
        .try_send(Response::from_ok(Id::Number(1), serde_json::Value::Null))
        .expect("queue client response that will fail routing");
    responses_tx
        .try_send(Response::from_ok(Id::Number(2), serde_json::Value::Null))
        .expect("queue client response that must not be routed after failure");
    responses_tx.disconnect();
    let (routing_failed_tx, routing_failed_rx) = oneshot::channel();

    let routing = tokio::spawn(route_client_responses(
        responses_rx,
        sink,
        routing_failed_tx,
        drain_rx,
    ));

    routing_failed_rx
        .await
        .expect("response routing failure should notify the transport waiter");
    timeout(Duration::from_secs(1), routing)
        .await
        .expect("response routing task should exit after a sink error")
        .expect("response routing task should not panic");
    assert_eq!(send_attempts.load(Ordering::SeqCst), 1);
}
