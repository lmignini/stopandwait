use rfd::FileDialog;
use std::{collections::VecDeque, ffi::OsStr, fs, net::UdpSocket, path::PathBuf, thread};
use stopandwait::{
    BIT_ERROR_PROBABILITY, EOF_MARKER, EOT_MARKER, FILTER_LEVEL, MAX_DATA_SIZE, PAYLOAD_SIZE,
    RX_PORT, TIMEOUT_DURATION, TX_PORT, build_len_prefixed_payload,
    packets::{
        Packet, SEQUENCE_ONE, SEQUENCE_ZERO, acknowledgement::ack::ACK, flip_sequence_byte,
        frame::Frame,
    },
    parse_len_prefixed_payload,
};

use clap::{Command, arg, value_parser}; // Clap v4
use std::net::Ipv4Addr;

const EMPTY_RECEIVED_BYTES: [u8; 2] = [0, 0];

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

fn prepare_message(file_to_transfer: &FileToTransfer) -> VecDeque<Frame> {
    assert!(PAYLOAD_SIZE % 8 == 0);
    let data_to_transfer = &file_to_transfer.content;
    let total_data_length_in_bytes = data_to_transfer.len();

    let n_full_data_frames =
        f64::floor(total_data_length_in_bytes as f64 / MAX_DATA_SIZE as f64) as usize;

    // Either 0 or 1 small frame at the end
    let small_payload_length = total_data_length_in_bytes % MAX_DATA_SIZE;
    let n_small_data_frames: usize;
    match small_payload_length {
        0 => n_small_data_frames = 0,
        _ => n_small_data_frames = 1,
    }

    // Separate into bytes to be sent with full frames and the last bytes to be sent with small frames
    let (full_data_bytes, small_data_bytes) =
        data_to_transfer.split_at(n_full_data_frames * MAX_DATA_SIZE);

    assert_eq!(
        full_data_bytes.len() + small_data_bytes.len(),
        total_data_length_in_bytes
    );

    let mut frames_to_be_transmitted: VecDeque<Frame> =
        VecDeque::with_capacity(n_full_data_frames + n_small_data_frames + 3); // Plus 3 for EOT, Checksum and Extension frames
    let mut current_sequence_byte = SEQUENCE_ZERO;
    for full_payload in full_data_bytes.chunks(MAX_DATA_SIZE) {
        frames_to_be_transmitted.push_back(Frame::new(
            &build_len_prefixed_payload(full_payload),
            current_sequence_byte,
        ));
        current_sequence_byte = flip_sequence_byte(current_sequence_byte);
    }
    for small_payload in small_data_bytes.chunks(small_payload_length) {
        frames_to_be_transmitted.push_back(Frame::new(
            &build_len_prefixed_payload(small_payload),
            current_sequence_byte,
        ));
        current_sequence_byte = flip_sequence_byte(current_sequence_byte);
    }

    let eof_frame = Frame::new(
        &build_len_prefixed_payload(EOF_MARKER),
        current_sequence_byte,
    );

    frames_to_be_transmitted.push_back(eof_frame);

    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let checksum = crc32fast::hash(data_to_transfer);

    let checksum_frame = Frame::new(
        &build_len_prefixed_payload(&checksum.to_be_bytes()),
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
        &build_len_prefixed_payload(filename_with_extension_bytes),
        current_sequence_byte,
    );

    frames_to_be_transmitted.push_back(filename_with_extension_frame);
    // Flip sequence byte
    current_sequence_byte = flip_sequence_byte(current_sequence_byte);

    let eot_frame = Frame::new(
        &build_len_prefixed_payload(EOT_MARKER),
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
fn main() {
    env_logger::builder()
        .filter_level(FILTER_LEVEL)
        .format_target(true)
        .init();

    let matches = Command::new("tx")
        .arg(
            arg!(--ip <VALUE>)
                .default_value("127.0.0.1")
                .value_parser(value_parser!(Ipv4Addr)),
        )
        .get_matches();

    let mut rx_ip_string = matches
        .get_one::<Ipv4Addr>("ip")
        .expect("IP Address is required")
        .to_string();

    rx_ip_string.push(':');
    rx_ip_string.push_str(RX_PORT);

    let mut rng = rand::rng();
    let socket = UdpSocket::bind(format!("0.0.0.0:{TX_PORT}")).unwrap();

    log::info!("Binding on socket {:?}", socket);
    socket.set_read_timeout(Some(TIMEOUT_DURATION)).unwrap();
    socket.connect(rx_ip_string).unwrap();

    // Ask for input file
    let file_to_transfer = ask_for_input_file_and_return_it().expect("Unable to read input file");

    log::info!("File size is {} bytes", file_to_transfer.content.len());

    let mut frames_to_send: VecDeque<Frame> = prepare_message(&file_to_transfer);

    let n_frames = frames_to_send.len();

    let mut current_frame: usize = 1;
    while frames_to_send.len() > 0 {
        let mut received_ack_bytes = [0; 2];
        let frame_to_send = frames_to_send
            .front()
            .unwrap()
            .simulate_errors_with_probability(BIT_ERROR_PROBABILITY, &mut rng);
        log::info!(
            "Sending frame {current_frame}/{n_frames} with {} bytes",
            frame_to_send.content.len()
        );
        socket.send(&frame_to_send.content).unwrap();

        log::debug!("Listening for ACK");
        // Wait for ACK bytes
        loop {
            let start_time = std::time::Instant::now();
            match socket.recv(&mut received_ack_bytes) {
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
        if received_ack_bytes == EMPTY_RECEIVED_BYTES {
            continue;
        }
        log::debug!("Received ACK");

        let ack = ACK::from_bytes(received_ack_bytes);

        if ack.is_valid() {
            log::debug!("ACK is correct - Moving on to next frame");
            current_frame += 1;
            frames_to_send.pop_front();
            continue;
        } else {
            // Invalid ACK;
            log::warn!("Received invalid ACK - Timing out");
            thread::sleep(TIMEOUT_DURATION);
            continue;
        }
    }
    // log::info!("{:x?}", file_to_transfer.content);
    log::info!(
        "Transmitted checksum: {}",
        crc32fast::hash(&file_to_transfer.content)
    );
    log::info!("Finished transmission, closing TX socket");
}
