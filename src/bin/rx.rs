use clap::{Command, arg, value_parser};
use core::time;
use rand::rngs::ThreadRng;
use std::{
    ffi::OsStr,
    fs,
    net::{IpAddr, Ipv4Addr, UdpSocket},
    path::PathBuf,
    str::FromStr,
};

use stopandwait::{
    EOF_MARKER, EOT_MARKER, FILTER_LEVEL, FRAME_OVERHEAD_LEN, RX_PORT, SOF_MARKER, SOT_MARKER,
    SOT_PAYLOAD_LEN, TX_PARAMETERS_PAYLOAD_LEN, TX_PORT,
    packets::{Packet, SEQUENCE_ONE, SEQUENCE_ZERO, acknowledgement::ack::ACK, frame::Frame},
    parse_len_prefixed_payload,
};
#[derive(PartialEq, Debug)]
enum WaitingFor {
    TransmissionStart,
    TransmissionParameters,
    StartOfFile,
    Frame,
    Checksum,
    Filename,
}

fn send_corrupted_ack(
    socket: &UdpSocket,
    sequence_byte_of_expected_package: u8,
    bep: f64,
    rng: &mut ThreadRng,
) {
    let mut buf_to_send = Vec::with_capacity(10);

    let ack = ACK::new(sequence_byte_of_expected_package);
    let ack_to_send = ack.simulate_errors_with_probability(bep, rng);

    buf_to_send.extend_from_slice(&ack_to_send.to_bytes());
    let socket_ip_addr = match socket.local_addr().unwrap().ip() {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => unimplemented!(),
    };
    buf_to_send.extend_from_slice(&socket_ip_addr.to_bits().to_be_bytes());
    buf_to_send.extend_from_slice(&crc32fast::hash(&buf_to_send[0..6]).to_be_bytes());

    socket.send(&buf_to_send).unwrap();
}

