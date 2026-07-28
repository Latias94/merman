use hickory_resolver::TokioResolver;
use hickory_resolver::config::{LookupIpStrategy, ResolverOpts};
use reqwest::Url;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NetworkAuthorization {
    Denied,
    PublicOnly,
    PrivateAllowed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NetworkPolicy {
    pub(crate) authorization: NetworkAuthorization,
    pub(crate) max_redirects: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) per_hop_timeout: Duration,
    pub(crate) workflow_timeout: Duration,
    pub(crate) max_body_bytes: Option<usize>,
    pub(crate) max_body_limit_id: &'static str,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SanitizedEndpoint(String);

impl SanitizedEndpoint {
    pub(crate) fn from_url(url: &Url) -> Result<Self, NetworkError> {
        let host = normalized_host(url).ok_or(NetworkError::MissingHost)?;
        let host = match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(address)) => format!("[{address}]"),
            _ => host.to_owned(),
        };
        let port = url
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default();
        Ok(Self(format!("{}://{host}{port}", url.scheme())))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SanitizedEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AddressScope {
    Public,
    Unspecified,
    ThisNetwork,
    Private,
    Shared,
    Loopback,
    LinkLocal,
    ProtocolAssignment,
    Documentation,
    Benchmarking,
    Deprecated,
    Reserved,
    Broadcast,
    Multicast,
    Ipv4Mapped,
    TranslationLocal,
    DiscardOnly,
    Dummy,
    SixToFour,
    SegmentRouting,
    UniqueLocal,
    SiteLocal,
}

impl fmt::Display for AddressScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Public => "public",
            Self::Unspecified => "unspecified",
            Self::ThisNetwork => "this-network",
            Self::Private => "private",
            Self::Shared => "shared",
            Self::Loopback => "loopback",
            Self::LinkLocal => "link-local",
            Self::ProtocolAssignment => "protocol-assignment",
            Self::Documentation => "documentation",
            Self::Benchmarking => "benchmarking",
            Self::Deprecated => "deprecated",
            Self::Reserved => "reserved",
            Self::Broadcast => "broadcast",
            Self::Multicast => "multicast",
            Self::Ipv4Mapped => "IPv4-mapped",
            Self::TranslationLocal => "local-translation",
            Self::DiscardOnly => "discard-only",
            Self::Dummy => "dummy",
            Self::SixToFour => "6to4",
            Self::SegmentRouting => "segment-routing",
            Self::UniqueLocal => "unique-local",
            Self::SiteLocal => "site-local",
        };
        formatter.write_str(label)
    }
}

impl AddressScope {
    fn is_permitted(self, authorization: NetworkAuthorization) -> bool {
        match authorization {
            NetworkAuthorization::Denied => false,
            NetworkAuthorization::PublicOnly => self == Self::Public,
            NetworkAuthorization::PrivateAllowed => matches!(
                self,
                Self::Public
                    | Self::Private
                    | Self::Shared
                    | Self::Loopback
                    | Self::LinkLocal
                    | Self::TranslationLocal
                    | Self::UniqueLocal
                    | Self::SiteLocal
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum NetworkError {
    #[error("network access is disabled")]
    Denied,
    #[error("invalid HTTP(S) URL")]
    InvalidUrl,
    #[error("network URL scheme `{scheme}` is not supported; expected http or https")]
    UnsupportedScheme { scheme: String },
    #[error("network URL must include a host")]
    MissingHost,
    #[error("network URL credentials are not allowed for {endpoint}")]
    CredentialsNotAllowed { endpoint: SanitizedEndpoint },
    #[error("network URL port 0 is not allowed for {endpoint}")]
    InvalidPort { endpoint: SanitizedEndpoint },
    #[error("localhost destination {endpoint} requires private network authorization")]
    LocalhostDenied { endpoint: SanitizedEndpoint },
    #[error("network request deadline exceeded while accessing {endpoint}")]
    DeadlineExceeded { endpoint: SanitizedEndpoint },
    #[error("network request timeout is too large")]
    TimeoutOverflow,
    #[error("failed to resolve {endpoint}: {source}")]
    Resolve {
        endpoint: SanitizedEndpoint,
        #[source]
        source: io::Error,
    },
    #[error("DNS returned no addresses for {endpoint}")]
    NoAddresses { endpoint: SanitizedEndpoint },
    #[error("network endpoint {endpoint} resolved to forbidden {scope} address {address}")]
    ForbiddenAddress {
        endpoint: SanitizedEndpoint,
        address: IpAddr,
        scope: AddressScope,
    },
    #[error(
        "network endpoint {endpoint} resolved to {scope} address {address}; private network authorization is required"
    )]
    PrivateAuthorizationRequired {
        endpoint: SanitizedEndpoint,
        address: IpAddr,
        scope: AddressScope,
    },
    #[error("HTTP transport setup failed for {endpoint}")]
    TransportSetup { endpoint: SanitizedEndpoint },
    #[error("HTTP connection timed out for {endpoint}")]
    TransportTimeout { endpoint: SanitizedEndpoint },
    #[error("HTTP connection failed for {endpoint}")]
    TransportConnect { endpoint: SanitizedEndpoint },
    #[error("HTTP request failed for {endpoint}")]
    TransportRequest { endpoint: SanitizedEndpoint },
    #[error("HTTP transport for {endpoint} connected to unapproved address {address}")]
    UnapprovedPeer {
        endpoint: SanitizedEndpoint,
        address: SocketAddr,
    },
    #[error("HTTP redirect from {endpoint} did not contain a valid Location header")]
    InvalidRedirectLocation { endpoint: SanitizedEndpoint },
    #[error("HTTP redirect from {endpoint} contains an invalid target URL")]
    InvalidRedirectTarget { endpoint: SanitizedEndpoint },
    #[error("HTTP redirect limit of {max_redirects} exceeded at {endpoint}")]
    TooManyRedirects {
        endpoint: SanitizedEndpoint,
        max_redirects: usize,
    },
    #[error("HTTP redirect loop detected at {endpoint}")]
    RedirectLoop { endpoint: SanitizedEndpoint },
    #[error("HTTP request to {endpoint} returned status {status}")]
    HttpStatus {
        endpoint: SanitizedEndpoint,
        status: u16,
    },
    #[error("HTTP response from {endpoint} exceeds the {max_body_bytes}-byte body limit ({limit})")]
    BodyTooLarge {
        endpoint: SanitizedEndpoint,
        max_body_bytes: usize,
        limit: &'static str,
    },
    #[error("failed to read the HTTP response body from {endpoint}")]
    BodyRead { endpoint: SanitizedEndpoint },
    #[error("failed to allocate memory for the HTTP response body from {endpoint}")]
    BodyAllocation { endpoint: SanitizedEndpoint },
}

impl NetworkError {
    pub(crate) const fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::DeadlineExceeded { .. }
                | Self::Resolve { .. }
                | Self::NoAddresses { .. }
                | Self::TransportSetup { .. }
                | Self::TransportTimeout { .. }
                | Self::TransportConnect { .. }
                | Self::TransportRequest { .. }
                | Self::UnapprovedPeer { .. }
                | Self::InvalidRedirectLocation { .. }
                | Self::InvalidRedirectTarget { .. }
                | Self::TooManyRedirects { .. }
                | Self::RedirectLoop { .. }
                | Self::HttpStatus { .. }
                | Self::BodyRead { .. }
                | Self::BodyAllocation { .. }
        )
    }
}

