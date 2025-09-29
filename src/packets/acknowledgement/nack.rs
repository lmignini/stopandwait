use rand::Rng;

use crate::packets::{NACK_VALUE, Packet, SequenceByte};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct NACK {
    content: u16,
}

impl NACK {
    pub const fn new(sequence_byte_of_rejected_package: SequenceByte) -> Self {
        Self {
            content: u16::from_be_bytes([NACK_VALUE, sequence_byte_of_rejected_package]),
        }
    }

    pub fn flip_bit(&mut self, bit_index: u8) {
        if bit_index < 16 {
            self.content ^= 1u16 << bit_index;
        }
    }

    pub fn get_ack_and_sequence_byte(&self) -> (u8, SequenceByte) {
        return (self.content.to_be_bytes()[0], self.content.to_be_bytes()[1]);
    }
}

impl Packet for NACK {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        let mut cloned_ack = self.clone();
        for i in 0..16 {
            if rng.random_bool(bit_error_probability) {
                cloned_ack.flip_bit(i);
            }
        }
        cloned_ack
    }
    fn is_valid(&self) -> bool {
        self.get_ack_and_sequence_byte().0 == NACK_VALUE
    }
}
