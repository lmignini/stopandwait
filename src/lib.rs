pub mod packets;

pub const RX_PORT: &str = "5000";
pub const TX_PORT: &str = "4000";
pub const PAYLOAD_SIZE: usize = 240;
const LEN_BYTES: usize = 2;

pub const MAX_DATA_SIZE: usize = PAYLOAD_SIZE - LEN_BYTES;
pub const BIT_ERROR_PROBABILITY: f64 = 0.000001;
pub const EOF_MARKER: &[u8] = b"__EOF__";
pub const EOT_MARKER: &[u8] = b"__EOT__";

pub const TIMEOUT_DURATION: std::time::Duration = std::time::Duration::from_millis(500);

/*
pub fn extend_payload_to_fixed_size(payload: &[u8]) -> [u8; PAYLOAD_SIZE] {
    let mut extended_payload: [u8; PAYLOAD_SIZE] = [0u8; PAYLOAD_SIZE];
    if payload.len() <= PAYLOAD_SIZE {
        for (i, byte) in payload.iter().enumerate() {
            extended_payload[i] = *byte;
        }
    } else {
        panic!("Payload is too big!");
    }
    extended_payload
}
*/
// Length-prefixed payload builder (u16 BE length + data + zero padding)
pub fn build_len_prefixed_payload(data: &[u8]) -> [u8; PAYLOAD_SIZE] {
    assert!(
        data.len() <= MAX_DATA_SIZE,
        "Dats is too big for length-prefixed payload"
    );
    let mut out = [0u8; PAYLOAD_SIZE];
    let len = data.len() as u16;
    out[0..2].copy_from_slice(&len.to_be_bytes());
    // Copy rest of data to payload
    out[2..2 + data.len()].copy_from_slice(data);
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
