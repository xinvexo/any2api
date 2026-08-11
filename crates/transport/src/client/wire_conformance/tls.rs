use std::time::Duration;

use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};

use crate::{
    ReqwestTransportManager,
    api::{TransportManager, TransportProxy},
};

use super::support::{assert_fixture, fixture_request};

const FIXTURE: &str =
    include_str!("../../../testdata/generic-rustls-hyper-v3/tls-client-hello.txt");

#[tokio::test]
async fn tls_client_hello_matches_the_versioned_wire_fixture() {
    let manager = ReqwestTransportManager::default();
    let client_hello = capture_client_hello(&manager).await;

    assert_fixture(&describe_client_hello(&client_hello), FIXTURE);
}

#[tokio::test]
async fn tls_extension_order_is_randomized_by_the_selected_rustls_stack() {
    let manager = ReqwestTransportManager::default();
    let mut observed = std::collections::BTreeSet::new();
    for _ in 0..6 {
        let client_hello = capture_client_hello(&manager).await;
        observed.insert(
            parse_client_hello(&client_hello)
                .extensions
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
        );
    }
    assert!(
        observed.len() > 1,
        "Rustls extension ordering policy changed; review the wire profile"
    );
}

async fn capture_client_hello(manager: &ReqwestTransportManager) -> Vec<u8> {
    let (address, captured) = spawn_client_hello_capture().await;
    let proxy = any2api_domain::ProxyProfile::direct();
    let result = manager
        .execute(
            TransportProxy::new(&proxy, None),
            fixture_request(&format!(
                "https://localhost:{}/v1/responses",
                address.port()
            )),
        )
        .await;
    assert!(
        result.is_err(),
        "raw ClientHello probe does not complete TLS"
    );
    tokio::time::timeout(Duration::from_secs(2), captured)
        .await
        .expect("ClientHello capture timeout")
        .expect("ClientHello capture")
}

async fn spawn_client_hello_capture() -> (std::net::SocketAddr, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ClientHello fixture listener");
    let address = listener.local_addr().expect("ClientHello fixture address");
    let (capture_tx, capture_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("ClientHello connection");
        let mut handshake = Vec::new();
        loop {
            let mut header = [0_u8; 5];
            stream
                .read_exact(&mut header)
                .await
                .expect("TLS record header");
            assert_eq!(header[0], 22, "TLS handshake record");
            let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
            let mut payload = vec![0_u8; length];
            stream
                .read_exact(&mut payload)
                .await
                .expect("TLS record payload");
            handshake.extend_from_slice(&payload);
            if handshake.len() >= 4 {
                let handshake_length = usize::from(handshake[1]) << 16
                    | usize::from(handshake[2]) << 8
                    | usize::from(handshake[3]);
                if handshake.len() >= handshake_length + 4 {
                    assert_eq!(handshake[0], 1, "ClientHello handshake");
                    handshake = handshake[4..handshake_length + 4].to_vec();
                    break;
                }
            }
        }
        capture_tx
            .send(handshake)
            .expect("ClientHello capture receiver");
    });
    (address, capture_rx)
}

fn describe_client_hello(bytes: &[u8]) -> String {
    let parsed = parse_client_hello(bytes);
    let mut extension_set = parsed
        .extensions
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    extension_set.sort_unstable();
    let mut output = format!(
        "legacy_version=0x{:04x}\nsession_id_length={}\ncipher_suites={}\ncompression_methods={}\nextension_order_policy=rustls_order_seed_randomized\nextension_set={}\n",
        parsed.legacy_version,
        parsed.session_id_length,
        hex_u16_list(&parsed.cipher_suites),
        hex_u8_list(parsed.compression),
        hex_u16_list(&extension_set),
    );
    let extension = |id| {
        parsed
            .extensions
            .iter()
            .find_map(|(current, data)| (*current == id).then_some(*data))
            .expect("required ClientHello extension")
    };
    output.push_str(&format!(
        "server_name={}\n",
        parse_server_name(extension(0x0000))
    ));
    output.push_str(&format!(
        "supported_groups={}\n",
        hex_u16_list(&parse_u16_vector(extension(0x000a)))
    ));
    output.push_str(&format!(
        "ec_point_formats={}\n",
        hex_u8_list(parse_u8_vector(extension(0x000b)))
    ));
    output.push_str(&format!(
        "signature_algorithms={}\n",
        hex_u16_list(&parse_u16_vector(extension(0x000d)))
    ));
    output.push_str(&format!(
        "alpn={}\n",
        parse_alpn(extension(0x0010)).join(",")
    ));
    output.push_str(&format!(
        "supported_versions={}\n",
        hex_u16_list(&parse_u16_vector_u8(extension(0x002b)))
    ));
    output.push_str(&format!(
        "key_share_groups={}\n",
        hex_u16_list(&parse_key_share_groups(extension(0x0033)))
    ));
    output
}

