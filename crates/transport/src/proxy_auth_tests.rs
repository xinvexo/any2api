use std::{str::FromStr, sync::Arc, time::Duration};

use any2api_domain::{
    ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId, RetrySafety,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, Method, StatusCode, Uri};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, ProxyCredentials, TransportManager, TransportProxy, TransportRequest,
    },
    error::{TransportErrorStage, TransportFailureScope},
};

#[tokio::test]
async fn http_proxy_basic_authentication_is_sent_to_the_proxy() {
    let (proxy_address, request) = spawn_http_response(StatusCode::OK, "authenticated").await;
    let manager = ReqwestTransportManager::default();
    let profile = network_proxy(ProxyKind::Http, proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "proxy-password".to_owned());
    let response = manager
        .execute(
            TransportProxy::new(&profile, Some(&credentials)),
            request_to("http://upstream.invalid/v1/test"),
        )
        .await
        .expect("authenticated proxy response");

    assert_eq!(response.status, StatusCode::OK);
    let request = request.await.expect("captured proxy request");
    let authorization = request
        .lines()
        .find(|line| {
            line.to_ascii_lowercase()
                .starts_with("proxy-authorization:")
        })
        .expect("proxy authorization header")
        .split_once(':')
        .expect("proxy authorization separator")
        .1
        .trim();
    assert_eq!(authorization, "Basic cHJveHktdXNlcjpwcm94eS1wYXNzd29yZA==");
    assert!(!format!("{credentials:?}").contains("proxy-password"));
    assert!(!format!("{profile:?}").contains("proxy-password"));
}

#[tokio::test]
async fn rejected_proxy_authentication_never_falls_back_to_direct() {
    let origin = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let (proxy_address, request) =
        spawn_http_response(StatusCode::PROXY_AUTHENTICATION_REQUIRED, "rejected").await;
    let manager = ReqwestTransportManager::default();
    let profile = network_proxy(ProxyKind::Http, proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "wrong-password".to_owned());

    let error = match manager
        .execute(
            TransportProxy::new(&profile, Some(&credentials)),
            request_to(&format!("http://{origin_address}/protected")),
        )
        .await
    {
        Ok(_) => panic!("proxy authentication rejection must not reach the provider classifier"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Proxy);
    assert_eq!(error.retry_safety, RetrySafety::RejectedBeforeExecution);
    assert!(
        !request
            .await
            .expect("captured proxy request")
            .contains("wrong-password")
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), origin.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn socks5_username_password_authentication_is_negotiated() {
    let (proxy_address, target, request) =
        spawn_authenticated_socks5_response("proxy-user", "proxy-password").await;
    let manager = ReqwestTransportManager::default();
    let profile = network_proxy(ProxyKind::Socks5, proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "proxy-password".to_owned());
    let response = manager
        .execute(
            TransportProxy::new(&profile, Some(&credentials)),
            request_to("http://authenticated.invalid/socks"),
        )
        .await
        .expect("authenticated SOCKS response");

    assert_eq!(
        collect_body(response).await,
        Bytes::from_static(b"socks-auth")
    );
    assert_eq!(
        target.await.expect("SOCKS target"),
        "authenticated.invalid:80"
    );
    assert!(
        request
            .await
            .expect("captured SOCKS request")
            .starts_with("GET /socks HTTP/1.1")
    );
}

#[test]
fn proxy_authentication_version_creates_a_new_client_generation() {
    let manager = ReqwestTransportManager::default();
    let original = network_proxy(
        ProxyKind::Http,
        "127.0.0.1:8080".parse().expect("proxy address"),
    )
    .set_authentication("proxy-user")
    .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "first".to_owned());
    let first = manager
        .client_for_proxy(TransportProxy::new(&original, Some(&credentials)))
        .expect("first client");
    let rotated = original
        .set_authentication("proxy-user")
        .expect("rotated authentication metadata");
    let rotated_credentials = ProxyCredentials::new("proxy-user".to_owned(), "second".to_owned());
    let second = manager
        .client_for_proxy(TransportProxy::new(&rotated, Some(&rotated_credentials)))
        .expect("rotated client");
    assert!(!Arc::ptr_eq(&first, &second));
}

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

fn network_proxy(kind: ProxyKind, address: std::net::SocketAddr) -> ProxyProfile {
    let address = ProxyAddress::new(address.ip().to_string(), address.port()).expect("address");
    let draft = ProxyDraft::new("Authenticated proxy", kind, address, true).expect("draft");
    ProxyProfile::create(ProxyProfileId::new(), draft).expect("profile")
}

async fn spawn_http_response(
    status: StatusCode,
    body: &'static str,
) -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP proxy listener");
    let address = listener.local_addr().expect("HTTP proxy address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP proxy connection");
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        let reason = status.canonical_reason().unwrap_or("Unknown");
        let response = format!(
            "HTTP/1.1 {} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            status.as_u16(),
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("HTTP proxy response");
    });
    (address, request_rx)
}

async fn spawn_authenticated_socks5_response(
    username: &'static str,
    password: &'static str,
) -> (
    std::net::SocketAddr,
    oneshot::Receiver<String>,
    oneshot::Receiver<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("SOCKS listener");
    let address = listener.local_addr().expect("SOCKS address");
    let (target_tx, target_rx) = oneshot::channel();
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("SOCKS connection");
        let target = accept_authenticated_socks5(&mut stream, username, password).await;
        target_tx.send(target).ok();
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nsocks-auth",
            )
            .await
            .expect("SOCKS response");
    });
    (address, target_rx, request_rx)
}

async fn accept_authenticated_socks5(
    stream: &mut TcpStream,
    expected_username: &str,
    expected_password: &str,
) -> String {
    let mut greeting = [0_u8; 2];
    stream.read_exact(&mut greeting).await.expect("greeting");
    assert_eq!(greeting[0], 5);
    let mut methods = vec![0_u8; usize::from(greeting[1])];
    stream.read_exact(&mut methods).await.expect("methods");
    assert!(methods.contains(&2));
    stream.write_all(&[5, 2]).await.expect("auth method");
    assert_eq!(stream.read_u8().await.expect("auth version"), 1);
    let username_length = stream.read_u8().await.expect("username length");
    let mut username = vec![0_u8; usize::from(username_length)];
    stream.read_exact(&mut username).await.expect("username");
    let password_length = stream.read_u8().await.expect("password length");
    let mut password = vec![0_u8; usize::from(password_length)];
    stream.read_exact(&mut password).await.expect("password");
    assert_eq!(username, expected_username.as_bytes());
    assert_eq!(password, expected_password.as_bytes());
    stream.write_all(&[1, 0]).await.expect("auth success");

    let mut request = [0_u8; 4];
    stream.read_exact(&mut request).await.expect("request");
    assert_eq!(&request[..3], &[5, 1, 0]);
    let host = match request[3] {
        3 => {
            let length = stream.read_u8().await.expect("domain length");
            let mut domain = vec![0_u8; usize::from(length)];
            stream.read_exact(&mut domain).await.expect("domain");
            String::from_utf8(domain).expect("domain UTF-8")
        }
        other => panic!("unexpected SOCKS address type {other}"),
    };
    let port = stream.read_u16().await.expect("port");
    stream
        .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
        .await
        .expect("connect response");
    format!("{host}:{port}")
}

async fn collect_body(mut response: crate::api::TransportResponse) -> Bytes {
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend_from_slice(&chunk.expect("response body chunk"));
    }
    body.into()
}

async fn read_http_head(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.expect("request read");
        assert!(read > 0, "request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("request UTF-8")
}