// TODO:
// Funzione per inviare ACK (anche per SOT e TX parameters)
// Fare tutto a lunghezza variabile
fn main() {
    env_logger::builder()
        .filter_level(FILTER_LEVEL)
        .format_target(true)
        .init();

    let matches = Command::new("tx")
        .args([arg!(--bep <VALUE>)
            .default_value("0.0")
            .value_parser(value_parser!(f64))])
        .get_matches();
    let bep = *matches
        .get_one::<f64>("bep")
        .expect("Bit error probability is required");
    let mut waiting_for: WaitingFor = WaitingFor::TransmissionStart;

    let mut rng = rand::rng();
    let socket = UdpSocket::bind(format!("0.0.0.0:{RX_PORT}")).unwrap();

    log::info!("Binding on socket {:?}", socket);

    let mut received_data: Vec<u8> = Vec::new();

    let mut buf_for_sot_frame = [0u8; SOT_PAYLOAD_LEN + FRAME_OVERHEAD_LEN];
    let mut buf_for_tx_parameters = [0u8; TX_PARAMETERS_PAYLOAD_LEN + FRAME_OVERHEAD_LEN];
    let mut buf_for_sof_frame = [0u8; SOF_MARKER.len() + FRAME_OVERHEAD_LEN];
    let mut buf_for_frames = [0u8; u16::MAX as usize + FRAME_OVERHEAD_LEN];

    let mut received_checksum: u32 = 0;
    let mut received_filename = OsStr::new("received").to_owned();
    let mut n_received_packets: usize = 1;
    let mut expected_sequence_byte;

    let mut frame_size: Option<u16> = None;

    let mut tx_ip_addr: Option<Ipv4Addr>;
    while waiting_for != WaitingFor::Frame {
        loop {
            match waiting_for {
                WaitingFor::TransmissionStart => {
                    log::debug!("Waiting for SOT marker");
                }
                WaitingFor::TransmissionParameters => {
                    log::debug!("Waiting for TX parameters frame");
                }
                WaitingFor::StartOfFile => {
                    log::debug!("Waiting for SOF marker");
                }
                _ => panic!(),
            }

            match waiting_for {
                WaitingFor::TransmissionStart => {
                    if socket.recv(&mut buf_for_sot_frame).is_ok() {
                        let received_sot_frame = Frame::from_bytes(&buf_for_sot_frame);
                        let received_payload = received_sot_frame
                            .get_payload_and_checksum_and_sequence_byte()
                            .0;
                        if received_sot_frame.is_valid()
                            && (&received_payload[0..SOT_MARKER.len()]) == SOT_MARKER
                        {
                            log::info!("-- Received SOT marker --");
                            waiting_for = WaitingFor::TransmissionParameters;
                            tx_ip_addr = Some(Ipv4Addr::from_bits(u32::from_be_bytes(
                                received_payload
                                    [received_payload.len() - 4..received_payload.len()]
                                    .try_into()
                                    .expect("Got last 4 bytes from slicing"),
                            )));

                            let mut tx_ip_string: String = tx_ip_addr.unwrap().to_string();
                            tx_ip_string.push(':');
                            tx_ip_string.push_str(TX_PORT);

                            log::info!("Connecting to TX IP: {}", tx_ip_string);
                            socket
                                .connect(tx_ip_string)
                                .expect("Failed to connect to TX");
                            send_corrupted_ack(&socket, SEQUENCE_ONE, bep, &mut rng);
                        } else {
                            log::warn!(
                                "Received a frame but it's not SOT marker or it is invalid, doing nothing..."
                            );
                        }
                        break;
                    }
                }
                WaitingFor::TransmissionParameters => {
                    if socket.recv(&mut buf_for_tx_parameters).is_ok() {
                        log::info!("-- Received TX parameters frame --");

                        let received_tx_parameters_frame =
                            Frame::from_bytes(&buf_for_tx_parameters);
                        let received_payload = received_tx_parameters_frame
                            .get_payload_and_checksum_and_sequence_byte()
                            .0;
                        if received_tx_parameters_frame.is_valid() {
                            frame_size = Some(
                                u16::from_be_bytes(
                                    received_payload[0..2]
                                        .try_into()
                                        .expect("Got the first 2 bytes from slicing"), // Get first 2 bytes
                                ) + 2,
                            );

                            waiting_for = WaitingFor::StartOfFile;
                            send_corrupted_ack(&socket, SEQUENCE_ZERO, bep, &mut rng);
                        } else {
                            log::warn!("TX parameters frame is invalid, doing nothing...");
                        }
                        break;
                    }
                }
                WaitingFor::StartOfFile => {
                    if socket.recv(&mut buf_for_sof_frame).is_ok() {
                        let received_sof_frame = Frame::from_bytes(&buf_for_sof_frame);
                        let received_payload = received_sof_frame
                            .get_payload_and_checksum_and_sequence_byte()
                            .0;
                        if received_sof_frame.is_valid() && received_payload == SOF_MARKER {
                            log::info!("-- Received SOF marker --");
                            waiting_for = WaitingFor::Frame;
                            send_corrupted_ack(&socket, SEQUENCE_ONE, bep, &mut rng);
                        } else {
                            log::warn!(
                                "Received a frame but it's not SOF marker or it is invalid, doing nothing..."
                            );
                        }
                        break;
                    }
                }

                _ => panic!(),
            }
        }
    }

    assert!(frame_size.is_some());
    log::info!("Frame size is {}", frame_size.expect("Asserted is Some"));

    log::debug!("Next frames are file frames");
    loop {
        loop {
            match waiting_for {
                WaitingFor::Frame => log::debug!("Waiting for frame"),
                WaitingFor::Checksum => log::debug!("Waiting for checksum"),
                WaitingFor::Filename => log::debug!("Waiting for filename"),
                _ => panic!("Invalid waiting state: {:?}", waiting_for),
            }

            if socket.recv(&mut buf_for_frames).is_ok() {
                break;
            }
        }

        log::info!("Received frame {n_received_packets}");
        let frame_size = frame_size.expect("Asserted before");
        let frame = Frame::from_bytes(&buf_for_frames[..frame_size as usize]);

        if (n_received_packets) % 2 == 0 {
            expected_sequence_byte = SEQUENCE_ZERO;
        } else {
            expected_sequence_byte = SEQUENCE_ONE;
        }
        if frame.is_valid() {
            log::debug!("Received frame is valid!");

            let (prefixed_payload, _, received_sequence_byte) =
                frame.get_payload_and_checksum_and_sequence_byte();

            if (expected_sequence_byte) == (received_sequence_byte) {
                // Received correctly sequenced frame
                let parsed_payload = parse_len_prefixed_payload(&prefixed_payload);

                match waiting_for {
                    WaitingFor::Frame => {
                        if parsed_payload != EOF_MARKER && parsed_payload != EOT_MARKER {
                            received_data.extend_from_slice(parsed_payload);
                        }
                    }
                    WaitingFor::Checksum => {
                        log::info!("-- Received Checksum frame, waiting for filename frame --");
                        received_checksum =
                            u32::from_be_bytes(*parsed_payload.first_chunk::<4>().unwrap());

                        waiting_for = WaitingFor::Filename
                    }
                    WaitingFor::Filename => {
                        log::info!("-- Received Filename frame --");
                        received_filename = unsafe {
                            OsStr::from_encoded_bytes_unchecked(parsed_payload).to_owned()
                        };
                        waiting_for = WaitingFor::Frame;
                    }
                    _ => todo!(),
                }

                if received_sequence_byte != ((n_received_packets % 2) as u8) { // 
                }
                n_received_packets += 1;
                match parsed_payload {
                    EOT_MARKER => log::info!("-- Received EOT marker --"),
                    EOF_MARKER => {
                        log::info!("-- Received EOF marker, waiting for checksum frame --");
                        waiting_for = WaitingFor::Checksum;
                    }
                    _ => (),
                }

                send_corrupted_ack(&socket, expected_sequence_byte, bep, &mut rng);

                log::debug!("Sending ACK for frame {}", n_received_packets);

                if parsed_payload == EOT_MARKER {
                    // Wait for timeout
                    socket
                        .set_read_timeout(Some(time::Duration::from_secs(1)))
                        .unwrap();

                    if socket.recv(&mut buf_for_frames).is_err() {
                        log::info!("TX has stopped transmission, closing RX socket");
                        break;
                    }
                }
            } else {
                // Received duplicate frame

                log::warn!("Received duplicate frame, discarding it silently...");

                let parsed_payload = parse_len_prefixed_payload(&prefixed_payload);

                if parsed_payload == EOT_MARKER {
                    socket
                        .set_read_timeout(Some(time::Duration::from_secs(1)))
                        .unwrap();

                    if socket.recv(&mut buf_for_frames).is_err() {
                        log::info!("TX has stopped transmission, closing RX socket");
                        break;
                    }
                }
            }
        } else {
            log::warn!("Received frame is NOT valid! - Doing nothing, waiting for timeout");
        }
    }

    let computed_checksum = crc32fast::hash(&received_data);

    log::info!("Received {} bytes of data", received_data.len());

    // log::info!("{:x?}", received_data);
    if received_checksum != computed_checksum {
        log::error!(
            "!! The received checksum is different from the computed one, received data is corrupted !!"
        )
    }

    log::info!(
        "-- Computed checksum: {computed_checksum} Received checksum: {received_checksum} --"
    );
    const RECEIVED_DIRECTORY_PATH: &'static str = "received/";
    fs::create_dir_all(RECEIVED_DIRECTORY_PATH).expect("Failed to create directory");
    let mut output_file_path = PathBuf::from_str(RECEIVED_DIRECTORY_PATH).unwrap();

    output_file_path.push(received_filename);
    log::info!("Writing to output file: {:?}", output_file_path);
    fs::write(output_file_path, received_data).expect("Failed to write to file");
}