trait AddressResolver {
    fn resolve(&mut self, host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>>;
}

#[derive(Default)]
struct SystemResolver {
    hickory: Option<HickoryResolver>,
}

impl AddressResolver for SystemResolver {
    fn resolve(&mut self, host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
        if self.hickory.is_none() {
            self.hickory = Some(HickoryResolver::from_system()?);
        }
        self.hickory
            .as_mut()
            .expect("the resolver was initialized above")
            .resolve(host, port, timeout)
    }
}

struct HickoryResolver {
    runtime: tokio::runtime::Runtime,
    resolver: TokioResolver,
}

impl HickoryResolver {
    fn from_system() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let resolver = runtime.block_on(async {
            // builder_tokio propagates system configuration failures. Do not
            // substitute public recursive resolvers when the host is misconfigured.
            let mut builder = TokioResolver::builder_tokio().map_err(|error| {
                resolver_io_error("failed to read the system DNS configuration", error)
            })?;
            configure_resolver_options(builder.options_mut());
            builder
                .build()
                .map_err(|error| resolver_io_error("failed to build the DNS resolver", error))
        })?;

        Ok(Self { runtime, resolver })
    }

    fn resolve(&mut self, host: &str, port: u16, timeout: Duration) -> io::Result<Vec<SocketAddr>> {
        let resolver = &self.resolver;
        // Preserve the spelling from the URL. An explicit trailing dot remains an
        // FQDN, while short, localhost, and .local names retain the operating
        // system's search-domain behavior.
        let lookup = self.runtime.block_on(with_lookup_timeout(timeout, async {
            resolver
                .lookup_ip(host)
                .await
                .map_err(|error| resolver_io_error("DNS lookup failed", error))
        }))?;

        // The lookup and all resolver work complete before the blocking HTTP
        // transport starts. No reqwest operation depends on this Tokio runtime.
        Ok(lookup
            .iter()
            .map(|address| SocketAddr::new(address, port))
            .collect())
    }
}

fn configure_resolver_options(options: &mut ResolverOpts) {
    options.attempts = 0;
    options.cache_size = 0;
    options.max_active_requests = 1;
    options.num_concurrent_reqs = 1;
    options.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
}

async fn with_lookup_timeout<T>(
    timeout: Duration,
    lookup: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    tokio::time::timeout(timeout, lookup)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS lookup timed out"))?
}

fn resolver_io_error(context: &str, error: impl fmt::Display) -> io::Error {
    io::Error::other(format!("{context}: {error}"))
}

struct ApprovedHop<'a> {
    url: &'a Url,
    endpoint: &'a SanitizedEndpoint,
    dns_name: Option<&'a str>,
    addresses: &'a [SocketAddr],
    connect_timeout: Duration,
    timeout: Duration,
}

struct HttpResponse<B> {
    status: u16,
    location: Option<String>,
    content_length: Option<u64>,
    peer_addr: Option<SocketAddr>,
    body: B,
}

trait HttpTransport {
    type Body: Read;

    fn get(&mut self, request: ApprovedHop<'_>) -> Result<HttpResponse<Self::Body>, NetworkError>;
}

struct ReqwestTransport;

impl HttpTransport for ReqwestTransport {
    type Body = reqwest::blocking::Response;

