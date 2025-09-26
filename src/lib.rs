use crate::{ack::ACK, frame::Frame, nack::NACK};

pub mod ack;
pub mod frame;
pub mod nack;
const ACK_VALUE: u8 = 0b0000_1100;

const NACK_VALUE: u8 = 0b1111_0011;
pub trait Packet {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self;
    fn is_valid(&self) -> bool;
}
#[derive(PartialEq)]
pub enum PacketType {
    Frame(Frame),
    ACK(ACK),
    NACK(NACK),
}

impl Packet for PacketType {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        match self {
            PacketType::Frame(frame) => PacketType::Frame(
                frame.simulate_errors_with_probability(bit_error_probability, rng),
            ),
            PacketType::ACK(ack) => {
                PacketType::ACK(ack.simulate_errors_with_probability(bit_error_probability, rng))
            }
            PacketType::NACK(nack) => {
                PacketType::NACK(nack.simulate_errors_with_probability(bit_error_probability, rng))
            }
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            PacketType::Frame(frame) => frame.is_valid(),
            PacketType::ACK(ack) => ack.is_valid(),
            PacketType::NACK(nack) => nack.is_valid(),
        }
    }
}
