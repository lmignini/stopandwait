use rfd::FileDialog;
use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs,
    net::{IpAddr, UdpSocket},
    path::PathBuf,
    thread,
    time::Duration,
};
use stopandwait::{
    EOF_MARKER, EOT_MARKER, FILTER_LEVEL, RX_PORT, SOF_MARKER, SOT_MARKER, TX_PORT,
    build_len_prefixed_payload,
    packets::{
        Packet, SEQUENCE_ONE, SEQUENCE_ZERO, acknowledgement::ack::ACK, flip_sequence_byte,
        frame::Frame,
    },
    parse_len_prefixed_payload,
};

use clap::{Command, arg, value_parser}; // Clap v4
use std::net::Ipv4Addr;

const EMPTY_RECEIVED_BYTES: [u8; 10] = [0u8; 10];
struct FileToTransfer {
    path: PathBuf,
    content: Vec<u8>,
}
impl FileToTransfer {
    fn new(file_path: PathBuf) -> std::io::Result<Self> {
        std::io::Result::Ok(Self {
            content: fs::read(&file_path)?,
            path: file_path,
        })
    }

    fn filename_with_extension(&self) -> &OsStr {
        self.path.file_name().unwrap()
    }
}
fn ask_for_input_file_and_return_it() -> std::io::Result<FileToTransfer> {
    let input_file_path = FileDialog::new()
        .set_directory("~/Downloads")
        .pick_file()
        .expect("Did not pick any file!");

    FileToTransfer::new(input_file_path)
}

