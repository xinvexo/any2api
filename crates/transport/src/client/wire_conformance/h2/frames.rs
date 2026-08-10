const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const FRAME_HEADER_BYTES: usize = 9;

pub(super) const HEADERS: u8 = 0x1;
pub(super) const PING: u8 = 0x6;
pub(super) const GOAWAY: u8 = 0x7;

#[derive(Clone, Copy)]
struct Frame<'a> {
    frame_type: u8,
    flags: u8,
    stream_id: u32,
    payload: &'a [u8],
}

pub(super) fn describe_initial_frames(bytes: &[u8]) -> String {
    let frames = client_frames(bytes, true);
    let mut output = String::from("preface=PRI * HTTP/2.0\\r\\n\\r\\nSM\\r\\n\\r\\n\n");
    for frame in frames {
        if frame.frame_type == HEADERS {
            output.push_str(&format!(
                "next=HEADERS flags=0x{:02x} stream={} length={}\n",
                frame.flags,
                frame.stream_id,
                frame.payload.len()
            ));
            break;
        }
        output.push_str(&format!(
            "frame={} flags=0x{:02x} stream={} length={}\n",
            frame_name(frame.frame_type),
            frame.flags,
            frame.stream_id,
            frame.payload.len()
        ));
        match frame.frame_type {
            0x4 => describe_settings(frame.payload, &mut output),
            0x8 if frame.payload.len() == 4 => {
                let increment =
                    u32::from_be_bytes(frame.payload.try_into().expect("window update"))
                        & 0x7fff_ffff;
                output.push_str(&format!("  increment={increment}\n"));
            }
            _ => {}
        }
    }
    output
}

pub(super) fn describe_client_lifecycle(bytes: &[u8]) -> String {
    let frames = client_frames(bytes, true);
    let mut output = String::from("client.preface=1\n");
    let mut described = 0;
    for frame in frames {
        if !matches!(frame.frame_type, 0x0 | 0x1 | 0x3 | 0x6 | 0x7 | 0x9) {
            continue;
        }
        described += 1;
        output.push_str(&format!(
            "client.frame={} flags=0x{:02x} stream={} length={}{}\n",
            frame_name(frame.frame_type),
            frame.flags,
            frame.stream_id,
            frame.payload.len(),
            goaway_detail(frame)
        ));
    }
    if described == 0 {
        output.push_str("client.frame=none\n");
    }
    output
}

pub(super) fn describe_server_control(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut described = 0;
    for frame in frames(bytes, 0, true) {
        if !matches!(frame.frame_type, PING | GOAWAY) {
            continue;
        }
        described += 1;
        output.push_str(&format!(
            "server.frame={} flags=0x{:02x} stream={} length={}{}\n",
            frame_name(frame.frame_type),
            frame.flags,
            frame.stream_id,
            frame.payload.len(),
            goaway_detail(frame)
        ));
    }
    if described == 0 {
        output.push_str("server.frame=none\n");
    }
    output
}

pub(super) fn contains_server_frame(bytes: &[u8], expected_type: u8) -> bool {
    frames(bytes, 0, false)
        .into_iter()
        .any(|frame| frame.frame_type == expected_type)
}

fn client_frames(bytes: &[u8], require_complete: bool) -> Vec<Frame<'_>> {
    assert!(bytes.starts_with(PREFACE), "HTTP/2 client preface");
    frames(bytes, PREFACE.len(), require_complete)
}

fn frames(bytes: &[u8], mut offset: usize, require_complete: bool) -> Vec<Frame<'_>> {
    let mut parsed = Vec::new();
    while offset + FRAME_HEADER_BYTES <= bytes.len() {
        let length = usize::from(bytes[offset]) << 16
            | usize::from(bytes[offset + 1]) << 8
            | usize::from(bytes[offset + 2]);
        let payload_start = offset + FRAME_HEADER_BYTES;
        let payload_end = payload_start + length;
        if payload_end > bytes.len() {
            assert!(!require_complete, "complete HTTP/2 frame");
            break;
        }
        parsed.push(Frame {
            frame_type: bytes[offset + 3],
            flags: bytes[offset + 4],
            stream_id: u32::from_be_bytes([
                bytes[offset + 5] & 0x7f,
                bytes[offset + 6],
                bytes[offset + 7],
                bytes[offset + 8],
            ]),
            payload: &bytes[payload_start..payload_end],
        });
        offset = payload_end;
    }
    if require_complete {
        assert_eq!(offset, bytes.len(), "complete HTTP/2 frame header");
    }
    parsed
}

fn goaway_detail(frame: Frame<'_>) -> String {
    if frame.frame_type != GOAWAY {
        return String::new();
    }
    assert!(frame.payload.len() >= 8, "GOAWAY payload");
    let last_stream = u32::from_be_bytes(
        frame.payload[..4]
            .try_into()
            .expect("GOAWAY last stream ID"),
    ) & 0x7fff_ffff;
    let error = u32::from_be_bytes(frame.payload[4..8].try_into().expect("GOAWAY error code"));
    format!(" last_stream={last_stream} error={error}")
}

fn describe_settings(payload: &[u8], output: &mut String) {
    assert_eq!(payload.len() % 6, 0, "SETTINGS entry size");
    for setting in payload.chunks_exact(6) {
        let id = u16::from_be_bytes([setting[0], setting[1]]);
        let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
        output.push_str(&format!("  {}={value}\n", setting_name(id)));
    }
}

const fn frame_name(frame_type: u8) -> &'static str {
    match frame_type {
        0x0 => "DATA",
        0x1 => "HEADERS",
        0x2 => "PRIORITY",
        0x3 => "RST_STREAM",
        0x4 => "SETTINGS",
        0x6 => "PING",
        0x7 => "GOAWAY",
        0x8 => "WINDOW_UPDATE",
        0x9 => "CONTINUATION",
        _ => "UNKNOWN",
    }
}

const fn setting_name(id: u16) -> &'static str {
    match id {
        0x1 => "header_table_size",
        0x2 => "enable_push",
        0x3 => "max_concurrent_streams",
        0x4 => "initial_window_size",
        0x5 => "max_frame_size",
        0x6 => "max_header_list_size",
        _ => "unknown_setting",
    }
}