fn parse_client_hello(bytes: &[u8]) -> ParsedClientHello<'_> {
    let mut input = Cursor::new(bytes);
    let legacy_version = input.u16();
    input.take(32);
    let session_id = input.vector_u8();
    let cipher_bytes = input.vector_u16();
    assert_eq!(cipher_bytes.len() % 2, 0, "cipher suite width");
    let cipher_suites = cipher_bytes
        .chunks_exact(2)
        .map(|value| u16::from_be_bytes([value[0], value[1]]))
        .collect::<Vec<_>>();
    let compression = input.vector_u8();
    let extension_bytes = input.vector_u16();
    assert!(input.is_empty(), "complete ClientHello body");

    let mut extensions = Vec::new();
    let mut extension_input = Cursor::new(extension_bytes);
    while !extension_input.is_empty() {
        let id = extension_input.u16();
        let data = extension_input.vector_u16();
        extensions.push((id, data));
    }

    ParsedClientHello {
        legacy_version,
        session_id_length: session_id.len(),
        cipher_suites,
        compression,
        extensions,
    }
}

struct ParsedClientHello<'a> {
    legacy_version: u16,
    session_id_length: usize,
    cipher_suites: Vec<u16>,
    compression: &'a [u8],
    extensions: Vec<(u16, &'a [u8])>,
}

fn parse_server_name(data: &[u8]) -> &str {
    let mut input = Cursor::new(data);
    let names = input.vector_u16();
    let mut names = Cursor::new(names);
    assert_eq!(names.u8(), 0, "DNS server name type");
    std::str::from_utf8(names.vector_u16()).expect("ASCII server name")
}

fn parse_u16_vector(data: &[u8]) -> Vec<u16> {
    let mut input = Cursor::new(data);
    parse_u16_values(input.vector_u16())
}

fn parse_u16_vector_u8(data: &[u8]) -> Vec<u16> {
    let mut input = Cursor::new(data);
    parse_u16_values(input.vector_u8())
}

fn parse_u16_values(data: &[u8]) -> Vec<u16> {
    assert_eq!(data.len() % 2, 0, "u16 vector width");
    data.chunks_exact(2)
        .map(|value| u16::from_be_bytes([value[0], value[1]]))
        .collect()
}

fn parse_u8_vector(data: &[u8]) -> &[u8] {
    let mut input = Cursor::new(data);
    input.vector_u8()
}

fn parse_alpn(data: &[u8]) -> Vec<String> {
    let mut input = Cursor::new(data);
    let mut protocols = Cursor::new(input.vector_u16());
    let mut output = Vec::new();
    while !protocols.is_empty() {
        output.push(
            std::str::from_utf8(protocols.vector_u8())
                .expect("ASCII ALPN")
                .to_owned(),
        );
    }
    output
}

fn parse_key_share_groups(data: &[u8]) -> Vec<u16> {
    let mut input = Cursor::new(data);
    let mut shares = Cursor::new(input.vector_u16());
    let mut groups = Vec::new();
    while !shares.is_empty() {
        groups.push(shares.u16());
        shares.vector_u16();
    }
    groups
}

fn hex_u16_list(values: &[u16]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex_u8_list(values: &[u8]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, length: usize) -> &'a [u8] {
        let end = self.offset.checked_add(length).expect("fixture offset");
        let value = self
            .bytes
            .get(self.offset..end)
            .expect("complete fixture field");
        self.offset = end;
        value
    }

    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn u16(&mut self) -> u16 {
        let value = self.take(2);
        u16::from_be_bytes([value[0], value[1]])
    }

    fn vector_u8(&mut self) -> &'a [u8] {
        let length = usize::from(self.u8());
        self.take(length)
    }

    fn vector_u16(&mut self) -> &'a [u8] {
        let length = usize::from(self.u16());
        self.take(length)
    }
}