    fn get(&mut self, request: ApprovedHop<'_>) -> Result<HttpResponse<Self::Body>, NetworkError> {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(request.timeout)
            .connect_timeout(request.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .referer(false)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd();
        if let Some(domain) = request.dns_name {
            builder = builder.resolve_to_addrs(domain, request.addresses);
        }

        let client = builder.build().map_err(|_| NetworkError::TransportSetup {
            endpoint: request.endpoint.clone(),
        })?;
        let response = client
            .get(request.url.clone())
            .timeout(request.timeout)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    NetworkError::TransportTimeout {
                        endpoint: request.endpoint.clone(),
                    }
                } else if error.is_connect() {
                    NetworkError::TransportConnect {
                        endpoint: request.endpoint.clone(),
                    }
                } else {
                    NetworkError::TransportRequest {
                        endpoint: request.endpoint.clone(),
                    }
                }
            })?;

        let status = response.status().as_u16();
        let location = if is_redirect_status(status) {
            Some(
                response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .ok_or_else(|| NetworkError::InvalidRedirectLocation {
                        endpoint: request.endpoint.clone(),
                    })?
                    .to_str()
                    .map_err(|_| NetworkError::InvalidRedirectLocation {
                        endpoint: request.endpoint.clone(),
                    })?
                    .to_owned(),
            )
        } else {
            None
        };
        let content_length = response.content_length();
        let peer_addr = response.remote_addr();

        Ok(HttpResponse {
            status,
            location,
            content_length,
            peer_addr,
            body: response,
        })
    }
}

struct FetchGuard {
    deadline: Instant,
    max_redirects: usize,
    redirects: usize,
    visited: HashSet<String>,
}

impl FetchGuard {
    fn new(
        initial_url: &Url,
        timeout: Duration,
        max_redirects: usize,
    ) -> Result<Self, NetworkError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(NetworkError::TimeoutOverflow)?;
        let mut visited = HashSet::new();
        visited.insert(initial_url.as_str().to_owned());
        Ok(Self {
            deadline,
            max_redirects,
            redirects: 0,
            visited,
        })
    }

    fn remaining(&self, endpoint: &SanitizedEndpoint) -> Result<Duration, NetworkError> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero());
        remaining.ok_or_else(|| NetworkError::DeadlineExceeded {
            endpoint: endpoint.clone(),
        })
    }

    fn follow_redirect(
        &mut self,
        target: &Url,
        endpoint: &SanitizedEndpoint,
    ) -> Result<(), NetworkError> {
        if self.redirects >= self.max_redirects {
            return Err(NetworkError::TooManyRedirects {
                endpoint: endpoint.clone(),
                max_redirects: self.max_redirects,
            });
        }
        if !self.visited.insert(target.as_str().to_owned()) {
            return Err(NetworkError::RedirectLoop {
                endpoint: endpoint.clone(),
            });
        }
        self.redirects += 1;
        Ok(())
    }
}

struct HopGuard {
    deadline: Instant,
}

impl HopGuard {
    fn new(
        workflow: &FetchGuard,
        endpoint: &SanitizedEndpoint,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let timeout = timeout.min(workflow.remaining(endpoint)?);
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(NetworkError::TimeoutOverflow)?;
        Ok(Self { deadline })
    }

    fn remaining(
        &self,
        workflow: &FetchGuard,
        endpoint: &SanitizedEndpoint,
    ) -> Result<Duration, NetworkError> {
        let hop_remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| NetworkError::DeadlineExceeded {
                endpoint: endpoint.clone(),
            })?;
        Ok(hop_remaining.min(workflow.remaining(endpoint)?))
    }
}

pub(crate) fn fetch_http_body(
    raw_url: &str,
    policy: NetworkPolicy,
) -> Result<Vec<u8>, NetworkError> {
    let mut resolver = SystemResolver::default();
    let mut transport = ReqwestTransport;
    fetch_http_body_with(raw_url, policy, &mut resolver, &mut transport)
}

