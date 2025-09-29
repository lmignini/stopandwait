pub mod acknowledgement;
pub mod frame;

const ACK_VALUE: u8 = 0b0000_1100;

const NACK_VALUE: u8 = 0b1111_0011;

type SequenceByte = u8;
pub const SEQUENCE_ZERO: SequenceByte = 0b0000_0000;
pub const SEQUENCE_ONE: SequenceByte = 0b1111_1111;

pub fn correct_sequence_byte(sequence_byte: SequenceByte) -> SequenceByte {
    if sequence_byte.count_ones() > sequence_byte.count_zeros() {
        return SEQUENCE_ONE;
    } else if sequence_byte.count_zeros() > sequence_byte.count_ones() {
        return SEQUENCE_ZERO;
    } else {
        log::info!("Ambiguous sequence byte detected, defaulting to ONE");
        return SEQUENCE_ONE;
    }
}
pub fn flip_sequence_byte(sequence_byte: SequenceByte) -> SequenceByte {
    if sequence_byte == SEQUENCE_ONE {
        return SEQUENCE_ZERO;
    } else if sequence_byte == SEQUENCE_ZERO {
        return SEQUENCE_ONE;
    } else {
        return flip_sequence_byte(correct_sequence_byte(sequence_byte));
    }
}
pub trait Packet {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self;
    fn is_valid(&self) -> bool;
}
#[derive(PartialEq, Debug)]
pub enum GenericPacket {
    Frame(frame::Frame),
    Acknowledgement(acknowledgement::GenericAcknowledgement),
}

impl Packet for GenericPacket {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        match self {
            GenericPacket::Frame(frame) => GenericPacket::Frame(
                frame.simulate_errors_with_probability(bit_error_probability, rng),
            ),
            GenericPacket::Acknowledgement(acknowledgement) => GenericPacket::Acknowledgement(
                acknowledgement.simulate_errors_with_probability(bit_error_probability, rng),
            ),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            GenericPacket::Frame(frame) => frame.is_valid(),
            GenericPacket::Acknowledgement(acknowledgement) => acknowledgement.is_valid(),
        }
    }
}
