use log;
use rfd::FileDialog;
use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
    fs::{self},
    ops::Range,
    path::{Path, PathBuf},
    sync::mpsc::{self},
    thread,
    time::{self, Duration, Instant},
};
use stopandwait::{Packet, PacketType, ack::ACK, frame::Frame, nack::NACK};
const FOLDER_PREFIX: &str = "assets/";
const FULL_PAYLOAD_LENGTH_IN_BYTES: usize = 480;
#[derive(Debug)]
#[allow(dead_code)]
struct TransferResults {
    // When calculating, does not differentiate between full and small frames
    received_bytes: Vec<u8>,
    transferred_frames: usize,
    transfer_time: f64,   // in ms
    effective_speed: f64, // in kB / s
    average_rtt: f64,     // in ms
    average_tries: f64,
    incorrect_packages: usize,
}

impl Display for TransferResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();
        result.push_str(&format!("Total transfer time: {} ms\n", self.transfer_time));
        result.push_str(&format!(
            "Effective transfer speed: {} kB/s\n",
            self.effective_speed
        ));
        result.push_str(&format!("Average RTT per frame: {} ms\n", self.average_rtt));
        result.push_str(&format!(
            "Average tries per frame: {:.2}\n",
            self.average_tries
        ));
        result.push_str(&format!(
            "Incorrect packages accepted: {}",
            self.incorrect_packages
        ));

        write!(f, "{}", result)
    }
}

fn prepare_message(
    payload_to_transfer: &Vec<u8>,
    full_payload_length_in_bytes: usize,
) -> VecDeque<Frame> {
    assert!(full_payload_length_in_bytes % 8 == 0);

    let total_payload_length_in_bytes = payload_to_transfer.len();

    let n_full_frames =
        f64::floor(total_payload_length_in_bytes as f64 / full_payload_length_in_bytes as f64)
            as usize;

    // Either 0 or 1 small frame at the end
    let small_payload_length = total_payload_length_in_bytes % full_payload_length_in_bytes;
    let n_small_frames: usize;
    match small_payload_length {
        0 => n_small_frames = 0,
        _ => n_small_frames = 1,
    }

    // Separate into bytes to be sent with full frames and the last bytes to be sent with small frames
    let (full_frames_bytes, small_frames_bytes) =
        payload_to_transfer.split_at(n_full_frames * full_payload_length_in_bytes);

    assert_eq!(
        full_frames_bytes.len() + small_frames_bytes.len(),
        total_payload_length_in_bytes
    );

    let mut frames_to_be_transmitted: VecDeque<Frame> =
        VecDeque::with_capacity(n_full_frames + n_small_frames);

    for full_payload in full_frames_bytes.chunks(full_payload_length_in_bytes) {
        frames_to_be_transmitted.push_back(Frame::new(full_payload));
    }
    for small_payload in small_frames_bytes.chunks(small_payload_length) {
        frames_to_be_transmitted.push_back(Frame::new(small_payload));
    }
    frames_to_be_transmitted

    /* eprintln!(
        "Sending {n_frames} frames with a full payload of {} bytes",
        full_payload_length_in_bytes
    ); */
}
fn _simulate_transfer(
    payload_to_transfer: &Vec<u8>,
    full_payload_length_in_bytes: usize,
    bit_error_probability: f64,
) -> TransferResults {
    let total_payload_length_in_bytes = payload_to_transfer.len();
    let mut frames_to_be_transmitted =
        prepare_message(payload_to_transfer, full_payload_length_in_bytes);
    let n_frames = frames_to_be_transmitted.len();
    let mut rng = rand::rng();
    let mut total_tries: usize = 0;
    let mut total_time = Duration::ZERO;
    let mut received_frames: Vec<Frame> = Vec::with_capacity(n_frames); // Just for performance, in reality the RX does not know n_frames

    let mut wrong_received_packets: usize = 0;
    while frames_to_be_transmitted.len() > 0 {
        let transfer_start_time = time::Instant::now();
        let mut sent_counter: usize = 1;
        /* eprintln!(
            "Transmitting frame {}/{}",
            n_frames - frames_to_be_transmitted.len() + 1,
            n_frames
        );*/
        let transmitted_frame = frames_to_be_transmitted
            .get(0)
            .expect("Condition checked in while loop, len > 0");

        // For testing purposes
        assert!(transmitted_frame.is_valid());
        // Simulate transmission line that can mutate the frame
        let mut received_frame =
            transmitted_frame.simulate_errors_with_probability(bit_error_probability, &mut rng);

        // Simulate receiver validating the frame
        while !received_frame.is_valid() {
            received_frame =
                transmitted_frame.simulate_errors_with_probability(bit_error_probability, &mut rng);
            sent_counter += 1;
        }
        /*
        let transmitted_ack = ACK::new();

        // Simulate transmission line that can mutate the ACK
        let received_ack =
            transmitted_ack.simulate_errors_with_probability(bit_error_probability, &mut rng);

        if !received_ack.is_valid() {
            eprintln!("TX received Invalid ACK!");
            continue;
        }

        */

        // Just for knowledge purposes

        if *transmitted_frame != received_frame {
            /* eprintln!(
                "ERROR: {}/{} TX frame != RX frame",
                n_frames - frames_to_be_transmitted.len() + 1,
                n_frames
            ); */
            wrong_received_packets += 1;
        }
        received_frames.push(received_frame);
        frames_to_be_transmitted.pop_front();

        let transfer_duration = transfer_start_time.elapsed();

        total_tries += sent_counter;
        total_time += transfer_duration;
        // println!("Sent in {} tries\n", sent_counter);
    }

    /*
    print!("\n");

    println!(
        "Frames transferred: {} ({} B)",
        n_frames, total_payload_length_in_bytes
    );
    println!(
        "Total transfer time: {} ms",
        total_time.as_secs_f64() * 1000.0
    );

    println!(
        "Effective transfer speed: {} kB/s",
        total_payload_length_in_bytes as f64 / total_time.as_secs_f64() as f64 / 1000.0
    );
    println!(
        "Average RTT per frame: {} ms",
        total_time.as_secs_f64() as f64 / n_frames as f64 * 1000.0
    );
    println!(
        "Average tries per frame: {:.2}",
        total_tries as f64 / n_frames as f64
    );

    println!("Incorrect packages accepted: {}", wrong_received_packets);

    */
    let mut received_bytes_vec =
        Vec::with_capacity(received_frames.len() * full_payload_length_in_bytes);
    for received_frame in received_frames {
        received_bytes_vec.append(&mut received_frame.get_original_payload());
    }
    TransferResults {
        received_bytes: received_bytes_vec,
        transferred_frames: n_frames,
        transfer_time: total_time.as_secs_f64() * 1000.0,
        effective_speed: total_payload_length_in_bytes as f64
            / total_time.as_secs_f64() as f64
            / 1000.0,
        average_rtt: total_time.as_secs_f64() as f64 / n_frames as f64 * 1000.0,
        average_tries: total_tries as f64 / n_frames as f64,
        incorrect_packages: wrong_received_packets,
    }
}