fn fetch_http_body_with<R, T>(
    raw_url: &str,
    policy: NetworkPolicy,
    resolver: &mut R,
    transport: &mut T,
) -> Result<Vec<u8>, NetworkError>
where
    R: AddressResolver,
    T: HttpTransport,
{
    if policy.authorization == NetworkAuthorization::Denied {
        return Err(NetworkError::Denied);
    }

    let mut current_url = parse_and_validate_url(raw_url)?;
    let mut guard = FetchGuard::new(&current_url, policy.workflow_timeout, policy.max_redirects)?;

    loop {
        let endpoint = SanitizedEndpoint::from_url(&current_url)?;
        let hop_guard = HopGuard::new(&guard, &endpoint, policy.per_hop_timeout)?;
        let resolve_timeout = hop_guard.remaining(&guard, &endpoint)?;
        let (dns_name, addresses) = resolve_and_authorize(
            &current_url,
            &endpoint,
            policy.authorization,
            resolve_timeout,
            resolver,
        )?;
        let request_timeout = hop_guard.remaining(&guard, &endpoint)?;
        let response = transport.get(ApprovedHop {
            url: &current_url,
            endpoint: &endpoint,
            dns_name: dns_name.as_deref(),
            addresses: &addresses,
            connect_timeout: request_timeout.min(policy.connect_timeout),
            timeout: request_timeout,
        })?;
        hop_guard.remaining(&guard, &endpoint)?;
        validate_peer(&endpoint, &addresses, response.peer_addr)?;

        if is_redirect_status(response.status) {
            let location = response.location.as_deref().ok_or_else(|| {
                NetworkError::InvalidRedirectLocation {
                    endpoint: endpoint.clone(),
                }
            })?;
            let next_url =
                current_url
                    .join(location)
                    .map_err(|_| NetworkError::InvalidRedirectTarget {
                        endpoint: endpoint.clone(),
                    })?;
            let next_url = validate_url(next_url)?;
            let next_endpoint = SanitizedEndpoint::from_url(&next_url)?;
            guard.follow_redirect(&next_url, &next_endpoint)?;
            current_url = next_url;
            continue;
        }

        if !(200..300).contains(&response.status) {
            return Err(NetworkError::HttpStatus {
                endpoint,
                status: response.status,
            });
        }
        if let (Some(length), Some(max_body_bytes)) =
            (response.content_length, policy.max_body_bytes)
            && length > max_body_bytes as u64
        {
            return Err(NetworkError::BodyTooLarge {
                endpoint,
                max_body_bytes,
                limit: policy.max_body_limit_id,
            });
        }
        return read_limited_body(
            response.body,
            &endpoint,
            policy.max_body_bytes,
            policy.max_body_limit_id,
            &guard,
            &hop_guard,
        );
    }
}

fn parse_and_validate_url(raw_url: &str) -> Result<Url, NetworkError> {
    let url = Url::parse(raw_url).map_err(|_| NetworkError::InvalidUrl)?;
    validate_url(url)
}

fn validate_url(mut url: Url) -> Result<Url, NetworkError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(NetworkError::UnsupportedScheme {
            scheme: url.scheme().to_owned(),
        });
    }
    let endpoint = SanitizedEndpoint::from_url(&url)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(NetworkError::CredentialsNotAllowed { endpoint });
    }
    if url.port() == Some(0) {
        return Err(NetworkError::InvalidPort { endpoint });
    }
    url.set_fragment(None);
    Ok(url)
}

fn resolve_and_authorize<R: AddressResolver>(
    url: &Url,
    endpoint: &SanitizedEndpoint,
    authorization: NetworkAuthorization,
    timeout: Duration,
    resolver: &mut R,
) -> Result<(Option<String>, Vec<SocketAddr>), NetworkError> {
    let host = normalized_host(url).ok_or(NetworkError::MissingHost)?;
    let port = url
        .port_or_known_default()
        .expect("validated HTTP(S) URLs always have a default port");

    let (dns_name, mut addresses) = match host.parse::<IpAddr>() {
        Ok(address) => (None, vec![SocketAddr::new(address, port)]),
        Err(_) => {
            if is_localhost_name(host) && authorization == NetworkAuthorization::PublicOnly {
                return Err(NetworkError::LocalhostDenied {
                    endpoint: endpoint.clone(),
                });
            }
            let addresses =
                resolver
                    .resolve(host, port, timeout)
                    .map_err(|source| NetworkError::Resolve {
                        endpoint: endpoint.clone(),
                        source,
                    })?;
            (Some(host.to_owned()), addresses)
        }
    };

    for address in &mut addresses {
        address.set_port(port);
    }
    if addresses.is_empty() {
        return Err(NetworkError::NoAddresses {
            endpoint: endpoint.clone(),
        });
    }

    for address in &addresses {
        let scope = classify_address(address.ip());
        if !scope.is_permitted(authorization) {
            if authorization == NetworkAuthorization::PublicOnly
                && scope.is_permitted(NetworkAuthorization::PrivateAllowed)
            {
                return Err(NetworkError::PrivateAuthorizationRequired {
                    endpoint: endpoint.clone(),
                    address: address.ip(),
                    scope,
                });
            }
            return Err(NetworkError::ForbiddenAddress {
                endpoint: endpoint.clone(),
                address: address.ip(),
                scope,
            });
        }
    }

    Ok((dns_name, addresses))
}

fn validate_peer(
    endpoint: &SanitizedEndpoint,
    approved_addresses: &[SocketAddr],
    peer_addr: Option<SocketAddr>,
) -> Result<(), NetworkError> {
    let Some(peer_addr) = peer_addr else {
        return Ok(());
    };
    if approved_addresses.contains(&peer_addr) {
        return Ok(());
    }
    Err(NetworkError::UnapprovedPeer {
        endpoint: endpoint.clone(),
        address: peer_addr,
    })
}

fn read_limited_body<R: Read>(
    mut reader: R,
    endpoint: &SanitizedEndpoint,
    max_body_bytes: Option<usize>,
    max_body_limit_id: &'static str,
    workflow_guard: &FetchGuard,
    hop_guard: &HopGuard,
) -> Result<Vec<u8>, NetworkError> {
    let mut body = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];

    loop {
        hop_guard.remaining(workflow_guard, endpoint)?;
        let read_length = max_body_bytes
            .map(|limit| {
                let remaining = limit - body.len();
                buffer.len().min(remaining.saturating_add(1))
            })
            .unwrap_or(buffer.len());
        let bytes_read =
            reader
                .read(&mut buffer[..read_length])
                .map_err(|_| NetworkError::BodyRead {
                    endpoint: endpoint.clone(),
                })?;
        hop_guard.remaining(workflow_guard, endpoint)?;
        if bytes_read == 0 {
            return Ok(body);
        }
        if let Some(max_body_bytes) = max_body_bytes
            && bytes_read > max_body_bytes - body.len()
        {
            return Err(NetworkError::BodyTooLarge {
                endpoint: endpoint.clone(),
                max_body_bytes,
                limit: max_body_limit_id,
            });
        }
        body.try_reserve(bytes_read)
            .map_err(|_| NetworkError::BodyAllocation {
                endpoint: endpoint.clone(),
            })?;
        body.extend_from_slice(&buffer[..bytes_read]);
    }
}

