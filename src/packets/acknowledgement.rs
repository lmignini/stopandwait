use crate::packets::Packet;

pub mod ack;
pub mod nack;

#[derive(PartialEq, Debug)]
pub enum GenericAcknowledgement {
    ACK(ack::ACK),
    NACK(nack::NACK),
}

impl Packet for GenericAcknowledgement {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        match self {
            GenericAcknowledgement::ACK(ack) => GenericAcknowledgement::ACK(
                ack.simulate_errors_with_probability(bit_error_probability, rng),
            ),
            GenericAcknowledgement::NACK(nack) => GenericAcknowledgement::NACK(
                nack.simulate_errors_with_probability(bit_error_probability, rng),
            ),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            GenericAcknowledgement::ACK(ack) => ack.is_valid(),
            GenericAcknowledgement::NACK(nack) => nack.is_valid(),
        }
    }
}