fn prepare_message(
    file_to_transfer: &FileToTransfer,
    data_size: u16,
    tx_ip_addr: Ipv4Addr,
) -> VecDeque<Frame> {
    let frame_size = data_size + 4 + 1; // 4 bytes checksum and 1 sequence byte
    assert!(data_size % 8 == 0);
    let data_to_transfer = &file_to_transfer.content;
    let total_data_length_in_bytes = data_to_transfer.len();

    let n_full_data_frames =
        f64::floor(total_data_length_in_bytes as f64 / data_size as f64) as usize;

    // Either 0 or 1 small frame at the end
    let small_payload_length = total_data_length_in_bytes % (data_size as usize);
    let n_small_data_frames: usize;
    match small_payload_length {
        0 => n_small_data_frames = 0,
        _ => n_small_data_frames = 1,
    }

    // Separate into bytes to be sent with full frames and the last bytes to be sent with small frames
    let (full_data_bytes, small_data_bytes) =
        data_to_transfer.split_at(n_full_data_frames * (data_size as usize));

    assert_eq!(
        full_data_bytes.len() + small_data_bytes.len(),
        total_data_length_in_bytes
    );

    let mut frames_to_be_transmitted: VecDeque<Frame> =
        VecDeque::with_capacity(n_full_data_frames + n_small_data_frames + 10); // Plus 10 for EOT, Checksum and Extension frames etc
    let mut current_sequence_byte = SEQUENCE_ZERO;

    let mut sot_payload: Vec<u8> = Vec::with_capacity(SOT_MARKER.len() + 4); // 4 bytes for IP
    sot_payload.extend_from_slice(SOT_MARKER);
    sot_payload.extend_from_slice(&tx_ip_addr.to_bits().to_be_bytes());
    let sot_frame = Frame::new(&sot_payload, SEQUENCE_ZERO);

    frames_to_be_transmitted.push_back(sot_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let tx_parameters_frame = Frame::new(&frame_size.to_be_bytes(), SEQUENCE_ONE);

    frames_to_be_transmitted.push_back(tx_parameters_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let sof_frame = Frame::new(SOF_MARKER, SEQUENCE_ZERO);

    frames_to_be_transmitted.push_back(sof_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    for full_payload in full_data_bytes.chunks(data_size as usize) {
        frames_to_be_transmitted.push_back(Frame::new(
            &build_len_prefixed_payload(full_payload, data_size),
            current_sequence_byte,
        ));
        current_sequence_byte = flip_sequence_byte(current_sequence_byte);
    }
    for small_payload in small_data_bytes.chunks(small_payload_length) {
        frames_to_be_transmitted.push_back(Frame::new(
            &build_len_prefixed_payload(small_payload, data_size),
            current_sequence_byte,
        ));
        current_sequence_byte = flip_sequence_byte(current_sequence_byte);
    }

    let eof_frame = Frame::new(
        &build_len_prefixed_payload(EOF_MARKER, data_size),
        current_sequence_byte,
    );

    frames_to_be_transmitted.push_back(eof_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let checksum = crc32fast::hash(data_to_transfer);

    let checksum_frame = Frame::new(
        &build_len_prefixed_payload(&checksum.to_be_bytes(), data_size),
        current_sequence_byte,
    );
    assert_eq!(
        checksum,
        u32::from_be_bytes(
            *parse_len_prefixed_payload(
                &checksum_frame
                    .get_payload_and_checksum_and_sequence_byte()
                    .0,
            )
            .first_chunk::<4>()
            .unwrap()
        ),
    );
    frames_to_be_transmitted.push_back(checksum_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let filename_with_extension = file_to_transfer.filename_with_extension();
    let filename_with_extension_bytes = filename_with_extension.as_encoded_bytes();
    let filename_with_extension_frame = Frame::new(
        &build_len_prefixed_payload(filename_with_extension_bytes, data_size),
        current_sequence_byte,
    );

    frames_to_be_transmitted.push_back(filename_with_extension_frame);
    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let eot_frame = Frame::new(
        &build_len_prefixed_payload(EOT_MARKER, data_size),
        current_sequence_byte,
    );

    frames_to_be_transmitted.push_back(eot_frame);

    let mut even_frames = frames_to_be_transmitted.iter().step_by(2);
    let mut odd_frames = frames_to_be_transmitted.iter().skip(1).step_by(2);
    assert!(
        even_frames
            .all(|frame| frame.get_payload_and_checksum_and_sequence_byte().2 == SEQUENCE_ZERO)
    );
    assert!(
        odd_frames
            .all(|frame| frame.get_payload_and_checksum_and_sequence_byte().2 == SEQUENCE_ONE)
    );

    frames_to_be_transmitted

    /* eprintln!(
        "Sending {n_frames} frames with a full payload of {} bytes",
        full_payload_length_in_bytes
    ); */
}

fn is_received_buf_valid(received: &[u8]) -> bool {
    assert!(received.len() == 10);

    let (payload, received_checksum) = received
        .split_last_chunk::<4>()
        .expect("Asserted len before");

    return crc32fast::hash(payload) == u32::from_be_bytes(*received_checksum);
}

fn main() {
    env_logger::builder()
        .filter_level(FILTER_LEVEL)
        .format_target(true)
        .init();

    let matches = Command::new("tx")
        .args([
            arg!(--data_size <VALUE>)
                .default_value("3840")
                .value_parser(value_parser!(u16)),
            arg!(--timeout_ms <VALUE>)
                .default_value("30")
                .value_parser(value_parser!(u16)),
            arg!(--bep <VALUE>)
                .default_value("0.0")
                .value_parser(value_parser!(f64)),
        ])
        .get_matches();

    let data_size = *matches
        .get_one::<u16>("data_size")
        .expect("Data size is required");

    let timeout_ms = *matches
        .get_one::<u16>("timeout_ms")
        .expect("Timeout duration is required");

    let bep = *matches
        .get_one::<f64>("bep")
        .expect("Bit error probability is required");

    let mut rng = rand::rng();
    let socket = UdpSocket::bind(format!("0.0.0.0:{TX_PORT}")).unwrap();

    // Enable sending to broadcast
    socket
        .set_broadcast(true)
        .expect("set_broadcast call failed");

    log::info!("Binding on socket {:?}", socket);
    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
        .unwrap();

    // Ask for input file
    let file_to_transfer = ask_for_input_file_and_return_it().expect("Unable to read input file");

    log::info!("File size is {} bytes", file_to_transfer.content.len());
    let socket_ip_addr = match socket.local_addr().unwrap().ip() {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => unimplemented!(),
    };
    let mut frames_to_send: VecDeque<Frame> =
        prepare_message(&file_to_transfer, data_size, socket_ip_addr);

    let n_frames = frames_to_send.len();

    let mut current_frame: usize = 1;

    let mut is_socket_connected = false;
    while frames_to_send.len() > 0 {
        let mut ack_buf = EMPTY_RECEIVED_BYTES; // 1 for ACK code, 1 for sequence byte, 4 for IP addr, 2 for crc32
        let frame_to_send = frames_to_send
            .front()
            .unwrap()
            .simulate_errors_with_probability(bep, &mut rng);
        log::info!(
            "Sending frame {current_frame}/{n_frames} with {} bytes",
            frame_to_send.content.len()
        );
        if is_socket_connected {
            socket.send(&frame_to_send.content).unwrap();
        } else {
            socket
                .send_to(
                    &frame_to_send.content,
                    format!("255.255.255.255:{}", RX_PORT),
                )
                .unwrap();
        }
        log::debug!("Listening for ACK");
        // Wait for ACK bytes
        loop {
            let start_time = std::time::Instant::now();
            match socket.recv(&mut ack_buf) {
                Ok(_) => {
                    break; // got ACK
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue; // try again
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    // Connected UDP may surface ICMP "port unreachable" immediately.
                    // Ignore and keep waiting until read timeout actually expires.
                    continue;
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    let elapsed = start_time.elapsed();
                    log::warn!("Did not receive ACK - Timeout in {:?}", elapsed);
                    break; // leaves buffer as zeros -> triggers retransmit below
                }
                Err(e) => {
                    log::error!("recv failed: {e}");
                    break;
                }
            }
        }
        if ack_buf == EMPTY_RECEIVED_BYTES {
            continue;
        }
        log::debug!("Received ACK");
        if is_received_buf_valid(&ack_buf) {
            let ack = ACK::from_bytes(ack_buf[0..2].try_into().expect("Sliced first two bytes"));

            let mut rx_ip_string = Ipv4Addr::from_bits(u32::from_be_bytes(
                ack_buf[2..6].try_into().expect("Sliced 4 bytes for IP"),
            ))
            .to_string();

            rx_ip_string.push(':');
            rx_ip_string.push_str(RX_PORT);
            log::info!("Connecting to RX IP: {}", rx_ip_string);
            socket.connect(rx_ip_string).unwrap(); // Connect to RX ip
            is_socket_connected = true;
            if ack.is_valid() {
                log::debug!("ACK is correct - Moving on to next frame");
                current_frame += 1;
                frames_to_send.pop_front();
                continue;
            } else {
                // Invalid ACK;
                log::warn!("Received invalid ACK - Timing out");
                thread::sleep(Duration::from_millis(timeout_ms as u64));
                continue;
            }
        };
    }
    // log::info!("{:x?}", file_to_transfer.content);
    log::info!(
        "Transmitted checksum: {}",
        crc32fast::hash(&file_to_transfer.content)
    );
    log::info!("Finished transmission, closing TX socket");
}