fn normalized_host(url: &Url) -> Option<&str> {
    let host = url.host_str()?;
    Some(
        host.strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host),
    )
}

fn is_localhost_name(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    host.eq_ignore_ascii_case("localhost")
        || host
            .to_ascii_lowercase()
            .strip_suffix(".localhost")
            .is_some()
}

fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

// This mirrors the IANA IPv4 and IPv6 special-purpose registries reviewed on
// 2026-07-27. Keep the date visible: global-reachability assignments can change.
// Multicast and deprecated IPv6 site-local space are additionally non-public
// for HTTP acquisition even though they are maintained in separate registries.
fn classify_address(address: IpAddr) -> AddressScope {
    match address {
        IpAddr::V4(address) => classify_ipv4(address),
        IpAddr::V6(address) => classify_ipv6(address),
    }
}

fn classify_ipv4(address: Ipv4Addr) -> AddressScope {
    let [a, b, c, d] = address.octets();

    if address.is_unspecified() {
        AddressScope::Unspecified
    } else if a == 0 {
        AddressScope::ThisNetwork
    } else if a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) {
        AddressScope::Private
    } else if a == 100 && (64..=127).contains(&b) {
        AddressScope::Shared
    } else if a == 127 {
        AddressScope::Loopback
    } else if a == 169 && b == 254 {
        AddressScope::LinkLocal
    } else if a == 192 && b == 0 && c == 0 && !matches!(d, 9 | 10) {
        AddressScope::ProtocolAssignment
    } else if (a == 192 && b == 0 && c == 2)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
    {
        AddressScope::Documentation
    } else if a == 192 && b == 88 && c == 99 {
        AddressScope::Deprecated
    } else if a == 198 && matches!(b, 18 | 19) {
        AddressScope::Benchmarking
    } else if (224..=239).contains(&a) {
        AddressScope::Multicast
    } else if address == Ipv4Addr::BROADCAST {
        AddressScope::Broadcast
    } else if a >= 240 {
        AddressScope::Reserved
    } else {
        AddressScope::Public
    }
}

fn classify_ipv6(address: Ipv6Addr) -> AddressScope {
    let segments = address.segments();
    let value = u128::from_be_bytes(address.octets());

    if address.is_unspecified() {
        AddressScope::Unspecified
    } else if address.is_loopback() {
        AddressScope::Loopback
    } else if matches!(segments, [0, 0, 0, 0, 0, 0xffff, _, _]) {
        AddressScope::Ipv4Mapped
    } else if matches!(segments, [0x64, 0xff9b, 1, _, _, _, _, _]) {
        AddressScope::TranslationLocal
    } else if matches!(segments, [0x100, 0, 0, 0, _, _, _, _]) {
        AddressScope::DiscardOnly
    } else if matches!(segments, [0x100, 0, 0, 1, _, _, _, _]) {
        AddressScope::Dummy
    } else if matches!(segments, [0x2001, 2, 0, _, _, _, _, _]) {
        AddressScope::Benchmarking
    } else if matches!(
        segments,
        [0x2001, second, _, _, _, _, _, _] if (0x10..=0x1f).contains(&second)
    ) {
        AddressScope::Deprecated
    } else if matches!(segments, [0x2001, second, _, _, _, _, _, _] if second < 0x200)
        && !is_public_ietf_protocol_assignment(value, segments)
    {
        AddressScope::ProtocolAssignment
    } else if matches!(segments, [0x2002, _, _, _, _, _, _, _]) {
        AddressScope::SixToFour
    } else if matches!(segments, [0x2001, 0x0db8, _, _, _, _, _, _])
        || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
    {
        AddressScope::Documentation
    } else if segments[0] == 0x5f00 {
        AddressScope::SegmentRouting
    } else if (segments[0] & 0xfe00) == 0xfc00 {
        AddressScope::UniqueLocal
    } else if (segments[0] & 0xffc0) == 0xfe80 {
        AddressScope::LinkLocal
    } else if (segments[0] & 0xffc0) == 0xfec0 {
        AddressScope::SiteLocal
    } else if (segments[0] & 0xff00) == 0xff00 {
        AddressScope::Multicast
    } else {
        AddressScope::Public
    }
}

