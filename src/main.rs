use rfd::FileDialog;
use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
    fs,
    ops::Range,
    path::Path,
    str::FromStr,
    time::{self, Duration},
};
use stopandwait::{Packet, ack::ACK, frame::Frame};

const FOLDER_PREFIX: &str = "assets/";

#[derive(Debug)]
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

fn simulate_transfer(
    payload_to_transfer: &Vec<u8>,
    full_payload_length_in_bytes: usize,
    bit_error_probability: f64,
) -> TransferResults {
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

    let n_frames = frames_to_be_transmitted.len();

    eprintln!(
        "Sending {n_frames} frames with a full payload of {} bytes",
        full_payload_length_in_bytes
    );

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
            simulate_transfer(
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
        .max_by(|(k1, r1), (k2, r2)| r1.effective_speed.total_cmp(&r2.effective_speed))
        .expect("I dont know how it could error");

    println!(
        "Best effective speed with {} bytes payload: {} kB/s",
        best_speed.0, best_speed.1.effective_speed
    );
}

#[tokio::main]
async fn main() {
    let input_file_path = FileDialog::new()
        .set_directory("~/Downloads")
        .pick_file()
        .expect("Did not pick any file!");
    // Read file extension
    let file_extension = input_file_path.extension();
    // Read payload into a vector of bytes
    let payload_to_transfer = fs::read(&input_file_path).expect("Unable to find file");

    // Define transfer parameters
    let bit_error_probability = f64::powi(20.0, -2);
    let _bytes_range = 24..=5000;
    let _byte_step = 256;

    let result = simulate_transfer(&payload_to_transfer, 256, bit_error_probability);
    println!("{}", result);
    let output_file_string = FOLDER_PREFIX.to_owned()
        + "received."
        + file_extension.unwrap_or_default().to_str().expect("Bo");
    let output_file_path = Path::new(&output_file_string);

    // Write to output file
    fs::write(output_file_path, result.received_bytes).expect("Failed to write to file");
}