fn _benchmark_payload_lengths(
    payload_to_transfer: &Vec<u8>,
    payload_range: Range<usize>,
    byte_step: usize,
    bit_error_probability: f64,
) {
    let mut results_map = HashMap::with_capacity(payload_range.clone().step_by(byte_step).count());
    for full_payload_length_in_bytes in payload_range.step_by(byte_step) {
        results_map.insert(
            full_payload_length_in_bytes,
            _simulate_transfer(
                &payload_to_transfer,
                full_payload_length_in_bytes,
                bit_error_probability,
            ),
        );
    }
    for (key, value) in &results_map {
        println!("{} bytes payload results:\n", key);
        println!("{}", value);
        println!("\n\n")
    }

    let best_speed = results_map
        .iter()
        .max_by(|(_k1, r1), (_k2, r2)| r1.effective_speed.total_cmp(&r2.effective_speed))
        .expect("I dont know how it could error");

    println!(
        "Best effective speed with {} bytes payload: {} kB/s",
        best_speed.0, best_speed.1.effective_speed
    );
}
#[derive(Clone)]
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
    fn extension(&self) -> String {
        self.path
            .extension()
            .expect("File has no extensions")
            .to_str()
            .unwrap()
            .to_string()
    }
}

fn ask_for_input_file_and_return_it() -> std::io::Result<FileToTransfer> {
    let input_file_path = FileDialog::new()
        .set_directory("~/Downloads")
        .pick_file()
        .expect("Did not pick any file!");

    FileToTransfer::new(input_file_path)
}