fn is_public_ietf_protocol_assignment(value: u128, segments: [u16; 8]) -> bool {
    const PCP_ANYCAST: u128 = 0x2001_0001_0000_0000_0000_0000_0000_0001;
    const TURN_ANYCAST: u128 = 0x2001_0001_0000_0000_0000_0000_0000_0002;
    const DNS_SD_ANYCAST: u128 = 0x2001_0001_0000_0000_0000_0000_0000_0003;

    matches!(value, PCP_ANYCAST | TURN_ANYCAST | DNS_SD_ANYCAST)
        || matches!(segments, [0x2001, 3, _, _, _, _, _, _])
        || matches!(segments, [0x2001, 4, 0x112, _, _, _, _, _])
        || matches!(
            segments,
            [0x2001, second, _, _, _, _, _, _] if (0x20..=0x3f).contains(&second)
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::io::Cursor;

    const PUBLIC_V4: &str = "93.184.216.34";

    struct FakeResolver {
        answers: HashMap<String, Vec<SocketAddr>>,
        calls: Vec<String>,
    }

    impl FakeResolver {
        fn new(answers: impl IntoIterator<Item = (&'static str, Vec<SocketAddr>)>) -> Self {
            Self {
                answers: answers
                    .into_iter()
                    .map(|(host, addresses)| (host.to_owned(), addresses))
                    .collect(),
                calls: Vec::new(),
            }
        }
    }

    impl AddressResolver for FakeResolver {
        fn resolve(
            &mut self,
            host: &str,
            _port: u16,
            _timeout: Duration,
        ) -> io::Result<Vec<SocketAddr>> {
            self.calls.push(host.to_owned());
            self.answers
                .get(host)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing fake DNS answer"))
        }
    }

    struct FakeTransport {
        responses: VecDeque<HttpResponse<Cursor<Vec<u8>>>>,
        calls: Vec<String>,
    }

    impl FakeTransport {
        fn new(responses: impl IntoIterator<Item = HttpResponse<Cursor<Vec<u8>>>>) -> Self {
            Self {
                responses: responses.into_iter().collect(),
                calls: Vec::new(),
            }
        }
    }

    impl HttpTransport for FakeTransport {
        type Body = Cursor<Vec<u8>>;

        fn get(
            &mut self,
            request: ApprovedHop<'_>,
        ) -> Result<HttpResponse<Self::Body>, NetworkError> {
            self.calls.push(request.url.as_str().to_owned());
            self.responses
                .pop_front()
                .ok_or_else(|| NetworkError::TransportRequest {
                    endpoint: request.endpoint.clone(),
                })
        }
    }

    fn response(status: u16, location: Option<&str>, body: &[u8]) -> HttpResponse<Cursor<Vec<u8>>> {
        HttpResponse {
            status,
            location: location.map(str::to_owned),
            content_length: Some(body.len() as u64),
            peer_addr: None,
            body: Cursor::new(body.to_vec()),
        }
    }

    fn socket(address: &str) -> SocketAddr {
        SocketAddr::new(address.parse().expect("valid test IP address"), 443)
    }

    fn public_policy(max_redirects: usize, max_body_bytes: usize) -> NetworkPolicy {
        NetworkPolicy {
            authorization: NetworkAuthorization::PublicOnly,
            max_redirects,
            connect_timeout: Duration::from_secs(1),
            per_hop_timeout: Duration::from_secs(2),
            workflow_timeout: Duration::from_secs(5),
            max_body_bytes: Some(max_body_bytes),
            max_body_limit_id: "test_body_bytes",
        }
    }

    #[test]
    fn configures_the_production_resolver_for_bounded_uncached_lookups() {
        let mut options = ResolverOpts::default();

        configure_resolver_options(&mut options);

        assert_eq!(options.attempts, 0);
        assert_eq!(options.cache_size, 0);
        assert_eq!(options.max_active_requests, 1);
        assert_eq!(options.num_concurrent_reqs, 1);
        assert_eq!(options.ip_strategy, LookupIpStrategy::Ipv4AndIpv6);
    }

    #[test]
    fn lookup_timeout_cancels_the_entire_pending_lookup() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime");

        let error = runtime
            .block_on(with_lookup_timeout(
                Duration::ZERO,
                std::future::pending::<io::Result<()>>(),
            ))
            .expect_err("a pending lookup must respect the outer timeout");

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn rejects_credentials_without_echoing_them() {
        let error =
            parse_and_validate_url("https://alice:secret@example.com/icons.json?token=private")
                .expect_err("credentials must be rejected");

        let message = error.to_string();
        assert!(matches!(error, NetworkError::CredentialsNotAllowed { .. }));
        assert_eq!(
            message,
            "network URL credentials are not allowed for https://example.com"
        );
        assert!(!message.contains("alice"));
        assert!(!message.contains("secret"));
        assert!(!message.contains("icons.json"));
        assert!(!message.contains("token"));
    }

    #[test]
    fn sanitized_endpoint_omits_path_query_fragment_and_credentials() {
        let url =
            Url::parse("https://alice:secret@example.com:8443/a/b?token=private#fragment").unwrap();
        let endpoint = SanitizedEndpoint::from_url(&url).unwrap();

        assert_eq!(endpoint.as_str(), "https://example.com:8443");

        let ipv6 = Url::parse("http://[2001:db8::1]:8080/private").unwrap();
        assert_eq!(
            SanitizedEndpoint::from_url(&ipv6).unwrap().as_str(),
            "http://[2001:db8::1]:8080"
        );
    }

    #[test]
    fn classifies_ipv4_special_ranges_from_the_iana_snapshot() {
        let cases = [
            ("93.184.216.34", AddressScope::Public),
            ("0.0.0.0", AddressScope::Unspecified),
            ("0.1.2.3", AddressScope::ThisNetwork),
            ("10.0.0.1", AddressScope::Private),
            ("100.64.0.1", AddressScope::Shared),
            ("127.0.0.1", AddressScope::Loopback),
            ("169.254.1.1", AddressScope::LinkLocal),
            ("172.31.255.255", AddressScope::Private),
            ("192.0.0.8", AddressScope::ProtocolAssignment),
            ("192.0.0.9", AddressScope::Public),
            ("192.0.0.10", AddressScope::Public),
            ("192.0.2.1", AddressScope::Documentation),
            ("192.88.99.1", AddressScope::Deprecated),
            ("192.168.1.1", AddressScope::Private),
            ("198.18.0.1", AddressScope::Benchmarking),
            ("198.51.100.1", AddressScope::Documentation),
            ("203.0.113.1", AddressScope::Documentation),
            ("224.0.0.1", AddressScope::Multicast),
            ("240.0.0.1", AddressScope::Reserved),
            ("255.255.255.255", AddressScope::Broadcast),
        ];

        for (address, expected) in cases {
            assert_eq!(
                classify_address(address.parse().unwrap()),
                expected,
                "{address}"
            );
        }
    }

    #[test]
    fn classifies_ipv6_special_ranges_from_the_iana_snapshot() {
        let cases = [
            ("2606:4700:4700::1111", AddressScope::Public),
            ("::", AddressScope::Unspecified),
            ("::1", AddressScope::Loopback),
            ("::ffff:127.0.0.1", AddressScope::Ipv4Mapped),
            ("64:ff9b::1", AddressScope::Public),
            ("64:ff9b:1::1", AddressScope::TranslationLocal),
            ("100::1", AddressScope::DiscardOnly),
            ("100:0:0:1::1", AddressScope::Dummy),
            ("2001::1", AddressScope::ProtocolAssignment),
            ("2001:1::1", AddressScope::Public),
            ("2001:1::2", AddressScope::Public),
            ("2001:1::3", AddressScope::Public),
            ("2001:2::1", AddressScope::Benchmarking),
            ("2001:2:1::1", AddressScope::ProtocolAssignment),
            ("2001:3::1", AddressScope::Public),
            ("2001:4:112::1", AddressScope::Public),
            ("2001:10::1", AddressScope::Deprecated),
            ("2001:20::1", AddressScope::Public),
            ("2001:30::1", AddressScope::Public),
            ("2001:db8::1", AddressScope::Documentation),
            ("2002::1", AddressScope::SixToFour),
            ("3fff::1", AddressScope::Documentation),
            ("3fff:1000::1", AddressScope::Public),
            ("5f00::1", AddressScope::SegmentRouting),
            ("fc00::1", AddressScope::UniqueLocal),
            ("fe80::1", AddressScope::LinkLocal),
            ("fec0::1", AddressScope::SiteLocal),
            ("ff02::1", AddressScope::Multicast),
        ];

        for (address, expected) in cases {
            assert_eq!(
                classify_address(address.parse().unwrap()),
                expected,
                "{address}"
            );
        }
    }

    #[test]
    fn rejects_public_to_private_redirect_before_the_second_request() {
        let mut resolver = FakeResolver::new([
            ("public.example", vec![socket(PUBLIC_V4)]),
            ("private.example", vec![socket("10.0.0.7")]),
        ]);
        let mut transport = FakeTransport::new([
            response(302, Some("https://private.example/icons.json"), b""),
            response(200, None, br#"{"icons":{}}"#),
        ]);

        let error = fetch_http_body_with(
            "https://public.example/start",
            public_policy(5, 1024),
            &mut resolver,
            &mut transport,
        )
        .expect_err("private redirect target must be rejected");

        assert!(matches!(
            error,
            NetworkError::PrivateAuthorizationRequired {
                scope: AddressScope::Private,
                ..
            }
        ));
        assert_eq!(resolver.calls, ["public.example", "private.example"]);
        assert_eq!(transport.calls, ["https://public.example/start"]);
    }

    #[test]
    fn applies_one_redirect_cap_to_the_whole_request() {
        let mut resolver = FakeResolver::new([
            ("one.example", vec![socket(PUBLIC_V4)]),
            ("two.example", vec![socket(PUBLIC_V4)]),
        ]);
        let mut transport = FakeTransport::new([
            response(301, Some("https://two.example/next"), b""),
            response(308, Some("https://one.example/end"), b""),
        ]);

        let error = fetch_http_body_with(
            "https://one.example/start",
            public_policy(1, 1024),
            &mut resolver,
            &mut transport,
        )
        .expect_err("the second redirect must exceed the shared cap");

        assert!(matches!(
            error,
            NetworkError::TooManyRedirects {
                max_redirects: 1,
                ..
            }
        ));
        assert_eq!(transport.calls.len(), 2);
    }

    #[test]
    fn recognizes_only_fetch_redirect_statuses() {
        for status in [301, 302, 303, 307, 308] {
            assert!(is_redirect_status(status), "{status}");
        }
        for status in [200, 300, 304, 305, 306, 309, 400] {
            assert!(!is_redirect_status(status), "{status}");
        }
    }

    #[test]
    fn detects_a_redirect_loop_before_repeating_transport() {
        let mut resolver = FakeResolver::new([("one.example", vec![socket(PUBLIC_V4)])]);
        let mut transport = FakeTransport::new([response(302, Some("/start"), b"")]);

        let error = fetch_http_body_with(
            "https://one.example/start",
            public_policy(5, 1024),
            &mut resolver,
            &mut transport,
        )
        .expect_err("the initial URL must not be revisited");

        assert!(matches!(error, NetworkError::RedirectLoop { .. }));
        assert_eq!(transport.calls, ["https://one.example/start"]);
    }

    #[test]
    fn resolves_relative_redirects_with_url_join() {
        let mut resolver = FakeResolver::new([("cdn.example", vec![socket(PUBLIC_V4)])]);
        let mut transport = FakeTransport::new([
            response(307, Some("../icons.json?token=kept"), b""),
            response(200, None, br#"{"icons":{}}"#),
        ]);

        let body = fetch_http_body_with(
            "https://cdn.example/packages/v1/start",
            public_policy(3, 1024),
            &mut resolver,
            &mut transport,
        )
        .unwrap();

        assert_eq!(body, br#"{"icons":{}}"#);
        assert_eq!(
            transport.calls,
            [
                "https://cdn.example/packages/v1/start",
                "https://cdn.example/packages/icons.json?token=kept",
            ]
        );
    }

    #[test]
    fn rejects_mixed_public_and_private_dns_before_transport() {
        let mut resolver = FakeResolver::new([(
            "mixed.example",
            vec![socket(PUBLIC_V4), socket("127.0.0.1")],
        )]);
        let mut transport = FakeTransport::new([]);

        let error = fetch_http_body_with(
            "https://mixed.example/icons.json",
            public_policy(3, 1024),
            &mut resolver,
            &mut transport,
        )
        .expect_err("mixed-scope DNS must reject the entire hop");

        assert!(matches!(
            error,
            NetworkError::PrivateAuthorizationRequired {
                scope: AddressScope::Loopback,
                ..
            }
        ));
        assert!(transport.calls.is_empty());
    }

    #[test]
    fn reads_only_one_byte_beyond_the_body_limit() {
        let mut resolver = FakeResolver::new([("cdn.example", vec![socket(PUBLIC_V4)])]);
        let mut oversized = response(200, None, b"12345");
        oversized.content_length = None;
        let mut transport = FakeTransport::new([oversized]);

        let error = fetch_http_body_with(
            "https://cdn.example/icons.json",
            public_policy(0, 4),
            &mut resolver,
            &mut transport,
        )
        .expect_err("streaming limit must reject the fifth byte");

        assert!(matches!(
            error,
            NetworkError::BodyTooLarge {
                max_body_bytes: 4,
                ..
            }
        ));
    }

    #[test]
    fn supports_an_explicitly_unbounded_trusted_body_policy() {
        let mut resolver = FakeResolver::new([("cdn.example", vec![socket(PUBLIC_V4)])]);
        let body = vec![b'x'; 3 * 8 * 1024 + 17];
        let mut response = response(200, None, &body);
        response.content_length = None;
        let mut transport = FakeTransport::new([response]);
        let mut policy = public_policy(0, 1);
        policy.max_body_bytes = None;

        let actual = fetch_http_body_with(
            "https://cdn.example/icons.json",
            policy,
            &mut resolver,
            &mut transport,
        )
        .expect("the trusted unbounded policy should stream the complete body");

        assert_eq!(actual, body);
    }

    #[test]
    fn classifies_network_failures_for_stable_exit_codes() {
        let endpoint = SanitizedEndpoint("https://example.com".to_owned());

        assert!(
            NetworkError::TransportTimeout {
                endpoint: endpoint.clone()
            }
            .is_operational()
        );
        assert!(
            NetworkError::HttpStatus {
                endpoint: endpoint.clone(),
                status: 503,
            }
            .is_operational()
        );
        assert!(
            !NetworkError::ForbiddenAddress {
                endpoint: endpoint.clone(),
                address: "127.0.0.1".parse().unwrap(),
                scope: AddressScope::Loopback,
            }
            .is_operational()
        );
        assert!(
            NetworkError::TooManyRedirects {
                endpoint,
                max_redirects: 3,
            }
            .is_operational()
        );
    }

    #[test]
    fn private_authorization_allows_loopback_but_not_multicast() {
        let policy = NetworkPolicy {
            authorization: NetworkAuthorization::PrivateAllowed,
            max_redirects: 0,
            connect_timeout: Duration::from_secs(1),
            per_hop_timeout: Duration::from_secs(2),
            workflow_timeout: Duration::from_secs(5),
            max_body_bytes: Some(32),
            max_body_limit_id: "test_body_bytes",
        };
        let mut resolver = FakeResolver::new([]);
        let mut transport = FakeTransport::new([response(200, None, br#"{"icons":{}}"#)]);

        let body = fetch_http_body_with(
            "http://127.0.0.1:8080/icons.json",
            policy,
            &mut resolver,
            &mut transport,
        )
        .unwrap();
        assert_eq!(body, br#"{"icons":{}}"#);

        let mut transport = FakeTransport::new([]);
        let error = fetch_http_body_with(
            "http://224.0.0.1/icons.json",
            policy,
            &mut resolver,
            &mut transport,
        )
        .expect_err("multicast is not a private unicast destination");
        assert!(matches!(
            error,
            NetworkError::ForbiddenAddress {
                scope: AddressScope::Multicast,
                ..
            }
        ));
    }
}
