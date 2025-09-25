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
