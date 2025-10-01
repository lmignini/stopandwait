use clap::{arg, value_parser};
use core::time;
use std::{
    ffi::OsStr,
    fs,
    net::{Ipv4Addr, UdpSocket},
    path::PathBuf,
    str::FromStr,
};

use clap::Command;
use stopandwait::{
    EOF_MARKER, EOT_MARKER, PAYLOAD_SIZE, RX_PORT, TIMEOUT_DURATION, TX_PORT,
    packets::{Packet, SEQUENCE_ONE, acknowledgement::ack::ACK, frame::Frame},
    parse_len_prefixed_payload,
};

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_target(true)
        .init();
    let matches = Command::new("rx")
        .arg(
            arg!(--ip <VALUE>)
                .default_value("127.0.0.1")
                .value_parser(value_parser!(Ipv4Addr)),
        )
        .get_matches();

    let mut tx_ip_string = matches
        .get_one::<Ipv4Addr>("ip")
        .expect("IP Address is required")
        .to_string();
    tx_ip_string.push(':');
    tx_ip_string.push_str(TX_PORT);

    // let mut rng = rand::rng();
    let socket = UdpSocket::bind(format!("0.0.0.0:{RX_PORT}")).unwrap();

    log::info!("Binding on socket {:?}", socket);
    socket.set_read_timeout(Some(TIMEOUT_DURATION)).unwrap();
    socket.connect(tx_ip_string).unwrap();

    let mut received_data: Vec<u8> = Vec::new();
    let mut buf = [0u8; PAYLOAD_SIZE + 4 + 1];
    let mut waiting_for_checksum: bool = false;
    let mut waiting_for_filename: bool = false;
    let mut received_checksum: u32 = 0;
    let mut received_filename = OsStr::new("received").to_owned();
    let mut n_received_packets: usize = 1;

    loop {
        loop {
            log::debug!("Listening for Frame");
            if socket.recv(&mut buf).is_ok() {
                break;
            }
        }

        log::info!("Received frame {n_received_packets}");

        let frame = Frame::from_bytes(&buf);

        if frame.is_valid() {
            assert!(!(waiting_for_checksum && waiting_for_filename));
            log::debug!("Received frame is valid!");
            let prefixed_payload = frame.get_payload_and_checksum_and_sequence_byte().0;
            let parsed_payload = parse_len_prefixed_payload(&prefixed_payload);
            if waiting_for_checksum == false && waiting_for_filename == false {
                if parsed_payload != EOF_MARKER && parsed_payload != EOT_MARKER {
                    received_data.extend_from_slice(parsed_payload);
                }
            } else if waiting_for_filename {
                log::info!("-- Received Filename frame --");
                received_filename =
                    unsafe { OsStr::from_encoded_bytes_unchecked(parsed_payload).to_owned() };
                waiting_for_filename = false;
            } else if waiting_for_checksum {
                log::info!("-- Received Checksum frame --");
                received_checksum = u32::from_be_bytes(*parsed_payload.first_chunk::<4>().unwrap());
                dbg!(received_checksum);
                waiting_for_checksum = false;
                waiting_for_filename = true;
            }
            n_received_packets += 1;
            match parsed_payload {
                EOT_MARKER => log::info!("-- Received EOT marker --"),
                EOF_MARKER => {
                    log::info!("-- Received EOF marker, waiting for checksum frame --");
                    waiting_for_checksum = true;
                }
                _ => (),
            }

            log::debug!("Sending ACK");
            let ack = ACK::new(SEQUENCE_ONE);
            socket.send(&ack.to_bytes()).unwrap();

            if parsed_payload == EOT_MARKER {
                // Wait for timeout
                socket
                    .set_read_timeout(Some(time::Duration::from_secs(2) + TIMEOUT_DURATION))
                    .unwrap();

                if socket.recv(&mut buf).is_err() {
                    log::info!("TX has stopped transmission, closing RX socket");
                    break;
                }
            }
        } else {
            log::warn!("Received frame is NOT valid! - Doing nothing, waiting for timeout");
        }
    }

    let computed_checksum = crc32fast::hash(&received_data);

    log::info!("Received {} bytes of data", received_data.len());

    // log::info!("{:x?}", received_data);
    assert_eq!(received_checksum, computed_checksum);
    log::info!("Computer checksum: {computed_checksum}");
    const RECEIVED_DIRECTORY_PATH: &'static str = "received/";
    fs::create_dir_all(RECEIVED_DIRECTORY_PATH).expect("Failed to create directory");
    let mut output_file_path = PathBuf::from_str(RECEIVED_DIRECTORY_PATH).unwrap();

    output_file_path.push(received_filename);
    log::info!("Writing to output file: {:?}", output_file_path);
    fs::write(output_file_path, received_data).expect("Failed to write to file");
}
