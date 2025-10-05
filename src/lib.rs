use log::LevelFilter;

pub mod packets;

// Free ports
pub const RX_PORT: &str = "29170";
pub const TX_PORT: &str = "29172";

const LEN_BYTES: usize = 2; // For safety measures, in case I need to add more bytes

pub const SOT_PAYLOAD_LEN: usize = SOT_MARKER.len() + 4; // 4 bytes for IP address of TX
pub const FRAME_OVERHEAD_LEN: usize = 4 + 1; // 4 byte checksum and 1 sequence byte
pub const TX_PARAMETERS_PAYLOAD_LEN: usize = 2;

// Start/End of file markers
pub const SOF_MARKER: &[u8] = b"__SOF__";
pub const EOF_MARKER: &[u8] = b"__EOF__";

// Start/End of transmission markers
pub const SOT_MARKER: &[u8] = b"__SOT__";
pub const EOT_MARKER: &[u8] = b"__EOT__";

pub const FILTER_LEVEL: LevelFilter = log::LevelFilter::Info;

// Length-prefixed payload builder (u16 BE length + data + zero padding)
pub fn build_len_prefixed_payload(data: &[u8], size: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(size as usize + 2);
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_be_bytes());
    // Copy rest of data to payload
    out.extend_from_slice(data);

    while out.len() != ((size as usize) + 2) {
        out.push(0);
    }

    assert!(out.len() == size as usize + 2);
    out
}

// Parse exact used bytes from a length-prefixed payload
pub fn parse_len_prefixed_payload(buf: &[u8]) -> &[u8] {
    assert!(buf.len() >= LEN_BYTES, "Payload too small");
    let len = u16::from_be_bytes([buf[0], buf[1]]) as usize;
    let end = LEN_BYTES + len;

    assert!(end <= buf.len(), "Invalid length in frame payload");
    &buf[2..end]
}