fn main() {
    // let (tx_a_to_b, rx_a_to_b) = mpsc::channel();
    // let (tx_b_to_a, rx_b_to_a): (Sender<PacketType>, Receiver<PacketType>) = mpsc::channel();

    const ACK: ACK = ACK::new();
    const NACK: NACK = NACK::new();

    let (tx_a_to_tl, rx_a_to_tl) = mpsc::channel();
    let (tx_tl_to_a, rx_tl_to_a) = mpsc::channel();
    let (tx_b_to_tl, rx_b_to_tl) = mpsc::channel();
    let (tx_tl_to_b, rx_tl_to_b) = mpsc::channel();

    env_logger::init();

    log::info!("Waiting for file input");

    // Ask for input file
    let file_to_transfer = ask_for_input_file_and_return_it().expect("Unable to read input file");
    // Read file extension
    let file_extension = file_to_transfer.extension();

    // TX thread
    let transmitter_thread = thread::spawn(move || {
        let mut frames_to_transmit = prepare_message(
            &file_to_transfer.clone().content,
            FULL_PAYLOAD_LENGTH_IN_BYTES,
        );

        let mut frames_transmitted: usize = 1;
        let total_number_of_frames_to_transmit = frames_to_transmit.len();
        log::info!("Starting transmission");
        while frames_to_transmit.len() > 0 {
            let current_frame: Frame = frames_to_transmit
                .front()
                .expect("Already checked that the deque is not empty in the for loop")
                .clone();
            log::info!(
                "Sending frame {}/{}",
                frames_transmitted,
                total_number_of_frames_to_transmit
            );
            tx_a_to_tl
                .send((current_frame, tx_tl_to_a.clone(), Instant::now()))
                .expect("Channel TX to TL should not be closed");

            let (acknowledgement, sent_time): (PacketType, Instant) = rx_tl_to_a
                .recv()
                .expect("Should get an acknowledgment for every sent frame");
            log::debug!(
                "Received acknowledgement package in {:?}- Starting inspection",
                sent_time.elapsed()
            );

            if acknowledgement.is_valid() {
                match acknowledgement {
                    PacketType::ACK(_) => {
                        log::debug!("Packet is a valid ACK - Moving on to next package");
                        frames_transmitted += 1;
                        frames_to_transmit.pop_front();
                    }
                    PacketType::NACK(_) => {
                        log::debug!("Packet is a valid NACK - Retrying same package");
                        continue;
                    }

                    _ => panic!("Should not be a Frame here"),
                }
            } else {
                log::debug!("Acknowledgement package is invalid - Retrying same package");
                continue;
            }
        }
        log::info!("Finished transmission");
    });

    // Define transfer parameters
    let bit_error_probability = f64::powi(10.0, -4);

    // TL thread
    let transmission_line_thread = thread::spawn(move || {
        let mut rng = rand::rng();
        let mut corrupted_packets_delivered_counter: usize = 0;
        loop {
            let (transmitted_frame, send_back_to_a_tx, send_instant) = match rx_a_to_tl.recv() {
                Ok(received) => received,
                Err(_) => {
                    // Both TX and RX don't know this
                    log::info!(
                        "Number of corrupted packages accepted by RX: {}",
                        corrupted_packets_delivered_counter
                    );
                    break;
                    // Write to output file
                }
            };

            let corrupted_frame =
                transmitted_frame.simulate_errors_with_probability(bit_error_probability, &mut rng);

            // Send corrupted packet to B
            tx_tl_to_b
                .send((corrupted_frame.clone(), tx_b_to_tl.clone(), send_instant))
                .expect("Receiving channel should not be closed");

            // Receive acknowledgement packet
            let (transmitted_acknowledge, sent_time): (PacketType, Instant) =
                rx_b_to_tl.recv().unwrap();

            let corrupted_acknowledge = transmitted_acknowledge
                .simulate_errors_with_probability(bit_error_probability, &mut rng);
            if transmitted_frame != corrupted_frame && corrupted_acknowledge == PacketType::ACK(ACK)
            {
                corrupted_packets_delivered_counter += 1;
            }
            send_back_to_a_tx
                .send((corrupted_acknowledge, sent_time))
                .expect("Channel from TL to TX should not be closed");
        }
    });

    // RX thread
    let receiver_thread = thread::spawn(move || {
        let mut received_frame_count: usize = 0;
        let mut received_frames: Vec<Frame> = Vec::with_capacity(2 ^ 20);
        loop {
            let (received_frame, reply_tx, send_instant) = match rx_tl_to_b.recv() {
                Ok(received) => received,
                Err(_) => {
                    log::info!(
                        "Tx has closed channel, stopped receiving and closing Rx channel as well"
                    );
                    let mut received_bytes_vec =
                        Vec::with_capacity(received_frames.len() * FULL_PAYLOAD_LENGTH_IN_BYTES);
                    for received_frame in received_frames {
                        received_bytes_vec.append(&mut received_frame.get_original_payload());
                    }

                    let output_file_string =
                        FOLDER_PREFIX.to_owned() + "received." + &file_extension;
                    let output_file_path = Path::new(&output_file_string);

                    log::info!("Writing to output file");
                    fs::write(output_file_path, received_bytes_vec)
                        .expect("Failed to write to file");
                    break;
                    // Write to output file
                }
            };

            log::debug!(
                "Received frame in {:?} - Starting processing",
                send_instant.elapsed()
            );
            let start_processing_time = Instant::now();
            let is_received_frame_valid = received_frame.is_valid();
            log::debug!(
                "Received frame - finished processing, took {:?}",
                start_processing_time.elapsed()
            );

            if is_received_frame_valid {
                received_frame_count += 1;
                received_frames.push(received_frame);
                log::debug!("Sending ACK for packet {}", received_frame_count);
                reply_tx
                    .send((PacketType::ACK(ACK), Instant::now()))
                    .unwrap();
            } else {
                log::debug!("Sending NACK for packet {}", received_frame_count);
                reply_tx
                    .send((PacketType::NACK(NACK), Instant::now()))
                    .unwrap();
            }
        }
    });

    let cleaning_thread = std::thread::spawn(move || {
        transmitter_thread.join().unwrap();
        log::info!("Finished transmitting");

        receiver_thread.join().unwrap();
        log::info!("Finished receiving");

        transmission_line_thread.join().unwrap();
    });

    cleaning_thread.join().unwrap();
}
