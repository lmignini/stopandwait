use rfd::FileDialog;
use std::{
    collections::VecDeque,
    fs,
    path::Path,
    time::{self, Duration},
};
use stopandwait::{Packet, ack::ACK, frame::Frame};

const FULL_PAYLOAD_LENGTH_IN_BYTES: usize = 120;

const FOLDER_PREFIX: &str = "assets/";

fn main() {
    assert!(FULL_PAYLOAD_LENGTH_IN_BYTES % 8 == 0);
    // let input_file_string = FOLDER_PREFIX.to_owned() + "transmitted.m4a";
    // let input_file_path = Path::new(&input_file_string);

    let input_file_path = FileDialog::new()
        .set_directory("~/Downloads")
        .pick_file()
        .expect("Did not pick any file!");
    // Read file extension
    let file_extension = input_file_path.extension();
    // Read payload into a vector of bytes
    let payload_to_transfer = fs::read(&input_file_path).expect("Unable to find file");

    let total_payload_length_in_bytes = payload_to_transfer.len();

    let n_full_frames =
        f64::floor(total_payload_length_in_bytes as f64 / FULL_PAYLOAD_LENGTH_IN_BYTES as f64)
            as usize;

    // Either 0 or 1 small frame at the end
    let small_payload_length = total_payload_length_in_bytes % FULL_PAYLOAD_LENGTH_IN_BYTES;
    let n_small_frames: usize;
    match small_payload_length {
        0 => n_small_frames = 0,
        _ => n_small_frames = 1,
    }

    // Separate into bytes to be sent with full frames and the last bytes to be sent with small frames
    let (full_frames_bytes, small_frames_bytes) =
        payload_to_transfer.split_at(n_full_frames * FULL_PAYLOAD_LENGTH_IN_BYTES);

    assert_eq!(
        full_frames_bytes.len() + small_frames_bytes.len(),
        total_payload_length_in_bytes
    );

    let mut frames_to_be_transmitted: VecDeque<Frame> =
        VecDeque::with_capacity(n_full_frames + n_small_frames);

    for full_payload in full_frames_bytes.chunks(FULL_PAYLOAD_LENGTH_IN_BYTES) {
        frames_to_be_transmitted.push_back(Frame::new(full_payload));
    }
    for small_payload in small_frames_bytes.chunks(small_payload_length) {
        frames_to_be_transmitted.push_back(Frame::new(small_payload));
    }

    let n_frames = frames_to_be_transmitted.len();
    let bit_error_probability = f64::powi(20.0, -2);
    let mut rng = rand::rng();
    let mut total_tries: usize = 0;
    let mut total_time = Duration::ZERO;
    let mut received_frames: Vec<Frame> = Vec::with_capacity(n_frames); // Just for performance, in reality the RX does not know n_frames

    let mut wrong_received_packets: usize = 0;
    while frames_to_be_transmitted.len() > 0 {
        let transmission_start_time = time::Instant::now();
        let mut sent_counter: usize = 1;
        eprintln!(
            "Transmitting frame {}/{}",
            n_frames - frames_to_be_transmitted.len() + 1,
            n_frames
        );
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
            eprintln!(
                "ERROR: {}/{} TX frame != RX frame",
                n_frames - frames_to_be_transmitted.len() + 1,
                n_frames
            );
            wrong_received_packets += 1;
        }
        received_frames.push(received_frame);
        frames_to_be_transmitted.pop_front();

        let transmission_duration = transmission_start_time.elapsed();

        total_tries += sent_counter;
        total_time += transmission_duration;
        // println!("Sent in {} tries\n", sent_counter);
    }

    print!("\n");

    println!(
        "Frames transferred: {} ({} B)",
        n_frames, total_payload_length_in_bytes
    );
    println!("Total transfer time: {} ms", total_time.as_millis());

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
    let mut received_bytes_vec =
        Vec::with_capacity(received_frames.len() * FULL_PAYLOAD_LENGTH_IN_BYTES);
    for received_frame in received_frames {
        received_bytes_vec.append(&mut received_frame.get_original_payload());
    }

    // Write to output file

    let output_file_string = FOLDER_PREFIX.to_owned()
        + "received."
        + file_extension.unwrap_or_default().to_str().expect("Bo");
    let output_file_path = Path::new(&output_file_string);
    fs::write(output_file_path, received_bytes_vec).expect("Failed to write to file");
}
